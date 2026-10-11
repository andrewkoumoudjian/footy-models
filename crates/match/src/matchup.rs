use crate::lineup::{SampledLineup, SampledStarter};
use footy_core::{MetricId, PlayerId};
use footy_state::{AbilityVector, PlayerState};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityDimension {
    Technical,
    BallProgression,
    Creativity,
    Finishing,
    Defending,
    Aerial,
    Physical,
    DecisionMaking,
    Mentality,
    Goalkeeping,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchupRule {
    /// `None` applies to every metric; `Some(metric)` specializes the duel to one event family.
    pub metric: Option<MetricId>,
    pub attacker_dimension: AbilityDimension,
    pub defender_dimension: AbilityDimension,
    pub weight: f64,
    /// Converts the source ability scale into a dimensionless contrast.
    pub scale: f64,
}

impl MatchupRule {
    fn valid(self) -> bool {
        self.weight.is_finite() && self.scale.is_finite() && self.scale > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerMatchupState {
    pub player: PlayerId,
    pub ability: AbilityVector,
}

impl From<&PlayerState> for PlayerMatchupState {
    fn from(value: &PlayerState) -> Self {
        Self { player: value.player, ability: value.ability }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchupEdge {
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub metric: MetricId,
    /// Spatial probability-like weight derived from the two tactical slots.
    pub spatial_weight: f64,
    /// Positive means the attacker has the modeled attribute advantage.
    pub ability_advantage: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairwiseSynergy {
    pub a: PlayerId,
    pub b: PlayerId,
    /// Optional metric specialization. Positive helps production; negative suppresses it.
    pub metric: Option<MetricId>,
    pub strength: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerMetricInteractionContext {
    pub player: PlayerId,
    pub metric: MetricId,
    /// Spatially weighted opponent advantage. Positive is favorable to the player.
    pub opponent_advantage: f64,
    /// Spatially weighted teammate synergy from the selected XI.
    pub teammate_support: f64,
}

/// Build the explicit player↔player interaction graph for one metric. The opponent formation is
/// mirrored into the attacking team's coordinate system so a left winger naturally overlaps with
/// the opponent's right-sided defenders rather than their left-sided ones.
pub fn build_metric_matchup_edges(
    attacking: &SampledLineup,
    defending: &SampledLineup,
    states: &[PlayerMatchupState],
    rules: &[MatchupRule],
    metric: MetricId,
    spatial_sigma: f64,
) -> Option<Vec<MatchupEdge>> {
    if !spatial_sigma.is_finite() || spatial_sigma <= 0.0 || rules.iter().any(|rule| !rule.valid()) {
        return None;
    }
    let state_map: BTreeMap<PlayerId, AbilityVector> = states.iter().map(|value| (value.player, value.ability)).collect();
    let applicable: Vec<MatchupRule> = rules.iter().copied().filter(|rule| rule.metric.is_none() || rule.metric == Some(metric)).collect();
    let mut edges = Vec::with_capacity(attacking.starters.len() * defending.starters.len());
    for attacker in &attacking.starters {
        let attacker_ability = *state_map.get(&attacker.player)?;
        for defender in &defending.starters {
            let defender_ability = *state_map.get(&defender.player)?;
            let spatial_weight = opposing_spatial_overlap(attacker, defender, spatial_sigma);
            if spatial_weight < 1e-6 { continue; }
            edges.push(MatchupEdge {
                attacker: attacker.player,
                defender: defender.player,
                metric,
                spatial_weight,
                ability_advantage: ability_advantage(attacker_ability, defender_ability, &applicable),
            });
        }
    }
    Some(edges)
}

/// Collapse the graph into the two context terms consumed by a player's metric process. This is
/// deliberately separate from the event model: the same matchup primitive can influence dribbles,
/// crosses, tackles, aerials, xG, or any future metric with different sensitivities.
///
/// Batched variant: builds the metric matchup graph once and returns the collapsed context for
/// every starter. The per-player entry point above remains for single lookups; the match simulator
/// uses this batch form so the O(starters × defenders) graph is not rebuilt per player-metric on
/// every clock tick.
pub fn player_metric_interaction_contexts(
    own_lineup: &SampledLineup,
    opponent_lineup: &SampledLineup,
    states: &[PlayerMatchupState],
    rules: &[MatchupRule],
    synergies: &[PairwiseSynergy],
    metric: MetricId,
    spatial_sigma: f64,
) -> Option<BTreeMap<PlayerId, PlayerMetricInteractionContext>> {
    let edges = build_metric_matchup_edges(own_lineup, opponent_lineup, states, rules, metric, spatial_sigma)?;
    let mut numerator = BTreeMap::<PlayerId, f64>::new();
    let mut denominator = BTreeMap::<PlayerId, f64>::new();
    for edge in &edges {
        *numerator.entry(edge.attacker).or_default() += edge.ability_advantage * edge.spatial_weight;
        *denominator.entry(edge.attacker).or_default() += edge.spatial_weight;
    }
    let mut out = BTreeMap::new();
    for starter in &own_lineup.starters {
        let total = denominator.get(&starter.player).copied().unwrap_or(0.0);
        let opponent_advantage = if total > 0.0 {
            numerator.get(&starter.player).copied().unwrap_or(0.0) / total
        } else {
            0.0
        };
        let mut teammate_numerator = 0.0;
        let mut teammate_denominator = 0.0;
        for teammate in own_lineup.starters.iter().filter(|value| value.player != starter.player) {
            let Some(strength) = synergy_strength(starter.player, teammate.player, metric, synergies) else { continue; };
            if !strength.is_finite() { continue; }
            let weight = teammate_spatial_overlap(starter, teammate, spatial_sigma);
            teammate_numerator += strength * weight;
            teammate_denominator += weight;
        }
        let teammate_support = if teammate_denominator > 0.0 { teammate_numerator / teammate_denominator } else { 0.0 };
        out.insert(starter.player, PlayerMetricInteractionContext {
            player: starter.player, metric, opponent_advantage, teammate_support,
        });
    }
    Some(out)
}

pub fn player_metric_interaction_context(
    player: PlayerId,
    metric: MetricId,
    own_lineup: &SampledLineup,
    opponent_lineup: &SampledLineup,
    states: &[PlayerMatchupState],
    rules: &[MatchupRule],
    synergies: &[PairwiseSynergy],
    spatial_sigma: f64,
) -> Option<PlayerMetricInteractionContext> {
    let starter = own_lineup.starter(player)?;
    let edges = build_metric_matchup_edges(own_lineup, opponent_lineup, states, rules, metric, spatial_sigma)?;
    let mut opponent_numerator = 0.0;
    let mut opponent_denominator = 0.0;
    for edge in edges.iter().filter(|edge| edge.attacker == player) {
        opponent_numerator += edge.ability_advantage * edge.spatial_weight;
        opponent_denominator += edge.spatial_weight;
    }
    let opponent_advantage = if opponent_denominator > 0.0 { opponent_numerator / opponent_denominator } else { 0.0 };

    let mut teammate_numerator = 0.0;
    let mut teammate_denominator = 0.0;
    for teammate in own_lineup.starters.iter().filter(|value| value.player != player) {
        let Some(strength) = synergy_strength(player, teammate.player, metric, synergies) else { continue; };
        if !strength.is_finite() { continue; }
        let weight = teammate_spatial_overlap(starter, teammate, spatial_sigma);
        teammate_numerator += strength * weight;
        teammate_denominator += weight;
    }
    let teammate_support = if teammate_denominator > 0.0 { teammate_numerator / teammate_denominator } else { 0.0 };
    Some(PlayerMetricInteractionContext { player, metric, opponent_advantage, teammate_support })
}

pub fn pairwise_ability_advantage(
    attacker: PlayerMatchupState,
    defender: PlayerMatchupState,
    rules: &[MatchupRule],
    metric: MetricId,
) -> Option<f64> {
    if rules.iter().any(|rule| !rule.valid()) { return None; }
    let applicable: Vec<MatchupRule> = rules.iter().copied().filter(|rule| rule.metric.is_none() || rule.metric == Some(metric)).collect();
    Some(ability_advantage(attacker.ability, defender.ability, &applicable))
}

fn ability_advantage(attacker: AbilityVector, defender: AbilityVector, rules: &[MatchupRule]) -> f64 {
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for rule in rules {
        let attack = ability_component(attacker, rule.attacker_dimension);
        let defend = ability_component(defender, rule.defender_dimension);
        numerator += rule.weight * (attack - defend) / rule.scale;
        denominator += rule.weight.abs();
    }
    if denominator > 0.0 { numerator / denominator } else { 0.0 }
}

fn ability_component(value: AbilityVector, dimension: AbilityDimension) -> f64 {
    f64::from(match dimension {
        AbilityDimension::Technical => value.technical,
        AbilityDimension::BallProgression => value.ball_progression,
        AbilityDimension::Creativity => value.creativity,
        AbilityDimension::Finishing => value.finishing,
        AbilityDimension::Defending => value.defending,
        AbilityDimension::Aerial => value.aerial,
        AbilityDimension::Physical => value.physical,
        AbilityDimension::DecisionMaking => value.decision_making,
        AbilityDimension::Mentality => value.mentality,
        AbilityDimension::Goalkeeping => value.goalkeeping,
    })
}

fn opposing_spatial_overlap(attacker: &SampledStarter, defender: &SampledStarter, sigma: f64) -> f64 {
    // Opponent coordinates are defined from its own left/goal, so both axes mirror into the
    // attacking team's frame.
    let defender_x = 1.0 - defender.x;
    let defender_y = 1.0 - defender.y;
    gaussian_overlap(attacker.x - defender_x, attacker.y - defender_y, sigma)
}

fn teammate_spatial_overlap(left: &SampledStarter, right: &SampledStarter, sigma: f64) -> f64 {
    gaussian_overlap(left.x - right.x, left.y - right.y, sigma)
}

fn gaussian_overlap(dx: f64, dy: f64, sigma: f64) -> f64 {
    (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp()
}

fn synergy_strength(a: PlayerId, b: PlayerId, metric: MetricId, links: &[PairwiseSynergy]) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for link in links {
        if !((link.a == a && link.b == b) || (link.a == b && link.b == a)) { continue; }
        if link.metric.is_some() && link.metric != Some(metric) { continue; }
        total += link.strength;
        found = true;
    }
    found.then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineup::SampledStarter;
    use footy_core::{ClubId, RoleId};

    fn ability(attacking: f32, defending: f32) -> AbilityVector {
        AbilityVector {
            technical: attacking,
            ball_progression: attacking,
            creativity: attacking,
            finishing: attacking,
            defending,
            aerial: defending,
            physical: attacking,
            decision_making: attacking,
            mentality: defending,
            goalkeeping: 0.0,
        }
    }

    fn lineup(club: u32, player: u64, x: f64, y: f64) -> SampledLineup {
        SampledLineup {
            club: ClubId(club), formation_id: 1,
            starters: vec![SampledStarter { player: PlayerId(player), slot: 1, role: RoleId(1), x, y, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 }],
        }
    }

    #[test]
    fn stronger_attacker_gets_positive_direct_matchup() {
        let attack = lineup(1, 1, 0.75, 0.1);
        let defend = lineup(2, 2, 0.25, 0.9);
        let states = [
            PlayerMatchupState { player: PlayerId(1), ability: ability(16.0, 8.0) },
            PlayerMatchupState { player: PlayerId(2), ability: ability(8.0, 10.0) },
        ];
        let metric = MetricId(7);
        let rules = [MatchupRule { metric: Some(metric), attacker_dimension: AbilityDimension::BallProgression, defender_dimension: AbilityDimension::Defending, weight: 1.0, scale: 20.0 }];
        let context = player_metric_interaction_context(PlayerId(1), metric, &attack, &defend, &states, &rules, &[], 0.25).unwrap();
        assert!(context.opponent_advantage > 0.0);
    }

    #[test]
    fn batched_contexts_match_per_player_contexts() {
        let own = SampledLineup {
            club: ClubId(1), formation_id: 1,
            starters: vec![
                SampledStarter { player: PlayerId(1), slot: 1, role: RoleId(1), x: 0.7, y: 0.1, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
                SampledStarter { player: PlayerId(3), slot: 2, role: RoleId(2), x: 0.6, y: 0.15, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
                SampledStarter { player: PlayerId(4), slot: 3, role: RoleId(3), x: 0.4, y: 0.7, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
            ],
        };
        let opponent = SampledLineup {
            club: ClubId(2), formation_id: 1,
            starters: vec![
                SampledStarter { player: PlayerId(2), slot: 1, role: RoleId(1), x: 0.3, y: 0.9, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
                SampledStarter { player: PlayerId(5), slot: 2, role: RoleId(2), x: 0.5, y: 0.8, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
            ],
        };
        let states = [
            PlayerMatchupState { player: PlayerId(1), ability: ability(15.0, 9.0) },
            PlayerMatchupState { player: PlayerId(3), ability: ability(11.0, 10.0) },
            PlayerMatchupState { player: PlayerId(4), ability: ability(9.0, 12.0) },
            PlayerMatchupState { player: PlayerId(2), ability: ability(10.0, 14.0) },
            PlayerMatchupState { player: PlayerId(5), ability: ability(10.0, 10.0) },
        ];
        let metric = MetricId(7);
        let rules = [MatchupRule { metric: Some(metric), attacker_dimension: AbilityDimension::BallProgression, defender_dimension: AbilityDimension::Defending, weight: 1.0, scale: 20.0 }];
        let synergies = [PairwiseSynergy { a: PlayerId(1), b: PlayerId(3), metric: Some(metric), strength: 0.2 }];
        let batched = player_metric_interaction_contexts(&own, &opponent, &states, &rules, &synergies, metric, 0.25).unwrap();
        for starter in &own.starters {
            let single = player_metric_interaction_context(
                starter.player, metric, &own, &opponent, &states, &rules, &synergies, 0.25,
            ).unwrap();
            let batch = batched.get(&starter.player).copied().unwrap();
            assert_eq!(single, batch, "batched context must match the per-player path");
        }
    }

    #[test]
    fn metric_specific_teammate_link_is_exposed() {
        let own = SampledLineup {
            club: ClubId(1), formation_id: 1,
            starters: vec![
                SampledStarter { player: PlayerId(1), slot: 1, role: RoleId(1), x: 0.7, y: 0.1, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
                SampledStarter { player: PlayerId(3), slot: 2, role: RoleId(2), x: 0.6, y: 0.15, minutes: 90.0, role_fit: 1.0, fatigue: 0.0 },
            ],
        };
        let opponent = lineup(2, 2, 0.2, 0.9);
        let states = [
            PlayerMatchupState { player: PlayerId(1), ability: ability(10.0, 10.0) },
            PlayerMatchupState { player: PlayerId(2), ability: ability(10.0, 10.0) },
            PlayerMatchupState { player: PlayerId(3), ability: ability(10.0, 10.0) },
        ];
        let metric = MetricId(4);
        let context = player_metric_interaction_context(
            PlayerId(1), metric, &own, &opponent, &states, &[],
            &[PairwiseSynergy { a: PlayerId(1), b: PlayerId(3), metric: Some(metric), strength: 0.2 }], 0.3,
        ).unwrap();
        assert!(context.teammate_support > 0.19);
    }
}
