use crate::{simulate_primitive_match, LineupCandidate, LineupProjectionInput, PrimitiveMatchForecast, PrimitiveMatchSimulationInput, SpecificPlayerIncidentProfile};
use footy_core::{ClubId, Confidence, Date, MetricId, PlayerId};
use footy_state::{PlayerState, PsychologyMeasure, PsychologyVector};
use std::collections::{BTreeMap, BTreeSet};

/// Centered psychological signal. `value` is always in [-1, 1]; confidence controls how much of
/// that signal is allowed to move a forecast. Missing information is not represented by zero: use
/// low confidence or omit the state so uncertainty can be widened explicitly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalMeasure {
    pub value: f64,
    pub confidence: Confidence,
}

impl MentalMeasure {
    pub fn new(value: f64, confidence: Confidence) -> Option<Self> {
        (value.is_finite() && (-1.0..=1.0).contains(&value)).then_some(Self { value, confidence })
    }

    pub fn effective(self) -> f64 { self.value * f64::from(self.confidence.get()) }
}

impl Default for MentalMeasure {
    fn default() -> Self { Self { value: 0.0, confidence: Confidence::ZERO } }
}

/// Maps a source trait scale (for example FM's 1..20 range) into a centered [-1, 1] state without
/// hard-coding one provider's scale into the match engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PsychologyTraitScale {
    pub center: f64,
    pub half_range: f64,
}

