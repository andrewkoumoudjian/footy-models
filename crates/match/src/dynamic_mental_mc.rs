use crate::channels::TeamProcessChannel;
use crate::lineup::{
    sample_lineup, FormationProjection, LineupCandidate, LineupProjection, LineupProjectionInput,
    PlayerLineupProjection, RoleProjection, SampledLineup, SampledStarter,
};
use crate::matchup::player_metric_interaction_context;
use crate::psychology::{
    apply_mental_model, MatchMentalContext, MentalDimension, MentalMatchModel, PlayerMentalState,
    TeamMentalDimension, TeamMentalState,
};
use crate::specific::{
    AttributeEventLoading, AttributeMatchupRule, PlayerAttributeState, PrimitiveMatchForecast,
    PrimitiveMatchSimulationInput, PrimitivePlayerChannelInfluence, PrimitivePlayerInfluence,
    PrimitivePlayerMetricForecast, PrimitiveTeamChannelForecast, SpecificEnvironmentMetricModifier,
    SpecificPlayerIncidentProfile, SpecificScoreStateMetricModifier, SpecificTeamContext,
};
use footy_core::{ClubId, MetricId, PlayerId, Probability, RoleId};
use footy_forecast::{DistributionSummary, MatchOutcomeForecast, Rng64, StatFamily};
use std::collections::{BTreeMap, BTreeSet};

/// Recursive within-match mental-state transition parameters. Coefficients are primitives and must
/// be calibrated/versioned by callers; there is intentionally no universal "momentum" multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicMentalTransitionParameters {
    /// Latent prior sigma when a mental measure is entirely unknown. Known measures shrink this
    /// sigma linearly with confidence; an entirely missing player uses the same neutral prior.
    pub initial_state_sigma_at_zero_confidence: f64,
    pub goal_for_morale: f64,
    pub goal_against_morale: f64,
    pub goal_for_belief: f64,
    pub goal_against_belief: f64,
    pub teammate_red_morale: f64,
    pub teammate_red_belief: f64,
    pub teammate_injury_morale: f64,
    pub own_red_morale: f64,
    pub own_injury_morale: f64,
    pub personal_success_self_belief: f64,
    pub personal_error_self_belief: f64,
    pub personal_success_morale: f64,
    pub personal_error_morale: f64,
    pub substituted_off_role_satisfaction: f64,
    pub substituted_on_motivation: f64,
    pub trailing_pressure_belief: f64,
    pub leading_belief_boost: f64,
    pub leadership_buffer_strength: f64,
    pub cohesion_buffer_strength: f64,
    pub motivation_morale_coupling: f64,
    pub motivation_belief_coupling: f64,
    /// Mean reversion is toward this path's sampled kickoff state, not toward population zero.
    pub base_mean_reversion: f64,
    pub professionalism_reversion_weight: f64,
    /// Path-level innovation keeps the mental process stochastic. Consistency dampens this sigma.
    pub segment_innovation_sigma: f64,
    pub consistency_volatility_damping: f64,
    /// Current on-pitch players recursively pull collective team state toward their own state.
    pub active_lineup_blend: f64,
    pub max_abs_state_delta_per_segment: f64,
}

impl Default for DynamicMentalTransitionParameters {
    fn default() -> Self {
        Self {
            initial_state_sigma_at_zero_confidence: 0.25,
            goal_for_morale: 0.08,
            goal_against_morale: -0.10,
            goal_for_belief: 0.10,
            goal_against_belief: -0.12,
            teammate_red_morale: -0.08,
            teammate_red_belief: -0.10,
            teammate_injury_morale: -0.03,
            own_red_morale: -0.15,
            own_injury_morale: -0.08,
            personal_success_self_belief: 0.05,
            personal_error_self_belief: -0.06,
            personal_success_morale: 0.02,
            personal_error_morale: -0.03,
            substituted_off_role_satisfaction: -0.04,
            substituted_on_motivation: 0.04,
            trailing_pressure_belief: -0.05,
            leading_belief_boost: 0.03,
            leadership_buffer_strength: 0.45,
            cohesion_buffer_strength: 0.35,
            motivation_morale_coupling: 0.35,
            motivation_belief_coupling: 0.25,
            base_mean_reversion: 0.08,
            professionalism_reversion_weight: 0.50,
            segment_innovation_sigma: 0.025,
            consistency_volatility_damping: 0.65,
            active_lineup_blend: 0.20,
            max_abs_state_delta_per_segment: 0.35,
        }
    }
}

