use crate::lineup::{
    sample_lineup, FormationProjection, LineupCandidate, LineupProjection, LineupProjectionInput,
    PlayerLineupProjection, RoleProjection, SampledLineup, SampledStarter,
};
use crate::matchup::player_metric_interaction_context;
use crate::psychology::{
    MatchMentalContext, MentalDimension, MentalMatchModel, PlayerMentalState, TeamMentalDimension,
    TeamMentalState,
};
use crate::second_clock::{EventTime, MatchSecond, SecondClockConfig, REGULATION_MATCH_SECONDS};
use crate::specific::{
    AttributeEventLoading, AttributeMatchupRule, PlayerAttributeState, PrimitiveMatchSimulationInput,
    PrimitivePlayerMetricForecast, SpecificEnvironmentMetricModifier, SpecificPlayerIncidentProfile,
    SpecificScoreStateMetricModifier, SpecificTeamContext,
};
use crate::DynamicMentalTransitionParameters;
use footy_core::{ClubId, MetricId, PlayerId, Probability, RoleId};
use footy_forecast::{DistributionSummary, MatchOutcomeForecast, Rng64, StatFamily};
use std::collections::{BTreeMap, BTreeSet};

pub const DYNAMIC_MENTAL_REFERENCE_SECONDS: f64 = 15.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondResolvedSimulationParameters {
    pub clock: SecondClockConfig,
    /// Hard cap on state-changing events in one integer second. This is only a safety guard against
    /// pathological calibration; normal football hazards are many orders below this bound.
    pub max_state_events_per_second: u16,
    /// P(recorded primary assist | attributed non-own goal).
    /// The 0.68 default is a **research prior**, not a calibrated EPL claim.
    /// Requires dated matching event labels and separate OOS validation before
    /// any changed value is approved for published predictions.
    pub assisted_goal_probability: f64,
}

impl Default for SecondResolvedSimulationParameters {
    fn default() -> Self {
        Self {
            clock: SecondClockConfig::default(),
            max_state_events_per_second: 64,
            assisted_goal_probability: 0.68,
        }
    }
}

