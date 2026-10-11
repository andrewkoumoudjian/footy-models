use crate::lineup::{sample_lineup, LineupProjectionInput, SampledStarter};
use crate::specific::{AttributeMatchupRule, PlayerAttributeState, SpecificPlayerEventModel};
use footy_core::{AttributeId, MetricId, PlayerId, Probability};
use footy_forecast::Rng64;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeMatchupDriver {
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub metric: MetricId,
    pub attacker_attribute: AttributeId,
    pub defender_attribute: AttributeId,
    /// Probability this attacker/defender pair is simultaneously active in spatially relevant
    /// starting slots under the projected-XI distribution.
    pub interaction_probability: Probability,
    pub mean_spatial_weight_given_interaction: f64,
    /// Unconditional expected contribution to the attacker's metric log-rate modifier. Summing the
    /// driver contributions for one attacker/metric recovers the fine-grained matchup term up to
    /// Monte Carlo error.
    pub expected_log_rate_contribution: f64,
}

#[derive(Default)]
struct DriverAccumulator {
    interactions: u32,
    spatial: f64,
    log_contribution: f64,
}

/// Project exact X-vs-Y AttributeId drivers over uncertain lineups. Attribution uses the same
/// spatial normalization and rule normalization as the preferred specific-match simulator.
pub fn project_attribute_matchup_drivers(
    attacking: &LineupProjectionInput,
    defending: &LineupProjectionInput,
    attribute_states: &[PlayerAttributeState],
    event_models: &[SpecificPlayerEventModel],
    rules: &[AttributeMatchupRule],
    metric: MetricId,
    spatial_sigma: f64,
    paths: u32,
    seed: u64,
) -> Option<Vec<AttributeMatchupDriver>> {
    if paths == 0 || !attacking.validate() || !defending.validate() || !spatial_sigma.is_finite() || spatial_sigma <= 0.0 {
        return None;
    }
    let states: BTreeMap<PlayerId, &PlayerAttributeState> = attribute_states.iter().map(|value| (value.player, value)).collect();
    let sensitivity: BTreeMap<PlayerId, f64> = event_models.iter().filter(|value| value.metric == metric).map(|value| (value.player, value.attribute_matchup_sensitivity)).collect();
    let applicable: Vec<AttributeMatchupRule> = rules.iter().copied().filter(|value| value.metric.is_none() || value.metric == Some(metric)).collect();
    if applicable.iter().any(|value| !value.weight.is_finite() || !value.scale.is_finite() || value.scale <= 0.0) { return None; }
    let mut rng = Rng64::new(seed);
    let mut accumulators = BTreeMap::<(PlayerId, PlayerId, AttributeId, AttributeId), DriverAccumulator>::new();

    for _ in 0..paths {
        let attack_lineup = sample_lineup(attacking, &mut rng)?;
        let defend_lineup = sample_lineup(defending, &mut rng)?;
        for attacker in &attack_lineup.starters {
            let Some(attacker_state) = states.get(&attacker.player).copied() else { continue; };
            let player_sensitivity = sensitivity.get(&attacker.player).copied().unwrap_or(0.0);
            let mut pair_rows: Vec<(&SampledStarter, f64, Vec<(AttributeMatchupRule, f64)>)> = Vec::new();
            let mut spatial_denominator = 0.0;
            for defender in &defend_lineup.starters {
                let Some(defender_state) = states.get(&defender.player).copied() else { continue; };
                let spatial = overlap(attacker, defender, spatial_sigma);
                if spatial < 1e-8 { continue; }
                let mut raw = Vec::new();
                let mut rule_denominator = 0.0;
                for rule in &applicable {
                    let Some(a) = attacker_state.value(rule.attacker_attribute) else { continue; };
                    let Some(d) = defender_state.value(rule.defender_attribute) else { continue; };
                    raw.push((*rule, rule.weight * (a - d) / rule.scale));
                    rule_denominator += rule.weight.abs();
                }
                if rule_denominator <= 0.0 { continue; }
                for (_, value) in &mut raw { *value /= rule_denominator; }
                spatial_denominator += spatial;
                pair_rows.push((defender, spatial, raw));
            }
            if spatial_denominator <= 0.0 { continue; }
            for (defender, spatial, raw) in pair_rows {
                for (rule, normalized_rule) in raw {
                    let entry = accumulators.entry((attacker.player, defender.player, rule.attacker_attribute, rule.defender_attribute)).or_default();
                    entry.interactions = entry.interactions.saturating_add(1);
                    entry.spatial += spatial;
                    entry.log_contribution += player_sensitivity * spatial * normalized_rule / spatial_denominator;
                }
            }
        }
    }

    let denominator = paths as f32;
    let mut output: Vec<AttributeMatchupDriver> = accumulators.into_iter().map(|((attacker, defender, attacker_attribute, defender_attribute), value)| {
        let interactions = f64::from(value.interactions.max(1));
        AttributeMatchupDriver {
            attacker,
            defender,
            metric,
            attacker_attribute,
            defender_attribute,
            interaction_probability: Probability::clamped(value.interactions as f32 / denominator),
            mean_spatial_weight_given_interaction: value.spatial / interactions,
            expected_log_rate_contribution: value.log_contribution / f64::from(paths),
        }
    }).collect();
    output.sort_by(|left, right| {
        right.expected_log_rate_contribution.abs().total_cmp(&left.expected_log_rate_contribution.abs())
            .then(left.attacker.cmp(&right.attacker))
            .then(left.defender.cmp(&right.defender))
            .then(left.attacker_attribute.cmp(&right.attacker_attribute))
    });
    Some(output)
}

