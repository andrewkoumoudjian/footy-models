use crate::channels::{CausalChannelGraph, TeamProcessChannel};
use crate::lineup::{project_lineup, sample_lineup, LineupCandidate, LineupProjection, LineupProjectionInput, SampledLineup, SampledStarter};
use crate::matchup::{player_metric_interaction_context, AbilityDimension, MatchupRule, PairwiseSynergy, PlayerMatchupState};
use crate::simulation::{GoalProcessParameters, MatchEnvironment, TeamMatchContext};
use footy_core::{ClubId, MatchId, MetricId, PlayerId, Probability};
use footy_forecast::{DistributionSummary, MatchOutcomeForecast, Rng64, StatFamily};
use footy_state::AbilityVector;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerEventModel {
    pub player: PlayerId,
    pub metric: MetricId,
    pub family: StatFamily,
    pub mean_per90: f64,
    pub variance_per90: f64,
    pub epistemic_sigma: f64,
    /// Peer/league/role reference. Realized production is centered against this before entering
    /// causal team channels.
    pub reference_per90: f64,
    pub matchup_sensitivity: f64,
    pub teammate_sensitivity: f64,
    pub role_fit_sensitivity: f64,
    pub fatigue_sensitivity: f64,
}

impl PlayerEventModel {
    fn valid(self) -> bool {
        non_negative(self.mean_per90)
            && non_negative(self.variance_per90)
            && non_negative(self.epistemic_sigma)
            && self.reference_per90.is_finite()
            && self.reference_per90 > 0.0
            && self.matchup_sensitivity.is_finite()
            && self.teammate_sensitivity.is_finite()
            && self.role_fit_sensitivity.is_finite()
            && self.role_fit_sensitivity >= 0.0
            && self.fatigue_sensitivity.is_finite()
            && self.fatigue_sensitivity >= 0.0
    }
}

/// FM/source-neutral ability can move a player's event process, but not the score directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityMetricLoading {
    pub metric: MetricId,
    pub dimension: AbilityDimension,
    pub reference: f64,
    pub scale: f64,
    pub log_rate_weight: f64,
}

