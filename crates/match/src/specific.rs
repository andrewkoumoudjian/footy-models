use crate::channels::{CausalChannelGraph, TeamProcessChannel};
use crate::lineup::{project_lineup, sample_lineup, LineupCandidate, LineupProjection, LineupProjectionInput, SampledLineup, SampledStarter};
use crate::matchup::{player_metric_interaction_context, MatchupRule, PairwiseSynergy, PlayerMatchupState};
use footy_core::{AttributeId, ClubId, MatchId, MetricId, PlayerId, Probability};
use footy_forecast::{DistributionSummary, MatchOutcomeForecast, Rng64, StatFamily};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerAttributeValue {
    pub attribute: AttributeId,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAttributeState {
    pub player: PlayerId,
    pub attributes: Vec<PlayerAttributeValue>,
}

impl PlayerAttributeState {
    pub fn value(&self, attribute: AttributeId) -> Option<f64> {
        self.attributes.iter().find(|value| value.attribute == attribute).map(|value| value.value)
    }

    fn valid(&self) -> bool {
        let mut ids = BTreeSet::new();
        self.attributes.iter().all(|value| ids.insert(value.attribute) && value.value.is_finite())
    }
}

/// How one raw canonical FM attribute moves a player's own event rate. This is different from a
/// direct matchup: e.g. finishing can affect shot conversion even when no specific defender is the
/// relevant counterpart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeEventLoading {
    pub metric: MetricId,
    pub attribute: AttributeId,
    pub reference: f64,
    pub scale: f64,
    pub log_rate_weight: f64,
}

impl AttributeEventLoading {
    fn valid(self) -> bool {
        self.reference.is_finite() && self.scale.is_finite() && self.scale > 0.0 && self.log_rate_weight.is_finite()
    }
}

/// Fine-grained direct opponent primitive such as pace-vs-recovery, dribbling-vs-tackling,
/// crossing-vs-positioning, aerial-vs-aerial, or strength-vs-strength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeMatchupRule {
    pub metric: Option<MetricId>,
    pub attacker_attribute: AttributeId,
    pub defender_attribute: AttributeId,
    pub weight: f64,
    pub scale: f64,
}

