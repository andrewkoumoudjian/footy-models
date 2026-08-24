use crate::{MentalDimension, MentalMatchModel, PrimitiveMatchSimulationInput, TeamMentalDimension};
use footy_core::{MetricId, PlayerId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MentalDriverSource {
    Player(MentalDimension),
    Team(TeamMentalDimension),
    MatchHostility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MentalEffectTarget {
    StartLogit,
    SubstituteLogit,
    MetricMean(MetricId),
    MetricVariance(MetricId),
    RedCardLogRisk,
}

/// One auditable mental-state contribution before global safety clamps are applied. `state_value`
/// is the centered raw state, `confidence` is the evidence confidence, and `contribution` is the
/// resulting additive log/logit-space effect from that one primitive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalDriverContribution {
    pub player: PlayerId,
    pub source: MentalDriverSource,
    pub target: MentalEffectTarget,
    pub state_value: f64,
    pub confidence: f64,
    pub context_scale: f64,
    pub contribution: f64,
}

/// Decompose the mental calibration into player-dimension and team-dimension contributions. This
/// does not run a second model and does not estimate a result effect directly; it explains the
/// upstream adjustments that `apply_mental_model` feeds into the specific-match Monte Carlo graph.
pub fn project_mental_drivers(
    input: &PrimitiveMatchSimulationInput,
    model: &MentalMatchModel,
) -> Option<Vec<MentalDriverContribution>> {
    if !input.validate() || !model.validate(input) { return None; }
    let states: BTreeMap<PlayerId, _> = model.players.iter().copied().map(|state| (state.player, state)).collect();
    let mut clubs = BTreeMap::new();
    for candidate in &input.home_lineup.candidates {
        if clubs.insert(candidate.player, input.home).is_some() { return None; }
    }
    for candidate in &input.away_lineup.candidates {
        if clubs.insert(candidate.player, input.away).is_some() { return None; }
    }
    let mut out = Vec::new();

    for candidate in input.home_lineup.candidates.iter().chain(input.away_lineup.candidates.iter()) {
        let Some(state) = states.get(&candidate.player).copied() else { continue; };
        for loading in &model.selection_loadings {
            let measure = state.measure(loading.dimension);
            let scale = context_scale(loading.dimension, model);
            let effective = measure.value * f64::from(measure.confidence.get()) * scale;
            out.push(MentalDriverContribution {
                player: candidate.player,
                source: MentalDriverSource::Player(loading.dimension),
                target: MentalEffectTarget::StartLogit,
                state_value: measure.value,
                confidence: f64::from(measure.confidence.get()),
                context_scale: scale,
                contribution: loading.start_logit_weight * effective,
            });
            out.push(MentalDriverContribution {
                player: candidate.player,
                source: MentalDriverSource::Player(loading.dimension),
                target: MentalEffectTarget::SubstituteLogit,
                state_value: measure.value,
                confidence: f64::from(measure.confidence.get()),
                context_scale: scale,
                contribution: loading.substitute_logit_weight * effective,
            });
        }
    }

    for event in &input.event_models {
        let club = *clubs.get(&event.player)?;
        let team = if club == input.home { model.home_team } else { model.away_team };
        if let Some(state) = states.get(&event.player).copied() {
            for loading in model.metric_loadings.iter().filter(|loading| loading.metric == event.metric) {
                let measure = state.measure(loading.dimension);
                let scale = context_scale(loading.dimension, model);
                let effective = measure.value * f64::from(measure.confidence.get()) * scale;
                out.push(MentalDriverContribution {
                    player: event.player,
                    source: MentalDriverSource::Player(loading.dimension),
                    target: MentalEffectTarget::MetricMean(event.metric),
                    state_value: measure.value,
                    confidence: f64::from(measure.confidence.get()),
                    context_scale: scale,
                    contribution: loading.mean_log_weight * effective,
                });
                out.push(MentalDriverContribution {
                    player: event.player,
                    source: MentalDriverSource::Player(loading.dimension),
                    target: MentalEffectTarget::MetricVariance(event.metric),
                    state_value: measure.value,
                    confidence: f64::from(measure.confidence.get()),
                    context_scale: scale,
                    contribution: loading.log_variance_weight * effective,
                });
            }
        }
        for loading in model.team_metric_loadings.iter().filter(|loading| loading.metric == event.metric) {
            let measure = team.measure(loading.dimension);
            let effective = measure.value * f64::from(measure.confidence.get());
            out.push(MentalDriverContribution {
                player: event.player,
                source: MentalDriverSource::Team(loading.dimension),
                target: MentalEffectTarget::MetricMean(event.metric),
                state_value: measure.value,
                confidence: f64::from(measure.confidence.get()),
                context_scale: 1.0,
                contribution: loading.mean_log_weight * effective,
            });
            out.push(MentalDriverContribution {
                player: event.player,
                source: MentalDriverSource::Team(loading.dimension),
                target: MentalEffectTarget::MetricVariance(event.metric),
                state_value: measure.value,
                confidence: f64::from(measure.confidence.get()),
                context_scale: 1.0,
                contribution: loading.log_variance_weight * effective,
            });
        }
    }

    for candidate in input.home_lineup.candidates.iter().chain(input.away_lineup.candidates.iter()) {
        let Some(state) = states.get(&candidate.player).copied() else { continue; };
        let discipline = state.stable.discipline;
        out.push(MentalDriverContribution {
            player: candidate.player,
            source: MentalDriverSource::Player(MentalDimension::Discipline),
            target: MentalEffectTarget::RedCardLogRisk,
            state_value: discipline.value,
            confidence: f64::from(discipline.confidence.get()),
            context_scale: 1.0,
            contribution: model.parameters.discipline_red_card_log_weight * discipline.effective(),
        });
        let morale = state.morale;
        out.push(MentalDriverContribution {
            player: candidate.player,
            source: MentalDriverSource::Player(MentalDimension::Morale),
            target: MentalEffectTarget::RedCardLogRisk,
            state_value: morale.value,
            confidence: f64::from(morale.confidence.get()),
            context_scale: 1.0,
            contribution: model.parameters.low_morale_red_card_log_weight * (-morale.effective()).max(0.0),
        });
        out.push(MentalDriverContribution {
            player: candidate.player,
            source: MentalDriverSource::MatchHostility,
            target: MentalEffectTarget::RedCardLogRisk,
            state_value: model.context.hostility,
            confidence: 1.0,
            context_scale: 1.0,
            contribution: model.parameters.hostility_red_card_log_weight * model.context.hostility,
        });
    }

    out.sort_by_key(|value| (value.player, value.target, value.source));
    Some(out)
}

fn context_scale(dimension: MentalDimension, model: &MentalMatchModel) -> f64 {
    match dimension {
        MentalDimension::PressureResilience => model.context.pressure_intensity,
        MentalDimension::Adaptability => model.context.environmental_novelty,
        MentalDimension::Discipline => 1.0 + model.context.hostility,
        _ => 1.0,
    }
}