pub fn strongest_attribute_matchup_drivers(
    values: &[AttributeMatchupDriver],
    attacker: PlayerId,
    metric: MetricId,
    limit: usize,
) -> Vec<AttributeMatchupDriver> {
    let mut output: Vec<AttributeMatchupDriver> = values.iter().copied().filter(|value| value.attacker == attacker && value.metric == metric).collect();
    output.sort_by(|left, right| right.expected_log_rate_contribution.abs().total_cmp(&left.expected_log_rate_contribution.abs()));
    output.truncate(limit);
    output
}

fn overlap(attacker: &SampledStarter, defender: &SampledStarter, sigma: f64) -> f64 {
    let dx = attacker.x - (1.0 - defender.x);
    let dy = attacker.y - (1.0 - defender.y);
    (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specific::PlayerAttributeValue;
    use crate::lineup::{FormationPrior, FormationTemplate, LineupCandidate, LineupSamplingParameters, LockedStarter, RoleFit, TacticalSlot};
    use footy_core::{ClubId, RoleId};
    use footy_forecast::StatFamily;

    fn formation() -> FormationTemplate {
        FormationTemplate { id: 1, slots: (0..11).map(|slot| TacticalSlot { slot, role: RoleId(slot as u16 + 1), x: 0.05 + f64::from(slot) * 0.075, y: f64::from(slot % 5) / 4.0 }).collect() }
    }

    fn lineup(club: u32, offset: u64) -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(club), formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (0..11).map(|index| LineupCandidate {
                player: PlayerId(offset + index), start_probability: 1.0, availability_probability: 1.0, ability_index: 10.0,
                fatigue: 0.0, expected_minutes_if_start: 90.0, minutes_sigma: 0.0, substitute_probability: 0.0,
                role_fits: (1..=11).map(|role| RoleFit { role: RoleId(role), fit: if role == index as u16 + 1 { 1.0 } else { 0.1 } }).collect(),
            }).collect(), locked_starters: vec![LockedStarter { player: PlayerId(offset), slot: Some(0) }], parameters: LineupSamplingParameters::default(),
        }
    }

    #[test]
    fn exposes_exact_attribute_pair_driver() {
        let attack = lineup(1, 1);
        let defend = lineup(2, 101);
        let pace = AttributeId(1);
        let recovery = AttributeId(2);
        let states: Vec<PlayerAttributeState> = attack.candidates.iter().chain(defend.candidates.iter()).map(|value| PlayerAttributeState {
            player: value.player,
            attributes: vec![PlayerAttributeValue { attribute: pace, value: if value.player.0 < 100 { 16.0 } else { 10.0 } }, PlayerAttributeValue { attribute: recovery, value: 10.0 }],
        }).collect();
        let metric = MetricId(3);
        let models: Vec<SpecificPlayerEventModel> = attack.candidates.iter().map(|value| SpecificPlayerEventModel {
            player: value.player, metric, family: StatFamily::ContinuousNonNegative, mean_per90: 1.0, variance_per90: 0.1,
            epistemic_sigma: 0.1, reference_per90: 1.0, aggregate_matchup_sensitivity: 0.0, attribute_matchup_sensitivity: 0.5,
            teammate_sensitivity: 0.0, role_fit_sensitivity: 0.0, fatigue_sensitivity: 0.0,
        }).collect();
        let rules = [AttributeMatchupRule { metric: Some(metric), attacker_attribute: pace, defender_attribute: recovery, weight: 1.0, scale: 20.0 }];
        let values = project_attribute_matchup_drivers(&attack, &defend, &states, &models, &rules, metric, 0.25, 100, 42).unwrap();
        assert!(values.iter().any(|value| value.attacker == PlayerId(1) && value.expected_log_rate_contribution > 0.0));
    }
}