impl AttributeMatchupRule {
    fn valid(self) -> bool { self.weight.is_finite() && self.scale.is_finite() && self.scale > 0.0 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificPlayerEventModel {
    pub player: PlayerId,
    pub metric: MetricId,
    pub family: StatFamily,
    pub mean_per90: f64,
    pub variance_per90: f64,
    pub epistemic_sigma: f64,
    pub reference_per90: f64,
    pub aggregate_matchup_sensitivity: f64,
    pub attribute_matchup_sensitivity: f64,
    pub teammate_sensitivity: f64,
    pub role_fit_sensitivity: f64,
    pub fatigue_sensitivity: f64,
}

impl SpecificPlayerEventModel {
    fn valid(self) -> bool {
        non_negative(self.mean_per90)
            && non_negative(self.variance_per90)
            && non_negative(self.epistemic_sigma)
            && self.reference_per90.is_finite()
            && self.reference_per90 > 0.0
            && self.aggregate_matchup_sensitivity.is_finite()
            && self.attribute_matchup_sensitivity.is_finite()
            && self.teammate_sensitivity.is_finite()
            && non_negative(self.role_fit_sensitivity)
            && non_negative(self.fatigue_sensitivity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificTeamMetricModifier {
    pub metric: MetricId,
    pub production_multiplier: f64,
    pub opponent_suppression_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecificTeamContext {
    pub club: ClubId,
    /// Residual team state after individual player processes: team structure, coaching/system,
    /// broad form not explained by the modeled player events, set-piece organization, etc.
    pub residual_attack_log_adjustment: f64,
    pub residual_defense_log_adjustment: f64,
    pub metric_modifiers: Vec<SpecificTeamMetricModifier>,
}

impl SpecificTeamContext {
    fn valid(&self) -> bool {
        self.residual_attack_log_adjustment.is_finite()
            && self.residual_defense_log_adjustment.is_finite()
            && self.metric_modifiers.iter().all(|value| {
                value.production_multiplier.is_finite() && value.production_multiplier > 0.0
                    && value.opponent_suppression_multiplier.is_finite() && value.opponent_suppression_multiplier > 0.0
            })
    }

    fn production_multiplier(&self, metric: MetricId) -> f64 {
        self.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| value.production_multiplier).product()
    }

    fn suppression_multiplier(&self, metric: MetricId) -> f64 {
        self.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| value.opponent_suppression_multiplier).product()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificEnvironmentMetricModifier {
    pub metric: MetricId,
    pub home_multiplier: f64,
    pub away_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecificMatchEnvironment {
    /// Venue/rest/travel/weather/referee/importance residuals. Prefer metric-level modifiers when a
    /// mechanism is known; these residuals exist for effects not represented by a modeled metric.
    pub home_residual_attack_log_adjustment: f64,
    pub away_residual_attack_log_adjustment: f64,
    pub metric_modifiers: Vec<SpecificEnvironmentMetricModifier>,
}

impl SpecificMatchEnvironment {
    fn valid(&self) -> bool {
        self.home_residual_attack_log_adjustment.is_finite()
            && self.away_residual_attack_log_adjustment.is_finite()
            && self.metric_modifiers.iter().all(|value| {
                value.home_multiplier.is_finite() && value.home_multiplier > 0.0
                    && value.away_multiplier.is_finite() && value.away_multiplier > 0.0
            })
    }

    fn multiplier(&self, metric: MetricId, is_home: bool) -> f64 {
        self.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| {
            if is_home { value.home_multiplier } else { value.away_multiplier }
        }).product()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificScoreStateMetricModifier {
    pub metric: MetricId,
    pub trailing_multiplier: f64,
    pub leading_multiplier: f64,
}

impl SpecificScoreStateMetricModifier {
    fn valid(self) -> bool {
        self.trailing_multiplier.is_finite() && self.trailing_multiplier > 0.0
            && self.leading_multiplier.is_finite() && self.leading_multiplier > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificPlayerIncidentProfile {
    pub player: PlayerId,
    pub injury_multiplier: f64,
    pub red_card_multiplier: f64,
}

impl SpecificPlayerIncidentProfile {
    fn valid(self) -> bool { non_negative(self.injury_multiplier) && non_negative(self.red_card_multiplier) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificInMatchParameters {
    pub injury_probability_per90: f64,
    pub red_card_probability_per90: f64,
    pub max_substitutions: u8,
    pub minimum_tactical_substitution_minute: f64,
}

impl SpecificInMatchParameters {
    pub fn valid(self) -> bool {
        self.injury_probability_per90.is_finite() && (0.0..=1.0).contains(&self.injury_probability_per90)
            && self.red_card_probability_per90.is_finite() && (0.0..=1.0).contains(&self.red_card_probability_per90)
            && self.max_substitutions <= 12
            && self.minimum_tactical_substitution_minute.is_finite()
            && (0.0..=90.0).contains(&self.minimum_tactical_substitution_minute)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificGoalProcess {
    pub goals_per_team_match: f64,
    pub home_advantage_multiplier: f64,
    pub path_log_rate_sigma: f64,
    /// Correlation between the two teams' per-path log goal rates. Zero reproduces the
    /// legacy independent sampling exactly (same RNG draws); positive values model shared
    /// match tempo (open/closed game affects both attacks together). Opt-in experiment
    /// parameter: stays 0.0 until walk-forward OOS evidence supports a nonzero value.
    pub path_rate_correlation: f64,
    pub max_abs_segment_log_adjustment: f64,
    /// Player events and score state are recomputed this many times. Six = 15-minute blocks.
    pub segments: u8,
}

impl SpecificGoalProcess {
    pub fn valid(self) -> bool {
        self.goals_per_team_match.is_finite() && self.goals_per_team_match > 0.0
            && self.home_advantage_multiplier.is_finite() && self.home_advantage_multiplier > 0.0
            && non_negative(self.path_log_rate_sigma)
            && self.path_rate_correlation.is_finite()
            && (-0.5..=0.9).contains(&self.path_rate_correlation)
            && self.max_abs_segment_log_adjustment.is_finite() && self.max_abs_segment_log_adjustment > 0.0
            && self.segments > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveMatchSimulationInput {
    pub match_id: MatchId,
    pub home: ClubId,
    pub away: ClubId,
    pub home_lineup: LineupProjectionInput,
    pub away_lineup: LineupProjectionInput,
    /// Coarse state remains useful for role-level priors and aggregate matchup rules.
    pub player_states: Vec<PlayerMatchupState>,
    /// Raw FM AttributeId values provide exact matchup mechanisms where available.
    pub attribute_states: Vec<PlayerAttributeState>,
    pub event_models: Vec<SpecificPlayerEventModel>,
    pub own_attribute_loadings: Vec<AttributeEventLoading>,
    pub aggregate_matchup_rules: Vec<MatchupRule>,
    pub attribute_matchup_rules: Vec<AttributeMatchupRule>,
    pub synergies: Vec<PairwiseSynergy>,
    pub channel_graph: CausalChannelGraph,
    pub score_state_modifiers: Vec<SpecificScoreStateMetricModifier>,
    pub incident_profiles: Vec<SpecificPlayerIncidentProfile>,
    pub in_match: SpecificInMatchParameters,
    pub home_context: SpecificTeamContext,
    pub away_context: SpecificTeamContext,
    pub environment: SpecificMatchEnvironment,
    pub goal_process: SpecificGoalProcess,
    pub matchup_spatial_sigma: f64,
    pub max_player_rate_log_adjustment: f64,
}

impl PrimitiveMatchSimulationInput {
    pub fn validate(&self) -> bool {
        if self.home == self.away
            || self.home_lineup.club != self.home || self.away_lineup.club != self.away
            || self.home_context.club != self.home || self.away_context.club != self.away
            || !self.home_lineup.validate() || !self.away_lineup.validate()
            || !self.home_context.valid() || !self.away_context.valid() || !self.environment.valid()
            || !self.channel_graph.validate() || !self.in_match.valid() || !self.goal_process.valid()
            || !self.matchup_spatial_sigma.is_finite() || self.matchup_spatial_sigma <= 0.0
            || !self.max_player_rate_log_adjustment.is_finite() || self.max_player_rate_log_adjustment <= 0.0
            || self.event_models.iter().any(|value| !value.valid())
            || self.own_attribute_loadings.iter().any(|value| !value.valid())
            || self.attribute_matchup_rules.iter().any(|value| !value.valid())
            || self.score_state_modifiers.iter().any(|value| !value.valid())
            || self.incident_profiles.iter().any(|value| !value.valid())
            || self.attribute_states.iter().any(|value| !value.valid())
            || self.synergies.iter().any(|value| !value.strength.is_finite())
        { return false; }
        let mut model_keys = BTreeSet::new();
        if !self.event_models.iter().all(|value| model_keys.insert((value.player, value.metric))) { return false; }
        let states: BTreeSet<PlayerId> = self.player_states.iter().map(|value| value.player).collect();
        let models: BTreeSet<PlayerId> = self.event_models.iter().map(|value| value.player).collect();
        let attributes: BTreeSet<PlayerId> = self.attribute_states.iter().map(|value| value.player).collect();
        let needs_attributes = !self.own_attribute_loadings.is_empty() || !self.attribute_matchup_rules.is_empty();
        let candidates: Vec<PlayerId> = self.home_lineup.candidates.iter().chain(self.away_lineup.candidates.iter()).map(|value| value.player).collect();
        if candidates.iter().any(|player| !states.contains(player) || !models.contains(player) || (needs_attributes && !attributes.contains(player))) { return false; }
        let incident_ids: BTreeSet<PlayerId> = self.incident_profiles.iter().map(|value| value.player).collect();
        incident_ids.len() == self.incident_profiles.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimitivePlayerMetricForecast {
    pub player: PlayerId,
    pub metric: MetricId,
    pub start_probability: Probability,
    pub appearance_probability: Probability,
    pub expected_minutes: f64,
    pub unconditional_total: DistributionSummary,
    pub conditional_on_appearance_total: Option<DistributionSummary>,
    pub conditional_on_appearance_per90: Option<DistributionSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimitivePlayerInfluence {
    pub player: PlayerId,
    pub start_probability: Probability,
    pub appearance_probability: Probability,
    pub expected_minutes: f64,
    pub mean_own_attribute_log_modifier: f64,
    pub mean_aggregate_matchup_log_modifier: f64,
    pub mean_attribute_matchup_log_modifier: f64,
    pub mean_teammate_log_modifier: f64,
    pub mean_role_log_modifier: f64,
    pub mean_fatigue_log_modifier: f64,
    pub mean_system_environment_log_modifier: f64,
    pub mean_score_state_log_modifier: f64,
    pub red_card_probability: Probability,
    pub injury_exit_probability: Probability,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimitivePlayerChannelInfluence {
    pub player: PlayerId,
    pub channel: TeamProcessChannel,
    pub mean_contribution: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimitiveTeamChannelForecast {
    pub club: ClubId,
    pub channel: TeamProcessChannel,
    pub mean_match_contribution: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveMatchForecast {
    pub outcome: MatchOutcomeForecast,
    pub home_lineup: LineupProjection,
    pub away_lineup: LineupProjection,
    pub player_metrics: Vec<PrimitivePlayerMetricForecast>,
    pub player_influences: Vec<PrimitivePlayerInfluence>,
    pub player_channels: Vec<PrimitivePlayerChannelInfluence>,
    pub team_channels: Vec<PrimitiveTeamChannelForecast>,
    pub paths: u32,
}

#[derive(Debug, Clone)]
struct ActivePlayer {
    sampled: SampledStarter,
    planned_exit: f64,
}

#[derive(Debug, Clone)]
struct ActiveTeam {
    club: ClubId,
    formation_id: u16,
    players: Vec<ActivePlayer>,
    used: BTreeSet<PlayerId>,
    substitutions: u8,
}

impl ActiveTeam {
    fn from_start(lineup: SampledLineup) -> Self {
        let used = lineup.starters.iter().map(|value| value.player).collect();
        let players = lineup.starters.into_iter().map(|sampled| ActivePlayer { planned_exit: sampled.minutes, sampled }).collect();
        Self { club: lineup.club, formation_id: lineup.formation_id, players, used, substitutions: 0 }
    }

    fn lineup(&self) -> SampledLineup {
        SampledLineup { club: self.club, formation_id: self.formation_id, starters: self.players.iter().map(|value| value.sampled).collect() }
    }
}

#[derive(Default)]
struct PathPlayer {
    started: bool,
    appeared: bool,
    minutes: f64,
    red: bool,
    injury: bool,
    factors: u32,
    own_attribute: f64,
    aggregate_matchup: f64,
    attribute_matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    score: f64,
    channels: BTreeMap<TeamProcessChannel, f64>,
}

#[derive(Default)]
struct AggregatePlayer {
    starts: u32,
    appearances: u32,
    minutes: f64,
    reds: u32,
    injuries: u32,
    factors: u32,
    own_attribute: f64,
    aggregate_matchup: f64,
    attribute_matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    score: f64,
    channels: BTreeMap<TeamProcessChannel, f64>,
}

#[derive(Default)]
struct PathMetric { total: f64, minutes: f64 }
#[derive(Default)]
struct AggregateMetric { unconditional: Vec<f64>, conditional: Vec<f64>, conditional_per90: Vec<f64> }

struct SegmentMetric {
    player: PlayerId,
    metric: MetricId,
    total: f64,
    deviation: f64,
    own_attribute: f64,
    aggregate_matchup: f64,
    attribute_matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    score: f64,
}

struct SegmentOutput {
    channels: Vec<(TeamProcessChannel, f64)>,
    attack: f64,
    defense: f64,
    metrics: Vec<SegmentMetric>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreState { Trailing, Level, Leading }

/// Primitive-first specific fixture simulation:
/// projected XI -> active roles -> player-vs-player interactions -> player events -> causal team
/// channels -> segment scoring -> game-state events -> next segment.
pub fn simulate_primitive_match(input: &PrimitiveMatchSimulationInput, paths: u32, seed: u64) -> Option<PrimitiveMatchForecast> {
    if paths == 0 || !input.validate() { return None; }
    let mut rng = Rng64::new(seed);
    let segment_minutes = 90.0 / f64::from(input.goal_process.segments);
    let mut home_goal_samples = Vec::with_capacity(paths as usize);
    let mut away_goal_samples = Vec::with_capacity(paths as usize);
    let mut home_wins = 0_u32;
    let mut draws = 0_u32;
    let mut away_wins = 0_u32;
    let mut players = BTreeMap::<PlayerId, AggregatePlayer>::new();
    let mut metrics = BTreeMap::<(PlayerId, MetricId), AggregateMetric>::new();
    let mut team_channels = BTreeMap::<(ClubId, TeamProcessChannel), f64>::new();
    for model in &input.event_models { metrics.entry((model.player, model.metric)).or_default(); }

    for _ in 0..paths {
        let home_start = sample_lineup(&input.home_lineup, &mut rng)?;
        let away_start = sample_lineup(&input.away_lineup, &mut rng)?;
        let mut home = ActiveTeam::from_start(home_start.clone());
        let mut away = ActiveTeam::from_start(away_start.clone());
        let mut path_players = BTreeMap::<PlayerId, PathPlayer>::new();
        let mut path_metrics = BTreeMap::<(PlayerId, MetricId), PathMetric>::new();
        for value in home_start.starters.iter().chain(away_start.starters.iter()) {
            path_players.entry(value.player).or_default().started = true;
        }
        let (path_home_rate, path_away_rate) = sample_correlated_log_rates(
            input.goal_process.goals_per_team_match * input.goal_process.home_advantage_multiplier,
            input.goal_process.goals_per_team_match,
            input.goal_process.path_log_rate_sigma,
            input.goal_process.path_rate_correlation,
            &mut rng,
        );
        let mut home_goals = 0_u32;
        let mut away_goals = 0_u32;

        for segment in 0..input.goal_process.segments {
            let start_minute = f64::from(segment) * segment_minutes;
            let end_minute = f64::from(segment + 1) * segment_minutes;
            let home_lineup = home.lineup();
            let away_lineup = away.lineup();
            let home_output = segment_output(input, &home_lineup, &away_lineup, true, score_state(home_goals, away_goals), segment_minutes, &mut rng)?;
            let away_output = segment_output(input, &away_lineup, &home_lineup, false, score_state(away_goals, home_goals), segment_minutes, &mut rng)?;
            accumulate_segment(&home_output, &home, input.home, segment_minutes, &mut path_players, &mut path_metrics, &mut team_channels);
            accumulate_segment(&away_output, &away, input.away, segment_minutes, &mut path_players, &mut path_metrics, &mut team_channels);

            let home_log = (input.environment.home_residual_attack_log_adjustment
                + input.home_context.residual_attack_log_adjustment - input.away_context.residual_defense_log_adjustment
                + home_output.attack - away_output.defense)
                .clamp(-input.goal_process.max_abs_segment_log_adjustment, input.goal_process.max_abs_segment_log_adjustment);
            let away_log = (input.environment.away_residual_attack_log_adjustment
                + input.away_context.residual_attack_log_adjustment - input.home_context.residual_defense_log_adjustment
                + away_output.attack - home_output.defense)
                .clamp(-input.goal_process.max_abs_segment_log_adjustment, input.goal_process.max_abs_segment_log_adjustment);
            home_goals = home_goals.saturating_add(poisson(path_home_rate * home_log.exp() / f64::from(input.goal_process.segments), &mut rng));
            away_goals = away_goals.saturating_add(poisson(path_away_rate * away_log.exp() / f64::from(input.goal_process.segments), &mut rng));

            if segment + 1 < input.goal_process.segments {
                evolve_active_team(input, &mut home, &input.home_lineup, start_minute, end_minute, segment_minutes, &mut path_players, &mut rng)?;
                evolve_active_team(input, &mut away, &input.away_lineup, start_minute, end_minute, segment_minutes, &mut path_players, &mut rng)?;
            }
        }

        home_goal_samples.push(f64::from(home_goals));
        away_goal_samples.push(f64::from(away_goals));
        match home_goals.cmp(&away_goals) {
            std::cmp::Ordering::Greater => home_wins = home_wins.saturating_add(1),
            std::cmp::Ordering::Equal => draws = draws.saturating_add(1),
            std::cmp::Ordering::Less => away_wins = away_wins.saturating_add(1),
        }
        for ((player, metric), aggregate) in &mut metrics {
            if let Some(value) = path_metrics.get(&(*player, *metric)) {
                aggregate.unconditional.push(value.total);
                if value.minutes > 0.0 {
                    aggregate.conditional.push(value.total);
                    aggregate.conditional_per90.push(value.total * 90.0 / value.minutes);
                }
            } else { aggregate.unconditional.push(0.0); }
        }
        for (player, value) in path_players {
            let aggregate = players.entry(player).or_default();
            if value.started { aggregate.starts = aggregate.starts.saturating_add(1); }
            if value.appeared { aggregate.appearances = aggregate.appearances.saturating_add(1); }
            if value.red { aggregate.reds = aggregate.reds.saturating_add(1); }
            if value.injury { aggregate.injuries = aggregate.injuries.saturating_add(1); }
            aggregate.minutes += value.minutes;
            aggregate.factors = aggregate.factors.saturating_add(value.factors);
            aggregate.own_attribute += value.own_attribute;
            aggregate.aggregate_matchup += value.aggregate_matchup;
            aggregate.attribute_matchup += value.attribute_matchup;
            aggregate.teammate += value.teammate;
            aggregate.role += value.role;
            aggregate.fatigue += value.fatigue;
            aggregate.system += value.system;
            aggregate.score += value.score;
            for (channel, amount) in value.channels { *aggregate.channels.entry(channel).or_default() += amount; }
        }
    }

    let denominator = paths as f32;
    let home_win = Probability::clamped(home_wins as f32 / denominator);
    let draw = Probability::clamped(draws as f32 / denominator);
    let away_win = Probability::clamped(away_wins as f32 / denominator);
    let outcome = MatchOutcomeForecast {
        match_id: input.match_id, home: input.home, away: input.away, home_win, draw, away_win,
        home_goals: summarize(&home_goal_samples)?, away_goals: summarize(&away_goal_samples)?,
        home_expected_points: 3.0 * f64::from(home_win.get()) + f64::from(draw.get()),
        away_expected_points: 3.0 * f64::from(away_win.get()) + f64::from(draw.get()), paths,
    };
    let home_lineup = project_lineup(&input.home_lineup, paths, seed ^ 0x5052_494d_484f_4d45)?;
    let away_lineup = project_lineup(&input.away_lineup, paths, seed ^ 0x5052_494d_4157_4159)?;
    let player_metrics = metrics.into_iter().map(|((player, metric), value)| {
        let summary = players.get(&player);
        Some(PrimitivePlayerMetricForecast {
            player, metric,
            start_probability: Probability::clamped(summary.map(|value| value.starts).unwrap_or(0) as f32 / denominator),
            appearance_probability: Probability::clamped(summary.map(|value| value.appearances).unwrap_or(0) as f32 / denominator),
            expected_minutes: summary.map(|value| value.minutes / f64::from(paths)).unwrap_or(0.0),
            unconditional_total: summarize(&value.unconditional)?,
            conditional_on_appearance_total: summarize(&value.conditional),
            conditional_on_appearance_per90: summarize(&value.conditional_per90),
        })
    }).collect::<Option<Vec<_>>>()?;
    let mut player_channels = Vec::new();
    let player_influences = players.into_iter().map(|(player, value)| {
        let factor_denominator = f64::from(value.factors.max(1));
        for (channel, amount) in &value.channels {
            player_channels.push(PrimitivePlayerChannelInfluence { player, channel: *channel, mean_contribution: *amount / f64::from(paths) });
        }
        PrimitivePlayerInfluence {
            player,
            start_probability: Probability::clamped(value.starts as f32 / denominator),
            appearance_probability: Probability::clamped(value.appearances as f32 / denominator),
            expected_minutes: value.minutes / f64::from(paths),
            mean_own_attribute_log_modifier: value.own_attribute / factor_denominator,
            mean_aggregate_matchup_log_modifier: value.aggregate_matchup / factor_denominator,
            mean_attribute_matchup_log_modifier: value.attribute_matchup / factor_denominator,
            mean_teammate_log_modifier: value.teammate / factor_denominator,
            mean_role_log_modifier: value.role / factor_denominator,
            mean_fatigue_log_modifier: value.fatigue / factor_denominator,
            mean_system_environment_log_modifier: value.system / factor_denominator,
            mean_score_state_log_modifier: value.score / factor_denominator,
            red_card_probability: Probability::clamped(value.reds as f32 / denominator),
            injury_exit_probability: Probability::clamped(value.injuries as f32 / denominator),
        }
    }).collect();
    player_channels.sort_by_key(|value| (value.player, value.channel));
    let team_channels = team_channels.into_iter().map(|((club, channel), total)| PrimitiveTeamChannelForecast {
        club, channel, mean_match_contribution: total / f64::from(paths),
    }).collect();
    Some(PrimitiveMatchForecast { outcome, home_lineup, away_lineup, player_metrics, player_influences, player_channels, team_channels, paths })
}

fn segment_output(
    input: &PrimitiveMatchSimulationInput,
    own: &SampledLineup,
    opponent: &SampledLineup,
    is_home: bool,
    score: ScoreState,
    segment_minutes: f64,
    rng: &mut Rng64,
) -> Option<SegmentOutput> {
    let own_context = if is_home { &input.home_context } else { &input.away_context };
    let opponent_context = if is_home { &input.away_context } else { &input.home_context };
    let attributes: BTreeMap<PlayerId, &PlayerAttributeState> = input.attribute_states.iter().map(|value| (value.player, value)).collect();
    let mut deviations = Vec::new();
    let mut segment_metrics = Vec::new();
    for starter in &own.starters {
        for model in input.event_models.iter().filter(|value| value.player == starter.player) {
            let aggregate = player_metric_interaction_context(
                starter.player, model.metric, own, opponent, &input.player_states,
                &input.aggregate_matchup_rules, &input.synergies, input.matchup_spatial_sigma,
            )?;
            let own_attribute_log = attributes.get(&starter.player).map(|state| own_attribute_adjustment(state, model.metric, &input.own_attribute_loadings)).unwrap_or(0.0);
            let attribute_advantage = fine_matchup_advantage(
                starter, opponent, attributes.get(&starter.player).copied(), &attributes,
                model.metric, &input.attribute_matchup_rules, input.matchup_spatial_sigma,
            )?;
            let aggregate_matchup_log = model.aggregate_matchup_sensitivity * aggregate.opponent_advantage;
            let attribute_matchup_log = model.attribute_matchup_sensitivity * attribute_advantage;
            let teammate_log = model.teammate_sensitivity * aggregate.teammate_support;
            let role_log = model.role_fit_sensitivity * (starter.role_fit - 1.0);
            let fatigue_log = -model.fatigue_sensitivity * starter.fatigue;
            let system_multiplier = own_context.production_multiplier(model.metric)
                * opponent_context.suppression_multiplier(model.metric)
                * input.environment.multiplier(model.metric, is_home);
            if !system_multiplier.is_finite() || system_multiplier <= 0.0 { return None; }
            let system_log = system_multiplier.ln();
            let score_log = score_multiplier(model.metric, score, &input.score_state_modifiers).ln();
            let log_adjustment = (own_attribute_log + aggregate_matchup_log + attribute_matchup_log + teammate_log + role_log + fatigue_log + system_log + score_log)
                .clamp(-input.max_player_rate_log_adjustment, input.max_player_rate_log_adjustment);
            let multiplier = log_adjustment.exp();
            let latent = sample_non_negative_normal(model.mean_per90 * multiplier, model.epistemic_sigma * multiplier, rng);
            let rate = sample_non_negative_normal(latent, model.variance_per90.sqrt() * multiplier, rng);
            let exposure = segment_minutes / 90.0;
            let total = match model.family {
                StatFamily::ContinuousNonNegative => rate * exposure,
                StatFamily::Count => f64::from(poisson(rate * exposure, rng)),
            };
            let effective_rate = if exposure > 0.0 { total / exposure } else { 0.0 };
            // Channel state is a rate condition. Segment duration already scales the event total
            // and the segment goal exposure, so applying exposure here again makes the result
            // depend on the arbitrary number of simulation segments.
            let deviation = effective_rate / model.reference_per90 - 1.0;
            deviations.push((model.metric, deviation));
            segment_metrics.push(SegmentMetric {
                player: starter.player, metric: model.metric, total, deviation,
                own_attribute: own_attribute_log, aggregate_matchup: aggregate_matchup_log,
                attribute_matchup: attribute_matchup_log, teammate: teammate_log, role: role_log,
                fatigue: fatigue_log, system: system_log, score: score_log,
            });
        }
    }
    let channels = input.channel_graph.evaluate(&deviations)?;
    Some(SegmentOutput { channels: channels.values, attack: channels.attack_log_adjustment, defense: channels.defense_log_adjustment, metrics: segment_metrics })
}

fn fine_matchup_advantage(
    attacker: &SampledStarter,
    opponent: &SampledLineup,
    attacker_state: Option<&PlayerAttributeState>,
    states: &BTreeMap<PlayerId, &PlayerAttributeState>,
    metric: MetricId,
    rules: &[AttributeMatchupRule],
    sigma: f64,
) -> Option<f64> {
    let applicable: Vec<AttributeMatchupRule> = rules.iter().copied().filter(|value| value.metric.is_none() || value.metric == Some(metric)).collect();
    if applicable.is_empty() { return Some(0.0); }
    let attacker_state = attacker_state?;
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for defender in &opponent.starters {
        let defender_state = *states.get(&defender.player)?;
        let spatial = opponent_overlap(attacker, defender, sigma);
        if spatial < 1e-8 { continue; }
        let mut pair = 0.0;
        let mut pair_weight = 0.0;
        for rule in &applicable {
            let Some(a) = attacker_state.value(rule.attacker_attribute) else { continue; };
            let Some(d) = defender_state.value(rule.defender_attribute) else { continue; };
            pair += rule.weight * (a - d) / rule.scale;
            pair_weight += rule.weight.abs();
        }
        if pair_weight <= 0.0 { continue; }
        numerator += spatial * pair / pair_weight;
        denominator += spatial;
    }
    Some(if denominator > 0.0 { numerator / denominator } else { 0.0 })
}

fn own_attribute_adjustment(state: &PlayerAttributeState, metric: MetricId, values: &[AttributeEventLoading]) -> f64 {
    values.iter().filter(|value| value.metric == metric).filter_map(|value| {
        state.value(value.attribute).map(|actual| value.log_rate_weight * (actual - value.reference) / value.scale)
    }).sum()
}

fn opponent_overlap(attacker: &SampledStarter, defender: &SampledStarter, sigma: f64) -> f64 {
    let dx = attacker.x - (1.0 - defender.x);
    let dy = attacker.y - (1.0 - defender.y);
    (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp()
}

fn score_multiplier(metric: MetricId, score: ScoreState, values: &[SpecificScoreStateMetricModifier]) -> f64 {
    values.iter().filter(|value| value.metric == metric).map(|value| match score {
        ScoreState::Trailing => value.trailing_multiplier,
        ScoreState::Level => 1.0,
        ScoreState::Leading => value.leading_multiplier,
    }).product()
}

fn score_state(own: u32, opponent: u32) -> ScoreState {
    match own.cmp(&opponent) {
        std::cmp::Ordering::Less => ScoreState::Trailing,
        std::cmp::Ordering::Equal => ScoreState::Level,
        std::cmp::Ordering::Greater => ScoreState::Leading,
    }
}

fn accumulate_segment(
    output: &SegmentOutput,
    active: &ActiveTeam,
    club: ClubId,
    segment_minutes: f64,
    players: &mut BTreeMap<PlayerId, PathPlayer>,
    metrics: &mut BTreeMap<(PlayerId, MetricId), PathMetric>,
    team_channels: &mut BTreeMap<(ClubId, TeamProcessChannel), f64>,
) {
    for value in &active.players {
        let player = players.entry(value.sampled.player).or_default();
        player.appeared = true;
        player.minutes += segment_minutes;
    }
    for (channel, amount) in &output.channels { *team_channels.entry((club, *channel)).or_default() += *amount; }
    let absolute_deviation = output.metrics.iter().map(|value| value.deviation.abs()).sum::<f64>();
    for value in &output.metrics {
        let metric = metrics.entry((value.player, value.metric)).or_default();
        metric.total += value.total;
        metric.minutes += segment_minutes;
        let player = players.entry(value.player).or_default();
        player.factors = player.factors.saturating_add(1);
        player.own_attribute += value.own_attribute;
        player.aggregate_matchup += value.aggregate_matchup;
        player.attribute_matchup += value.attribute_matchup;
        player.teammate += value.teammate;
        player.role += value.role;
        player.fatigue += value.fatigue;
        player.system += value.system;
        player.score += value.score;
        if absolute_deviation > 0.0 {
            let share = value.deviation.abs() / absolute_deviation;
            for (channel, amount) in &output.channels { *player.channels.entry(*channel).or_default() += share * *amount; }
        }
    }
}

fn evolve_active_team(
    input: &PrimitiveMatchSimulationInput,
    active: &mut ActiveTeam,
    projection: &LineupProjectionInput,
    _start_minute: f64,
    end_minute: f64,
    segment_minutes: f64,
    path_players: &mut BTreeMap<PlayerId, PathPlayer>,
    rng: &mut Rng64,
) -> Option<()> {
    let incident_map: BTreeMap<PlayerId, SpecificPlayerIncidentProfile> = input.incident_profiles.iter().copied().map(|value| (value.player, value)).collect();
    let injury_base = segment_probability(input.in_match.injury_probability_per90, segment_minutes);
    let red_base = segment_probability(input.in_match.red_card_probability_per90, segment_minutes);
    let mut removals = BTreeSet::new();
    let mut replacements = Vec::new();
    for value in &active.players {
        let profile = incident_map.get(&value.sampled.player).copied().unwrap_or(SpecificPlayerIncidentProfile { player: value.sampled.player, injury_multiplier: 1.0, red_card_multiplier: 1.0 });
        if rng.bernoulli((red_base * profile.red_card_multiplier).clamp(0.0, 1.0)) {
            removals.insert(value.sampled.player);
            path_players.entry(value.sampled.player).or_default().red = true;
            continue;
        }
        if rng.bernoulli((injury_base * profile.injury_multiplier).clamp(0.0, 1.0)) {
            removals.insert(value.sampled.player);
            path_players.entry(value.sampled.player).or_default().injury = true;
            replacements.push(value.sampled);
        }
    }
    if end_minute >= input.in_match.minimum_tactical_substitution_minute {
        for value in &active.players {
            if removals.contains(&value.sampled.player) { continue; }
            if value.planned_exit <= end_minute + 0.5 * segment_minutes && end_minute < 90.0 {
                removals.insert(value.sampled.player);
                replacements.push(value.sampled);
            }
        }
    }
    active.players.retain(|value| !removals.contains(&value.sampled.player));
    for outgoing in replacements {
        if active.substitutions >= input.in_match.max_substitutions { break; }
        let Some(replacement) = sample_replacement(projection, active, outgoing, end_minute, rng) else { continue; };
        active.used.insert(replacement.sampled.player);
        active.players.push(replacement);
        active.substitutions = active.substitutions.saturating_add(1);
    }
    Some(())
}

fn sample_replacement(
    projection: &LineupProjectionInput,
    active: &ActiveTeam,
    outgoing: SampledStarter,
    entered_minute: f64,
    rng: &mut Rng64,
) -> Option<ActivePlayer> {
    let candidates: Vec<&LineupCandidate> = projection.candidates.iter().filter(|value| {
        !active.used.contains(&value.player) && value.substitute_probability > 0.0 && value.availability_probability > 0.0
    }).collect();
    if candidates.is_empty() { return None; }
    let weights = candidates.iter().map(|value| {
        value.substitute_probability * value.availability_probability
            * (value.role_fit(outgoing.role) + 0.02).powf(projection.parameters.role_fit_power)
            * (-projection.parameters.fatigue_weight * value.fatigue).exp()
    });
    let selected = candidates[weighted_index(weights, rng)?];
    if !rng.bernoulli(selected.availability_probability) { return None; }
    Some(ActivePlayer {
        sampled: SampledStarter {
            player: selected.player, slot: outgoing.slot, role: outgoing.role, x: outgoing.x, y: outgoing.y,
            minutes: (90.0 - entered_minute).max(0.0), role_fit: selected.role_fit(outgoing.role), fatigue: selected.fatigue,
        },
        planned_exit: 90.0,
    })
}

/// Common-random-number intervention. Removing X reprojects the XI, replacement chain, pairwise
/// matchups, player events, causal channels, and result; it never subtracts a static rating.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimitivePlayerAbsenceCounterfactual {
    pub player: PlayerId,
    pub baseline: PrimitiveMatchForecast,
    pub without_player: PrimitiveMatchForecast,
    pub home_win_probability_delta: f64,
    pub draw_probability_delta: f64,
    pub away_win_probability_delta: f64,
    pub home_expected_goals_delta: f64,
    pub away_expected_goals_delta: f64,
    pub home_expected_points_delta: f64,
    pub away_expected_points_delta: f64,
}

pub fn simulate_primitive_player_absence(
    input: &PrimitiveMatchSimulationInput,
    player: PlayerId,
    paths: u32,
    seed: u64,
) -> Option<PrimitivePlayerAbsenceCounterfactual> {
    let home_has = input.home_lineup.candidates.iter().any(|value| value.player == player);
    let away_has = input.away_lineup.candidates.iter().any(|value| value.player == player);
    if home_has == away_has { return None; }
    let baseline = simulate_primitive_match(input, paths, seed)?;
    let mut intervention = input.clone();
    let lineup = if home_has { &mut intervention.home_lineup } else { &mut intervention.away_lineup };
    lineup.locked_starters.retain(|value| value.player != player);
    let candidate = lineup.candidates.iter_mut().find(|value| value.player == player)?;
    candidate.start_probability = 0.0;
    candidate.availability_probability = 0.0;
    candidate.substitute_probability = 0.0;
    let without_player = simulate_primitive_match(&intervention, paths, seed)?;
    Some(PrimitivePlayerAbsenceCounterfactual {
        player,
        home_win_probability_delta: f64::from(baseline.outcome.home_win.get() - without_player.outcome.home_win.get()),
        draw_probability_delta: f64::from(baseline.outcome.draw.get() - without_player.outcome.draw.get()),
        away_win_probability_delta: f64::from(baseline.outcome.away_win.get() - without_player.outcome.away_win.get()),
        home_expected_goals_delta: baseline.outcome.home_goals.mean - without_player.outcome.home_goals.mean,
        away_expected_goals_delta: baseline.outcome.away_goals.mean - without_player.outcome.away_goals.mean,
        home_expected_points_delta: baseline.outcome.home_expected_points - without_player.outcome.home_expected_points,
        away_expected_points_delta: baseline.outcome.away_expected_points - without_player.outcome.away_expected_points,
        baseline, without_player,
    })
}

fn segment_probability(probability_per90: f64, minutes: f64) -> f64 {
    1.0 - (1.0 - probability_per90.clamp(0.0, 1.0)).powf(minutes / 90.0)
}

fn weighted_index<I>(weights: I, rng: &mut Rng64) -> Option<usize>
where I: IntoIterator<Item = f64> {
    let values: Vec<f64> = weights.into_iter().map(|value| if value.is_finite() && value > 0.0 { value } else { 0.0 }).collect();
    let total = values.iter().sum::<f64>();
    if values.is_empty() || total <= 0.0 { return None; }
    let target = rng.uniform() * total;
    let mut cumulative = 0.0;
    for (index, value) in values.iter().enumerate() {
        cumulative += *value;
        if target <= cumulative { return Some(index); }
    }
    Some(values.len() - 1)
}

fn sample_non_negative_normal(mean: f64, sigma: f64, rng: &mut Rng64) -> f64 {
    if sigma <= f64::EPSILON { mean.max(0.0) } else { (mean + sigma * rng.normal()).max(0.0) }
}

fn sample_log_rate(mean: f64, sigma: f64, rng: &mut Rng64) -> f64 {
    if sigma <= f64::EPSILON { return mean.max(f64::MIN_POSITIVE); }
    (mean.ln() - 0.5 * sigma.powi(2) + sigma * rng.normal()).exp()
}

/// Per-path team goal-rate pair with optional shared-tempo correlation. A correlation of
/// exactly zero takes the legacy two-independent-draw path so identical seeds reproduce
/// identical streams bit-for-bit; nonzero correlation adds one shared normal draw plus two
/// idiosyncratic draws (shared variance fraction = rho, idiosyncratic = 1 - rho).
fn sample_correlated_log_rates(
    home_mean: f64,
    away_mean: f64,
    sigma: f64,
    correlation: f64,
    rng: &mut Rng64,
) -> (f64, f64) {
    if sigma <= f64::EPSILON {
        return (home_mean.max(f64::MIN_POSITIVE), away_mean.max(f64::MIN_POSITIVE));
    }
    if correlation.abs() <= f64::EPSILON {
        return (sample_log_rate(home_mean, sigma, rng), sample_log_rate(away_mean, sigma, rng));
    }
    let rho = correlation.clamp(-0.5, 0.9);
    let shared = rng.normal();
    let home_idiosyncratic = rng.normal();
    let away_idiosyncratic = rng.normal();
    let shared_scale = (rho.abs() * sigma * sigma).sqrt();
    let idiosyncratic_scale = ((1.0 - rho.abs()) * sigma * sigma).sqrt();
    let sign = rho.signum();
    let home_log = home_mean.ln() - 0.5 * sigma.powi(2)
        + shared_scale * shared
        + idiosyncratic_scale * home_idiosyncratic;
    let away_log = away_mean.ln() - 0.5 * sigma.powi(2)
        + sign * shared_scale * shared
        + idiosyncratic_scale * away_idiosyncratic;
    (home_log.exp(), away_log.exp())
}

fn poisson(lambda: f64, rng: &mut Rng64) -> u32 {
    if !lambda.is_finite() || lambda <= 0.0 { return 0; }
    if lambda < 30.0 {
        let threshold = (-lambda).exp();
        let mut product = 1.0;
        let mut count = 0_u32;
        loop {
            product *= rng.uniform();
            if product <= threshold { return count; }
            count = count.saturating_add(1);
        }
    }
    (lambda + lambda.sqrt() * rng.normal()).round().max(0.0).min(f64::from(u32::MAX)) as u32
}

fn summarize(values: &[f64]) -> Option<DistributionSummary> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) { return None; }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let variance = if sorted.len() > 1 { sorted.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (sorted.len() - 1) as f64 } else { 0.0 };
    Some(DistributionSummary {
        p10: quantile(&sorted, 0.10), p25: quantile(&sorted, 0.25), p50: quantile(&sorted, 0.50),
        p75: quantile(&sorted, 0.75), p90: quantile(&sorted, 0.90), mean,
        standard_error: (variance / sorted.len() as f64).sqrt(), paths: sorted.len().min(u32::MAX as usize) as u32,
    })
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    if sorted.len() == 1 { return sorted[0]; }
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper { sorted[lower] } else {
        let fraction = position - lower as f64;
        sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
    }
}

fn non_negative(value: f64) -> bool { value.is_finite() && value >= 0.0 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{ChannelEdge, GoalChannelLoading, MetricChannelLoading};
    use crate::lineup::{FormationPrior, FormationTemplate, LineupSamplingParameters, RoleFit, TacticalSlot};
    use crate::matchup::AbilityDimension;
    use footy_core::RoleId;
    use footy_state::AbilityVector;

    fn formation() -> FormationTemplate {
        FormationTemplate { id: 433, slots: (0..11).map(|slot| TacticalSlot {
            slot, role: RoleId(slot as u16 + 1), x: 0.05 + f64::from(slot) * 0.075, y: f64::from(slot % 5) / 4.0,
        }).collect() }
    }

    fn lineup(club: u32, offset: u64) -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(club), formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (0..16).map(|index| LineupCandidate {
                player: PlayerId(offset + index), start_probability: if index < 11 { 0.86 } else { 0.15 }, availability_probability: 1.0,
                ability_index: if club == 1 { 14.0 } else { 11.0 }, fatigue: 0.0,
                expected_minutes_if_start: if index < 3 { 64.0 } else { 88.0 }, minutes_sigma: 2.0,
                substitute_probability: if index >= 11 { 0.8 } else { 0.05 },
                role_fits: (1..=11).map(|role| RoleFit { role: RoleId(role), fit: if role == index as u16 + 1 { 1.0 } else { 0.25 } }).collect(),
            }).collect(), locked_starters: Vec::new(), parameters: LineupSamplingParameters::default(),
        }
    }

    fn input() -> PrimitiveMatchSimulationInput {
        let home = lineup(1, 1);
        let away = lineup(2, 101);
        let all: Vec<PlayerId> = home.candidates.iter().chain(away.candidates.iter()).map(|value| value.player).collect();
        let metric = MetricId(1);
        let pace = AttributeId(1);
        let recovery = AttributeId(2);
        PrimitiveMatchSimulationInput {
            match_id: MatchId(9), home: ClubId(1), away: ClubId(2), home_lineup: home, away_lineup: away,
            player_states: all.iter().map(|player| PlayerMatchupState { player: *player, ability: AbilityVector::from_components([if player.0 < 100 { 14.0 } else { 11.0 }; 10]) }).collect(),
            attribute_states: all.iter().map(|player| PlayerAttributeState { player: *player, attributes: vec![
                PlayerAttributeValue { attribute: pace, value: if player.0 < 100 { 16.0 } else { 11.0 } },
                PlayerAttributeValue { attribute: recovery, value: if player.0 < 100 { 13.0 } else { 10.0 } },
            ] }).collect(),
            event_models: all.iter().map(|player| SpecificPlayerEventModel {
                player: *player, metric, family: StatFamily::ContinuousNonNegative,
                mean_per90: if player.0 < 100 { 1.15 } else { 0.85 }, variance_per90: 0.02, epistemic_sigma: 0.02, reference_per90: 1.0,
                aggregate_matchup_sensitivity: 0.05, attribute_matchup_sensitivity: 0.2, teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.1, fatigue_sensitivity: 0.1,
            }).collect(),
            own_attribute_loadings: vec![AttributeEventLoading { metric, attribute: pace, reference: 12.0, scale: 20.0, log_rate_weight: 0.1 }],
            aggregate_matchup_rules: vec![MatchupRule { metric: Some(metric), attacker_dimension: AbilityDimension::BallProgression, defender_dimension: AbilityDimension::Defending, weight: 1.0, scale: 20.0 }],
            attribute_matchup_rules: vec![AttributeMatchupRule { metric: Some(metric), attacker_attribute: pace, defender_attribute: recovery, weight: 1.0, scale: 20.0 }],
            synergies: Vec::new(),
            channel_graph: CausalChannelGraph {
                metric_loadings: vec![MetricChannelLoading { metric, channel: TeamProcessChannel::Progression, weight: 1.0 }],
                edges: vec![
                    ChannelEdge { from: TeamProcessChannel::Progression, to: TeamProcessChannel::ChanceCreation, weight: 0.6 },
                    ChannelEdge { from: TeamProcessChannel::ChanceCreation, to: TeamProcessChannel::ShotQuality, weight: 0.5 },
                ],
                goal_loadings: vec![GoalChannelLoading { channel: TeamProcessChannel::ShotQuality, attack_log_weight: 0.08, defense_log_weight: 0.0 }],
            },
            score_state_modifiers: vec![SpecificScoreStateMetricModifier { metric, trailing_multiplier: 1.1, leading_multiplier: 0.92 }],
            incident_profiles: Vec::new(),
            in_match: SpecificInMatchParameters { injury_probability_per90: 0.0, red_card_probability_per90: 0.0, max_substitutions: 5, minimum_tactical_substitution_minute: 45.0 },
            home_context: SpecificTeamContext { club: ClubId(1), residual_attack_log_adjustment: 0.0, residual_defense_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            away_context: SpecificTeamContext { club: ClubId(2), residual_attack_log_adjustment: 0.0, residual_defense_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            environment: SpecificMatchEnvironment { home_residual_attack_log_adjustment: 0.0, away_residual_attack_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            goal_process: SpecificGoalProcess { goals_per_team_match: 1.3, home_advantage_multiplier: 1.08, path_log_rate_sigma: 0.05, path_rate_correlation: 0.0, max_abs_segment_log_adjustment: 1.5, segments: 6 },
            matchup_spatial_sigma: 0.25, max_player_rate_log_adjustment: 1.0,
        }
    }

    #[test]
    fn projected_lineup_player_events_channels_and_result_are_one_reproducible_process() {
        let first = simulate_primitive_match(&input(), 500, 42).unwrap();
        let second = simulate_primitive_match(&input(), 500, 42).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.paths, 500);
        let p = first.outcome.home_win.get() + first.outcome.draw.get() + first.outcome.away_win.get();
        assert!((p - 1.0).abs() < 1e-6);
        assert!(first.team_channels.iter().any(|value| value.channel == TeamProcessChannel::ShotQuality));
    }

    #[test]
    fn channel_rate_signal_does_not_depend_on_segment_length() {
        let mut input = input();
        for model in &mut input.event_models {
            model.variance_per90 = 0.0;
            model.epistemic_sigma = 0.0;
            model.aggregate_matchup_sensitivity = 0.0;
            model.attribute_matchup_sensitivity = 0.0;
            model.teammate_sensitivity = 0.0;
            model.role_fit_sensitivity = 0.0;
            model.fatigue_sensitivity = 0.0;
        }
        input.own_attribute_loadings.clear();
        input.aggregate_matchup_rules.clear();
        input.attribute_matchup_rules.clear();
        input.score_state_modifiers.clear();

        let sampled = |projection: &LineupProjectionInput| SampledLineup {
            club: projection.club,
            formation_id: 433,
            starters: projection
                .candidates
                .iter()
                .take(11)
                .enumerate()
                .map(|(index, candidate)| SampledStarter {
                    player: candidate.player,
                    slot: index as u8,
                    role: RoleId(index as u16 + 1),
                    x: 0.05 + index as f64 * 0.075,
                    y: (index % 5) as f64 / 4.0,
                    minutes: 90.0,
                    role_fit: 1.0,
                    fatigue: 0.0,
                })
                .collect(),
        };
        let home = sampled(&input.home_lineup);
        let away = sampled(&input.away_lineup);
        let mut rng_15 = Rng64::new(17);
        let mut rng_30 = Rng64::new(17);
        let fifteen = segment_output(&input, &home, &away, true, ScoreState::Level, 15.0, &mut rng_15).unwrap();
        let thirty = segment_output(&input, &home, &away, true, ScoreState::Level, 30.0, &mut rng_30).unwrap();
        assert!((fifteen.attack - thirty.attack).abs() < 1e-12);
        assert!((fifteen.defense - thirty.defense).abs() < 1e-12);
    }

    #[test]
    fn bench_players_receive_stochastic_substitute_appearances() {
        let result = simulate_primitive_match(&input(), 300, 7).unwrap();
        assert!(result.player_influences.iter().any(|value| value.player.0 >= 12 && value.player.0 <= 16 && value.appearance_probability.get() > 0.0));
    }

    #[test]
    fn fine_attribute_matchup_is_separately_attributed() {
        let result = simulate_primitive_match(&input(), 300, 11).unwrap();
        assert!(result.player_influences.iter().any(|value| value.player.0 < 100 && value.mean_attribute_matchup_log_modifier > 0.0));
    }

    #[test]
    fn player_absence_reprojects_every_downstream_layer() {
        let result = simulate_primitive_player_absence(&input(), PlayerId(1), 300, 9).unwrap();
        assert_eq!(result.player, PlayerId(1));
        assert_ne!(result.baseline.home_lineup.players, result.without_player.home_lineup.players);
    }

    #[test]
    fn zero_correlation_preserves_independent_rate_stream() {
        let mut first = Rng64::new(99);
        let mut second = Rng64::new(99);
        let (home, away) = sample_correlated_log_rates(1.4, 1.2, 0.2, 0.0, &mut first);
        let expected_home = sample_log_rate(1.4, 0.2, &mut second);
        let expected_away = sample_log_rate(1.2, 0.2, &mut second);
        assert!((home - expected_home).abs() < 1e-12);
        assert!((away - expected_away).abs() < 1e-12);
    }

    #[test]
    fn positive_correlation_co_moves_path_rates() {
        let samples = 4_000;
        let mut independent = Rng64::new(7);
        let mut shared = Rng64::new(7);
        let mut independent_log_home = Vec::with_capacity(samples);
        let mut independent_log_away = Vec::with_capacity(samples);
        let mut shared_log_home = Vec::with_capacity(samples);
        let mut shared_log_away = Vec::with_capacity(samples);
        for _ in 0..samples {
            let (home, away) = sample_correlated_log_rates(1.4, 1.2, 0.25, 0.0, &mut independent);
            independent_log_home.push(home.ln());
            independent_log_away.push(away.ln());
            let (home, away) = sample_correlated_log_rates(1.4, 1.2, 0.25, 0.6, &mut shared);
            shared_log_home.push(home.ln());
            shared_log_away.push(away.ln());
        }
        assert!(correlation(&shared_log_home, &shared_log_away) > correlation(&independent_log_home, &independent_log_away) + 0.2);
    }

    #[test]
    fn goal_process_rejects_implausible_correlation() {
        let mut process = SpecificGoalProcess {
            goals_per_team_match: 1.3,
            home_advantage_multiplier: 1.08,
            path_log_rate_sigma: 0.1,
            path_rate_correlation: 0.95,
            max_abs_segment_log_adjustment: 1.5,
            segments: 6,
        };
        assert!(!process.valid());
        process.path_rate_correlation = 0.4;
        assert!(process.valid());
    }

    fn correlation(left: &[f64], right: &[f64]) -> f64 {
        let n = left.len() as f64;
        let mean_left = left.iter().sum::<f64>() / n;
        let mean_right = right.iter().sum::<f64>() / n;
        let covariance = left.iter().zip(right).map(|(a, b)| (a - mean_left) * (b - mean_right)).sum::<f64>() / n;
        let variance_left = left.iter().map(|a| (a - mean_left).powi(2)).sum::<f64>() / n;
        let variance_right = right.iter().map(|b| (b - mean_right).powi(2)).sum::<f64>() / n;
        covariance / (variance_left * variance_right).sqrt().max(f64::MIN_POSITIVE)
    }
}
