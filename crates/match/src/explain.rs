use crate::lineup::{sample_lineup, LineupProjectionInput};
use crate::matchup::{build_metric_matchup_edges, MatchupRule, PlayerMatchupState};
use footy_core::{MetricId, PlayerId, Probability};
use footy_forecast::Rng64;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedMatchupEdge {
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub metric: MetricId,
    /// Probability both players occupy spatially interacting starting slots on the same path.
    pub interaction_probability: Probability,
    pub mean_spatial_weight_given_interaction: f64,
    pub mean_ability_advantage_given_interaction: f64,
    /// Unconditional E[spatial_weight * ability_advantage] across all lineup paths.
    pub expected_weighted_advantage: f64,
}

#[derive(Default)]
struct EdgeAccumulator {
    interactions: u32,
    spatial: f64,
    advantage: f64,
    weighted_advantage: f64,
}

/// Project the actual X↔Y matchup distribution rather than explaining a single guessed XI. A pair
/// that only meets on 30% of lineup paths remains a 30%-probability driver in the final explanation.
pub fn project_metric_matchups(
    attacking: &LineupProjectionInput,
    defending: &LineupProjectionInput,
    states: &[PlayerMatchupState],
    rules: &[MatchupRule],
    metric: MetricId,
    spatial_sigma: f64,
    paths: u32,
    seed: u64,
) -> Option<Vec<ProjectedMatchupEdge>> {
    if paths == 0 || !attacking.validate() || !defending.validate() { return None; }
    let mut rng = Rng64::new(seed);
    let mut accumulators = BTreeMap::<(PlayerId, PlayerId), EdgeAccumulator>::new();
    for _ in 0..paths {
        let attack_lineup = sample_lineup(attacking, &mut rng)?;
        let defend_lineup = sample_lineup(defending, &mut rng)?;
        let edges = build_metric_matchup_edges(&attack_lineup, &defend_lineup, states, rules, metric, spatial_sigma)?;
        for edge in edges {
            let entry = accumulators.entry((edge.attacker, edge.defender)).or_default();
            entry.interactions = entry.interactions.saturating_add(1);
            entry.spatial += edge.spatial_weight;
            entry.advantage += edge.ability_advantage;
            entry.weighted_advantage += edge.spatial_weight * edge.ability_advantage;
        }
    }
    let denominator = paths as f32;
    Some(accumulators.into_iter().map(|((attacker, defender), value)| {
        let interaction_denominator = f64::from(value.interactions.max(1));
        ProjectedMatchupEdge {
            attacker,
            defender,
            metric,
            interaction_probability: Probability::clamped(value.interactions as f32 / denominator),
            mean_spatial_weight_given_interaction: value.spatial / interaction_denominator,
            mean_ability_advantage_given_interaction: value.advantage / interaction_denominator,
            expected_weighted_advantage: value.weighted_advantage / f64::from(paths),
        }
    }).collect())
}

pub fn strongest_projected_opponents(
    edges: &[ProjectedMatchupEdge],
    attacker: PlayerId,
    limit: usize,
) -> Vec<ProjectedMatchupEdge> {
    let mut values: Vec<ProjectedMatchupEdge> = edges.iter().copied().filter(|value| value.attacker == attacker).collect();
    values.sort_by(|left, right| {
        right.expected_weighted_advantage.abs().total_cmp(&left.expected_weighted_advantage.abs())
            .then_with(|| right.interaction_probability.get().total_cmp(&left.interaction_probability.get()))
    });
    values.truncate(limit);
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineup::{
        FormationPrior, FormationTemplate, LineupCandidate, LineupSamplingParameters, LockedStarter, RoleFit, TacticalSlot,
    };
    use crate::matchup::AbilityDimension;
    use footy_core::{ClubId, RoleId};
    use footy_state::AbilityVector;

    fn formation() -> FormationTemplate {
        FormationTemplate { id: 1, slots: (0..11).map(|slot| TacticalSlot {
            slot, role: RoleId(slot as u16 + 1), x: 0.05 + f64::from(slot) * 0.075, y: f64::from(slot % 5) / 4.0,
        }).collect() }
    }

    fn lineup(club: u32, offset: u64) -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(club), formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (0..12).map(|index| LineupCandidate {
                player: PlayerId(offset + index), start_probability: if index < 11 { 0.9 } else { 0.1 }, availability_probability: 1.0,
                ability_index: 10.0, fatigue: 0.0, expected_minutes_if_start: 85.0, minutes_sigma: 0.0, substitute_probability: 0.1,
                role_fits: (1..=11).map(|role| RoleFit { role: RoleId(role), fit: if role == index as u16 + 1 { 1.0 } else { 0.1 } }).collect(),
            }).collect(),
            locked_starters: vec![LockedStarter { player: PlayerId(offset), slot: Some(0) }],
            parameters: LineupSamplingParameters::default(),
        }
    }

    #[test]
    fn projected_edges_preserve_lineup_probability() {
        let home = lineup(1, 1);
        let away = lineup(2, 101);
        let states: Vec<PlayerMatchupState> = home.candidates.iter().chain(away.candidates.iter()).map(|candidate| PlayerMatchupState {
            player: candidate.player,
            ability: AbilityVector::from_components([10.0; 10]),
        }).collect();
        let metric = MetricId(1);
        let rules = [MatchupRule { metric: Some(metric), attacker_dimension: AbilityDimension::Technical, defender_dimension: AbilityDimension::Defending, weight: 1.0, scale: 20.0 }];
        let edges = project_metric_matchups(&home, &away, &states, &rules, metric, 0.25, 200, 42).unwrap();
        assert!(edges.iter().any(|edge| edge.attacker == PlayerId(1)));
        assert!(edges.iter().all(|edge| edge.interaction_probability.get() > 0.0));
    }
}