impl SecondResolvedSimulationParameters {
    pub fn validate(self) -> bool {
        self.clock.validate()
            && self.max_state_events_per_second > 0
            && self.assisted_goal_probability.is_finite()
            && (0.0..=1.0).contains(&self.assisted_goal_probability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecondResolvedStateEventKind {
    HomeGoal,
    AwayGoal,
    RedCard,
    Injury,
    TacticalSubstitution,
    ForcedSubstitution,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondResolvedEventSummary {
    pub kind: SecondResolvedStateEventKind,
    pub mean_per_path: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondResolvedPlayerMentalForecast {
    pub player: PlayerId,
    pub initial_morale_mean: f64,
    pub final_morale_mean: f64,
    pub initial_self_belief_mean: f64,
    pub final_self_belief_mean: f64,
    pub initial_motivation_mean: f64,
    pub final_motivation_mean: f64,
    pub mean_abs_morale_change: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondResolvedDynamicMentalForecast {
    pub outcome: MatchOutcomeForecast,
    pub home_lineup: LineupProjection,
    pub away_lineup: LineupProjection,
    pub player_metrics: Vec<PrimitivePlayerMetricForecast>,
    pub player_mental: Vec<SecondResolvedPlayerMentalForecast>,
    pub state_events: Vec<SecondResolvedEventSummary>,
    pub same_second_collision_probability: Probability,
    pub mean_state_events_per_path: f64,
    pub simulated_match_seconds: u32,
    pub paths: u32,
    /// Path-observed event probabilities in actual six 15-minute clock windows.
    pub windows_15min: Vec<SecondResolvedWindowForecast>,
    /// Ninety path-observed, non-overlapping regulation-minute bins.
    pub minutes_1min: Vec<SecondResolvedWindowForecast>,
    /// Sparse player-level minute aggregates, divided by the number of full paths.
    pub player_minute_events: Vec<SecondResolvedPlayerMinuteForecast>,
    /// Expected goals that could not be assigned to a currently active player.
    pub unattributed_goals_per_match: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondResolvedWindowForecast {
    pub start_minute: u16,
    pub end_minute: u16,
    pub probability_any_goal: f64,
    pub probability_home_goal: f64,
    pub probability_away_goal: f64,
    pub probability_red_card: f64,
    pub probability_substitution: f64,
    pub expected_home_goals: f64,
    pub expected_away_goals: f64,
    pub expected_home_shots: f64,
    pub expected_away_shots: f64,
    pub end_home_lead: f64,
    pub end_draw: f64,
    pub end_away_lead: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondResolvedPlayerMinuteForecast {
    pub minute: u16,
    pub player: PlayerId,
    pub expected_shots: f64,
    pub expected_goals: f64,
    /// Heuristic sampled assist attribution, not a fitted assist model.
    pub expected_assists_proxy: f64,
}

#[derive(Debug, Clone, Default)]
struct PlayerMinuteAggregate { shots: f64, goals: u32, assists: u32 }

#[derive(Debug, Clone, Default)]
struct PathWindow {
    home_goals: u32, away_goals: u32, red_cards: u32, substitutions: u32,
    home_shots: f64, away_shots: f64,
    cumulative_home: u32, cumulative_away: u32,
}

#[derive(Debug, Clone, Default)]
struct WindowAggregate {
    any_goal: u32, home_goal: u32, away_goal: u32, red_card: u32, substitution: u32,
    home_goals: u64, away_goals: u64, home_shots: f64, away_shots: f64,
    home_lead: u32, draw: u32, away_lead: u32,
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

    fn add(&mut self, dimension: MentalDimension, delta: f64, cap: f64) -> f64 {
        let before = self.value(dimension);
        let after = (before + delta.clamp(-cap, cap)).clamp(-1.0, 1.0);
        self.current.insert(dimension, after);
        after - before
    }
}

#[derive(Debug, Clone)]
struct PathTeamMental {
    current: BTreeMap<TeamMentalDimension, f64>,
}

impl PathTeamMental {
    fn value(&self, dimension: TeamMentalDimension) -> f64 {
        self.current.get(&dimension).copied().unwrap_or(0.0)
    }

    fn add(&mut self, dimension: TeamMentalDimension, delta: f64, cap: f64) {
        self.current.insert(
            dimension,
            (self.value(dimension) + delta.clamp(-cap, cap)).clamp(-1.0, 1.0),
        );
    }
}

#[derive(Debug, Clone)]
struct ActivePlayer {
    sampled: SampledStarter,
    planned_exit_second: MatchSecond,
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
        let players = lineup.starters.into_iter().map(|sampled| ActivePlayer {
            planned_exit_second: MatchSecond(
                (sampled.minutes * 60.0).round().clamp(1.0, f64::from(REGULATION_MATCH_SECONDS)) as u32,
            ),
            sampled,
        }).collect();
        Self { club: lineup.club, formation_id: lineup.formation_id, players, used, substitutions: 0 }
    }

    fn lineup(&self) -> SampledLineup {
        SampledLineup {
            club: self.club,
            formation_id: self.formation_id,
            starters: self.players.iter().map(|value| value.sampled).collect(),
        }
    }

    fn active_ids(&self) -> Vec<PlayerId> {
        self.players.iter().map(|value| value.sampled.player).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreState { Trailing, Level, Leading }

#[derive(Debug, Clone)]
enum StateEvent {
    HomeGoal,
    AwayGoal,
    Red { club: ClubId, player: PlayerId },
    Injury { club: ClubId, player: PlayerId },
    TacticalSub { club: ClubId, player: PlayerId },
}

#[derive(Debug, Clone)]
struct TimedStateEvent {
    time: EventTime,
    fraction: f64,
    event: StateEvent,
}

#[derive(Debug, Clone, Copy)]
struct PlayerRate {
    adjusted_per90: f64,
    variance_per90: f64,
    family: StatFamily,
    reference_per90: f64,
}

#[derive(Debug, Clone)]
struct RateSnapshot {
    home_goal_intensity_per_second: f64,
    away_goal_intensity_per_second: f64,
    player_rates: BTreeMap<(PlayerId, MetricId), PlayerRate>,
}

#[derive(Default)]
struct PathMetric { total: f64, seconds: f64 }

#[derive(Default)]
struct AggregateMetric {
    unconditional: Vec<f64>,
    conditional: Vec<f64>,
    conditional_per90: Vec<f64>,
}

#[derive(Default)]
struct ParticipationAggregate {
    appearances: u32,
    seconds: f64,
}

#[derive(Default)]
struct MentalAggregate {
    observations: u32,
    initial_morale: f64,
    final_morale: f64,
    initial_belief: f64,
    final_belief: f64,
    initial_motivation: f64,
    final_motivation: f64,
    abs_morale: f64,
}

#[derive(Default)]
struct LineupAggregate {
    formations: BTreeMap<u16, u32>,
    starts: BTreeMap<PlayerId, u32>,
    starter_minutes: BTreeMap<PlayerId, f64>,
    roles: BTreeMap<(PlayerId, RoleId), u32>,
}

/// Second-resolution specific-match Monte Carlo. `paths=n` means exactly n independent complete
/// match histories. Every path owns its kickoff XI, latent player rates, mental state, event clock,
/// score, incidents, substitutions, and final result.
///
/// Within an integer match second the engine uses a continuous competing-hazard clock. After each
/// state-changing event, all remaining hazards for that second are discarded and recomputed from
/// the new score/lineup/mental state. This resolves goal/card/injury/substitution ordering without
/// pretending source data observed true sub-second timestamps.
pub fn simulate_second_resolved_dynamic_mental_match(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    transition: DynamicMentalTransitionParameters,
    parameters: SecondResolvedSimulationParameters,
    paths: u32,
    seed: u64,
) -> Option<SecondResolvedDynamicMentalForecast> {
    if paths == 0 || !input.validate() || !mental_model.validate(input)
        || !transition.validate() || !parameters.validate()
    { return None; }

    let end = parameters.clock.match_end();
    let source_mental: BTreeMap<PlayerId, PlayerMentalState> = mental_model.players.iter().copied()
        .map(|value| (value.player, value)).collect();
    let home_squad: Vec<PlayerId> = input.home_lineup.candidates.iter().map(|value| value.player).collect();
    let away_squad: Vec<PlayerId> = input.away_lineup.candidates.iter().map(|value| value.player).collect();
    if home_squad.iter().any(|player| away_squad.contains(player)) { return None; }
    let all_squad: Vec<PlayerId> = home_squad.iter().chain(away_squad.iter()).copied().collect();

    let mut rng = Rng64::new(seed);
    let mut home_goals = Vec::with_capacity(paths as usize);
    let mut away_goals = Vec::with_capacity(paths as usize);
    let mut home_wins = 0_u32;
    let mut draws = 0_u32;
    let mut away_wins = 0_u32;
    let mut metric_aggregate = BTreeMap::<(PlayerId, MetricId), AggregateMetric>::new();
    let mut participation = BTreeMap::<PlayerId, ParticipationAggregate>::new();
    let mut mental_aggregate = BTreeMap::<PlayerId, MentalAggregate>::new();
    let mut event_counts = BTreeMap::<SecondResolvedStateEventKind, u64>::new();
    let mut total_state_events = 0_u64;
    let mut collision_paths = 0_u32;
    let mut windows = [WindowAggregate::default(), WindowAggregate::default(), WindowAggregate::default(),
        WindowAggregate::default(), WindowAggregate::default(), WindowAggregate::default()];
    let minute_len = (end.0 as usize).div_ceil(60);
    let mut minute_windows = vec![WindowAggregate::default(); minute_len];
    let mut player_minute_aggregate = BTreeMap::<(u16, PlayerId), PlayerMinuteAggregate>::new();
    let mut unattributed_goals = 0_u32;
    let home_player_ids: BTreeSet<PlayerId> = home_squad.iter().copied().collect();
    let mut home_lineups = LineupAggregate::default();
    let mut away_lineups = LineupAggregate::default();
    for model in &input.event_models { metric_aggregate.entry((model.player, model.metric)).or_default(); }

    for _ in 0..paths {
        let mut mental_players = BTreeMap::<PlayerId, PathMentalPlayer>::new();
        for player in &all_squad {
            mental_players.insert(
                *player,
                source_mental.get(player).copied()
                    .map(|state| sample_player_mental(state, transition, &mut rng))
                    .unwrap_or_else(|| sample_unknown_mental(transition, &mut rng)),
            );
        }
        let mut home_projection = input.home_lineup.clone();
        let mut away_projection = input.away_lineup.clone();
        adjust_lineup_for_path_mental(&mut home_projection, &mental_players, mental_model);
        adjust_lineup_for_path_mental(&mut away_projection, &mental_players, mental_model);
        let home_start = sample_lineup(&home_projection, &mut rng)?;
        let away_start = sample_lineup(&away_projection, &mut rng)?;
        accumulate_starting_lineup(&home_start, &mut home_lineups);
        accumulate_starting_lineup(&away_start, &mut away_lineups);
        let mut home = ActiveTeam::from_start(home_start);
        let mut away = ActiveTeam::from_start(away_start);
        let mut home_mental = sample_team_mental(mental_model.home_team, transition, &mut rng);
        let mut away_mental = sample_team_mental(mental_model.away_team, transition, &mut rng);
        blend_team_with_active(&mut home_mental, &home, &mental_players, transition.active_lineup_blend);
        blend_team_with_active(&mut away_mental, &away, &mental_players, transition.active_lineup_blend);

        // One epistemic draw per player/metric/path, never once per second.
        let mut latent_rate = BTreeMap::<(PlayerId, MetricId), f64>::new();
        for model in &input.event_models {
            latent_rate.insert(
                (model.player, model.metric),
                sample_non_negative_normal(model.mean_per90, model.epistemic_sigma, &mut rng),
            );
        }
        // Match-level goal-rate uncertainty is likewise one draw per team/path.
        let home_goal_base = sample_log_rate(
            input.goal_process.goals_per_team_match * input.goal_process.home_advantage_multiplier,
            input.goal_process.path_log_rate_sigma,
            &mut rng,
        );
        let away_goal_base = sample_log_rate(
            input.goal_process.goals_per_team_match,
            input.goal_process.path_log_rate_sigma,
            &mut rng,
        );

        let mut path_metrics = BTreeMap::<(PlayerId, MetricId), PathMetric>::new();
        let mut score_home = 0_u32;
        let mut score_away = 0_u32;
        let mut path_windows = [PathWindow::default(), PathWindow::default(), PathWindow::default(),
            PathWindow::default(), PathWindow::default(), PathWindow::default()];
        let mut path_minutes = vec![PathWindow::default(); minute_len];
        let mut path_collision = false;
        let mut second = MatchSecond::KICKOFF;

        while second < end {
            // Scheduled substitutions receive their latent sub-second order once. A stochastic event
            // earlier in the same second does not redraw their time; it may only invalidate them.
            let mut scheduled = scheduled_substitutions_for_second(
                second, &home, &away, input, &mut rng,
            );
            scheduled.sort_by(|left, right| {
                left.fraction.total_cmp(&right.fraction)
                    .then(left.time.within_second.cmp(&right.time.within_second))
            });
            let mut cursor = 0.0_f64;
            let mut events_this_second = 0_u16;

            loop {
                if cursor >= 1.0 - 1e-12 { break; }
                if events_this_second >= parameters.max_state_events_per_second { return None; }
                let snapshot = build_rate_snapshot(
                    input, mental_model, &mental_players, &home_mental, &away_mental,
                    &home, &away, &latent_rate, score_home, score_away,
                    home_goal_base, away_goal_base,
                )?;
                let scheduled_next = scheduled.iter().find(|value| value.fraction >= cursor - 1e-12)
                    .cloned();
                let stochastic = sample_stochastic_event(
                    input, mental_model, &mental_players, &home, &away, &snapshot,
                    second, cursor, &mut rng,
                )?;
                let next = earliest_event(scheduled_next, stochastic);
                let end_fraction = next.as_ref().map(|value| value.fraction).unwrap_or(1.0).clamp(cursor, 1.0);
                let elapsed = end_fraction - cursor;
                if elapsed > 0.0 {
                    let minute_index = (second.0 as usize / 60).min(minute_len - 1);
                    let personal = advance_player_metrics(&snapshot, elapsed, &mut path_metrics,
                        &mut rng, &mut path_windows[(second.0 as usize / 900).min(5)],
                        &mut path_minutes[minute_index], minute_index as u16,
                        &mut player_minute_aggregate, &home_player_ids);
                    apply_personal_feedback(&home, &personal, &mut mental_players, transition, elapsed);
                    apply_personal_feedback(&away, &personal, &mut mental_players, transition, elapsed);
                    advance_continuous_mental(
                        &home_squad, &home, &mut home_mental, &mut mental_players,
                        score_state(score_home, score_away), mental_model.context, transition, elapsed, &mut rng,
                    );
                    advance_continuous_mental(
                        &away_squad, &away, &mut away_mental, &mut mental_players,
                        score_state(score_away, score_home), mental_model.context, transition, elapsed, &mut rng,
                    );
                }
                cursor = end_fraction;
                let Some(event) = next else { break; };
                // Remove this scheduled event by its stable time before state mutation/recomputation.
                scheduled.retain(|value| !(value.time == event.time && value.fraction == event.fraction));
                events_this_second = events_this_second.saturating_add(1);
                if events_this_second >= 2 { path_collision = true; }
                total_state_events = total_state_events.saturating_add(1);
                let minute_index = (second.0 as usize / 60).min(minute_len - 1);
                let window = &mut path_windows[(second.0 as usize / 900).min(5)];
                let minute_window = &mut path_minutes[minute_index];
                match &event.event {
                    StateEvent::HomeGoal | StateEvent::AwayGoal => {
                        let home_goal = matches!(event.event, StateEvent::HomeGoal);
                        if home_goal { window.home_goals += 1; minute_window.home_goals += 1; }
                        else { window.away_goals += 1; minute_window.away_goals += 1; }
                        let active = if home_goal { &home } else { &away };
                        if let Some((scorer, assist)) = attribute_goal(
                            active, &snapshot, parameters.assisted_goal_probability, &mut rng
                        ) {
                            // A modeled non-own goal entails a shot. The old goal
                            // process was separate from sampled player shots;
                            // record the event's shot everywhere, including the
                            // team/individual metric marginals, so the exported
                            // shot counts cannot omit a scored shot.
                            if home_goal {
                                window.home_shots += 1.0;
                                minute_window.home_shots += 1.0;
                            } else {
                                window.away_shots += 1.0;
                                minute_window.away_shots += 1.0;
                            }
                            path_metrics.entry((scorer,MetricId(1))).or_default().total += 1.0;
                            let attribution=player_minute_aggregate.entry((minute_index as u16, scorer)).or_default();
                            attribution.goals += 1;
                            attribution.shots += 1.0;
                            if let Some(assister) = assist {
                                player_minute_aggregate.entry((minute_index as u16, assister)).or_default().assists += 1;
                            }
                        } else {unattributed_goals += 1;}
                    }
                    StateEvent::Red { .. } => { window.red_cards += 1; minute_window.red_cards += 1; }
                    StateEvent::Injury { .. } | StateEvent::TacticalSub { .. } => {
                        window.substitutions += 1; minute_window.substitutions += 1;
                    }
                }
                apply_state_event(
                    event.event, input, mental_model, transition,
                    &home_squad, &away_squad,
                    &mut home, &mut away, &home_projection, &away_projection,
                    &mut home_mental, &mut away_mental, &mut mental_players,
                    &mut score_home, &mut score_away, &mut event_counts, &mut rng,
                )?;
            }
            second = MatchSecond(second.0.saturating_add(1));
        }

        accumulate_path_windows(&mut windows, &mut path_windows);
        accumulate_path_windows(&mut minute_windows, &mut path_minutes);
        if path_collision { collision_paths = collision_paths.saturating_add(1); }
        home_goals.push(f64::from(score_home));
        away_goals.push(f64::from(score_away));
        match score_home.cmp(&score_away) {
            std::cmp::Ordering::Greater => home_wins = home_wins.saturating_add(1),
            std::cmp::Ordering::Equal => draws = draws.saturating_add(1),
            std::cmp::Ordering::Less => away_wins = away_wins.saturating_add(1),
        }
        let mut path_player_seconds = BTreeMap::<PlayerId, f64>::new();
        for ((player, metric), aggregate) in &mut metric_aggregate {
            if let Some(value) = path_metrics.get(&(*player, *metric)) {
                aggregate.unconditional.push(value.total);
                if value.seconds > 0.0 {
                    aggregate.conditional.push(value.total);
                    aggregate.conditional_per90.push(value.total * f64::from(REGULATION_MATCH_SECONDS) / value.seconds);
                    path_player_seconds.entry(*player).and_modify(|seconds| *seconds = seconds.max(value.seconds)).or_insert(value.seconds);
                }
            } else {
                aggregate.unconditional.push(0.0);
            }
        }
        for (player, seconds) in path_player_seconds {
            let value = participation.entry(player).or_default();
            value.appearances = value.appearances.saturating_add(1);
            value.seconds += seconds;
        }
        for (player, state) in mental_players {
            let aggregate = mental_aggregate.entry(player).or_default();
            aggregate.observations = aggregate.observations.saturating_add(1);
            aggregate.initial_morale += state.kickoff_value(MentalDimension::Morale);
            aggregate.final_morale += state.value(MentalDimension::Morale);
            aggregate.initial_belief += state.kickoff_value(MentalDimension::SelfBelief);
            aggregate.final_belief += state.value(MentalDimension::SelfBelief);
            aggregate.initial_motivation += state.kickoff_value(MentalDimension::Motivation);
            aggregate.final_motivation += state.value(MentalDimension::Motivation);
            aggregate.abs_morale += (state.value(MentalDimension::Morale) - state.kickoff_value(MentalDimension::Morale)).abs();
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
        home_goals: summarize(&home_goals)?,
        away_goals: summarize(&away_goals)?,
        home_expected_points: 3.0 * f64::from(home_win.get()) + f64::from(draw.get()),
        away_expected_points: 3.0 * f64::from(away_win.get()) + f64::from(draw.get()),
        paths,
    };
    let home_lineup = lineup_projection(&input.home_lineup, &home_lineups, paths);
    let away_lineup = lineup_projection(&input.away_lineup, &away_lineups, paths);
    let player_metrics = metric_aggregate.into_iter().map(|((player, metric), value)| {
        let starts = home_lineups.starts.get(&player).copied().unwrap_or(0)
            + away_lineups.starts.get(&player).copied().unwrap_or(0);
        let participation = participation.get(&player);
        Some(PrimitivePlayerMetricForecast {
            player,
            metric,
            start_probability: Probability::clamped(starts as f32 / denominator),
            appearance_probability: Probability::clamped(
                participation.map(|value| value.appearances).unwrap_or(0) as f32 / denominator,
            ),
            expected_minutes: participation.map(|value| value.seconds / 60.0 / f64::from(paths)).unwrap_or(0.0),
            unconditional_total: summarize(&value.unconditional)?,
            conditional_on_appearance_total: summarize(&value.conditional),
            conditional_on_appearance_per90: summarize(&value.conditional_per90),
        })
    }).collect::<Option<Vec<_>>>()?;
    let player_mental = mental_aggregate.into_iter().map(|(player, value)| {
        let n = f64::from(value.observations.max(1));
        SecondResolvedPlayerMentalForecast {
            player,
            initial_morale_mean: value.initial_morale / n,
            final_morale_mean: value.final_morale / n,
            initial_self_belief_mean: value.initial_belief / n,
            final_self_belief_mean: value.final_belief / n,
            initial_motivation_mean: value.initial_motivation / n,
            final_motivation_mean: value.final_motivation / n,
            mean_abs_morale_change: value.abs_morale / n,
        }
    }).collect();
    let state_events = event_counts.into_iter().map(|(kind, count)| SecondResolvedEventSummary {
        kind,
        mean_per_path: count as f64 / f64::from(paths),
    }).collect();
    let windows_15min = summarize_clock_windows(&windows, paths, 15);
    let minutes_1min = summarize_clock_windows(&minute_windows, paths, 1);
    let player_minute_events = player_minute_aggregate.into_iter()
        .map(|((minute, player), totals)| SecondResolvedPlayerMinuteForecast {
            minute, player,
            expected_shots: totals.shots / f64::from(paths),
            expected_goals: f64::from(totals.goals) / f64::from(paths),
            expected_assists_proxy: f64::from(totals.assists) / f64::from(paths),
        }).collect();
    Some(SecondResolvedDynamicMentalForecast {
        outcome, windows_15min, minutes_1min, player_minute_events,
        unattributed_goals_per_match: f64::from(unattributed_goals) / f64::from(paths),
        home_lineup,
        away_lineup,
        player_metrics,
        player_mental,
        state_events,
        same_second_collision_probability: Probability::clamped(collision_paths as f32 / denominator),
        mean_state_events_per_path: total_state_events as f64 / f64::from(paths),
        simulated_match_seconds: end.0,
        paths,
    })
}

fn accumulate_path_windows(aggregates: &mut [WindowAggregate], path: &mut [PathWindow]) {
    let (mut home_running, mut away_running) = (0_u32, 0_u32);
    for (aggregate, window) in aggregates.iter_mut().zip(path) {
        home_running += window.home_goals;
        away_running += window.away_goals;
        window.cumulative_home = home_running;
        window.cumulative_away = away_running;
        aggregate.any_goal += u32::from(window.home_goals + window.away_goals > 0);
        aggregate.home_goal += u32::from(window.home_goals > 0);
        aggregate.away_goal += u32::from(window.away_goals > 0);
        aggregate.red_card += u32::from(window.red_cards > 0);
        aggregate.substitution += u32::from(window.substitutions > 0);
        aggregate.home_goals += u64::from(window.home_goals);
        aggregate.away_goals += u64::from(window.away_goals);
        aggregate.home_shots += window.home_shots;
        aggregate.away_shots += window.away_shots;
        if home_running > away_running { aggregate.home_lead += 1; }
        else if home_running < away_running { aggregate.away_lead += 1; }
        else { aggregate.draw += 1; }
    }
}

fn summarize_clock_windows(aggregates: &[WindowAggregate], paths: u32, size_minutes: u16)
    -> Vec<SecondResolvedWindowForecast>
{
    aggregates.iter().enumerate().map(|(i,w)| {
        let d = f64::from(paths);
        SecondResolvedWindowForecast {
            start_minute:(i as u16)*size_minutes,
            end_minute:((i+1) as u16)*size_minutes,
            probability_any_goal:f64::from(w.any_goal)/d,
            probability_home_goal:f64::from(w.home_goal)/d,
            probability_away_goal:f64::from(w.away_goal)/d,
            probability_red_card:f64::from(w.red_card)/d,
            probability_substitution:f64::from(w.substitution)/d,
            expected_home_goals:w.home_goals as f64/d,
            expected_away_goals:w.away_goals as f64/d,
            expected_home_shots:w.home_shots/d, expected_away_shots:w.away_shots/d,
            end_home_lead:f64::from(w.home_lead)/d,
            end_draw:f64::from(w.draw)/d,
            end_away_lead:f64::from(w.away_lead)/d,
        }
    }).collect()
}

/// Conditional goalscorer and optional assisting-player assignment, never draws
/// a second goal. xG/shot and xA rate weights are lineup/FМ-adjusted, but the
/// assisted-goal prior is explicitly supplied by the simulation parameters.
fn attribute_goal(team: &ActiveTeam, snapshot: &RateSnapshot, assist_probability: f64, rng: &mut Rng64)
    -> Option<(PlayerId, Option<PlayerId>)>
{
    let ids=team.active_ids();
    let scorer=ids.get(weighted_index(ids.iter().map(|id| {
        let xg=snapshot.player_rates.get(&(*id,MetricId(2))).map_or(0.0,|r|r.adjusted_per90);
        let shots=snapshot.player_rates.get(&(*id,MetricId(1))).map_or(0.0,|r|r.adjusted_per90);
        (xg.max(0.0)+0.05*shots.max(0.0)).max(0.01)
    }),rng)?)?.to_owned();
    let assist=if ids.len()>1 && rng.uniform()<assist_probability {
        let eligible:Vec<PlayerId>=ids.into_iter().filter(|id|*id!=scorer).collect();
        weighted_index(eligible.iter().map(|id| {
            snapshot.player_rates.get(&(*id,MetricId(3))).map_or(0.01,|r|r.adjusted_per90.max(0.01))
        }),rng).map(|index|eligible[index])
    }else{None};
    Some((scorer,assist))
}

#[allow(clippy::too_many_arguments)]
fn build_rate_snapshot(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    mental_players: &BTreeMap<PlayerId, PathMentalPlayer>,
    home_mental: &PathTeamMental,
    away_mental: &PathTeamMental,
    home: &ActiveTeam,
    away: &ActiveTeam,
    latent: &BTreeMap<(PlayerId, MetricId), f64>,
    home_goals: u32,
    away_goals: u32,
    home_goal_base: f64,
    away_goal_base: f64,
) -> Option<RateSnapshot> {
    let home_lineup = home.lineup();
    let away_lineup = away.lineup();
    let home_rates = team_rates(
        input, mental_model, mental_players, home_mental, &home_lineup, &away_lineup,
        latent, true, score_state(home_goals, away_goals),
    )?;
    let away_rates = team_rates(
        input, mental_model, mental_players, away_mental, &away_lineup, &home_lineup,
        latent, false, score_state(away_goals, home_goals),
    )?;
    let home_deviations: Vec<(MetricId, f64)> = home_rates.iter().map(|((_, metric), rate)| {
        (*metric, rate.adjusted_per90 / rate.reference_per90 - 1.0)
    }).collect();
    let away_deviations: Vec<(MetricId, f64)> = away_rates.iter().map(|((_, metric), rate)| {
        (*metric, rate.adjusted_per90 / rate.reference_per90 - 1.0)
    }).collect();
    let home_channels = input.channel_graph.evaluate(&home_deviations)?;
    let away_channels = input.channel_graph.evaluate(&away_deviations)?;
    let home_log = (
        input.environment.home_residual_attack_log_adjustment
            + input.home_context.residual_attack_log_adjustment
            - input.away_context.residual_defense_log_adjustment
            + home_channels.attack_log_adjustment - away_channels.defense_log_adjustment
    ).clamp(-input.goal_process.max_abs_segment_log_adjustment, input.goal_process.max_abs_segment_log_adjustment);
    let away_log = (
        input.environment.away_residual_attack_log_adjustment
            + input.away_context.residual_attack_log_adjustment
            - input.home_context.residual_defense_log_adjustment
            + away_channels.attack_log_adjustment - home_channels.defense_log_adjustment
    ).clamp(-input.goal_process.max_abs_segment_log_adjustment, input.goal_process.max_abs_segment_log_adjustment);
    let mut player_rates = home_rates;
    player_rates.extend(away_rates);
    Some(RateSnapshot {
        home_goal_intensity_per_second: home_goal_base * home_log.exp() / f64::from(REGULATION_MATCH_SECONDS),
        away_goal_intensity_per_second: away_goal_base * away_log.exp() / f64::from(REGULATION_MATCH_SECONDS),
        player_rates,
    })
}

#[allow(clippy::too_many_arguments)]
fn team_rates(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    mental_players: &BTreeMap<PlayerId, PathMentalPlayer>,
    team_mental: &PathTeamMental,
    own: &SampledLineup,
    opponent: &SampledLineup,
    latent: &BTreeMap<(PlayerId, MetricId), f64>,
    is_home: bool,
    score: ScoreState,
) -> Option<BTreeMap<(PlayerId, MetricId), PlayerRate>> {
    let own_context = if is_home { &input.home_context } else { &input.away_context };
    let opponent_context = if is_home { &input.away_context } else { &input.home_context };
    let attributes: BTreeMap<PlayerId, &PlayerAttributeState> = input.attribute_states.iter().map(|value| (value.player, value)).collect();
    let mut out = BTreeMap::new();
    for starter in &own.starters {
        for model in input.event_models.iter().filter(|value| value.player == starter.player) {
            let interaction = player_metric_interaction_context(
                starter.player, model.metric, own, opponent, &input.player_states,
                &input.aggregate_matchup_rules, &input.synergies, input.matchup_spatial_sigma,
            )?;
            let own_attribute = attributes.get(&starter.player)
                .map(|state| own_attribute_adjustment(state, model.metric, &input.own_attribute_loadings))
                .unwrap_or(0.0);
            let fine = fine_matchup_advantage(
                starter, opponent, attributes.get(&starter.player).copied(), &attributes,
                model.metric, &input.attribute_matchup_rules, input.matchup_spatial_sigma,
            )?;
            let system = team_production_multiplier(own_context, model.metric)
                * team_suppression_multiplier(opponent_context, model.metric)
                * environment_multiplier(&input.environment.metric_modifiers, model.metric, is_home);
            if !system.is_finite() || system <= 0.0 { return None; }
            let (mental_mean, mental_variance) = mental_metric_adjustment(
                model.metric, mental_players.get(&starter.player), team_mental, mental_model,
            );
            let log_adjustment = (
                own_attribute
                    + model.aggregate_matchup_sensitivity * interaction.opponent_advantage
                    + model.attribute_matchup_sensitivity * fine
                    + model.teammate_sensitivity * interaction.teammate_support
                    + model.role_fit_sensitivity * (starter.role_fit - 1.0)
                    - model.fatigue_sensitivity * starter.fatigue
                    + system.ln()
                    + score_multiplier(model.metric, score, &input.score_state_modifiers).ln()
                    + mental_mean
            ).clamp(-input.max_player_rate_log_adjustment, input.max_player_rate_log_adjustment);
            out.insert((starter.player, model.metric), PlayerRate {
                adjusted_per90: latent.get(&(starter.player, model.metric)).copied()? * log_adjustment.exp(),
                variance_per90: (model.variance_per90 * mental_variance.exp()).max(0.0),
                family: model.family,
                reference_per90: model.reference_per90,
            });
        }
    }
    Some(out)
}

fn scheduled_substitutions_for_second(
    second: MatchSecond,
    home: &ActiveTeam,
    away: &ActiveTeam,
    input: &PrimitiveMatchSimulationInput,
    rng: &mut Rng64,
) -> Vec<TimedStateEvent> {
    let minimum = (input.in_match.minimum_tactical_substitution_minute * 60.0).round().max(0.0) as u32;
    if second.0 < minimum || second.0 >= REGULATION_MATCH_SECONDS { return Vec::new(); }
    let mut out = Vec::new();
    for (club, team) in [(input.home, home), (input.away, away)] {
        if team.substitutions >= input.in_match.max_substitutions { continue; }
        for active in &team.players {
            if active.planned_exit_second == second {
                out.push(TimedStateEvent {
                    time: EventTime::at_second(second, rng),
                    fraction: rng.uniform(),
                    event: StateEvent::TacticalSub { club, player: active.sampled.player },
                });
            }
        }
    }
    out
}

fn sample_stochastic_event(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    mental_players: &BTreeMap<PlayerId, PathMentalPlayer>,
    home: &ActiveTeam,
    away: &ActiveTeam,
    snapshot: &RateSnapshot,
    second: MatchSecond,
    cursor: f64,
    rng: &mut Rng64,
) -> Option<Option<TimedStateEvent>> {
    let profiles: BTreeMap<PlayerId, SpecificPlayerIncidentProfile> = input.incident_profiles.iter().copied()
        .map(|value| (value.player, value)).collect();
    let red_intensity = probability_per90_to_intensity_per_second(input.in_match.red_card_probability_per90)?;
    let injury_intensity = probability_per90_to_intensity_per_second(input.in_match.injury_probability_per90)?;
    let mut hazards = vec![
        (StateEvent::HomeGoal, snapshot.home_goal_intensity_per_second.max(0.0)),
        (StateEvent::AwayGoal, snapshot.away_goal_intensity_per_second.max(0.0)),
    ];
    for (club, team) in [(input.home, home), (input.away, away)] {
        for active in &team.players {
            let profile = profiles.get(&active.sampled.player).copied().unwrap_or(SpecificPlayerIncidentProfile {
                player: active.sampled.player, injury_multiplier: 1.0, red_card_multiplier: 1.0,
            });
            hazards.push((
                StateEvent::Red { club, player: active.sampled.player },
                red_intensity * profile.red_card_multiplier * dynamic_red_multiplier(mental_players.get(&active.sampled.player), mental_model),
            ));
            hazards.push((
                StateEvent::Injury { club, player: active.sampled.player },
                injury_intensity * profile.injury_multiplier,
            ));
        }
    }
    let total = hazards.iter().map(|(_, rate)| rate.max(0.0)).sum::<f64>();
    if total <= 0.0 { return Some(None); }
    let wait = -rng.uniform().clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON).ln() / total;
    let fraction = cursor + wait;
    if fraction >= 1.0 { return Some(None); }
    let mut target = rng.uniform() * total;
    for (event, rate) in hazards {
        let rate = rate.max(0.0);
        if target <= rate {
            return Some(Some(TimedStateEvent {
                time: EventTime::at_second(second, rng), fraction, event,
            }));
        }
        target -= rate;
    }
    Some(None)
}

fn earliest_event(left: Option<TimedStateEvent>, right: Option<TimedStateEvent>) -> Option<TimedStateEvent> {
    match (left, right) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value),
        (Some(left), Some(right)) => {
            if left.fraction < right.fraction { Some(left) }
            else if right.fraction < left.fraction { Some(right) }
            else if left.time.within_second <= right.time.within_second { Some(left) }
            else { Some(right) }
        }
    }
}

/// Sample player-stat increments over a fraction of one second. Count metrics are Poisson processes;
/// continuous non-negative metrics use a gamma-process increment so both mean and variance scale
/// linearly with elapsed time. Returns an exposure-scaled standardized personal-performance signal.
fn advance_player_metrics(
    snapshot: &RateSnapshot,
    elapsed_seconds: f64,
    path_metrics: &mut BTreeMap<(PlayerId, MetricId), PathMetric>,
    rng: &mut Rng64,
    window: &mut PathWindow,
    minute: &mut PathWindow,
    minute_index: u16,
    player_minutes: &mut BTreeMap<(u16,PlayerId),PlayerMinuteAggregate>,
    home_players: &BTreeSet<PlayerId>,
) -> BTreeMap<PlayerId, f64> {
    let exposure = elapsed_seconds / f64::from(REGULATION_MATCH_SECONDS);
    let mut signal_sum = BTreeMap::<PlayerId, f64>::new();
    let mut signal_count = BTreeMap::<PlayerId, u32>::new();
    for (key, rate) in &snapshot.player_rates {
        let mean = rate.adjusted_per90.max(0.0) * exposure;
        let variance = match rate.family {
            StatFamily::Count => mean,
            StatFamily::ContinuousNonNegative => rate.variance_per90.max(0.0) * exposure,
        };
        let realized = match rate.family {
            StatFamily::Count => f64::from(poisson(mean, rng)),
            StatFamily::ContinuousNonNegative => gamma_increment(mean, variance, rng),
        };
        if key.1.0 == 1 {
            if home_players.contains(&key.0) {window.home_shots += realized;minute.home_shots += realized;}
            else {window.away_shots += realized;minute.away_shots += realized;}
            if realized > 0.0 {player_minutes.entry((minute_index,key.0)).or_default().shots += realized;}
        }
        let value = path_metrics.entry(*key).or_default();
        value.total += realized;
        value.seconds += elapsed_seconds;
        let standardized = if variance > 1e-12 { (realized - mean) / variance.sqrt() } else { 0.0 };
        let scaled = standardized.tanh() * (elapsed_seconds / DYNAMIC_MENTAL_REFERENCE_SECONDS).sqrt();
        *signal_sum.entry(key.0).or_default() += scaled;
        *signal_count.entry(key.0).or_default() += 1;
    }
    signal_sum.into_iter().map(|(player, total)| {
        let count = f64::from(signal_count.get(&player).copied().unwrap_or(1));
        (player, total / count.sqrt())
    }).collect()
}

fn apply_personal_feedback(
    active: &ActiveTeam,
    signals: &BTreeMap<PlayerId, f64>,
    players: &mut BTreeMap<PlayerId, PathMentalPlayer>,
    transition: DynamicMentalTransitionParameters,
    _elapsed_seconds: f64,
) {
    for player in active.active_ids() {
        let Some(signal) = signals.get(&player).copied() else { continue; };
        let Some(state) = players.get_mut(&player) else { continue; };
        let (belief_weight, morale_weight) = if signal >= 0.0 {
            (transition.personal_success_self_belief, transition.personal_success_morale)
        } else {
            (-transition.personal_error_self_belief, -transition.personal_error_morale)
        };
        let magnitude = signal.abs();
        let belief = state.add(
            MentalDimension::SelfBelief,
            magnitude * belief_weight,
            transition.max_abs_state_delta_per_segment,
        );
        let morale = state.add(
            MentalDimension::Morale,
            magnitude * morale_weight,
            transition.max_abs_state_delta_per_segment,
        );
        state.add(
            MentalDimension::Motivation,
            transition.motivation_belief_coupling * belief + transition.motivation_morale_coupling * morale,
            transition.max_abs_state_delta_per_segment,
        );
    }
}

fn advance_continuous_mental(
    squad: &[PlayerId],
    active: &ActiveTeam,
    team: &mut PathTeamMental,
    players: &mut BTreeMap<PlayerId, PathMentalPlayer>,
    score: ScoreState,
    context: MatchMentalContext,
    transition: DynamicMentalTransitionParameters,
    elapsed_seconds: f64,
    rng: &mut Rng64,
) {
    if elapsed_seconds <= 0.0 { return; }
    for player in squad {
        let Some(state) = players.get_mut(player) else { continue; };
        let pressure_reference = match score {
            ScoreState::Trailing => transition.trailing_pressure_belief * context.pressure_intensity
                * (1.0 - unit_support(state.value(MentalDimension::PressureResilience))),
            ScoreState::Leading => transition.leading_belief_boost * context.pressure_intensity,
            ScoreState::Level => 0.0,
        };
        state.add(
            MentalDimension::SelfBelief,
            pressure_reference * elapsed_seconds / DYNAMIC_MENTAL_REFERENCE_SECONDS,
            transition.max_abs_state_delta_per_segment,
        );
        let professionalism = unit_support(state.value(MentalDimension::Professionalism));
        let reference_fraction = (
            transition.base_mean_reversion * (1.0 + transition.professionalism_reversion_weight * professionalism)
        ).clamp(0.0, 1.0);
        let reversion = scaled_fraction(reference_fraction, elapsed_seconds, DYNAMIC_MENTAL_REFERENCE_SECONDS);
        for dimension in [MentalDimension::Morale, MentalDimension::SelfBelief, MentalDimension::Motivation] {
            state.add(
                dimension,
                reversion * (state.kickoff_value(dimension) - state.value(dimension)),
                transition.max_abs_state_delta_per_segment,
            );
        }
        let sigma_reference = transition.segment_innovation_sigma
            * (1.0 - transition.consistency_volatility_damping * unit_support(state.value(MentalDimension::Consistency))).max(0.0);
        let sigma = sigma_reference * (elapsed_seconds / DYNAMIC_MENTAL_REFERENCE_SECONDS).sqrt();
        if sigma > 0.0 {
            state.add(MentalDimension::Morale, sigma * rng.normal(), transition.max_abs_state_delta_per_segment);
            state.add(MentalDimension::SelfBelief, sigma * rng.normal(), transition.max_abs_state_delta_per_segment);
            state.add(MentalDimension::Motivation, sigma * rng.normal(), transition.max_abs_state_delta_per_segment);
        }
    }
    let blend = scaled_fraction(transition.active_lineup_blend, elapsed_seconds, DYNAMIC_MENTAL_REFERENCE_SECONDS);
    blend_team_with_active(team, active, players, blend);
}

#[allow(clippy::too_many_arguments)]
fn apply_state_event(
    event: StateEvent,
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    transition: DynamicMentalTransitionParameters,
    home_squad: &[PlayerId],
    away_squad: &[PlayerId],
    home: &mut ActiveTeam,
    away: &mut ActiveTeam,
    home_projection: &LineupProjectionInput,
    away_projection: &LineupProjectionInput,
    home_mental: &mut PathTeamMental,
    away_mental: &mut PathTeamMental,
    players: &mut BTreeMap<PlayerId, PathMentalPlayer>,
    home_goals: &mut u32,
    away_goals: &mut u32,
    counts: &mut BTreeMap<SecondResolvedStateEventKind, u64>,
    rng: &mut Rng64,
) -> Option<()> {
    match event {
        StateEvent::HomeGoal => {
            *home_goals = home_goals.saturating_add(1);
            *counts.entry(SecondResolvedStateEventKind::HomeGoal).or_default() += 1;
            goal_shock(home_squad, home_mental, players, true, transition);
            goal_shock(away_squad, away_mental, players, false, transition);
        }
        StateEvent::AwayGoal => {
            *away_goals = away_goals.saturating_add(1);
            *counts.entry(SecondResolvedStateEventKind::AwayGoal).or_default() += 1;
            goal_shock(away_squad, away_mental, players, true, transition);
            goal_shock(home_squad, home_mental, players, false, transition);
        }
        StateEvent::Red { club, player } => {
            let (team, squad, mental) = select_team_mut(club, input, home, away, home_squad, away_squad, home_mental, away_mental)?;
            if !team.players.iter().any(|value| value.sampled.player == player) { return Some(()); }
            team.players.retain(|value| value.sampled.player != player);
            *counts.entry(SecondResolvedStateEventKind::RedCard).or_default() += 1;
            incident_shock(squad, mental, players, player, true, transition);
        }
        StateEvent::Injury { club, player } => {
            let (team, projection, squad, mental) = if club == input.home {
                (&mut *home, home_projection, home_squad, &mut *home_mental)
            } else if club == input.away {
                (&mut *away, away_projection, away_squad, &mut *away_mental)
            } else { return None; };
            let Some(outgoing) = team.players.iter().find(|value| value.sampled.player == player).map(|value| value.sampled) else { return Some(()); };
            team.players.retain(|value| value.sampled.player != player);
            *counts.entry(SecondResolvedStateEventKind::Injury).or_default() += 1;
            incident_shock(squad, mental, players, player, false, transition);
            if team.substitutions < input.in_match.max_substitutions {
                if let Some(replacement) = sample_replacement(projection, team, outgoing, players, mental_model, rng) {
                    team.used.insert(replacement.sampled.player);
                    team.players.push(replacement);
                    team.substitutions = team.substitutions.saturating_add(1);
                    *counts.entry(SecondResolvedStateEventKind::ForcedSubstitution).or_default() += 1;
                }
            }
        }
        StateEvent::TacticalSub { club, player } => {
            let (team, projection) = if club == input.home { (&mut *home, home_projection) }
                else if club == input.away { (&mut *away, away_projection) }
                else { return None; };
            if team.substitutions >= input.in_match.max_substitutions { return Some(()); }
            let Some(outgoing) = team.players.iter().find(|value| value.sampled.player == player).map(|value| value.sampled) else { return Some(()); };
            let Some(replacement) = sample_replacement(projection, team, outgoing, players, mental_model, rng) else { return Some(()); };
            team.players.retain(|value| value.sampled.player != player);
            team.used.insert(replacement.sampled.player);
            team.players.push(replacement.clone());
            team.substitutions = team.substitutions.saturating_add(1);
            if let Some(state) = players.get_mut(&player) {
                state.add(MentalDimension::RoleSatisfaction, transition.substituted_off_role_satisfaction, transition.max_abs_state_delta_per_segment);
            }
            if let Some(state) = players.get_mut(&replacement.sampled.player) {
                state.add(MentalDimension::Motivation, transition.substituted_on_motivation, transition.max_abs_state_delta_per_segment);
            }
            *counts.entry(SecondResolvedStateEventKind::TacticalSubstitution).or_default() += 1;
        }
    }
    blend_team_with_active(home_mental, home, players, transition.active_lineup_blend);
    blend_team_with_active(away_mental, away, players, transition.active_lineup_blend);
    Some(())
}

#[allow(clippy::type_complexity)]
fn select_team_mut<'a>(
    club: ClubId,
    input: &PrimitiveMatchSimulationInput,
    home: &'a mut ActiveTeam,
    away: &'a mut ActiveTeam,
    home_squad: &'a [PlayerId],
    away_squad: &'a [PlayerId],
    home_mental: &'a mut PathTeamMental,
    away_mental: &'a mut PathTeamMental,
) -> Option<(&'a mut ActiveTeam, &'a [PlayerId], &'a mut PathTeamMental)> {
    if club == input.home { Some((home, home_squad, home_mental)) }
    else if club == input.away { Some((away, away_squad, away_mental)) }
    else { None }
}

fn goal_shock(
    squad: &[PlayerId],
    team: &mut PathTeamMental,
    players: &mut BTreeMap<PlayerId, PathMentalPlayer>,
    scored: bool,
    transition: DynamicMentalTransitionParameters,
) {
    let morale = if scored { transition.goal_for_morale } else { transition.goal_against_morale };
    let belief = if scored { transition.goal_for_belief } else { transition.goal_against_belief };
    team.add(TeamMentalDimension::CollectiveMorale, morale, transition.max_abs_state_delta_per_segment);
    team.add(TeamMentalDimension::CollectiveBelief, belief, transition.max_abs_state_delta_per_segment);
    for player in squad {
        if let Some(state) = players.get_mut(player) {
            let dm = state.add(MentalDimension::Morale, morale, transition.max_abs_state_delta_per_segment);
            let db = state.add(MentalDimension::SelfBelief, belief, transition.max_abs_state_delta_per_segment);
            state.add(
                MentalDimension::Motivation,
                transition.motivation_morale_coupling * dm + transition.motivation_belief_coupling * db,
                transition.max_abs_state_delta_per_segment,
            );
        }
    }
}

fn incident_shock(
    squad: &[PlayerId],
    team: &mut PathTeamMental,
    players: &mut BTreeMap<PlayerId, PathMentalPlayer>,
    affected: PlayerId,
    red: bool,
    transition: DynamicMentalTransitionParameters,
) {
    let teammate_morale = if red { transition.teammate_red_morale } else { transition.teammate_injury_morale };
    let teammate_belief = if red { transition.teammate_red_belief } else { 0.0 };
    team.add(TeamMentalDimension::CollectiveMorale, teammate_morale, transition.max_abs_state_delta_per_segment);
    team.add(TeamMentalDimension::CollectiveBelief, teammate_belief, transition.max_abs_state_delta_per_segment);
    for player in squad {
        let Some(state) = players.get_mut(player) else { continue; };
        if *player == affected {
            state.add(
                MentalDimension::Morale,
                if red { transition.own_red_morale } else { transition.own_injury_morale },
                transition.max_abs_state_delta_per_segment,
            );
        } else {
            state.add(MentalDimension::Morale, teammate_morale, transition.max_abs_state_delta_per_segment);
            state.add(MentalDimension::SelfBelief, teammate_belief, transition.max_abs_state_delta_per_segment);
        }
    }
}

fn sample_replacement(
    projection: &LineupProjectionInput,
    active: &ActiveTeam,
    outgoing: SampledStarter,
    players: &BTreeMap<PlayerId, PathMentalPlayer>,
    mental_model: &MentalMatchModel,
    rng: &mut Rng64,
) -> Option<ActivePlayer> {
    let candidates: Vec<&LineupCandidate> = projection.candidates.iter().filter(|value| {
        !active.used.contains(&value.player) && value.substitute_probability > 0.0 && value.availability_probability > 0.0
    }).collect();
    if candidates.is_empty() { return None; }
    let weights = candidates.iter().map(|candidate| {
        let mental_delta = players.get(&candidate.player).map(|state| {
            mental_model.selection_loadings.iter().map(|loading| {
                loading.substitute_logit_weight
                    * (state.value(loading.dimension) - state.kickoff_value(loading.dimension))
                    * context_scale(loading.dimension, mental_model.context)
            }).sum::<f64>().clamp(
                -mental_model.parameters.max_abs_selection_logit_adjustment,
                mental_model.parameters.max_abs_selection_logit_adjustment,
            )
        }).unwrap_or(0.0);
        adjust_probability(candidate.substitute_probability, mental_delta)
            * candidate.availability_probability
            * (candidate.role_fit(outgoing.role) + 0.02).powf(projection.parameters.role_fit_power)
            * (-projection.parameters.fatigue_weight * candidate.fatigue).exp()
    });
    let selected = candidates[weighted_index(weights, rng)?];
    if !rng.bernoulli(selected.availability_probability) { return None; }
    Some(ActivePlayer {
        sampled: SampledStarter {
            player: selected.player, slot: outgoing.slot, role: outgoing.role, x: outgoing.x, y: outgoing.y,
            minutes: outgoing.minutes, role_fit: selected.role_fit(outgoing.role), fatigue: selected.fatigue,
        },
        planned_exit_second: MatchSecond::REGULATION_END,
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

fn sample_player_mental(state: PlayerMentalState, transition: DynamicMentalTransitionParameters, rng: &mut Rng64) -> PathMentalPlayer {
    let mut kickoff = BTreeMap::new();
    for dimension in all_dimensions() {
        let measure = state.measure(dimension);
        let confidence = f64::from(measure.confidence.get());
        kickoff.insert(
            dimension,
            (measure.value * confidence + transition.initial_state_sigma_at_zero_confidence * (1.0 - confidence) * rng.normal()).clamp(-1.0, 1.0),
        );
    }
    PathMentalPlayer { current: kickoff.clone(), kickoff }
}

fn sample_unknown_mental(transition: DynamicMentalTransitionParameters, rng: &mut Rng64) -> PathMentalPlayer {
    let mut kickoff = BTreeMap::new();
    for dimension in all_dimensions() {
        kickoff.insert(dimension, (transition.initial_state_sigma_at_zero_confidence * rng.normal()).clamp(-1.0, 1.0));
    }
    PathMentalPlayer { current: kickoff.clone(), kickoff }
}

fn sample_team_mental(state: TeamMentalState, transition: DynamicMentalTransitionParameters, rng: &mut Rng64) -> PathTeamMental {
    let mut kickoff = BTreeMap::new();
    for dimension in [TeamMentalDimension::CollectiveMorale, TeamMentalDimension::CollectiveBelief, TeamMentalDimension::Cohesion, TeamMentalDimension::Leadership] {
        let measure = state.measure(dimension);
        let confidence = f64::from(measure.confidence.get());
        kickoff.insert(
            dimension,
            (measure.value * confidence + transition.initial_state_sigma_at_zero_confidence * (1.0 - confidence) * rng.normal()).clamp(-1.0, 1.0),
        );
    }
    PathTeamMental { current: kickoff }
}

fn adjust_lineup_for_path_mental(lineup: &mut LineupProjectionInput, states: &BTreeMap<PlayerId, PathMentalPlayer>, model: &MentalMatchModel) {
    for candidate in &mut lineup.candidates {
        let Some(state) = states.get(&candidate.player) else { continue; };
        let mut start = 0.0;
        let mut substitute = 0.0;
        for loading in &model.selection_loadings {
            let effect = state.value(loading.dimension) * context_scale(loading.dimension, model.context);
            start += loading.start_logit_weight * effect;
            substitute += loading.substitute_logit_weight * effect;
        }
        candidate.start_probability = adjust_probability(candidate.start_probability, start.clamp(
            -model.parameters.max_abs_selection_logit_adjustment, model.parameters.max_abs_selection_logit_adjustment,
        ));
        candidate.substitute_probability = adjust_probability(candidate.substitute_probability, substitute.clamp(
            -model.parameters.max_abs_selection_logit_adjustment, model.parameters.max_abs_selection_logit_adjustment,
        ));
    }
}

fn blend_team_with_active(team: &mut PathTeamMental, active: &ActiveTeam, players: &BTreeMap<PlayerId, PathMentalPlayer>, blend: f64) {
    if blend <= 0.0 || active.players.is_empty() { return; }
    let ids = active.active_ids();
    for (td, pd) in [
        (TeamMentalDimension::CollectiveMorale, MentalDimension::Morale),
        (TeamMentalDimension::CollectiveBelief, MentalDimension::SelfBelief),
        (TeamMentalDimension::Leadership, MentalDimension::Leadership),
        (TeamMentalDimension::Cohesion, MentalDimension::Teamwork),
    ] {
        let target = ids.iter().map(|player| players.get(player).map(|state| state.value(pd)).unwrap_or(0.0)).sum::<f64>() / ids.len() as f64;
        team.current.insert(td, ((1.0 - blend) * team.value(td) + blend * target).clamp(-1.0, 1.0));
    }
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
        let dx = attacker.x - (1.0 - defender.x);
        let dy = attacker.y - (1.0 - defender.y);
        let spatial = (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp();
        if spatial < 1e-8 { continue; }
        let mut pair = 0.0;
        let mut weight = 0.0;
        for rule in &applicable {
            let Some(a) = attacker_state.value(rule.attacker_attribute) else { continue; };
            let Some(d) = defender_state.value(rule.defender_attribute) else { continue; };
            pair += rule.weight * (a - d) / rule.scale;
            weight += rule.weight.abs();
        }
        if weight > 0.0 { numerator += spatial * pair / weight; denominator += spatial; }
    }
    Some(if denominator > 0.0 { numerator / denominator } else { 0.0 })
}

fn own_attribute_adjustment(state: &PlayerAttributeState, metric: MetricId, values: &[AttributeEventLoading]) -> f64 {
    values.iter().filter(|value| value.metric == metric).filter_map(|value| {
        state.value(value.attribute).map(|actual| value.log_rate_weight * (actual - value.reference) / value.scale)
    }).sum()
}

fn dynamic_red_multiplier(state: Option<&PathMentalPlayer>, model: &MentalMatchModel) -> f64 {
    let mut log_risk = model.parameters.hostility_red_card_log_weight * model.context.hostility;
    if let Some(state) = state {
        log_risk += model.parameters.discipline_red_card_log_weight * state.value(MentalDimension::Discipline);
        log_risk += model.parameters.low_morale_red_card_log_weight * (-state.value(MentalDimension::Morale)).max(0.0);
    }
    log_risk.clamp(-2.0, 2.0).exp()
}

fn context_scale(dimension: MentalDimension, context: MatchMentalContext) -> f64 {
    match dimension {
        MentalDimension::PressureResilience => context.pressure_intensity,
        MentalDimension::Adaptability => context.environmental_novelty,
        MentalDimension::Discipline => 1.0 + context.hostility,
        _ => 1.0,
    }
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

fn score_state(own: u32, opponent: u32) -> ScoreState {
    match own.cmp(&opponent) {
        std::cmp::Ordering::Less => ScoreState::Trailing,
        std::cmp::Ordering::Equal => ScoreState::Level,
        std::cmp::Ordering::Greater => ScoreState::Leading,
    }
}

fn all_dimensions() -> [MentalDimension; 14] {
    [MentalDimension::Morale, MentalDimension::SelfBelief, MentalDimension::Form, MentalDimension::FormTrend,
        MentalDimension::Motivation, MentalDimension::RoleSatisfaction, MentalDimension::Professionalism,
        MentalDimension::Ambition, MentalDimension::Consistency, MentalDimension::PressureResilience,
        MentalDimension::Adaptability, MentalDimension::Leadership, MentalDimension::Teamwork, MentalDimension::Discipline]
}

fn accumulate_starting_lineup(lineup: &SampledLineup, aggregate: &mut LineupAggregate) {
    *aggregate.formations.entry(lineup.formation_id).or_default() += 1;
    for starter in &lineup.starters {
        *aggregate.starts.entry(starter.player).or_default() += 1;
        *aggregate.starter_minutes.entry(starter.player).or_default() += starter.minutes;
        *aggregate.roles.entry((starter.player, starter.role)).or_default() += 1;
    }
}

fn lineup_projection(input: &LineupProjectionInput, aggregate: &LineupAggregate, paths: u32) -> LineupProjection {
    let denominator = paths as f32;
    let formations = input.formations.iter().map(|value| FormationProjection {
        formation_id: value.formation.id,
        probability: Probability::clamped(aggregate.formations.get(&value.formation.id).copied().unwrap_or(0) as f32 / denominator),
    }).collect();
    let players = input.candidates.iter().map(|candidate| {
        let starts = aggregate.starts.get(&candidate.player).copied().unwrap_or(0);
        let minutes = aggregate.starter_minutes.get(&candidate.player).copied().unwrap_or(0.0);
        let roles = if starts > 0 { candidate.role_fits.iter().filter_map(|fit| {
            let count = aggregate.roles.get(&(candidate.player, fit.role)).copied().unwrap_or(0);
            (count > 0).then_some(RoleProjection { role: fit.role, probability_given_start: Probability::clamped(count as f32 / starts as f32) })
        }).collect() } else { Vec::new() };
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

fn probability_per90_to_intensity_per_second(probability: f64) -> Option<f64> {
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) { return None; }
    if probability <= 0.0 { return Some(0.0); }
    if probability >= 1.0 { return Some(f64::INFINITY); }
    Some(-(1.0 - probability).ln() / f64::from(REGULATION_MATCH_SECONDS))
}

fn scaled_fraction(reference_fraction: f64, elapsed_seconds: f64, reference_seconds: f64) -> f64 {
    if reference_fraction <= 0.0 || elapsed_seconds <= 0.0 { return 0.0; }
    if reference_fraction >= 1.0 { return 1.0; }
    1.0 - (1.0 - reference_fraction).powf(elapsed_seconds / reference_seconds)
}

fn adjust_probability(probability: f64, logit_delta: f64) -> f64 {
    if probability <= 0.0 { return 0.0; }
    if probability >= 1.0 { return 1.0; }
    let logit = (probability / (1.0 - probability)).ln() + logit_delta;
    1.0 / (1.0 + (-logit).exp())
}

fn unit_support(value: f64) -> f64 { ((value + 1.0) * 0.5).clamp(0.0, 1.0) }

fn weighted_index<I>(weights: I, rng: &mut Rng64) -> Option<usize>
where I: IntoIterator<Item = f64> {
    let values: Vec<f64> = weights.into_iter().map(|value| if value.is_finite() && value > 0.0 { value } else { 0.0 }).collect();
    let total = values.iter().sum::<f64>();
    if values.is_empty() || total <= 0.0 { return None; }
    let mut target = rng.uniform() * total;
    for (index, value) in values.iter().enumerate() {
        if target <= *value { return Some(index); }
        target -= *value;
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

/// Marsaglia-Tsang gamma sampler. Parameterized by desired mean/variance so continuous non-negative
/// player statistics form an independent-increment process whose 90-minute variance is invariant to
/// clock resolution.
fn gamma_increment(mean: f64, variance: f64, rng: &mut Rng64) -> f64 {
    if mean <= 0.0 { return 0.0; }
    if variance <= f64::EPSILON { return mean; }
    let shape = mean * mean / variance;
    let scale = variance / mean;
    sample_gamma_shape(shape, rng) * scale
}

fn sample_gamma_shape(shape: f64, rng: &mut Rng64) -> f64 {
    if shape <= 0.0 { return 0.0; }
    if shape < 1.0 {
        let u = rng.uniform().clamp(f64::MIN_POSITIVE, 1.0);
        return sample_gamma_shape(shape + 1.0, rng) * u.powf(1.0 / shape);
    }
    let d = shape - 1.0 / 3.0;
    let c = (1.0 / (9.0 * d)).sqrt();
    loop {
        let x = rng.normal();
        let v0 = 1.0 + c * x;
        if v0 <= 0.0 { continue; }
        let v = v0 * v0 * v0;
        let u = rng.uniform();
        if u < 1.0 - 0.0331 * x.powi(4) || u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
            return d * v;
        }
    }
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
    if lower == upper { sorted[lower] }
    else { let fraction = position - lower as f64; sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assisted_goal_calibration_parameter_requires_valid_probability() {
        let prior=SecondResolvedSimulationParameters::default();
        assert_eq!(prior.assisted_goal_probability, 0.68);
        assert!(prior.validate());
        for allowed in [0.0,0.5,1.0] {
            assert!(SecondResolvedSimulationParameters {
                assisted_goal_probability: allowed, ..prior
            }.validate());
        }
        for bad in [-0.01,1.01,f64::NAN,f64::INFINITY] {
            assert!(!SecondResolvedSimulationParameters {
                assisted_goal_probability: bad, ..prior
            }.validate());
        }
    }

    #[test]
    fn gamma_increment_is_non_negative() {
        let mut rng = Rng64::new(1);
        for _ in 0..1000 { assert!(gamma_increment(0.01, 0.02, &mut rng) >= 0.0); }
    }

    #[test]
    fn reversion_scaling_recovers_reference_fraction() {
        let per_second = scaled_fraction(0.08, 1.0, DYNAMIC_MENTAL_REFERENCE_SECONDS);
        let rebuilt = 1.0 - (1.0 - per_second).powf(DYNAMIC_MENTAL_REFERENCE_SECONDS);
        assert!((rebuilt - 0.08).abs() < 1e-10);
    }

    #[test]
    fn probability_boundaries_are_exact() {
        assert_eq!(adjust_probability(0.0, 100.0), 0.0);
        assert_eq!(adjust_probability(1.0, -100.0), 1.0);
    }

    #[test]
    fn per90_incident_probability_converts_to_exact_continuous_intensity() {
        let p90 = 0.12;
        let intensity = probability_per90_to_intensity_per_second(p90).unwrap();
        let rebuilt = 1.0 - (-intensity * f64::from(REGULATION_MATCH_SECONDS)).exp();
        assert!((rebuilt - p90).abs() < 1e-12);
    }
}