impl PsychologyTraitScale {
    pub fn normalize(self, value: PsychologyMeasure) -> Option<MentalMeasure> {
        if !self.center.is_finite() || !self.half_range.is_finite() || self.half_range <= 0.0 { return None; }
        let normalized = ((f64::from(value.value) - self.center) / self.half_range).clamp(-1.0, 1.0);
        MentalMeasure::new(normalized, value.confidence)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StablePsychologyState {
    pub professionalism: MentalMeasure,
    pub ambition: MentalMeasure,
    pub consistency: MentalMeasure,
    pub pressure_resilience: MentalMeasure,
    pub adaptability: MentalMeasure,
    pub leadership: MentalMeasure,
    pub teamwork: MentalMeasure,
    pub discipline: MentalMeasure,
}

pub fn stable_psychology_from_vector(vector: PsychologyVector, scale: PsychologyTraitScale) -> Option<StablePsychologyState> {
    Some(StablePsychologyState {
        professionalism: scale.normalize(vector.professionalism)?,
        ambition: scale.normalize(vector.ambition)?,
        consistency: scale.normalize(vector.consistency)?,
        pressure_resilience: scale.normalize(vector.pressure)?,
        adaptability: scale.normalize(vector.adaptability)?,
        leadership: scale.normalize(vector.leadership)?,
        teamwork: scale.normalize(vector.teamwork)?,
        discipline: scale.normalize(vector.discipline)?,
    })
}

pub fn stable_psychology_from_player_state(state: &PlayerState, scale: PsychologyTraitScale) -> Option<StablePsychologyState> {
    stable_psychology_from_vector(state.psychology?, scale)
}

/// Form observation is explicitly a residual versus the player's pre-match expectation. This
/// prevents a run of easy fixtures, lucky finishing, or strong team results from being counted once
/// as underlying ability and a second time as "form".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormObservation {
    pub player: PlayerId,
    pub date: Date,
    pub observed_per90: f64,
    pub expected_per90: f64,
    /// Meaningful one-unit residual scale for the metric or composite performance index.
    pub residual_scale: f64,
    pub minutes: u16,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormState {
    pub player: PlayerId,
    pub as_of: Date,
    pub level: MentalMeasure,
    pub trend: MentalMeasure,
    pub effective_matches: f64,
}

/// Causal exponentially weighted form primitive. Only observations on/before `as_of` are used;
/// low samples shrink toward neutral rather than being promoted to extreme form.
pub fn build_form_state(
    observations: &[FormObservation],
    player: PlayerId,
    as_of: Date,
    horizon_days: u16,
    half_life_days: f64,
    prior_strength_matches: f64,
) -> Option<FormState> {
    if horizon_days == 0 || !prior_strength_matches.is_finite() || prior_strength_matches < 0.0 { return None; }
    let half_life = if half_life_days.is_finite() && half_life_days > 0.0 { half_life_days } else { f64::from(horizon_days) / 2.0 };
    let split = i64::from(horizon_days) / 2;
    let mut total_weight = 0.0;
    let mut weighted = 0.0;
    let mut recent_weight = 0.0;
    let mut recent = 0.0;
    let mut prior_weight = 0.0;
    let mut prior = 0.0;
    for observation in observations {
        if observation.player != player || observation.minutes == 0 || !observation.observed_per90.is_finite()
            || !observation.expected_per90.is_finite() || !observation.residual_scale.is_finite() || observation.residual_scale <= 0.0
        { continue; }
        let age = as_of.days_since(observation.date);
        if age < 0 || age >= i64::from(horizon_days) { continue; }
        let residual = ((observation.observed_per90 - observation.expected_per90) / observation.residual_scale).tanh();
        let exposure = f64::from(observation.minutes.min(90)) / 90.0;
        let weight = (-std::f64::consts::LN_2 * age as f64 / half_life).exp()
            * exposure * f64::from(observation.confidence.get());
        if weight <= 0.0 { continue; }
        total_weight += weight;
        weighted += residual * weight;
        if age < split { recent_weight += weight; recent += residual * weight; }
        else { prior_weight += weight; prior += residual * weight; }
    }
    let denominator = total_weight + prior_strength_matches;
    let level = if denominator > 0.0 { weighted / denominator } else { 0.0 };
    let confidence = support_confidence(total_weight, prior_strength_matches);
    let trend_value = if recent_weight > 0.0 && prior_weight > 0.0 {
        (recent / recent_weight - prior / prior_weight).clamp(-1.0, 1.0)
    } else { 0.0 };
    let trend_support = recent_weight.min(prior_weight);
    Some(FormState {
        player,
        as_of,
        level: MentalMeasure::new(level.clamp(-1.0, 1.0), Confidence::clamped(confidence as f32))?,
        trend: MentalMeasure::new(trend_value, Confidence::clamped(support_confidence(trend_support, prior_strength_matches) as f32))?,
        effective_matches: total_weight,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoraleDriver {
    DirectSignal,
    PerformanceFeedback,
    SelectionAndRole,
    TeamEnvironment,
    CareerSecurity,
    SocialSupport,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoraleObservation {
    pub player: PlayerId,
    pub date: Date,
    pub driver: MoraleDriver,
    pub value: f64,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoraleDriverWeights {
    pub direct_signal: f64,
    pub performance_feedback: f64,
    pub selection_and_role: f64,
    pub team_environment: f64,
    pub career_security: f64,
    pub social_support: f64,
}

impl Default for MoraleDriverWeights {
    fn default() -> Self {
        Self { direct_signal: 1.0, performance_feedback: 1.0, selection_and_role: 1.0, team_environment: 1.0, career_security: 1.0, social_support: 1.0 }
    }
}

impl MoraleDriverWeights {
    fn get(self, driver: MoraleDriver) -> f64 {
        match driver {
            MoraleDriver::DirectSignal => self.direct_signal,
            MoraleDriver::PerformanceFeedback => self.performance_feedback,
            MoraleDriver::SelectionAndRole => self.selection_and_role,
            MoraleDriver::TeamEnvironment => self.team_environment,
            MoraleDriver::CareerSecurity => self.career_security,
            MoraleDriver::SocialSupport => self.social_support,
        }
    }

    fn valid(self) -> bool {
        [self.direct_signal, self.performance_feedback, self.selection_and_role, self.team_environment, self.career_security, self.social_support]
            .iter().all(|value| value.is_finite() && *value >= 0.0)
    }
}

/// Morale is dynamic and evidence-weighted. The engine intentionally does not infer it directly
/// from wins/losses; callers can feed a team-result signal, but its sign/strength is explicit.
pub fn build_morale_state(
    observations: &[MoraleObservation],
    player: PlayerId,
    as_of: Date,
    horizon_days: u16,
    half_life_days: f64,
    weights: MoraleDriverWeights,
    prior_strength: f64,
) -> Option<MentalMeasure> {
    if horizon_days == 0 || !weights.valid() || !prior_strength.is_finite() || prior_strength < 0.0 { return None; }
    let half_life = if half_life_days.is_finite() && half_life_days > 0.0 { half_life_days } else { f64::from(horizon_days) / 2.0 };
    let mut support = 0.0;
    let mut weighted = 0.0;
    for observation in observations {
        if observation.player != player || !observation.value.is_finite() || !(-1.0..=1.0).contains(&observation.value) { continue; }
        let age = as_of.days_since(observation.date);
        if age < 0 || age >= i64::from(horizon_days) { continue; }
        let weight = weights.get(observation.driver) * f64::from(observation.confidence.get())
            * (-std::f64::consts::LN_2 * age as f64 / half_life).exp();
        support += weight;
        weighted += observation.value * weight;
    }
    let denominator = support + prior_strength;
    let value = if denominator > 0.0 { weighted / denominator } else { 0.0 };
    MentalMeasure::new(value.clamp(-1.0, 1.0), Confidence::clamped(support_confidence(support, prior_strength) as f32))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotivationWeights {
    pub ambition: f64,
    pub morale: f64,
    pub role_satisfaction: f64,
    pub form: f64,
}

impl Default for MotivationWeights {
    fn default() -> Self { Self { ambition: 1.0, morale: 1.0, role_satisfaction: 1.0, form: 0.5 } }
}

pub fn derive_motivation(
    stable: StablePsychologyState,
    morale: MentalMeasure,
    role_satisfaction: MentalMeasure,
    form: MentalMeasure,
    weights: MotivationWeights,
) -> Option<MentalMeasure> {
    let values = [
        (stable.ambition, weights.ambition),
        (morale, weights.morale),
        (role_satisfaction, weights.role_satisfaction),
        (form, weights.form),
    ];
    if values.iter().any(|(_, weight)| !weight.is_finite() || *weight < 0.0) { return None; }
    combine_measures(&values)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerMentalState {
    pub player: PlayerId,
    pub stable: StablePsychologyState,
    pub morale: MentalMeasure,
    pub self_belief: MentalMeasure,
    pub form: MentalMeasure,
    pub form_trend: MentalMeasure,
    pub motivation: MentalMeasure,
    pub role_satisfaction: MentalMeasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MentalDimension {
    Morale,
    SelfBelief,
    Form,
    FormTrend,
    Motivation,
    RoleSatisfaction,
    Professionalism,
    Ambition,
    Consistency,
    PressureResilience,
    Adaptability,
    Leadership,
    Teamwork,
    Discipline,
}

impl PlayerMentalState {
    pub fn measure(self, dimension: MentalDimension) -> MentalMeasure {
        match dimension {
            MentalDimension::Morale => self.morale,
            MentalDimension::SelfBelief => self.self_belief,
            MentalDimension::Form => self.form,
            MentalDimension::FormTrend => self.form_trend,
            MentalDimension::Motivation => self.motivation,
            MentalDimension::RoleSatisfaction => self.role_satisfaction,
            MentalDimension::Professionalism => self.stable.professionalism,
            MentalDimension::Ambition => self.stable.ambition,
            MentalDimension::Consistency => self.stable.consistency,
            MentalDimension::PressureResilience => self.stable.pressure_resilience,
            MentalDimension::Adaptability => self.stable.adaptability,
            MentalDimension::Leadership => self.stable.leadership,
            MentalDimension::Teamwork => self.stable.teamwork,
            MentalDimension::Discipline => self.stable.discipline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TeamMentalDimension { CollectiveMorale, CollectiveBelief, Cohesion, Leadership }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamMentalState {
    pub club: ClubId,
    pub collective_morale: MentalMeasure,
    pub collective_belief: MentalMeasure,
    pub cohesion: MentalMeasure,
    pub leadership: MentalMeasure,
}

impl TeamMentalState {
    pub fn measure(self, dimension: TeamMentalDimension) -> MentalMeasure {
        match dimension {
            TeamMentalDimension::CollectiveMorale => self.collective_morale,
            TeamMentalDimension::CollectiveBelief => self.collective_belief,
            TeamMentalDimension::Cohesion => self.cohesion,
            TeamMentalDimension::Leadership => self.leadership,
        }
    }
}

/// Project team social state over the uncertain XI using starter probabilities. A direct cohesion
/// measure can override the teamwork/role-satisfaction proxy when better evidence exists.
pub fn project_team_mental_state(
    club: ClubId,
    lineup: &LineupProjectionInput,
    states: &[PlayerMentalState],
    direct_cohesion: Option<MentalMeasure>,
) -> Option<TeamMentalState> {
    if lineup.club != club || !lineup.validate() { return None; }
    let map: BTreeMap<PlayerId, PlayerMentalState> = states.iter().copied().map(|state| (state.player, state)).collect();
    let collective_morale = projected_measure(lineup, &map, |state| state.morale)?;
    let collective_belief = projected_measure(lineup, &map, |state| state.self_belief)?;
    let leadership = projected_measure(lineup, &map, |state| state.stable.leadership)?;
    let cohesion = match direct_cohesion {
        Some(value) => value,
        None => {
            let teamwork = projected_measure(lineup, &map, |state| state.stable.teamwork)?;
            let role = projected_measure(lineup, &map, |state| state.role_satisfaction)?;
            combine_measures(&[(teamwork, 1.0), (role, 0.5)])?
        }
    };
    Some(TeamMentalState { club, collective_morale, collective_belief, cohesion, leadership })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchMentalContext {
    /// Importance/stakes of the fixture. Scales pressure-resilience effects.
    pub pressure_intensity: f64,
    /// Novel tactical/cultural/environmental conditions. Scales adaptability effects.
    pub environmental_novelty: f64,
    /// Hostile/emotionally charged environment. Exposed separately for discipline modeling.
    pub hostility: f64,
}

impl MatchMentalContext {
    fn valid(self) -> bool {
        [self.pressure_intensity, self.environmental_novelty, self.hostility]
            .iter().all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    }

    fn scale(self, dimension: MentalDimension) -> f64 {
        match dimension {
            MentalDimension::PressureResilience => self.pressure_intensity,
            MentalDimension::Adaptability => self.environmental_novelty,
            MentalDimension::Discipline => 1.0 + self.hostility,
            _ => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalMetricLoading {
    pub metric: MetricId,
    pub dimension: MentalDimension,
    /// Effect on the player's expected event rate in log space.
    pub mean_log_weight: f64,
    /// Effect on match-to-match rate variance in log space. A calibrated consistency loading can,
    /// for example, reduce variance without pretending consistency changes the mean.
    pub log_variance_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamMentalMetricLoading {
    pub metric: MetricId,
    pub dimension: TeamMentalDimension,
    pub mean_log_weight: f64,
    pub log_variance_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalSelectionLoading {
    pub dimension: MentalDimension,
    pub start_logit_weight: f64,
    pub substitute_logit_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalModelParameters {
    pub max_abs_mean_log_adjustment: f64,
    pub max_abs_variance_log_adjustment: f64,
    pub max_abs_selection_logit_adjustment: f64,
    /// Relative epistemic sigma added when a player's mental state is unavailable.
    pub missing_state_relative_sigma: f64,
    /// Explicit red-card mechanism; sign is calibrated by the caller rather than hard-coded.
    pub discipline_red_card_log_weight: f64,
    pub low_morale_red_card_log_weight: f64,
    pub hostility_red_card_log_weight: f64,
}

impl Default for MentalModelParameters {
    fn default() -> Self {
        Self {
            max_abs_mean_log_adjustment: 0.35,
            max_abs_variance_log_adjustment: 0.50,
            max_abs_selection_logit_adjustment: 1.0,
            missing_state_relative_sigma: 0.05,
            discipline_red_card_log_weight: 0.0,
            low_morale_red_card_log_weight: 0.0,
            hostility_red_card_log_weight: 0.0,
        }
    }
}

impl MentalModelParameters {
    fn valid(self) -> bool {
        self.max_abs_mean_log_adjustment.is_finite() && self.max_abs_mean_log_adjustment >= 0.0
            && self.max_abs_variance_log_adjustment.is_finite() && self.max_abs_variance_log_adjustment >= 0.0
            && self.max_abs_selection_logit_adjustment.is_finite() && self.max_abs_selection_logit_adjustment >= 0.0
            && self.missing_state_relative_sigma.is_finite() && self.missing_state_relative_sigma >= 0.0
            && self.discipline_red_card_log_weight.is_finite()
            && self.low_morale_red_card_log_weight.is_finite()
            && self.hostility_red_card_log_weight.is_finite()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MentalMatchModel {
    pub players: Vec<PlayerMentalState>,
    pub home_team: TeamMentalState,
    pub away_team: TeamMentalState,
    pub metric_loadings: Vec<MentalMetricLoading>,
    pub team_metric_loadings: Vec<TeamMentalMetricLoading>,
    pub selection_loadings: Vec<MentalSelectionLoading>,
    pub context: MatchMentalContext,
    pub parameters: MentalModelParameters,
}

impl MentalMatchModel {
    pub fn validate(&self, input: &PrimitiveMatchSimulationInput) -> bool {
        if self.home_team.club != input.home || self.away_team.club != input.away || !self.context.valid() || !self.parameters.valid() { return false; }
        let mut players = BTreeSet::new();
        if !self.players.iter().all(|state| players.insert(state.player)) { return false; }
        self.metric_loadings.iter().all(|value| value.mean_log_weight.is_finite() && value.log_variance_weight.is_finite())
            && self.team_metric_loadings.iter().all(|value| value.mean_log_weight.is_finite() && value.log_variance_weight.is_finite())
            && self.selection_loadings.iter().all(|value| value.start_logit_weight.is_finite() && value.substitute_logit_weight.is_finite())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalMetricAdjustment {
    pub player: PlayerId,
    pub metric: MetricId,
    pub mean_log_adjustment: f64,
    pub variance_log_adjustment: f64,
    pub relative_uncertainty_added: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MentalPlayerAdjustment {
    pub player: PlayerId,
    pub start_probability_before: f64,
    pub start_probability_after: f64,
    pub substitute_probability_before: f64,
    pub substitute_probability_after: f64,
    pub red_card_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MentalAdjustedMatchInput {
    pub input: PrimitiveMatchSimulationInput,
    pub player_adjustments: Vec<MentalPlayerAdjustment>,
    pub metric_adjustments: Vec<MentalMetricAdjustment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MentalMatchForecast {
    pub forecast: PrimitiveMatchForecast,
    pub player_adjustments: Vec<MentalPlayerAdjustment>,
    pub metric_adjustments: Vec<MentalMetricAdjustment>,
}

/// Apply pre-match morale/form/psychology to the same primitives consumed by the specific-match
/// simulator. This is a calibration transform, not a parallel result model: it moves selection,
/// player event-rate distributions, event variance, epistemic uncertainty, and discipline risk;
/// the downstream lineup/matchup/channel/goal graph remains unchanged.
pub fn apply_mental_model(input: &PrimitiveMatchSimulationInput, model: &MentalMatchModel) -> Option<MentalAdjustedMatchInput> {
    if !input.validate() || !model.validate(input) { return None; }
    let mut adjusted = input.clone();
    let state_map: BTreeMap<PlayerId, PlayerMentalState> = model.players.iter().copied().map(|state| (state.player, state)).collect();
    let club_map = player_club_map(input)?;
    let mut player_adjustments = Vec::new();

    for lineup in [&mut adjusted.home_lineup, &mut adjusted.away_lineup] {
        for candidate in &mut lineup.candidates {
            let before_start = candidate.start_probability;
            let before_sub = candidate.substitute_probability;
            let state = state_map.get(&candidate.player).copied();
            let mut start_delta = 0.0;
            let mut substitute_delta = 0.0;
            if let Some(state) = state {
                for loading in &model.selection_loadings {
                    let effect = contextual_effect(state.measure(loading.dimension), loading.dimension, model.context);
                    start_delta += loading.start_logit_weight * effect;
                    substitute_delta += loading.substitute_logit_weight * effect;
                }
            }
            start_delta = start_delta.clamp(-model.parameters.max_abs_selection_logit_adjustment, model.parameters.max_abs_selection_logit_adjustment);
            substitute_delta = substitute_delta.clamp(-model.parameters.max_abs_selection_logit_adjustment, model.parameters.max_abs_selection_logit_adjustment);
            candidate.start_probability = adjust_probability(candidate.start_probability, start_delta);
            candidate.substitute_probability = adjust_probability(candidate.substitute_probability, substitute_delta);
            player_adjustments.push(MentalPlayerAdjustment {
                player: candidate.player,
                start_probability_before: before_start,
                start_probability_after: candidate.start_probability,
                substitute_probability_before: before_sub,
                substitute_probability_after: candidate.substitute_probability,
                red_card_multiplier: 1.0,
            });
        }
    }

    let mut metric_adjustments = Vec::new();
    for event in &mut adjusted.event_models {
        let state = state_map.get(&event.player).copied();
        let club = *club_map.get(&event.player)?;
        let team = if club == input.home { model.home_team } else if club == input.away { model.away_team } else { return None; };
        let mut mean_log = 0.0;
        let mut variance_log = 0.0;
        let mut relative_uncertainty2 = 0.0;
        if let Some(state) = state {
            for loading in model.metric_loadings.iter().filter(|loading| loading.metric == event.metric) {
                let measure = state.measure(loading.dimension);
                let scale = model.context.scale(loading.dimension);
                let effective = measure.effective() * scale;
                mean_log += loading.mean_log_weight * effective;
                variance_log += loading.log_variance_weight * effective;
                let missing_confidence = 1.0 - f64::from(measure.confidence.get());
                relative_uncertainty2 += (loading.mean_log_weight.abs() * scale * missing_confidence).powi(2);
            }
        } else {
            relative_uncertainty2 += model.parameters.missing_state_relative_sigma.powi(2);
        }
        for loading in model.team_metric_loadings.iter().filter(|loading| loading.metric == event.metric) {
            let measure = team.measure(loading.dimension);
            let effective = measure.effective();
            mean_log += loading.mean_log_weight * effective;
            variance_log += loading.log_variance_weight * effective;
            let missing_confidence = 1.0 - f64::from(measure.confidence.get());
            relative_uncertainty2 += (loading.mean_log_weight.abs() * missing_confidence).powi(2);
        }
        mean_log = mean_log.clamp(-model.parameters.max_abs_mean_log_adjustment, model.parameters.max_abs_mean_log_adjustment);
        variance_log = variance_log.clamp(-model.parameters.max_abs_variance_log_adjustment, model.parameters.max_abs_variance_log_adjustment);
        event.mean_per90 *= mean_log.exp();
        event.variance_per90 *= variance_log.exp();
        let relative_uncertainty = relative_uncertainty2.sqrt();
        let added_sigma = event.mean_per90 * relative_uncertainty;
        event.epistemic_sigma = (event.epistemic_sigma.powi(2) + added_sigma.powi(2)).sqrt();
        metric_adjustments.push(MentalMetricAdjustment {
            player: event.player,
            metric: event.metric,
            mean_log_adjustment: mean_log,
            variance_log_adjustment: variance_log,
            relative_uncertainty_added: relative_uncertainty,
        });
    }

    for candidate in input.home_lineup.candidates.iter().chain(input.away_lineup.candidates.iter()) {
        let Some(state) = state_map.get(&candidate.player).copied() else { continue; };
        let discipline = state.stable.discipline.effective();
        let low_morale = (-state.morale.effective()).max(0.0);
        let log_multiplier = model.parameters.discipline_red_card_log_weight * discipline
            + model.parameters.low_morale_red_card_log_weight * low_morale
            + model.parameters.hostility_red_card_log_weight * model.context.hostility;
        let multiplier = log_multiplier.clamp(-2.0, 2.0).exp();
        if let Some(profile) = adjusted.incident_profiles.iter_mut().find(|profile| profile.player == candidate.player) {
            profile.red_card_multiplier *= multiplier;
        } else {
            adjusted.incident_profiles.push(SpecificPlayerIncidentProfile { player: candidate.player, injury_multiplier: 1.0, red_card_multiplier: multiplier });
        }
        if let Some(record) = player_adjustments.iter_mut().find(|record| record.player == candidate.player) {
            record.red_card_multiplier = multiplier;
        }
    }

    adjusted.incident_profiles.sort_by_key(|value| value.player);
    player_adjustments.sort_by_key(|value| value.player);
    metric_adjustments.sort_by_key(|value| (value.player, value.metric));
    adjusted.validate().then_some(MentalAdjustedMatchInput { input: adjusted, player_adjustments, metric_adjustments })
}

pub fn simulate_mental_match(
    input: &PrimitiveMatchSimulationInput,
    model: &MentalMatchModel,
    paths: u32,
    seed: u64,
) -> Option<MentalMatchForecast> {
    let adjusted = apply_mental_model(input, model)?;
    let forecast = simulate_primitive_match(&adjusted.input, paths, seed)?;
    Some(MentalMatchForecast { forecast, player_adjustments: adjusted.player_adjustments, metric_adjustments: adjusted.metric_adjustments })
}

fn player_club_map(input: &PrimitiveMatchSimulationInput) -> Option<BTreeMap<PlayerId, ClubId>> {
    let mut map = BTreeMap::new();
    for (club, lineup) in [(input.home, &input.home_lineup), (input.away, &input.away_lineup)] {
        for candidate in &lineup.candidates {
            if map.insert(candidate.player, club).is_some() { return None; }
        }
    }
    Some(map)
}

fn contextual_effect(measure: MentalMeasure, dimension: MentalDimension, context: MatchMentalContext) -> f64 {
    measure.effective() * context.scale(dimension)
}

fn adjust_probability(probability: f64, logit_delta: f64) -> f64 {
    if probability <= 0.0 { return 0.0; }
    if probability >= 1.0 { return 1.0; }
    let logit = (probability / (1.0 - probability)).ln() + logit_delta;
    1.0 / (1.0 + (-logit).exp())
}

fn projected_measure<F>(
    lineup: &LineupProjectionInput,
    states: &BTreeMap<PlayerId, PlayerMentalState>,
    selector: F,
) -> Option<MentalMeasure>
where F: Fn(PlayerMentalState) -> MentalMeasure {
    let mut values = Vec::new();
    for candidate in &lineup.candidates {
        let Some(state) = states.get(&candidate.player).copied() else { continue; };
        values.push((selector(state), candidate.start_probability.max(0.0)));
    }
    combine_measures(&values)
}

fn combine_measures(values: &[(MentalMeasure, f64)]) -> Option<MentalMeasure> {
    let mut weighted = 0.0;
    let mut support = 0.0;
    let mut possible = 0.0;
    for (measure, weight) in values {
        if !weight.is_finite() || *weight < 0.0 { return None; }
        possible += *weight;
        let effective_weight = *weight * f64::from(measure.confidence.get());
        support += effective_weight;
        weighted += measure.value * effective_weight;
    }
    if possible <= 0.0 { return None; }
    let value = if support > 0.0 { weighted / support } else { 0.0 };
    MentalMeasure::new(value.clamp(-1.0, 1.0), Confidence::clamped((support / possible).clamp(0.0, 1.0) as f32))
}

fn support_confidence(support: f64, prior_strength: f64) -> f64 {
    if support <= 0.0 { 0.0 }
    else if prior_strength <= 0.0 { 1.0 }
    else { (support / (support + prior_strength)).clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttributeEventLoading, AttributeMatchupRule, CausalChannelGraph, FormationPrior, FormationTemplate, GoalChannelLoading, LineupSamplingParameters, MatchupRule, MetricChannelLoading, PairwiseSynergy, PlayerAttributeState, PlayerAttributeValue, PlayerMatchupState, RoleFit, SpecificEnvironmentMetricModifier, SpecificGoalProcess, SpecificInMatchParameters, SpecificMatchEnvironment, SpecificPlayerEventModel, SpecificScoreStateMetricModifier, SpecificTeamContext, TacticalSlot, TeamProcessChannel};
    use footy_core::{AttributeId, MatchId, RoleId};
    use footy_forecast::StatFamily;
    use footy_state::AbilityVector;

    fn measure(value: f64) -> MentalMeasure { MentalMeasure::new(value, Confidence::ONE).unwrap() }

    fn stable(value: f64) -> StablePsychologyState {
        StablePsychologyState {
            professionalism: measure(value), ambition: measure(value), consistency: measure(value), pressure_resilience: measure(value),
            adaptability: measure(value), leadership: measure(value), teamwork: measure(value), discipline: measure(value),
        }
    }

    #[test]
    fn form_is_residual_and_never_uses_future_observations() {
        let player = PlayerId(1);
        let observations = [
            FormObservation { player, date: Date(5), observed_per90: 1.5, expected_per90: 1.0, residual_scale: 0.5, minutes: 90, confidence: Confidence::ONE },
            FormObservation { player, date: Date(20), observed_per90: 100.0, expected_per90: 1.0, residual_scale: 0.5, minutes: 90, confidence: Confidence::ONE },
        ];
        let state = build_form_state(&observations, player, Date(10), 30, 15.0, 1.0).unwrap();
        assert!(state.level.value > 0.0 && state.level.value < 1.0);
        assert!(state.effective_matches > 0.0 && state.effective_matches < 1.1);
    }

    #[test]
    fn low_confidence_morale_is_shrunk() {
        let observation = MoraleObservation { player: PlayerId(1), date: Date(5), driver: MoraleDriver::DirectSignal, value: 1.0, confidence: Confidence::clamped(0.2) };
        let state = build_morale_state(&[observation], PlayerId(1), Date(5), 30, 15.0, MoraleDriverWeights::default(), 1.0).unwrap();
        assert!(state.value > 0.0 && state.value < 0.5);
        assert!(state.confidence.get() < 0.5);
    }

    #[test]
    fn motivation_keeps_stable_and_dynamic_inputs_separate() {
        let value = derive_motivation(stable(0.5), measure(-0.5), measure(0.2), measure(0.1), MotivationWeights::default()).unwrap();
        assert!(value.value > -0.5 && value.value < 0.5);
    }
}