impl DynamicMentalTransitionParameters {
    pub fn validate(self) -> bool {
        let values = [
            self.initial_state_sigma_at_zero_confidence,
            self.goal_for_morale,
            self.goal_against_morale,
            self.goal_for_belief,
            self.goal_against_belief,
            self.teammate_red_morale,
            self.teammate_red_belief,
            self.teammate_injury_morale,
            self.own_red_morale,
            self.own_injury_morale,
            self.personal_success_self_belief,
            self.personal_error_self_belief,
            self.personal_success_morale,
            self.personal_error_morale,
            self.substituted_off_role_satisfaction,
            self.substituted_on_motivation,
            self.trailing_pressure_belief,
            self.leading_belief_boost,
            self.leadership_buffer_strength,
            self.cohesion_buffer_strength,
            self.motivation_morale_coupling,
            self.motivation_belief_coupling,
            self.base_mean_reversion,
            self.professionalism_reversion_weight,
            self.segment_innovation_sigma,
            self.consistency_volatility_damping,
            self.active_lineup_blend,
            self.max_abs_state_delta_per_segment,
        ];
        values.iter().all(|value| value.is_finite())
            && self.initial_state_sigma_at_zero_confidence >= 0.0
            && self.leadership_buffer_strength >= 0.0
            && self.cohesion_buffer_strength >= 0.0
            && self.base_mean_reversion >= 0.0
            && self.professionalism_reversion_weight >= 0.0
            && self.segment_innovation_sigma >= 0.0
            && (0.0..=1.0).contains(&self.consistency_volatility_damping)
            && (0.0..=1.0).contains(&self.active_lineup_blend)
            && self.max_abs_state_delta_per_segment > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DynamicMentalTransitionDriver {
    GoalFor,
    GoalAgainst,
    PersonalSuccess,
    PersonalError,
    OwnRedCard,
    OwnInjury,
    TeammateRedCard,
    TeammateInjury,
    SubstitutedOff,
    SubstitutedOn,
    ScorePressure,
    MeanReversion,
    Innovation,
    ActiveLineupSocialState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicMentalDriverContribution {
    pub player: PlayerId,
    pub driver: DynamicMentalTransitionDriver,
    /// Can exceed one because the same transition type may occur in several segments of one path.
    pub mean_occurrences_per_path: f64,
    pub mean_morale_delta_per_path: f64,
    pub mean_self_belief_delta_per_path: f64,
    pub mean_motivation_delta_per_path: f64,
    pub mean_role_satisfaction_delta_per_path: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicPlayerMentalForecast {
    pub player: PlayerId,
    pub initial_morale_mean: f64,
    pub final_morale_mean: f64,
    pub initial_self_belief_mean: f64,
    pub final_self_belief_mean: f64,
    pub initial_motivation_mean: f64,
    pub final_motivation_mean: f64,
    pub final_role_satisfaction_mean: f64,
    pub probability_finish_morale_above_kickoff: Probability,
    pub mean_abs_morale_path_change: f64,
    /// Average dynamic mental contribution to modeled player-event means. Stable technical/system
    /// effects remain in the ordinary primitive influence summary.
    pub mean_event_mental_log_adjustment: f64,
    pub mean_event_mental_log_variance_adjustment: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicTeamMentalForecast {
    pub club: ClubId,
    pub initial_collective_morale_mean: f64,
    pub final_collective_morale_mean: f64,
    pub initial_collective_belief_mean: f64,
    pub final_collective_belief_mean: f64,
    pub final_cohesion_mean: f64,
    pub final_leadership_mean: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicMentalMatchForecast {
    pub forecast: PrimitiveMatchForecast,
    pub players: Vec<DynamicPlayerMentalForecast>,
    pub teams: Vec<DynamicTeamMentalForecast>,
    pub transition_drivers: Vec<DynamicMentalDriverContribution>,
    /// Exact number of independent full-match Monte Carlo paths used by every output distribution.
    pub paths: u32,
}

#[derive(Debug, Clone)]
struct PathMentalPlayer {
    kickoff: BTreeMap<MentalDimension, f64>,
    current: BTreeMap<MentalDimension, f64>,
}

impl PathMentalPlayer {
    fn value(&self, dimension: MentalDimension) -> f64 {
        self.current.get(&dimension).copied().unwrap_or(0.0)
    }

    fn kickoff_value(&self, dimension: MentalDimension) -> f64 {
        self.kickoff.get(&dimension).copied().unwrap_or(0.0)
    }

    fn add(&mut self, dimension: MentalDimension, delta: f64, max_abs_delta: f64) -> f64 {
        let bounded = delta.clamp(-max_abs_delta, max_abs_delta);
        let entry = self.current.entry(dimension).or_default();
        let before = *entry;
        *entry = (*entry + bounded).clamp(-1.0, 1.0);
        *entry - before
    }
}

#[derive(Debug, Clone)]
struct PathTeamMental {
    club: ClubId,
    kickoff: BTreeMap<TeamMentalDimension, f64>,
    current: BTreeMap<TeamMentalDimension, f64>,
}

impl PathTeamMental {
    fn value(&self, dimension: TeamMentalDimension) -> f64 {
        self.current.get(&dimension).copied().unwrap_or(0.0)
    }

    fn kickoff_value(&self, dimension: TeamMentalDimension) -> f64 {
        self.kickoff.get(&dimension).copied().unwrap_or(0.0)
    }

    fn add(&mut self, dimension: TeamMentalDimension, delta: f64, max_abs_delta: f64) {
        let entry = self.current.entry(dimension).or_default();
        *entry = (*entry + delta.clamp(-max_abs_delta, max_abs_delta)).clamp(-1.0, 1.0);
    }
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

    fn player_ids(&self) -> Vec<PlayerId> { self.players.iter().map(|value| value.sampled.player).collect() }
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
    mental_mean: f64,
    mental_variance: f64,
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
    mental_mean: f64,
    mental_variance: f64,
    channels: BTreeMap<TeamProcessChannel, f64>,
}

#[derive(Default)]
struct PathMetric { total: f64, minutes: f64 }
#[derive(Default)]
struct AggregateMetric { unconditional: Vec<f64>, conditional: Vec<f64>, conditional_per90: Vec<f64> }

#[derive(Default)]
struct MentalPlayerAggregate {
    observations: u32,
    initial_morale: f64,
    final_morale: f64,
    initial_belief: f64,
    final_belief: f64,
    initial_motivation: f64,
    final_motivation: f64,
    final_role: f64,
    above: u32,
    abs_change: f64,
}

#[derive(Default)]
struct MentalTeamAggregate {
    observations: u32,
    initial_morale: f64,
    final_morale: f64,
    initial_belief: f64,
    final_belief: f64,
    final_cohesion: f64,
    final_leadership: f64,
}

#[derive(Default)]
struct DriverAggregate {
    occurrences: u32,
    morale: f64,
    belief: f64,
    motivation: f64,
    role: f64,
}

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
    mental_mean: f64,
    mental_variance: f64,
}

struct SegmentOutput {
    channels: Vec<(TeamProcessChannel, f64)>,
    attack: f64,
    defense: f64,
    metrics: Vec<SegmentMetric>,
}

#[derive(Default)]
struct TeamTransitionEvents {
    red_cards: Vec<PlayerId>,
    injuries: Vec<PlayerId>,
    substitutions: Vec<(PlayerId, PlayerId)>,
}

#[derive(Default)]
struct LineupAggregate {
    formation_counts: BTreeMap<u16, u32>,
    starts: BTreeMap<PlayerId, u32>,
    minutes: BTreeMap<PlayerId, f64>,
    roles: BTreeMap<(PlayerId, RoleId), u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreState { Trailing, Level, Leading }

/// Simulate one fixture through exactly `paths` complete Monte Carlo histories.
///
/// Path order:
/// latent kickoff psychology -> mental-conditioned projected XI -> active player graph -> segment
/// player events -> causal football channels -> goals/incidents/substitutions -> recursive mental
/// transition -> rebuilt active graph -> next segment. Nothing from path i is shared with path j.
pub fn simulate_dynamic_mental_match(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    transition: DynamicMentalTransitionParameters,
    paths: u32,
    seed: u64,
) -> Option<DynamicMentalMatchForecast> {
    if paths == 0 || !transition.validate() || !input.validate() { return None; }
    // Reuse existing mental-model validation, but do not consume its deterministic adjusted input.
    // This engine applies mental state after sampling it inside each path.
    let _ = apply_mental_model(input, mental_model)?;
    let mut rng = Rng64::new(seed);
    let segment_minutes = 90.0 / f64::from(input.goal_process.segments);
    let source_states: BTreeMap<PlayerId, PlayerMentalState> = mental_model.players.iter().copied().map(|value| (value.player, value)).collect();
    let home_squad: Vec<PlayerId> = input.home_lineup.candidates.iter().map(|value| value.player).collect();
    let away_squad: Vec<PlayerId> = input.away_lineup.candidates.iter().map(|value| value.player).collect();
    if home_squad.iter().any(|player| away_squad.contains(player)) { return None; }
    let all_squad: Vec<PlayerId> = home_squad.iter().chain(away_squad.iter()).copied().collect();

    let mut home_goal_samples = Vec::with_capacity(paths as usize);
    let mut away_goal_samples = Vec::with_capacity(paths as usize);
    let mut home_wins = 0_u32;
    let mut draws = 0_u32;
    let mut away_wins = 0_u32;
    let mut players = BTreeMap::<PlayerId, AggregatePlayer>::new();
    let mut metrics = BTreeMap::<(PlayerId, MetricId), AggregateMetric>::new();
    let mut team_channels = BTreeMap::<(ClubId, TeamProcessChannel), f64>::new();
    let mut mental_players = BTreeMap::<PlayerId, MentalPlayerAggregate>::new();
    let mut mental_teams = BTreeMap::<ClubId, MentalTeamAggregate>::new();
    let mut drivers = BTreeMap::<(PlayerId, DynamicMentalTransitionDriver), DriverAggregate>::new();
    let mut home_lineups = LineupAggregate::default();
    let mut away_lineups = LineupAggregate::default();
    for event in &input.event_models { metrics.entry((event.player, event.metric)).or_default(); }

    for _ in 0..paths {
        let mut path_mental = BTreeMap::<PlayerId, PathMentalPlayer>::new();
        for player in &all_squad {
            let state = match source_states.get(player) {
                Some(value) => sample_player_mental(*value, transition, &mut rng),
                None => sample_unknown_player_mental(transition, &mut rng),
            };
            path_mental.insert(*player, state);
        }
        let mut home_projection = input.home_lineup.clone();
        let mut away_projection = input.away_lineup.clone();
        adjust_lineup_for_path_mental(&mut home_projection, &path_mental, mental_model);
        adjust_lineup_for_path_mental(&mut away_projection, &path_mental, mental_model);
        let home_start = sample_lineup(&home_projection, &mut rng)?;
        let away_start = sample_lineup(&away_projection, &mut rng)?;
        accumulate_lineup(&home_start, &mut home_lineups);
        accumulate_lineup(&away_start, &mut away_lineups);
        let mut home = ActiveTeam::from_start(home_start.clone());
        let mut away = ActiveTeam::from_start(away_start.clone());
        let mut home_team_mental = sample_team_mental(mental_model.home_team, transition, &mut rng);
        let mut away_team_mental = sample_team_mental(mental_model.away_team, transition, &mut rng);
        blend_team_with_active(&mut home_team_mental, &home, &path_mental, transition.active_lineup_blend);
        blend_team_with_active(&mut away_team_mental, &away, &path_mental, transition.active_lineup_blend);
        let mut path_players = BTreeMap::<PlayerId, PathPlayer>::new();
        let mut path_metrics = BTreeMap::<(PlayerId, MetricId), PathMetric>::new();
        for starter in home_start.starters.iter().chain(away_start.starters.iter()) {
            path_players.entry(starter.player).or_default().started = true;
        }
        let path_home_rate = sample_log_rate(
            input.goal_process.goals_per_team_match * input.goal_process.home_advantage_multiplier,
            input.goal_process.path_log_rate_sigma,
            &mut rng,
        );
        let path_away_rate = sample_log_rate(
            input.goal_process.goals_per_team_match,
            input.goal_process.path_log_rate_sigma,
            &mut rng,
        );
        let mut home_goals = 0_u32;
        let mut away_goals = 0_u32;

        for segment in 0..input.goal_process.segments {
            let end_minute = f64::from(segment + 1) * segment_minutes;
            let home_lineup = home.lineup();
            let away_lineup = away.lineup();
            let home_output = segment_output(
                input, mental_model, &path_mental, &home_team_mental, &home_lineup, &away_lineup,
                true, score_state(home_goals, away_goals), segment_minutes, &mut rng,
            )?;
            let away_output = segment_output(
                input, mental_model, &path_mental, &away_team_mental, &away_lineup, &home_lineup,
                false, score_state(away_goals, home_goals), segment_minutes, &mut rng,
            )?;
            accumulate_segment(&home_output, &home, input.home, segment_minutes, &mut path_players, &mut path_metrics, &mut team_channels);
            accumulate_segment(&away_output, &away, input.away, segment_minutes, &mut path_players, &mut path_metrics, &mut team_channels);

            let home_log = (
                input.environment.home_residual_attack_log_adjustment
                    + input.home_context.residual_attack_log_adjustment
                    - input.away_context.residual_defense_log_adjustment
                    + home_output.attack
                    - away_output.defense
            ).clamp(-input.goal_process.max_abs_segment_log_adjustment, input.goal_process.max_abs_segment_log_adjustment);
            let away_log = (
                input.environment.away_residual_attack_log_adjustment
                    + input.away_context.residual_attack_log_adjustment
                    - input.home_context.residual_defense_log_adjustment
                    + away_output.attack
                    - home_output.defense
            ).clamp(-input.goal_process.max_abs_segment_log_adjustment, input.goal_process.max_abs_segment_log_adjustment);
            let home_segment_goals = poisson(path_home_rate * home_log.exp() / f64::from(input.goal_process.segments), &mut rng);
            let away_segment_goals = poisson(path_away_rate * away_log.exp() / f64::from(input.goal_process.segments), &mut rng);
            home_goals = home_goals.saturating_add(home_segment_goals);
            away_goals = away_goals.saturating_add(away_segment_goals);

            let mut home_events = TeamTransitionEvents::default();
            let mut away_events = TeamTransitionEvents::default();
            if segment + 1 < input.goal_process.segments {
                home_events = evolve_active_team(
                    input, mental_model, &path_mental, &mut home, &home_projection,
                    end_minute, segment_minutes, &mut path_players, &mut rng,
                )?;
                away_events = evolve_active_team(
                    input, mental_model, &path_mental, &mut away, &away_projection,
                    end_minute, segment_minutes, &mut path_players, &mut rng,
                )?;
            }

            transition_team_and_players(
                &home_squad,
                &home,
                &mut home_team_mental,
                &mut path_mental,
                &personal_segment_signal(&home_output),
                home_segment_goals,
                away_segment_goals,
                &home_events,
                score_state(home_goals, away_goals),
                mental_model.context,
                transition,
                &mut drivers,
                &mut rng,
            );
            transition_team_and_players(
                &away_squad,
                &away,
                &mut away_team_mental,
                &mut path_mental,
                &personal_segment_signal(&away_output),
                away_segment_goals,
                home_segment_goals,
                &away_events,
                score_state(away_goals, home_goals),
                mental_model.context,
                transition,
                &mut drivers,
                &mut rng,
            );
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
            } else {
                aggregate.unconditional.push(0.0);
            }
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
            aggregate.mental_mean += value.mental_mean;
            aggregate.mental_variance += value.mental_variance;
            for (channel, amount) in value.channels { *aggregate.channels.entry(channel).or_default() += amount; }
        }
        for (player, state) in path_mental {
            let aggregate = mental_players.entry(player).or_default();
            let initial_morale = state.kickoff_value(MentalDimension::Morale);
            let final_morale = state.value(MentalDimension::Morale);
            aggregate.observations = aggregate.observations.saturating_add(1);
            aggregate.initial_morale += initial_morale;
            aggregate.final_morale += final_morale;
            aggregate.initial_belief += state.kickoff_value(MentalDimension::SelfBelief);
            aggregate.final_belief += state.value(MentalDimension::SelfBelief);
            aggregate.initial_motivation += state.kickoff_value(MentalDimension::Motivation);
            aggregate.final_motivation += state.value(MentalDimension::Motivation);
            aggregate.final_role += state.value(MentalDimension::RoleSatisfaction);
            aggregate.abs_change += (final_morale - initial_morale).abs();
            if final_morale > initial_morale { aggregate.above = aggregate.above.saturating_add(1); }
        }
        accumulate_team_mental(&home_team_mental, &mut mental_teams);
        accumulate_team_mental(&away_team_mental, &mut mental_teams);
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
        home_goals: summarize(&home_goal_samples)?,
        away_goals: summarize(&away_goal_samples)?,
        home_expected_points: 3.0 * f64::from(home_win.get()) + f64::from(draw.get()),
        away_expected_points: 3.0 * f64::from(away_win.get()) + f64::from(draw.get()),
        paths,
    };
    let home_lineup = lineup_projection_from_aggregate(&input.home_lineup, &home_lineups, paths);
    let away_lineup = lineup_projection_from_aggregate(&input.away_lineup, &away_lineups, paths);
    let player_metrics = metrics.into_iter().map(|((player, metric), value)| {
        let summary = players.get(&player);
        Some(PrimitivePlayerMetricForecast {
            player,
            metric,
            start_probability: Probability::clamped(summary.map(|value| value.starts).unwrap_or(0) as f32 / denominator),
            appearance_probability: Probability::clamped(summary.map(|value| value.appearances).unwrap_or(0) as f32 / denominator),
            expected_minutes: summary.map(|value| value.minutes / f64::from(paths)).unwrap_or(0.0),
            unconditional_total: summarize(&value.unconditional)?,
            conditional_on_appearance_total: summarize(&value.conditional),
            conditional_on_appearance_per90: summarize(&value.conditional_per90),
        })
    }).collect::<Option<Vec<_>>>()?;
    let mut player_channels = Vec::new();
    let mut mental_event_effects = BTreeMap::<PlayerId, (f64, f64, f64)>::new();
    let player_influences = players.into_iter().map(|(player, value)| {
        let factor_denominator = f64::from(value.factors.max(1));
        mental_event_effects.insert(player, (value.mental_mean, value.mental_variance, factor_denominator));
        for (channel, amount) in &value.channels {
            player_channels.push(PrimitivePlayerChannelInfluence {
                player,
                channel: *channel,
                mean_contribution: *amount / f64::from(paths),
            });
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
        club,
        channel,
        mean_match_contribution: total / f64::from(paths),
    }).collect();
    let forecast = PrimitiveMatchForecast {
        outcome,
        home_lineup,
        away_lineup,
        player_metrics,
        player_influences,
        player_channels,
        team_channels,
        paths,
    };

    let players = mental_players.into_iter().map(|(player, value)| {
        let n = f64::from(value.observations.max(1));
        let (mental_mean, mental_variance, factors) = mental_event_effects.get(&player).copied().unwrap_or((0.0, 0.0, 1.0));
        DynamicPlayerMentalForecast {
            player,
            initial_morale_mean: value.initial_morale / n,
            final_morale_mean: value.final_morale / n,
            initial_self_belief_mean: value.initial_belief / n,
            final_self_belief_mean: value.final_belief / n,
            initial_motivation_mean: value.initial_motivation / n,
            final_motivation_mean: value.final_motivation / n,
            final_role_satisfaction_mean: value.final_role / n,
            probability_finish_morale_above_kickoff: Probability::clamped(value.above as f32 / value.observations.max(1) as f32),
            mean_abs_morale_path_change: value.abs_change / n,
            mean_event_mental_log_adjustment: mental_mean / factors,
            mean_event_mental_log_variance_adjustment: mental_variance / factors,
        }
    }).collect();
    let teams = mental_teams.into_iter().map(|(club, value)| {
        let n = f64::from(value.observations.max(1));
        DynamicTeamMentalForecast {
            club,
            initial_collective_morale_mean: value.initial_morale / n,
            final_collective_morale_mean: value.final_morale / n,
            initial_collective_belief_mean: value.initial_belief / n,
            final_collective_belief_mean: value.final_belief / n,
            final_cohesion_mean: value.final_cohesion / n,
            final_leadership_mean: value.final_leadership / n,
        }
    }).collect();
    let transition_drivers = drivers.into_iter().map(|((player, driver), value)| DynamicMentalDriverContribution {
        player,
        driver,
        mean_occurrences_per_path: f64::from(value.occurrences) / f64::from(paths),
        mean_morale_delta_per_path: value.morale / f64::from(paths),
        mean_self_belief_delta_per_path: value.belief / f64::from(paths),
        mean_motivation_delta_per_path: value.motivation / f64::from(paths),
        mean_role_satisfaction_delta_per_path: value.role / f64::from(paths),
    }).collect();
    Some(DynamicMentalMatchForecast { forecast, players, teams, transition_drivers, paths })
}

fn all_dimensions() -> [MentalDimension; 14] {
    [
        MentalDimension::Morale,
        MentalDimension::SelfBelief,
        MentalDimension::Form,
        MentalDimension::FormTrend,
        MentalDimension::Motivation,
        MentalDimension::RoleSatisfaction,
        MentalDimension::Professionalism,
        MentalDimension::Ambition,
        MentalDimension::Consistency,
        MentalDimension::PressureResilience,
        MentalDimension::Adaptability,
        MentalDimension::Leadership,
        MentalDimension::Teamwork,
        MentalDimension::Discipline,
    ]
}

fn sample_player_mental(
    state: PlayerMentalState,
    transition: DynamicMentalTransitionParameters,
    rng: &mut Rng64,
) -> PathMentalPlayer {
    let mut kickoff = BTreeMap::new();
    for dimension in all_dimensions() {
        let measure = state.measure(dimension);
        let confidence = f64::from(measure.confidence.get());
        let mean = measure.value * confidence;
        let sigma = transition.initial_state_sigma_at_zero_confidence * (1.0 - confidence);
        kickoff.insert(dimension, (mean + sigma * rng.normal()).clamp(-1.0, 1.0));
    }
    PathMentalPlayer { current: kickoff.clone(), kickoff }
}

fn sample_unknown_player_mental(
    transition: DynamicMentalTransitionParameters,
    rng: &mut Rng64,
) -> PathMentalPlayer {
    let mut kickoff = BTreeMap::new();
    for dimension in all_dimensions() {
        kickoff.insert(
            dimension,
            (transition.initial_state_sigma_at_zero_confidence * rng.normal()).clamp(-1.0, 1.0),
        );
    }
    PathMentalPlayer { current: kickoff.clone(), kickoff }
}

fn sample_team_mental(
    state: TeamMentalState,
    transition: DynamicMentalTransitionParameters,
    rng: &mut Rng64,
) -> PathTeamMental {
    let dimensions = [
        TeamMentalDimension::CollectiveMorale,
        TeamMentalDimension::CollectiveBelief,
        TeamMentalDimension::Cohesion,
        TeamMentalDimension::Leadership,
    ];
    let mut kickoff = BTreeMap::new();
    for dimension in dimensions {
        let measure = state.measure(dimension);
        let confidence = f64::from(measure.confidence.get());
        let mean = measure.value * confidence;
        let sigma = transition.initial_state_sigma_at_zero_confidence * (1.0 - confidence);
        kickoff.insert(dimension, (mean + sigma * rng.normal()).clamp(-1.0, 1.0));
    }
    PathTeamMental { club: state.club, current: kickoff.clone(), kickoff }
}

fn adjust_lineup_for_path_mental(
    lineup: &mut LineupProjectionInput,
    states: &BTreeMap<PlayerId, PathMentalPlayer>,
    model: &MentalMatchModel,
) {
    for candidate in &mut lineup.candidates {
        let Some(state) = states.get(&candidate.player) else { continue; };
        let mut start_delta = 0.0;
        let mut substitute_delta = 0.0;
        for loading in &model.selection_loadings {
            let effect = state.value(loading.dimension) * context_scale(loading.dimension, model.context);
            start_delta += loading.start_logit_weight * effect;
            substitute_delta += loading.substitute_logit_weight * effect;
        }
        start_delta = start_delta.clamp(-model.parameters.max_abs_selection_logit_adjustment, model.parameters.max_abs_selection_logit_adjustment);
        substitute_delta = substitute_delta.clamp(-model.parameters.max_abs_selection_logit_adjustment, model.parameters.max_abs_selection_logit_adjustment);
        candidate.start_probability = adjust_probability(candidate.start_probability, start_delta);
        candidate.substitute_probability = adjust_probability(candidate.substitute_probability, substitute_delta);
    }
}

fn segment_output(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    mental_players: &BTreeMap<PlayerId, PathMentalPlayer>,
    team_mental: &PathTeamMental,
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
        for event in input.event_models.iter().filter(|value| value.player == starter.player) {
            let aggregate = player_metric_interaction_context(
                starter.player,
                event.metric,
                own,
                opponent,
                &input.player_states,
                &input.aggregate_matchup_rules,
                &input.synergies,
                input.matchup_spatial_sigma,
            )?;
            let own_attribute_log = attributes.get(&starter.player)
                .map(|state| own_attribute_adjustment(state, event.metric, &input.own_attribute_loadings))
                .unwrap_or(0.0);
            let attribute_advantage = fine_matchup_advantage(
                starter,
                opponent,
                attributes.get(&starter.player).copied(),
                &attributes,
                event.metric,
                &input.attribute_matchup_rules,
                input.matchup_spatial_sigma,
            )?;
            let aggregate_matchup_log = event.aggregate_matchup_sensitivity * aggregate.opponent_advantage;
            let attribute_matchup_log = event.attribute_matchup_sensitivity * attribute_advantage;
            let teammate_log = event.teammate_sensitivity * aggregate.teammate_support;
            let role_log = event.role_fit_sensitivity * (starter.role_fit - 1.0);
            let fatigue_log = -event.fatigue_sensitivity * starter.fatigue;
            let system_multiplier = team_production_multiplier(own_context, event.metric)
                * team_suppression_multiplier(opponent_context, event.metric)
                * environment_multiplier(&input.environment.metric_modifiers, event.metric, is_home);
            if !system_multiplier.is_finite() || system_multiplier <= 0.0 { return None; }
            let system_log = system_multiplier.ln();
            let score_log = score_multiplier(event.metric, score, &input.score_state_modifiers).ln();
            let (mental_mean_log, mental_variance_log) = mental_metric_adjustment(
                event.metric,
                mental_players.get(&starter.player),
                team_mental,
                mental_model,
            );
            let log_adjustment = (
                own_attribute_log
                    + aggregate_matchup_log
                    + attribute_matchup_log
                    + teammate_log
                    + role_log
                    + fatigue_log
                    + system_log
                    + score_log
                    + mental_mean_log
            ).clamp(-input.max_player_rate_log_adjustment, input.max_player_rate_log_adjustment);
            let multiplier = log_adjustment.exp();
            let latent = sample_non_negative_normal(
                event.mean_per90 * multiplier,
                event.epistemic_sigma * multiplier,
                rng,
            );
            let rate = sample_non_negative_normal(
                latent,
                (event.variance_per90 * mental_variance_log.exp()).max(0.0).sqrt() * multiplier,
                rng,
            );
            let exposure = segment_minutes / 90.0;
            let total = match event.family {
                StatFamily::ContinuousNonNegative => rate * exposure,
                StatFamily::Count => f64::from(poisson(rate * exposure, rng)),
            };
            let effective_rate = if exposure > 0.0 { total / exposure } else { 0.0 };
            let deviation = (effective_rate / event.reference_per90 - 1.0) * exposure;
            deviations.push((event.metric, deviation));
            segment_metrics.push(SegmentMetric {
                player: starter.player,
                metric: event.metric,
                total,
                deviation,
                own_attribute: own_attribute_log,
                aggregate_matchup: aggregate_matchup_log,
                attribute_matchup: attribute_matchup_log,
                teammate: teammate_log,
                role: role_log,
                fatigue: fatigue_log,
                system: system_log,
                score: score_log,
                mental_mean: mental_mean_log,
                mental_variance: mental_variance_log,
            });
        }
    }
    let channels = input.channel_graph.evaluate(&deviations)?;
    Some(SegmentOutput {
        channels: channels.values,
        attack: channels.attack_log_adjustment,
        defense: channels.defense_log_adjustment,
        metrics: segment_metrics,
    })
}

fn mental_metric_adjustment(
    metric: MetricId,
    state: Option<&PathMentalPlayer>,
    team: &PathTeamMental,
    model: &MentalMatchModel,
) -> (f64, f64) {
    let mut mean = 0.0;
    let mut variance = 0.0;
    if let Some(state) = state {
        for loading in model.metric_loadings.iter().filter(|value| value.metric == metric) {
            let value = state.value(loading.dimension) * context_scale(loading.dimension, model.context);
            mean += loading.mean_log_weight * value;
            variance += loading.log_variance_weight * value;
        }
    }
    for loading in model.team_metric_loadings.iter().filter(|value| value.metric == metric) {
        let value = team.value(loading.dimension);
        mean += loading.mean_log_weight * value;
        variance += loading.log_variance_weight * value;
    }
    (
        mean.clamp(-model.parameters.max_abs_mean_log_adjustment, model.parameters.max_abs_mean_log_adjustment),
        variance.clamp(-model.parameters.max_abs_variance_log_adjustment, model.parameters.max_abs_variance_log_adjustment),
    )
}

fn transition_team_and_players(
    squad: &[PlayerId],
    active: &ActiveTeam,
    team: &mut PathTeamMental,
    players: &mut BTreeMap<PlayerId, PathMentalPlayer>,
    personal: &BTreeMap<PlayerId, f64>,
    goals_for: u32,
    goals_against: u32,
    events: &TeamTransitionEvents,
    score: ScoreState,
    context: MatchMentalContext,
    parameters: DynamicMentalTransitionParameters,
    drivers: &mut BTreeMap<(PlayerId, DynamicMentalTransitionDriver), DriverAggregate>,
    rng: &mut Rng64,
) {
    let active_ids = active.player_ids();
    let leadership = active_average(players, &active_ids, MentalDimension::Leadership);
    let teamwork = active_average(players, &active_ids, MentalDimension::Teamwork);
    let leadership_buffer = (parameters.leadership_buffer_strength * unit_support(leadership)).clamp(0.0, 0.95);
    let cohesion_buffer = (parameters.cohesion_buffer_strength
        * unit_support(team.value(TeamMentalDimension::Cohesion).max(teamwork))).clamp(0.0, 0.95);
    let negative_buffer = (1.0 - leadership_buffer) * (1.0 - cohesion_buffer);
    let goal_morale_raw = f64::from(goals_for) * parameters.goal_for_morale
        + f64::from(goals_against) * parameters.goal_against_morale;
    let goal_belief_raw = f64::from(goals_for) * parameters.goal_for_belief
        + f64::from(goals_against) * parameters.goal_against_belief;
    let goal_morale = if goal_morale_raw < 0.0 { goal_morale_raw * negative_buffer } else { goal_morale_raw };
    let goal_belief = if goal_belief_raw < 0.0 { goal_belief_raw * negative_buffer } else { goal_belief_raw };
    team.add(TeamMentalDimension::CollectiveMorale, goal_morale, parameters.max_abs_state_delta_per_segment);
    team.add(TeamMentalDimension::CollectiveBelief, goal_belief, parameters.max_abs_state_delta_per_segment);
    if !events.red_cards.is_empty() {
        team.add(
            TeamMentalDimension::Cohesion,
            -0.25 * parameters.teammate_red_morale.abs() * events.red_cards.len() as f64,
            parameters.max_abs_state_delta_per_segment,
        );
    }
    team.current.insert(TeamMentalDimension::Leadership, leadership.clamp(-1.0, 1.0));

    for player in squad {
        let Some(state) = players.get_mut(player) else { continue; };
        let is_active = active_ids.contains(player);
        let mut morale_delta = goal_morale;
        let mut belief_delta = goal_belief;
        if goals_for > 0 {
            record_driver(
                drivers, *player, DynamicMentalTransitionDriver::GoalFor,
                f64::from(goals_for) * parameters.goal_for_morale,
                f64::from(goals_for) * parameters.goal_for_belief, 0.0, 0.0,
            );
        }
        if goals_against > 0 {
            record_driver(
                drivers, *player, DynamicMentalTransitionDriver::GoalAgainst,
                f64::from(goals_against) * parameters.goal_against_morale * negative_buffer,
                f64::from(goals_against) * parameters.goal_against_belief * negative_buffer, 0.0, 0.0,
            );
        }
        if is_active {
            if let Some(signal) = personal.get(player).copied() {
                if signal >= 0.0 {
                    let belief = signal * parameters.personal_success_self_belief;
                    let morale = signal * parameters.personal_success_morale;
                    belief_delta += belief;
                    morale_delta += morale;
                    record_driver(drivers, *player, DynamicMentalTransitionDriver::PersonalSuccess, morale, belief, 0.0, 0.0);
                } else {
                    let magnitude = -signal;
                    let belief = magnitude * parameters.personal_error_self_belief;
                    let morale = magnitude * parameters.personal_error_morale;
                    belief_delta += belief;
                    morale_delta += morale;
                    record_driver(drivers, *player, DynamicMentalTransitionDriver::PersonalError, morale, belief, 0.0, 0.0);
                }
            }
        }
        if events.red_cards.contains(player) {
            morale_delta += parameters.own_red_morale;
            record_driver(drivers, *player, DynamicMentalTransitionDriver::OwnRedCard, parameters.own_red_morale, 0.0, 0.0, 0.0);
        } else if !events.red_cards.is_empty() {
            let morale = parameters.teammate_red_morale * events.red_cards.len() as f64 * negative_buffer;
            let belief = parameters.teammate_red_belief * events.red_cards.len() as f64 * negative_buffer;
            morale_delta += morale;
            belief_delta += belief;
            record_driver(drivers, *player, DynamicMentalTransitionDriver::TeammateRedCard, morale, belief, 0.0, 0.0);
        }
        if events.injuries.contains(player) {
            morale_delta += parameters.own_injury_morale;
            record_driver(drivers, *player, DynamicMentalTransitionDriver::OwnInjury, parameters.own_injury_morale, 0.0, 0.0, 0.0);
        } else if !events.injuries.is_empty() {
            let morale = parameters.teammate_injury_morale * events.injuries.len() as f64 * negative_buffer;
            morale_delta += morale;
            record_driver(drivers, *player, DynamicMentalTransitionDriver::TeammateInjury, morale, 0.0, 0.0, 0.0);
        }
        let pressure_delta = match score {
            ScoreState::Trailing => parameters.trailing_pressure_belief
                * context.pressure_intensity
                * (1.0 - unit_support(state.value(MentalDimension::PressureResilience))),
            ScoreState::Leading => parameters.leading_belief_boost * context.pressure_intensity,
            ScoreState::Level => 0.0,
        };
        belief_delta += pressure_delta;
        if pressure_delta != 0.0 {
            record_driver(drivers, *player, DynamicMentalTransitionDriver::ScorePressure, 0.0, pressure_delta, 0.0, 0.0);
        }

        let actual_morale = state.add(MentalDimension::Morale, morale_delta, parameters.max_abs_state_delta_per_segment);
        let actual_belief = state.add(MentalDimension::SelfBelief, belief_delta, parameters.max_abs_state_delta_per_segment);
        let motivation_delta = parameters.motivation_morale_coupling * actual_morale
            + parameters.motivation_belief_coupling * actual_belief;
        let actual_motivation = state.add(MentalDimension::Motivation, motivation_delta, parameters.max_abs_state_delta_per_segment);
        if actual_motivation != 0.0 {
            record_driver(drivers, *player, DynamicMentalTransitionDriver::ActiveLineupSocialState, 0.0, 0.0, actual_motivation, 0.0);
        }

        let reversion = (
            parameters.base_mean_reversion
                * (1.0 + parameters.professionalism_reversion_weight * unit_support(state.value(MentalDimension::Professionalism)))
        ).clamp(0.0, 1.0);
        let mut rm = 0.0;
        let mut rb = 0.0;
        let mut rmot = 0.0;
        for dimension in [MentalDimension::Morale, MentalDimension::SelfBelief, MentalDimension::Motivation] {
            let delta = reversion * (state.kickoff_value(dimension) - state.value(dimension));
            let actual = state.add(dimension, delta, parameters.max_abs_state_delta_per_segment);
            match dimension {
                MentalDimension::Morale => rm = actual,
                MentalDimension::SelfBelief => rb = actual,
                MentalDimension::Motivation => rmot = actual,
                _ => {}
            }
        }
        if rm != 0.0 || rb != 0.0 || rmot != 0.0 {
            record_driver(drivers, *player, DynamicMentalTransitionDriver::MeanReversion, rm, rb, rmot, 0.0);
        }

        let sigma = parameters.segment_innovation_sigma
            * (1.0 - parameters.consistency_volatility_damping * unit_support(state.value(MentalDimension::Consistency))).max(0.0);
        if sigma > 0.0 {
            let dm = state.add(MentalDimension::Morale, sigma * rng.normal(), parameters.max_abs_state_delta_per_segment);
            let db = state.add(MentalDimension::SelfBelief, sigma * rng.normal(), parameters.max_abs_state_delta_per_segment);
            let dmot = state.add(MentalDimension::Motivation, sigma * rng.normal(), parameters.max_abs_state_delta_per_segment);
            record_driver(drivers, *player, DynamicMentalTransitionDriver::Innovation, dm, db, dmot, 0.0);
        }
    }

    for (outgoing, incoming) in &events.substitutions {
        if let Some(state) = players.get_mut(outgoing) {
            let delta = state.add(
                MentalDimension::RoleSatisfaction,
                parameters.substituted_off_role_satisfaction,
                parameters.max_abs_state_delta_per_segment,
            );
            record_driver(drivers, *outgoing, DynamicMentalTransitionDriver::SubstitutedOff, 0.0, 0.0, 0.0, delta);
        }
        if let Some(state) = players.get_mut(incoming) {
            let delta = state.add(
                MentalDimension::Motivation,
                parameters.substituted_on_motivation,
                parameters.max_abs_state_delta_per_segment,
            );
            record_driver(drivers, *incoming, DynamicMentalTransitionDriver::SubstitutedOn, 0.0, 0.0, delta, 0.0);
        }
    }
    blend_team_with_active(team, active, players, parameters.active_lineup_blend);
}

fn personal_segment_signal(output: &SegmentOutput) -> BTreeMap<PlayerId, f64> {
    let mut sum = BTreeMap::<PlayerId, f64>::new();
    let mut count = BTreeMap::<PlayerId, u32>::new();
    for value in &output.metrics {
        *sum.entry(value.player).or_default() += value.deviation;
        *count.entry(value.player).or_default() += 1;
    }
    sum.into_iter().map(|(player, total)| {
        let n = f64::from(count.get(&player).copied().unwrap_or(1));
        (player, (total / n.sqrt()).tanh())
    }).collect()
}

fn blend_team_with_active(
    team: &mut PathTeamMental,
    active: &ActiveTeam,
    players: &BTreeMap<PlayerId, PathMentalPlayer>,
    blend: f64,
) {
    let ids = active.player_ids();
    if ids.is_empty() { return; }
    let targets = [
        (TeamMentalDimension::CollectiveMorale, active_average(players, &ids, MentalDimension::Morale)),
        (TeamMentalDimension::CollectiveBelief, active_average(players, &ids, MentalDimension::SelfBelief)),
        (TeamMentalDimension::Leadership, active_average(players, &ids, MentalDimension::Leadership)),
        (TeamMentalDimension::Cohesion, active_average(players, &ids, MentalDimension::Teamwork)),
    ];
    for (dimension, target) in targets {
        let current = team.value(dimension);
        team.current.insert(dimension, ((1.0 - blend) * current + blend * target).clamp(-1.0, 1.0));
    }
}

fn active_average(
    players: &BTreeMap<PlayerId, PathMentalPlayer>,
    ids: &[PlayerId],
    dimension: MentalDimension,
) -> f64 {
    if ids.is_empty() { return 0.0; }
    ids.iter().map(|player| players.get(player).map(|state| state.value(dimension)).unwrap_or(0.0)).sum::<f64>() / ids.len() as f64
}

fn evolve_active_team(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    mental_players: &BTreeMap<PlayerId, PathMentalPlayer>,
    active: &mut ActiveTeam,
    projection: &LineupProjectionInput,
    end_minute: f64,
    segment_minutes: f64,
    path_players: &mut BTreeMap<PlayerId, PathPlayer>,
    rng: &mut Rng64,
) -> Option<TeamTransitionEvents> {
    let incident_map: BTreeMap<PlayerId, SpecificPlayerIncidentProfile> = input.incident_profiles.iter().copied().map(|value| (value.player, value)).collect();
    let injury_base = segment_probability(input.in_match.injury_probability_per90, segment_minutes);
    let red_base = segment_probability(input.in_match.red_card_probability_per90, segment_minutes);
    let mut removals = BTreeSet::new();
    let mut replacements = Vec::new();
    let mut events = TeamTransitionEvents::default();
    for value in &active.players {
        let profile = incident_map.get(&value.sampled.player).copied().unwrap_or(SpecificPlayerIncidentProfile {
            player: value.sampled.player,
            injury_multiplier: 1.0,
            red_card_multiplier: 1.0,
        });
        let red_multiplier = dynamic_red_multiplier(mental_players.get(&value.sampled.player), mental_model);
        if rng.bernoulli((red_base * profile.red_card_multiplier * red_multiplier).clamp(0.0, 1.0)) {
            removals.insert(value.sampled.player);
            path_players.entry(value.sampled.player).or_default().red = true;
            events.red_cards.push(value.sampled.player);
            continue;
        }
        if rng.bernoulli((injury_base * profile.injury_multiplier).clamp(0.0, 1.0)) {
            removals.insert(value.sampled.player);
            path_players.entry(value.sampled.player).or_default().injury = true;
            events.injuries.push(value.sampled.player);
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
        let Some(replacement) = sample_replacement(
            projection, active, outgoing, end_minute, mental_players, mental_model, rng,
        ) else { continue; };
        events.substitutions.push((outgoing.player, replacement.sampled.player));
        active.used.insert(replacement.sampled.player);
        active.players.push(replacement);
        active.substitutions = active.substitutions.saturating_add(1);
    }
    Some(events)
}

fn sample_replacement(
    projection: &LineupProjectionInput,
    active: &ActiveTeam,
    outgoing: SampledStarter,
    entered_minute: f64,
    mental_players: &BTreeMap<PlayerId, PathMentalPlayer>,
    mental_model: &MentalMatchModel,
    rng: &mut Rng64,
) -> Option<ActivePlayer> {
    let candidates: Vec<&LineupCandidate> = projection.candidates.iter().filter(|value| {
        !active.used.contains(&value.player)
            && value.substitute_probability > 0.0
            && value.availability_probability > 0.0
    }).collect();
    if candidates.is_empty() { return None; }
    let weights = candidates.iter().map(|candidate| {
        // `projection` already contains the sampled kickoff mental effect. Only apply the recursive
        // change since kickoff here; reapplying the full state would double-count psychology.
        let within_match_delta = mental_players.get(&candidate.player).map(|state| {
            let mut delta = 0.0;
            for loading in &mental_model.selection_loadings {
                let current = state.value(loading.dimension);
                let kickoff = state.kickoff_value(loading.dimension);
                delta += loading.substitute_logit_weight
                    * (current - kickoff)
                    * context_scale(loading.dimension, mental_model.context);
            }
            delta.clamp(
                -mental_model.parameters.max_abs_selection_logit_adjustment,
                mental_model.parameters.max_abs_selection_logit_adjustment,
            )
        }).unwrap_or(0.0);
        adjust_probability(candidate.substitute_probability, within_match_delta)
            * candidate.availability_probability
            * (candidate.role_fit(outgoing.role) + 0.02).powf(projection.parameters.role_fit_power)
            * (-projection.parameters.fatigue_weight * candidate.fatigue).exp()
    });
    let selected = candidates[weighted_index(weights, rng)?];
    if !rng.bernoulli(selected.availability_probability) { return None; }
    Some(ActivePlayer {
        sampled: SampledStarter {
            player: selected.player,
            slot: outgoing.slot,
            role: outgoing.role,
            x: outgoing.x,
            y: outgoing.y,
            minutes: (90.0 - entered_minute).max(0.0),
            role_fit: selected.role_fit(outgoing.role),
            fatigue: selected.fatigue,
        },
        planned_exit: 90.0,
    })
}

fn dynamic_red_multiplier(state: Option<&PathMentalPlayer>, model: &MentalMatchModel) -> f64 {
    let mut log_risk = model.parameters.hostility_red_card_log_weight * model.context.hostility;
    if let Some(state) = state {
        log_risk += model.parameters.discipline_red_card_log_weight * state.value(MentalDimension::Discipline);
        log_risk += model.parameters.low_morale_red_card_log_weight * (-state.value(MentalDimension::Morale)).max(0.0);
    }
    log_risk.clamp(-2.0, 2.0).exp()
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
    let applicable: Vec<AttributeMatchupRule> = rules.iter().copied()
        .filter(|value| value.metric.is_none() || value.metric == Some(metric))
        .collect();
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
            let Some(attack) = attacker_state.value(rule.attacker_attribute) else { continue; };
            let Some(defend) = defender_state.value(rule.defender_attribute) else { continue; };
            pair += rule.weight * (attack - defend) / rule.scale;
            pair_weight += rule.weight.abs();
        }
        if pair_weight <= 0.0 { continue; }
        numerator += spatial * pair / pair_weight;
        denominator += spatial;
    }
    Some(if denominator > 0.0 { numerator / denominator } else { 0.0 })
}

fn own_attribute_adjustment(
    state: &PlayerAttributeState,
    metric: MetricId,
    values: &[AttributeEventLoading],
) -> f64 {
    values.iter().filter(|value| value.metric == metric).filter_map(|value| {
        state.value(value.attribute).map(|actual| value.log_rate_weight * (actual - value.reference) / value.scale)
    }).sum()
}

fn opponent_overlap(attacker: &SampledStarter, defender: &SampledStarter, sigma: f64) -> f64 {
    let dx = attacker.x - (1.0 - defender.x);
    let dy = attacker.y - (1.0 - defender.y);
    (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp()
}

fn team_production_multiplier(context: &SpecificTeamContext, metric: MetricId) -> f64 {
    context.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| value.production_multiplier).product()
}

fn team_suppression_multiplier(context: &SpecificTeamContext, metric: MetricId) -> f64 {
    context.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| value.opponent_suppression_multiplier).product()
}

fn environment_multiplier(values: &[SpecificEnvironmentMetricModifier], metric: MetricId, is_home: bool) -> f64 {
    values.iter().filter(|value| value.metric == metric).map(|value| if is_home { value.home_multiplier } else { value.away_multiplier }).product()
}

fn score_multiplier(metric: MetricId, score: ScoreState, values: &[SpecificScoreStateMetricModifier]) -> f64 {
    values.iter().filter(|value| value.metric == metric).map(|value| match score {
        ScoreState::Trailing => value.trailing_multiplier,
        ScoreState::Level => 1.0,
        ScoreState::Leading => value.leading_multiplier,
    }).product()
}

fn context_scale(dimension: MentalDimension, context: MatchMentalContext) -> f64 {
    match dimension {
        MentalDimension::PressureResilience => context.pressure_intensity,
        MentalDimension::Adaptability => context.environmental_novelty,
        MentalDimension::Discipline => 1.0 + context.hostility,
        _ => 1.0,
    }
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
    for (channel, amount) in &output.channels {
        *team_channels.entry((club, *channel)).or_default() += *amount;
    }
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
        player.mental_mean += value.mental_mean;
        player.mental_variance += value.mental_variance;
        if absolute_deviation > 0.0 {
            let share = value.deviation.abs() / absolute_deviation;
            for (channel, amount) in &output.channels {
                *player.channels.entry(*channel).or_default() += share * *amount;
            }
        }
    }
}

fn accumulate_lineup(lineup: &SampledLineup, aggregate: &mut LineupAggregate) {
    *aggregate.formation_counts.entry(lineup.formation_id).or_default() += 1;
    for starter in &lineup.starters {
        *aggregate.starts.entry(starter.player).or_default() += 1;
        *aggregate.minutes.entry(starter.player).or_default() += starter.minutes;
        *aggregate.roles.entry((starter.player, starter.role)).or_default() += 1;
    }
}

fn lineup_projection_from_aggregate(
    input: &LineupProjectionInput,
    aggregate: &LineupAggregate,
    paths: u32,
) -> LineupProjection {
    let denominator = paths as f32;
    let formations = input.formations.iter().map(|value| FormationProjection {
        formation_id: value.formation.id,
        probability: Probability::clamped(
            aggregate.formation_counts.get(&value.formation.id).copied().unwrap_or(0) as f32 / denominator,
        ),
    }).collect();
    let players = input.candidates.iter().map(|candidate| {
        let starts = aggregate.starts.get(&candidate.player).copied().unwrap_or(0);
        let minutes = aggregate.minutes.get(&candidate.player).copied().unwrap_or(0.0);
        let roles = if starts > 0 {
            candidate.role_fits.iter().filter_map(|fit| {
                let count = aggregate.roles.get(&(candidate.player, fit.role)).copied().unwrap_or(0);
                (count > 0).then_some(RoleProjection {
                    role: fit.role,
                    probability_given_start: Probability::clamped(count as f32 / starts as f32),
                })
            }).collect()
        } else {
            Vec::new()
        };
        PlayerLineupProjection {
            player: candidate.player,
            start_probability: Probability::clamped(starts as f32 / denominator),
            expected_minutes_unconditional: minutes / f64::from(paths),
            expected_minutes_if_start: if starts > 0 { minutes / f64::from(starts) } else { 0.0 },
            roles,
        }
    }).collect();
    LineupProjection { club: input.club, formations, players, paths }
}

fn accumulate_team_mental(team: &PathTeamMental, aggregates: &mut BTreeMap<ClubId, MentalTeamAggregate>) {
    let value = aggregates.entry(team.club).or_default();
    value.observations = value.observations.saturating_add(1);
    value.initial_morale += team.kickoff_value(TeamMentalDimension::CollectiveMorale);
    value.final_morale += team.value(TeamMentalDimension::CollectiveMorale);
    value.initial_belief += team.kickoff_value(TeamMentalDimension::CollectiveBelief);
    value.final_belief += team.value(TeamMentalDimension::CollectiveBelief);
    value.final_cohesion += team.value(TeamMentalDimension::Cohesion);
    value.final_leadership += team.value(TeamMentalDimension::Leadership);
}

fn record_driver(
    drivers: &mut BTreeMap<(PlayerId, DynamicMentalTransitionDriver), DriverAggregate>,
    player: PlayerId,
    driver: DynamicMentalTransitionDriver,
    morale: f64,
    belief: f64,
    motivation: f64,
    role: f64,
) {
    let value = drivers.entry((player, driver)).or_default();
    value.occurrences = value.occurrences.saturating_add(1);
    value.morale += morale;
    value.belief += belief;
    value.motivation += motivation;
    value.role += role;
}

fn unit_support(value: f64) -> f64 { ((value + 1.0) * 0.5).clamp(0.0, 1.0) }

fn adjust_probability(probability: f64, logit_delta: f64) -> f64 {
    if probability <= 0.0 { return 0.0; }
    if probability >= 1.0 { return 1.0; }
    let logit = (probability / (1.0 - probability)).ln() + logit_delta;
    1.0 / (1.0 + (-logit).exp())
}

fn segment_probability(probability_per90: f64, minutes: f64) -> f64 {
    1.0 - (1.0 - probability_per90.clamp(0.0, 1.0)).powf(minutes / 90.0)
}

fn weighted_index<I>(weights: I, rng: &mut Rng64) -> Option<usize>
where
    I: IntoIterator<Item = f64>,
{
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
    } else {
        0.0
    };
    Some(DistributionSummary {
        p10: quantile(&sorted, 0.10),
        p25: quantile(&sorted, 0.25),
        p50: quantile(&sorted, 0.50),
        p75: quantile(&sorted, 0.75),
        p90: quantile(&sorted, 0.90),
        mean,
        standard_error: (variance / sorted.len() as f64).sqrt(),
        paths: sorted.len().min(u32::MAX as usize) as u32,
    })
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    if sorted.len() == 1 { return sorted[0]; }
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let fraction = position - lower as f64;
        sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{CausalChannelGraph, ChannelEdge, GoalChannelLoading, MetricChannelLoading};
    use crate::lineup::{FormationPrior, FormationTemplate, LineupSamplingParameters, RoleFit, TacticalSlot};
    use crate::matchup::{AbilityDimension, MatchupRule, PlayerMatchupState};
    use crate::psychology::{
        MentalMeasure, MentalMetricLoading, MentalModelParameters, MentalSelectionLoading,
        StablePsychologyState,
    };
    use crate::specific::{
        PlayerAttributeValue, SpecificGoalProcess, SpecificInMatchParameters, SpecificMatchEnvironment,
        SpecificPlayerEventModel,
    };
    use footy_core::{AttributeId, Confidence, MatchId};
    use footy_state::AbilityVector;

    fn formation() -> FormationTemplate {
        FormationTemplate {
            id: 433,
            slots: (0_u8..11).map(|slot| TacticalSlot {
                slot,
                role: RoleId(u16::from(slot) + 1),
                x: 0.05 + f64::from(slot) * 0.075,
                y: f64::from(slot % 5) / 4.0,
            }).collect(),
        }
    }

    fn lineup(club: u32, offset: u64) -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(club),
            formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (0_u64..16).map(|index| LineupCandidate {
                player: PlayerId(offset + index),
                start_probability: if index < 11 { 0.86 } else { 0.15 },
                availability_probability: 1.0,
                ability_index: 12.0,
                fatigue: 0.0,
                expected_minutes_if_start: if index < 3 { 64.0 } else { 88.0 },
                minutes_sigma: 2.0,
                substitute_probability: if index >= 11 { 0.8 } else { 0.05 },
                role_fits: (1_u16..=11).map(|role| RoleFit {
                    role: RoleId(role),
                    fit: if u64::from(role) == index + 1 { 1.0 } else { 0.25 },
                }).collect(),
            }).collect(),
            locked_starters: Vec::new(),
            parameters: LineupSamplingParameters::default(),
        }
    }

    fn measure(value: f64) -> MentalMeasure {
        MentalMeasure::new(value, Confidence::ONE).unwrap()
    }

    fn player_mental(player: PlayerId) -> PlayerMentalState {
        PlayerMentalState {
            player,
            stable: StablePsychologyState {
                professionalism: measure(0.4),
                ambition: measure(0.4),
                consistency: measure(0.5),
                pressure_resilience: measure(0.3),
                adaptability: measure(0.2),
                leadership: measure(0.2),
                teamwork: measure(0.3),
                discipline: measure(0.4),
            },
            morale: measure(0.0),
            self_belief: measure(0.0),
            form: measure(0.0),
            form_trend: measure(0.0),
            motivation: measure(0.0),
            role_satisfaction: measure(0.0),
        }
    }

    fn setup(include_all_mental_states: bool) -> (PrimitiveMatchSimulationInput, MentalMatchModel) {
        let home = lineup(1, 1);
        let away = lineup(2, 101);
        let all: Vec<PlayerId> = home.candidates.iter().chain(away.candidates.iter()).map(|value| value.player).collect();
        let metric = MetricId(1);
        let pace = footy_core::AttributeId(1);
        let recovery = footy_core::AttributeId(2);
        let input = PrimitiveMatchSimulationInput {
            match_id: MatchId(9),
            home: ClubId(1),
            away: ClubId(2),
            home_lineup: home,
            away_lineup: away,
            player_states: all.iter().map(|player| PlayerMatchupState {
                player: *player,
                ability: AbilityVector::from_components([12.0; 10]),
            }).collect(),
            attribute_states: all.iter().map(|player| PlayerAttributeState {
                player: *player,
                attributes: vec![
                    PlayerAttributeValue { attribute: pace, value: 12.0 },
                    PlayerAttributeValue { attribute: recovery, value: 12.0 },
                ],
            }).collect(),
            event_models: all.iter().map(|player| SpecificPlayerEventModel {
                player: *player,
                metric,
                family: StatFamily::ContinuousNonNegative,
                mean_per90: 1.0,
                variance_per90: 0.03,
                epistemic_sigma: 0.02,
                reference_per90: 1.0,
                aggregate_matchup_sensitivity: 0.05,
                attribute_matchup_sensitivity: 0.05,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.1,
                fatigue_sensitivity: 0.1,
            }).collect(),
            own_attribute_loadings: vec![AttributeEventLoading {
                metric, attribute: pace, reference: 12.0, scale: 20.0, log_rate_weight: 0.1,
            }],
            aggregate_matchup_rules: vec![MatchupRule {
                metric: Some(metric),
                attacker_dimension: AbilityDimension::BallProgression,
                defender_dimension: AbilityDimension::Defending,
                weight: 1.0,
                scale: 20.0,
            }],
            attribute_matchup_rules: vec![AttributeMatchupRule {
                metric: Some(metric),
                attacker_attribute: pace,
                defender_attribute: recovery,
                weight: 1.0,
                scale: 20.0,
            }],
            synergies: Vec::new(),
            channel_graph: CausalChannelGraph {
                metric_loadings: vec![MetricChannelLoading {
                    metric, channel: TeamProcessChannel::Progression, weight: 1.0,
                }],
                edges: vec![ChannelEdge {
                    from: TeamProcessChannel::Progression,
                    to: TeamProcessChannel::ChanceCreation,
                    weight: 0.6,
                }],
                goal_loadings: vec![GoalChannelLoading {
                    channel: TeamProcessChannel::ChanceCreation,
                    attack_log_weight: 0.08,
                    defense_log_weight: 0.0,
                }],
            },
            score_state_modifiers: vec![SpecificScoreStateMetricModifier {
                metric, trailing_multiplier: 1.05, leading_multiplier: 0.97,
            }],
            incident_profiles: Vec::new(),
            in_match: SpecificInMatchParameters {
                injury_probability_per90: 0.01,
                red_card_probability_per90: 0.01,
                max_substitutions: 5,
                minimum_tactical_substitution_minute: 45.0,
            },
            home_context: SpecificTeamContext {
                club: ClubId(1), residual_attack_log_adjustment: 0.0,
                residual_defense_log_adjustment: 0.0, metric_modifiers: Vec::new(),
            },
            away_context: SpecificTeamContext {
                club: ClubId(2), residual_attack_log_adjustment: 0.0,
                residual_defense_log_adjustment: 0.0, metric_modifiers: Vec::new(),
            },
            environment: crate::specific::SpecificMatchEnvironment {
                home_residual_attack_log_adjustment: 0.0,
                away_residual_attack_log_adjustment: 0.0,
                metric_modifiers: vec![SpecificEnvironmentMetricModifier {
                    metric, home_multiplier: 1.0, away_multiplier: 1.0,
                }],
            },
            goal_process: SpecificGoalProcess {
                goals_per_team_match: 1.4,
                home_advantage_multiplier: 1.08,
                path_log_rate_sigma: 0.05,
                max_abs_segment_log_adjustment: 1.5,
                segments: 6,
            },
            matchup_spatial_sigma: 0.25,
            max_player_rate_log_adjustment: 1.0,
        };
        let team = |club| TeamMentalState {
            club,
            collective_morale: measure(0.0),
            collective_belief: measure(0.0),
            cohesion: measure(0.2),
            leadership: measure(0.2),
        };
        let mental_players: Vec<PlayerMentalState> = all.iter().enumerate().filter_map(|(index, player)| {
            (include_all_mental_states || index % 3 != 0).then_some(player_mental(*player))
        }).collect();
        let mental = MentalMatchModel {
            players: mental_players,
            home_team: team(ClubId(1)),
            away_team: team(ClubId(2)),
            metric_loadings: vec![MentalMetricLoading {
                metric,
                dimension: MentalDimension::SelfBelief,
                mean_log_weight: 0.10,
                log_variance_weight: 0.0,
            }],
            team_metric_loadings: Vec::new(),
            selection_loadings: vec![MentalSelectionLoading {
                dimension: MentalDimension::Motivation,
                start_logit_weight: 0.10,
                substitute_logit_weight: 0.10,
            }],
            context: MatchMentalContext {
                pressure_intensity: 0.8,
                environmental_novelty: 0.2,
                hostility: 0.2,
            },
            parameters: MentalModelParameters::default(),
        };
        (input, mental)
    }

    #[test]
    fn exact_n_full_match_paths_are_reproducible() {
        let (input, mental) = setup(true);
        let first = simulate_dynamic_mental_match(
            &input, &mental, DynamicMentalTransitionParameters::default(), 250, 42,
        ).unwrap();
        let second = simulate_dynamic_mental_match(
            &input, &mental, DynamicMentalTransitionParameters::default(), 250, 42,
        ).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.paths, 250);
        assert_eq!(first.forecast.paths, 250);
        assert_eq!(first.forecast.outcome.paths, 250);
        assert_eq!(first.forecast.outcome.home_goals.paths, 250);
        assert_eq!(first.forecast.outcome.away_goals.paths, 250);
    }

    #[test]
    fn in_match_events_recursively_move_next_segment_mental_state() {
        let (input, mental) = setup(true);
        let result = simulate_dynamic_mental_match(
            &input, &mental, DynamicMentalTransitionParameters::default(), 400, 7,
        ).unwrap();
        assert!(result.players.iter().any(|value| value.mean_abs_morale_path_change > 0.0));
        assert!(!result.transition_drivers.is_empty());
        assert!(result.players.iter().any(|value| value.mean_event_mental_log_adjustment.abs() > 0.0));
    }

    #[test]
    fn missing_player_psychology_is_latent_uncertainty_not_fake_neutral_truth() {
        let (input, mental) = setup(false);
        let result = simulate_dynamic_mental_match(
            &input, &mental, DynamicMentalTransitionParameters::default(), 300, 19,
        ).unwrap();
        assert_eq!(result.players.len(), input.home_lineup.candidates.len() + input.away_lineup.candidates.len());
        assert!(result.players.iter().any(|value| value.initial_morale_mean.abs() < 0.10 && value.mean_abs_morale_path_change > 0.0));
    }

    #[test]
    fn probability_boundaries_are_exact_under_mental_adjustment() {
        assert_eq!(adjust_probability(0.0, 10.0), 0.0);
        assert_eq!(adjust_probability(1.0, -10.0), 1.0);
    }

    #[test]
    fn zero_transition_coefficients_hold_transient_state_when_lineup_blend_is_disabled() {
        let (input, mental) = setup(true);
        let mut transition = DynamicMentalTransitionParameters::default();
        transition.goal_for_morale = 0.0;
        transition.goal_against_morale = 0.0;
        transition.goal_for_belief = 0.0;
        transition.goal_against_belief = 0.0;
        transition.teammate_red_morale = 0.0;
        transition.teammate_red_belief = 0.0;
        transition.teammate_injury_morale = 0.0;
        transition.own_red_morale = 0.0;
        transition.own_injury_morale = 0.0;
        transition.personal_success_self_belief = 0.0;
        transition.personal_error_self_belief = 0.0;
        transition.personal_success_morale = 0.0;
        transition.personal_error_morale = 0.0;
        transition.substituted_off_role_satisfaction = 0.0;
        transition.substituted_on_motivation = 0.0;
        transition.trailing_pressure_belief = 0.0;
        transition.leading_belief_boost = 0.0;
        transition.base_mean_reversion = 0.0;
        transition.segment_innovation_sigma = 0.0;
        transition.active_lineup_blend = 0.0;
        let result = simulate_dynamic_mental_match(&input, &mental, transition, 100, 9).unwrap();
        assert!(result.players.iter().all(|value| {
            (value.final_morale_mean - value.initial_morale_mean).abs() < 1e-9
                && (value.final_self_belief_mean - value.initial_self_belief_mean).abs() < 1e-9
        }));
    }
}