impl AbilityMetricLoading {
    fn valid(self) -> bool {
        self.reference.is_finite()
            && self.scale.is_finite()
            && self.scale > 0.0
            && self.log_rate_weight.is_finite()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreStateMetricModifier {
    pub metric: MetricId,
    pub trailing_multiplier: f64,
    pub leading_multiplier: f64,
}

impl ScoreStateMetricModifier {
    fn valid(self) -> bool {
        self.trailing_multiplier.is_finite()
            && self.trailing_multiplier > 0.0
            && self.leading_multiplier.is_finite()
            && self.leading_multiplier > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerIncidentProfile {
    pub player: PlayerId,
    pub injury_multiplier: f64,
    pub red_card_multiplier: f64,
}

impl PlayerIncidentProfile {
    fn valid(self) -> bool {
        non_negative(self.injury_multiplier) && non_negative(self.red_card_multiplier)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InMatchEventParameters {
    /// Calibrated probability for a continuously active player over 90 minutes.
    pub injury_probability_per90: f64,
    pub red_card_probability_per90: f64,
    pub max_substitutions: u8,
    /// Tactical substitutions are not made before this match minute unless forced by injury.
    pub minimum_tactical_substitution_minute: f64,
}

impl InMatchEventParameters {
    pub fn valid(self) -> bool {
        self.injury_probability_per90.is_finite()
            && (0.0..=1.0).contains(&self.injury_probability_per90)
            && self.red_card_probability_per90.is_finite()
            && (0.0..=1.0).contains(&self.red_card_probability_per90)
            && self.max_substitutions <= 12
            && self.minimum_tactical_substitution_minute.is_finite()
            && (0.0..=90.0).contains(&self.minimum_tactical_substitution_minute)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalMatchSimulationInput {
    pub match_id: MatchId,
    pub home: ClubId,
    pub away: ClubId,
    pub home_lineup: LineupProjectionInput,
    pub away_lineup: LineupProjectionInput,
    pub player_states: Vec<PlayerMatchupState>,
    pub event_models: Vec<PlayerEventModel>,
    pub ability_metric_loadings: Vec<AbilityMetricLoading>,
    pub matchup_rules: Vec<MatchupRule>,
    pub synergies: Vec<PairwiseSynergy>,
    pub channel_graph: CausalChannelGraph,
    pub score_state_modifiers: Vec<ScoreStateMetricModifier>,
    pub incident_profiles: Vec<PlayerIncidentProfile>,
    pub in_match_events: InMatchEventParameters,
    pub home_context: TeamMatchContext,
    pub away_context: TeamMatchContext,
    pub environment: MatchEnvironment,
    pub goal_process: GoalProcessParameters,
    pub matchup_spatial_sigma: f64,
    pub max_player_rate_log_adjustment: f64,
}

impl CausalMatchSimulationInput {
    pub fn validate(&self) -> bool {
        if self.home == self.away
            || self.home_lineup.club != self.home
            || self.away_lineup.club != self.away
            || self.home_context.club != self.home
            || self.away_context.club != self.away
            || !self.home_lineup.validate()
            || !self.away_lineup.validate()
            || !self.channel_graph.validate()
            || !self.in_match_events.valid()
            || !self.goal_process.valid()
            || !self.matchup_spatial_sigma.is_finite()
            || self.matchup_spatial_sigma <= 0.0
            || !self.max_player_rate_log_adjustment.is_finite()
            || self.max_player_rate_log_adjustment <= 0.0
            || self.event_models.iter().any(|value| !value.valid())
            || self.ability_metric_loadings.iter().any(|value| !value.valid())
            || self.score_state_modifiers.iter().any(|value| !value.valid())
            || self.incident_profiles.iter().any(|value| !value.valid())
            || self.synergies.iter().any(|value| !value.strength.is_finite())
        {
            return false;
        }
        let mut model_keys = BTreeSet::new();
        if !self.event_models.iter().all(|value| model_keys.insert((value.player, value.metric))) { return false; }
        let state_players: BTreeSet<PlayerId> = self.player_states.iter().map(|value| value.player).collect();
        let model_players: BTreeSet<PlayerId> = self.event_models.iter().map(|value| value.player).collect();
        let required: Vec<PlayerId> = self.home_lineup.candidates.iter().chain(self.away_lineup.candidates.iter()).map(|value| value.player).collect();
        if required.iter().any(|player| !state_players.contains(player) || !model_players.contains(player)) { return false; }
        let incident_players: BTreeSet<PlayerId> = self.incident_profiles.iter().map(|value| value.player).collect();
        incident_players.len() == self.incident_profiles.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalPlayerMetricForecast {
    pub player: PlayerId,
    pub metric: MetricId,
    pub start_probability: Probability,
    pub appearance_probability: Probability,
    pub expected_minutes: f64,
    /// Includes zero when the player never enters the match on that path.
    pub unconditional_total: DistributionSummary,
    pub conditional_on_appearance_total: Option<DistributionSummary>,
    pub conditional_on_appearance_per90: Option<DistributionSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerChannelInfluence {
    pub player: PlayerId,
    pub channel: TeamProcessChannel,
    pub mean_contribution: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CausalPlayerInfluenceSummary {
    pub player: PlayerId,
    pub start_probability: Probability,
    pub appearance_probability: Probability,
    pub expected_minutes: f64,
    pub mean_ability_log_modifier: f64,
    pub mean_matchup_log_modifier: f64,
    pub mean_teammate_log_modifier: f64,
    pub mean_role_log_modifier: f64,
    pub mean_fatigue_log_modifier: f64,
    pub mean_system_environment_log_modifier: f64,
    pub mean_score_state_log_modifier: f64,
    pub red_card_probability: Probability,
    pub injury_exit_probability: Probability,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamChannelForecast {
    pub club: ClubId,
    pub channel: TeamProcessChannel,
    pub mean_match_contribution: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalMatchForecast {
    pub outcome: MatchOutcomeForecast,
    pub home_lineup: LineupProjection,
    pub away_lineup: LineupProjection,
    pub player_metrics: Vec<CausalPlayerMetricForecast>,
    pub player_influences: Vec<CausalPlayerInfluenceSummary>,
    pub player_channels: Vec<PlayerChannelInfluence>,
    pub team_channels: Vec<TeamChannelForecast>,
    pub paths: u32,
}

#[derive(Debug, Clone)]
struct ActivePlayer {
    starter: SampledStarter,
    entered_minute: f64,
    planned_exit_minute: f64,
}

#[derive(Debug, Clone)]
struct ActiveTeam {
    club: ClubId,
    formation_id: u16,
    players: Vec<ActivePlayer>,
    used_players: BTreeSet<PlayerId>,
    substitutions: u8,
}

impl ActiveTeam {
    fn from_lineup(lineup: SampledLineup) -> Self {
        let used_players = lineup.starters.iter().map(|value| value.player).collect();
        let players = lineup.starters.into_iter().map(|starter| ActivePlayer {
            planned_exit_minute: starter.minutes,
            entered_minute: 0.0,
            starter,
        }).collect();
        Self { club: lineup.club, formation_id: lineup.formation_id, players, used_players, substitutions: 0 }
    }

    fn sampled_lineup(&self) -> SampledLineup {
        SampledLineup { club: self.club, formation_id: self.formation_id, starters: self.players.iter().map(|value| value.starter).collect() }
    }
}

#[derive(Default)]
struct PlayerPathAccumulator {
    started: bool,
    appeared: bool,
    minutes: f64,
    red_carded: bool,
    injured_out: bool,
    factor_observations: u32,
    ability: f64,
    matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    score_state: f64,
    channels: BTreeMap<TeamProcessChannel, f64>,
}

#[derive(Default)]
struct PlayerAggregate {
    starts: u32,
    appearances: u32,
    minutes: f64,
    red_cards: u32,
    injury_exits: u32,
    factor_observations: u32,
    ability: f64,
    matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    score_state: f64,
    channels: BTreeMap<TeamProcessChannel, f64>,
}

#[derive(Default)]
struct MetricPathSample {
    total: f64,
    minutes: f64,
}

#[derive(Default)]
struct MetricAggregate {
    unconditional: Vec<f64>,
    conditional: Vec<f64>,
    conditional_per90: Vec<f64>,
}

struct SegmentPlayerMetric {
    player: PlayerId,
    metric: MetricId,
    total: f64,
    deviation: f64,
    ability_log: f64,
    matchup_log: f64,
    teammate_log: f64,
    role_log: f64,
    fatigue_log: f64,
    system_log: f64,
    score_state_log: f64,
}

struct SegmentTeamOutput {
    channel_values: Vec<(TeamProcessChannel, f64)>,
    attack_log_adjustment: f64,
    defense_log_adjustment: f64,
    player_metrics: Vec<SegmentPlayerMetric>,
}

/// Preferred specific-match simulator. Unknown lineups, roles, starter minutes, substitutions,
/// injuries, red cards, player event rates, causal team channels, scoring intensities, and the final
/// result all live on the same deterministic Monte Carlo path.
pub fn simulate_causal_match(
    input: &CausalMatchSimulationInput,
    paths: u32,
    seed: u64,
) -> Option<CausalMatchForecast> {
    if paths == 0 || !input.validate() { return None; }
    let mut rng = Rng64::new(seed);
    let mut home_goals_samples = Vec::with_capacity(paths as usize);
    let mut away_goals_samples = Vec::with_capacity(paths as usize);
    let mut home_wins = 0_u32;
    let mut draws = 0_u32;
    let mut away_wins = 0_u32;
    let mut player_aggregate = BTreeMap::<PlayerId, PlayerAggregate>::new();
    let mut metric_aggregate = BTreeMap::<(PlayerId, MetricId), MetricAggregate>::new();
    let mut team_channel_totals = BTreeMap::<(ClubId, TeamProcessChannel), f64>::new();
    for model in &input.event_models { metric_aggregate.entry((model.player, model.metric)).or_default(); }

    let segment_minutes = 90.0 / f64::from(input.goal_process.segments);
    for _ in 0..paths {
        let home_start = sample_lineup(&input.home_lineup, &mut rng)?;
        let away_start = sample_lineup(&input.away_lineup, &mut rng)?;
        let mut home = ActiveTeam::from_lineup(home_start.clone());
        let mut away = ActiveTeam::from_lineup(away_start.clone());
        let mut path_players = BTreeMap::<PlayerId, PlayerPathAccumulator>::new();
        let mut path_metrics = BTreeMap::<(PlayerId, MetricId), MetricPathSample>::new();
        for starter in home_start.starters.iter().chain(away_start.starters.iter()) {
            path_players.entry(starter.player).or_default().started = true;
        }

        let base_home_rate = input.goal_process.goals_per_team_match * input.goal_process.home_advantage_multiplier;
        let base_away_rate = input.goal_process.goals_per_team_match;
        let path_home_rate = sample_log_rate(base_home_rate, input.goal_process.path_log_rate_sigma, &mut rng);
        let path_away_rate = sample_log_rate(base_away_rate, input.goal_process.path_log_rate_sigma, &mut rng);
        let mut home_goals = 0_u32;
        let mut away_goals = 0_u32;

        for segment in 0..input.goal_process.segments {
            let start_minute = f64::from(segment) * segment_minutes;
            let end_minute = f64::from(segment + 1) * segment_minutes;
            let home_score_state = score_state(home_goals, away_goals);
            let away_score_state = score_state(away_goals, home_goals);
            let home_lineup = home.sampled_lineup();
            let away_lineup = away.sampled_lineup();

            let home_output = simulate_segment_team(input, &home_lineup, &away_lineup, true, home_score_state, segment_minutes, &mut rng)?;
            let away_output = simulate_segment_team(input, &away_lineup, &home_lineup, false, away_score_state, segment_minutes, &mut rng)?;
            accumulate_segment(
                &home_output, &home, segment_minutes, &mut path_players, &mut path_metrics,
                &mut team_channel_totals, input.home,
            );
            accumulate_segment(
                &away_output, &away, segment_minutes, &mut path_players, &mut path_metrics,
                &mut team_channel_totals, input.away,
            );

            let home_adjustment = (
                input.environment.home_attack_log_adjustment
                    + input.home_context.attack_log_adjustment
                    - input.away_context.defense_log_adjustment
                    + home_output.attack_log_adjustment
                    - away_output.defense_log_adjustment
            ).clamp(-input.goal_process.max_abs_log_adjustment, input.goal_process.max_abs_log_adjustment);
            let away_adjustment = (
                input.environment.away_attack_log_adjustment
                    + input.away_context.attack_log_adjustment
                    - input.home_context.defense_log_adjustment
                    + away_output.attack_log_adjustment
                    - home_output.defense_log_adjustment
            ).clamp(-input.goal_process.max_abs_log_adjustment, input.goal_process.max_abs_log_adjustment);
            home_goals = home_goals.saturating_add(poisson(path_home_rate * home_adjustment.exp() / f64::from(input.goal_process.segments), &mut rng));
            away_goals = away_goals.saturating_add(poisson(path_away_rate * away_adjustment.exp() / f64::from(input.goal_process.segments), &mut rng));

            if segment + 1 < input.goal_process.segments {
                process_end_of_segment(input, &mut home, &input.home_lineup, start_minute, end_minute, segment_minutes, &mut path_players, &mut rng)?;
                process_end_of_segment(input, &mut away, &input.away_lineup, start_minute, end_minute, segment_minutes, &mut path_players, &mut rng)?;
            }
        }

        home_goals_samples.push(f64::from(home_goals));
        away_goals_samples.push(f64::from(away_goals));
        match home_goals.cmp(&away_goals) {
            std::cmp::Ordering::Greater => home_wins = home_wins.saturating_add(1),
            std::cmp::Ordering::Equal => draws = draws.saturating_add(1),
            std::cmp::Ordering::Less => away_wins = away_wins.saturating_add(1),
        }

        for ((player, metric), aggregate) in &mut metric_aggregate {
            if let Some(value) = path_metrics.get(&(*player, *metric)) {
                aggregate.unconditional.push(value.total);
                if value.minutes > 0.0 {
                    aggregate.conditional.push(value.total);
                    aggregate.conditional_per90.push(value.total * 90.0 / value.minutes);
                }
            } else {
                aggregate.unconditional.push(0.0);
            }
        }
        for (player, value) in path_players {
            let aggregate = player_aggregate.entry(player).or_default();
            if value.started { aggregate.starts = aggregate.starts.saturating_add(1); }
            if value.appeared { aggregate.appearances = aggregate.appearances.saturating_add(1); }
            if value.red_carded { aggregate.red_cards = aggregate.red_cards.saturating_add(1); }
            if value.injured_out { aggregate.injury_exits = aggregate.injury_exits.saturating_add(1); }
            aggregate.minutes += value.minutes;
            aggregate.factor_observations = aggregate.factor_observations.saturating_add(value.factor_observations);
            aggregate.ability += value.ability;
            aggregate.matchup += value.matchup;
            aggregate.teammate += value.teammate;
            aggregate.role += value.role;
            aggregate.fatigue += value.fatigue;
            aggregate.system += value.system;
            aggregate.score_state += value.score_state;
            for (channel, contribution) in value.channels {
                *aggregate.channels.entry(channel).or_default() += contribution;
            }
        }
    }

    let denominator = paths as f32;
    let home_win = Probability::clamped(home_wins as f32 / denominator);
    let draw = Probability::clamped(draws as f32 / denominator);
    let away_win = Probability::clamped(away_wins as f32 / denominator);
    let outcome = MatchOutcomeForecast {
        match_id: input.match_id,
        home: input.home,
        away: input.away,
        home_win,
        draw,
        away_win,
        home_goals: summarize(&home_goals_samples)?,
        away_goals: summarize(&away_goals_samples)?,
        home_expected_points: 3.0 * f64::from(home_win.get()) + f64::from(draw.get()),
        away_expected_points: 3.0 * f64::from(away_win.get()) + f64::from(draw.get()),
        paths,
    };
    let home_lineup = project_lineup(&input.home_lineup, paths, seed ^ 0x4341_5553_414c_484f)?;
    let away_lineup = project_lineup(&input.away_lineup, paths, seed ^ 0x4341_5553_414c_4157)?;

    let player_metrics = metric_aggregate.into_iter().map(|((player, metric), value)| {
        let player_summary = player_aggregate.get(&player);
        Some(CausalPlayerMetricForecast {
            player,
            metric,
            start_probability: Probability::clamped(player_summary.map(|value| value.starts).unwrap_or(0) as f32 / denominator),
            appearance_probability: Probability::clamped(player_summary.map(|value| value.appearances).unwrap_or(0) as f32 / denominator),
            expected_minutes: player_summary.map(|value| value.minutes / f64::from(paths)).unwrap_or(0.0),
            unconditional_total: summarize(&value.unconditional)?,
            conditional_on_appearance_total: summarize(&value.conditional),
            conditional_on_appearance_per90: summarize(&value.conditional_per90),
        })
    }).collect::<Option<Vec<_>>>()?;

    let mut player_channels = Vec::new();
    let player_influences = player_aggregate.into_iter().map(|(player, value)| {
        let factor_denominator = f64::from(value.factor_observations.max(1));
        for (channel, total) in &value.channels {
            player_channels.push(PlayerChannelInfluence { player, channel: *channel, mean_contribution: *total / f64::from(paths) });
        }
        CausalPlayerInfluenceSummary {
            player,
            start_probability: Probability::clamped(value.starts as f32 / denominator),
            appearance_probability: Probability::clamped(value.appearances as f32 / denominator),
            expected_minutes: value.minutes / f64::from(paths),
            mean_ability_log_modifier: value.ability / factor_denominator,
            mean_matchup_log_modifier: value.matchup / factor_denominator,
            mean_teammate_log_modifier: value.teammate / factor_denominator,
            mean_role_log_modifier: value.role / factor_denominator,
            mean_fatigue_log_modifier: value.fatigue / factor_denominator,
            mean_system_environment_log_modifier: value.system / factor_denominator,
            mean_score_state_log_modifier: value.score_state / factor_denominator,
            red_card_probability: Probability::clamped(value.red_cards as f32 / denominator),
            injury_exit_probability: Probability::clamped(value.injury_exits as f32 / denominator),
        }
    }).collect();
    player_channels.sort_by_key(|value| (value.player, value.channel));
    let team_channels = team_channel_totals.into_iter().map(|((club, channel), total)| TeamChannelForecast {
        club,
        channel,
        mean_match_contribution: total / f64::from(paths),
    }).collect();

    Some(CausalMatchForecast { outcome, home_lineup, away_lineup, player_metrics, player_influences, player_channels, team_channels, paths })
}

fn simulate_segment_team(
    input: &CausalMatchSimulationInput,
    own: &SampledLineup,
    opponent: &SampledLineup,
    is_home: bool,
    score_state: ScoreState,
    segment_minutes: f64,
    rng: &mut Rng64,
) -> Option<SegmentTeamOutput> {
    let own_context = if is_home { &input.home_context } else { &input.away_context };
    let opponent_context = if is_home { &input.away_context } else { &input.home_context };
    let state_map: BTreeMap<PlayerId, AbilityVector> = input.player_states.iter().map(|value| (value.player, value.ability)).collect();
    let mut metric_deviations = Vec::<(MetricId, f64)>::new();
    let mut player_metrics = Vec::new();

    for starter in &own.starters {
        for model in input.event_models.iter().filter(|value| value.player == starter.player) {
            let interaction = player_metric_interaction_context(
                starter.player, model.metric, own, opponent, &input.player_states,
                &input.matchup_rules, &input.synergies, input.matchup_spatial_sigma,
            )?;
            let ability = *state_map.get(&starter.player)?;
            let ability_log = ability_metric_adjustment(ability, model.metric, &input.ability_metric_loadings);
            let matchup_log = model.matchup_sensitivity * interaction.opponent_advantage;
            let teammate_log = model.teammate_sensitivity * interaction.teammate_support;
            let role_log = model.role_fit_sensitivity * (starter.role_fit - 1.0);
            let fatigue_log = -model.fatigue_sensitivity * starter.fatigue;
            let system_multiplier = own_context.production_multiplier(model.metric)
                * opponent_context.suppression_multiplier(model.metric)
                * input.environment.metric_multiplier(model.metric, is_home);
            if !system_multiplier.is_finite() || system_multiplier <= 0.0 { return None; }
            let system_log = system_multiplier.ln();
            let score_multiplier = score_state_multiplier(model.metric, score_state, &input.score_state_modifiers);
            let score_state_log = score_multiplier.ln();
            let total_log = (ability_log + matchup_log + teammate_log + role_log + fatigue_log + system_log + score_state_log)
                .clamp(-input.max_player_rate_log_adjustment, input.max_player_rate_log_adjustment);
            let multiplier = total_log.exp();
            let mean = model.mean_per90 * multiplier;
            let latent = sample_non_negative_normal(mean, model.epistemic_sigma * multiplier, rng);
            let realized_rate = sample_non_negative_normal(latent, model.variance_per90.sqrt() * multiplier, rng);
            let exposure = segment_minutes / 90.0;
            let total = match model.family {
                StatFamily::ContinuousNonNegative => realized_rate * exposure,
                StatFamily::Count => f64::from(poisson(realized_rate * exposure, rng)),
            };
            let effective_rate = if exposure > 0.0 { total / exposure } else { 0.0 };
            let deviation = (effective_rate / model.reference_per90 - 1.0) * exposure;
            metric_deviations.push((model.metric, deviation));
            player_metrics.push(SegmentPlayerMetric {
                player: starter.player,
                metric: model.metric,
                total,
                deviation,
                ability_log,
                matchup_log,
                teammate_log,
                role_log,
                fatigue_log,
                system_log,
                score_state_log,
            });
        }
    }
    let evaluation = input.channel_graph.evaluate(&metric_deviations)?;
    Some(SegmentTeamOutput {
        channel_values: evaluation.values,
        attack_log_adjustment: evaluation.attack_log_adjustment,
        defense_log_adjustment: evaluation.defense_log_adjustment,
        player_metrics,
    })
}

fn accumulate_segment(
    output: &SegmentTeamOutput,
    active: &ActiveTeam,
    segment_minutes: f64,
    players: &mut BTreeMap<PlayerId, PlayerPathAccumulator>,
    metrics: &mut BTreeMap<(PlayerId, MetricId), MetricPathSample>,
    team_channels: &mut BTreeMap<(ClubId, TeamProcessChannel), f64>,
    club: ClubId,
) {
    for active_player in &active.players {
        let value = players.entry(active_player.starter.player).or_default();
        value.appeared = true;
        value.minutes += segment_minutes;
    }
    for (channel, value) in &output.channel_values {
        *team_channels.entry((club, *channel)).or_default() += *value;
    }
    // Allocate each channel back to players in proportion to their absolute source deviation.
    // This preserves exact team totals while exposing which player supplied the upstream shock.
    let total_abs_deviation = output.player_metrics.iter().map(|value| value.deviation.abs()).sum::<f64>();
    for value in &output.player_metrics {
        let metric = metrics.entry((value.player, value.metric)).or_default();
        metric.total += value.total;
        metric.minutes += segment_minutes;
        let player = players.entry(value.player).or_default();
        player.factor_observations = player.factor_observations.saturating_add(1);
        player.ability += value.ability_log;
        player.matchup += value.matchup_log;
        player.teammate += value.teammate_log;
        player.role += value.role_log;
        player.fatigue += value.fatigue_log;
        player.system += value.system_log;
        player.score_state += value.score_state_log;
        if total_abs_deviation > 0.0 {
            let share = value.deviation.abs() / total_abs_deviation;
            for (channel, channel_value) in &output.channel_values {
                *player.channels.entry(*channel).or_default() += share * *channel_value;
            }
        }
    }
}

fn process_end_of_segment(
    input: &CausalMatchSimulationInput,
    active: &mut ActiveTeam,
    projection: &LineupProjectionInput,
    _start_minute: f64,
    end_minute: f64,
    segment_minutes: f64,
    path_players: &mut BTreeMap<PlayerId, PlayerPathAccumulator>,
    rng: &mut Rng64,
) -> Option<()> {
    let injury_base = segment_probability(input.in_match_events.injury_probability_per90, segment_minutes);
    let red_base = segment_probability(input.in_match_events.red_card_probability_per90, segment_minutes);
    let incident_map: BTreeMap<PlayerId, PlayerIncidentProfile> = input.incident_profiles.iter().copied().map(|value| (value.player, value)).collect();
    let mut remove = BTreeSet::<PlayerId>::new();
    let mut forced_replacements = Vec::<SampledStarter>::new();

    for player in &active.players {
        let profile = incident_map.get(&player.starter.player).copied().unwrap_or(PlayerIncidentProfile { player: player.starter.player, injury_multiplier: 1.0, red_card_multiplier: 1.0 });
        if rng.bernoulli((red_base * profile.red_card_multiplier).clamp(0.0, 1.0)) {
            remove.insert(player.starter.player);
            path_players.entry(player.starter.player).or_default().red_carded = true;
            continue;
        }
        if rng.bernoulli((injury_base * profile.injury_multiplier).clamp(0.0, 1.0)) {
            remove.insert(player.starter.player);
            path_players.entry(player.starter.player).or_default().injured_out = true;
            forced_replacements.push(player.starter);
        }
    }

    // Tactical exits use the sampled starter-minute target from the lineup process. The half-segment
    // tolerance maps a continuous target to the nearest future simulation boundary.
    if end_minute >= input.in_match_events.minimum_tactical_substitution_minute {
        for player in &active.players {
            if remove.contains(&player.starter.player) { continue; }
            if player.planned_exit_minute <= end_minute + 0.5 * segment_minutes && end_minute < 90.0 {
                remove.insert(player.starter.player);
                forced_replacements.push(player.starter);
            }
        }
    }
    active.players.retain(|value| !remove.contains(&value.starter.player));

    for outgoing in forced_replacements {
        if active.substitutions >= input.in_match_events.max_substitutions { break; }
        let Some(replacement) = sample_replacement(projection, active, outgoing, end_minute, rng) else { continue; };
        active.used_players.insert(replacement.starter.player);
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
    let candidates: Vec<&LineupCandidate> = projection.candidates.iter().filter(|candidate| {
        !active.used_players.contains(&candidate.player)
            && candidate.substitute_probability > 0.0
            && candidate.availability_probability > 0.0
    }).collect();
    if candidates.is_empty() { return None; }
    let weights = candidates.iter().map(|candidate| {
        candidate.substitute_probability
            * candidate.availability_probability
            * (candidate.role_fit(outgoing.role) + 0.02).powf(projection.parameters.role_fit_power)
            * (-projection.parameters.fatigue_weight * candidate.fatigue).exp()
    });
    let selected = candidates[weighted_index(weights, rng)?];
    if !rng.bernoulli(selected.availability_probability) { return None; }
    Some(ActivePlayer {
        starter: SampledStarter {
            player: selected.player,
            slot: outgoing.slot,
            role: outgoing.role,
            x: outgoing.x,
            y: outgoing.y,
            minutes: (90.0 - entered_minute).max(0.0),
            role_fit: selected.role_fit(outgoing.role),
            fatigue: selected.fatigue,
        },
        entered_minute,
        planned_exit_minute: 90.0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreState { Trailing, Level, Leading }

fn score_state(own: u32, opponent: u32) -> ScoreState {
    match own.cmp(&opponent) {
        std::cmp::Ordering::Less => ScoreState::Trailing,
        std::cmp::Ordering::Equal => ScoreState::Level,
        std::cmp::Ordering::Greater => ScoreState::Leading,
    }
}

fn score_state_multiplier(metric: MetricId, state: ScoreState, values: &[ScoreStateMetricModifier]) -> f64 {
    values.iter().filter(|value| value.metric == metric).map(|value| match state {
        ScoreState::Trailing => value.trailing_multiplier,
        ScoreState::Level => 1.0,
        ScoreState::Leading => value.leading_multiplier,
    }).product()
}

fn ability_metric_adjustment(ability: AbilityVector, metric: MetricId, values: &[AbilityMetricLoading]) -> f64 {
    values.iter().filter(|value| value.metric == metric).map(|value| {
        value.log_rate_weight * (ability_component(ability, value.dimension) - value.reference) / value.scale
    }).sum()
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

fn segment_probability(probability_per90: f64, segment_minutes: f64) -> f64 {
    1.0 - (1.0 - probability_per90.clamp(0.0, 1.0)).powf(segment_minutes / 90.0)
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

fn sample_log_rate(mean: f64, log_sigma: f64, rng: &mut Rng64) -> f64 {
    if log_sigma <= f64::EPSILON { return mean.max(f64::MIN_POSITIVE); }
    (mean.ln() - 0.5 * log_sigma.powi(2) + log_sigma * rng.normal()).exp()
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
    let variance = if sorted.len() > 1 {
        sorted.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (sorted.len() - 1) as f64
    } else { 0.0 };
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
    use crate::lineup::{FormationPrior, FormationTemplate, LineupCandidate, LineupSamplingParameters, RoleFit, TacticalSlot};
    use footy_core::RoleId;

    fn formation() -> FormationTemplate {
        FormationTemplate { id: 433, slots: (0..11).map(|slot| TacticalSlot {
            slot, role: RoleId(slot as u16 + 1), x: 0.05 + f64::from(slot) * 0.075, y: f64::from(slot % 5) / 4.0,
        }).collect() }
    }

    fn lineup(club: u32, offset: u64) -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(club), formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (0..16).map(|index| LineupCandidate {
                player: PlayerId(offset + index), start_probability: if index < 11 { 0.85 } else { 0.15 }, availability_probability: 1.0,
                ability_index: if club == 1 { 14.0 } else { 11.0 }, fatigue: 0.0,
                expected_minutes_if_start: if index < 3 { 65.0 } else { 88.0 }, minutes_sigma: 2.0, substitute_probability: if index >= 11 { 0.8 } else { 0.05 },
                role_fits: (1..=11).map(|role| RoleFit { role: RoleId(role), fit: if role == index as u16 + 1 { 1.0 } else { 0.25 } }).collect(),
            }).collect(), locked_starters: Vec::new(), parameters: LineupSamplingParameters::default(),
        }
    }

    fn input() -> CausalMatchSimulationInput {
        let home = lineup(1, 1);
        let away = lineup(2, 101);
        let all: Vec<PlayerId> = home.candidates.iter().chain(away.candidates.iter()).map(|value| value.player).collect();
        let metric = MetricId(1);
        CausalMatchSimulationInput {
            match_id: MatchId(9), home: ClubId(1), away: ClubId(2), home_lineup: home, away_lineup: away,
            player_states: all.iter().map(|player| PlayerMatchupState { player: *player, ability: AbilityVector::from_components([if player.0 < 100 { 14.0 } else { 11.0 }; 10]) }).collect(),
            event_models: all.iter().map(|player| PlayerEventModel {
                player: *player, metric, family: StatFamily::ContinuousNonNegative,
                mean_per90: if player.0 < 100 { 1.2 } else { 0.8 }, variance_per90: 0.02, epistemic_sigma: 0.02, reference_per90: 1.0,
                matchup_sensitivity: 0.1, teammate_sensitivity: 0.0, role_fit_sensitivity: 0.1, fatigue_sensitivity: 0.1,
            }).collect(),
            ability_metric_loadings: vec![AbilityMetricLoading { metric, dimension: AbilityDimension::Technical, reference: 12.5, scale: 20.0, log_rate_weight: 0.2 }],
            matchup_rules: vec![MatchupRule { metric: Some(metric), attacker_dimension: AbilityDimension::Technical, defender_dimension: AbilityDimension::Defending, weight: 1.0, scale: 20.0 }],
            synergies: Vec::new(),
            channel_graph: CausalChannelGraph {
                metric_loadings: vec![MetricChannelLoading { metric, channel: TeamProcessChannel::Progression, weight: 1.0 }],
                edges: vec![
                    ChannelEdge { from: TeamProcessChannel::Progression, to: TeamProcessChannel::ChanceCreation, weight: 0.6 },
                    ChannelEdge { from: TeamProcessChannel::ChanceCreation, to: TeamProcessChannel::ShotQuality, weight: 0.5 },
                ],
                goal_loadings: vec![GoalChannelLoading { channel: TeamProcessChannel::ShotQuality, attack_log_weight: 0.08, defense_log_weight: 0.0 }],
            },
            score_state_modifiers: vec![ScoreStateMetricModifier { metric, trailing_multiplier: 1.1, leading_multiplier: 0.92 }],
            incident_profiles: Vec::new(),
            in_match_events: InMatchEventParameters { injury_probability_per90: 0.0, red_card_probability_per90: 0.0, max_substitutions: 5, minimum_tactical_substitution_minute: 45.0 },
            home_context: TeamMatchContext { club: ClubId(1), attack_log_adjustment: 0.0, defense_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            away_context: TeamMatchContext { club: ClubId(2), attack_log_adjustment: 0.0, defense_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            environment: MatchEnvironment { home_attack_log_adjustment: 0.0, away_attack_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            goal_process: GoalProcessParameters { goals_per_team_match: 1.3, home_advantage_multiplier: 1.08, path_log_rate_sigma: 0.05, max_abs_log_adjustment: 1.5, segments: 6, trailing_attack_log_boost: 0.0, leading_attack_log_suppression: 0.0 },
            matchup_spatial_sigma: 0.25, max_player_rate_log_adjustment: 1.0,
        }
    }

    #[test]
    fn full_causal_match_is_reproducible() {
        let first = simulate_causal_match(&input(), 500, 42).unwrap();
        let second = simulate_causal_match(&input(), 500, 42).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.paths, 500);
        let probability = first.outcome.home_win.get() + first.outcome.draw.get() + first.outcome.away_win.get();
        assert!((probability - 1.0).abs() < 1e-6);
    }

    #[test]
    fn planned_starter_minutes_create_substitute_appearances() {
        let forecast = simulate_causal_match(&input(), 300, 7).unwrap();
        assert!(forecast.player_influences.iter().any(|value| value.player.0 >= 12 && value.player.0 <= 16 && value.appearance_probability.get() > 0.0));
    }

    #[test]
    fn channel_graph_is_visible_in_output() {
        let forecast = simulate_causal_match(&input(), 300, 9).unwrap();
        assert!(forecast.team_channels.iter().any(|value| value.channel == TeamProcessChannel::ShotQuality));
    }
}
