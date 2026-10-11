use crate::lineup::{project_lineup, sample_lineup, LineupProjection, LineupProjectionInput, SampledLineup};
use crate::matchup::{
    player_metric_interaction_context, MatchupRule, PairwiseSynergy, PlayerMatchupState,
};
use footy_core::{ClubId, MatchId, MetricId, PlayerId, Probability};
use footy_forecast::{
    DistributionSummary, MatchOutcomeForecast, PlayerStatSimulationInput, Rng64, StatFamily,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerMetricImpactModel {
    pub player: PlayerId,
    pub metric: MetricId,
    pub family: StatFamily,
    pub mean_per90: f64,
    pub variance_per90: f64,
    pub epistemic_sigma: f64,
    /// Peer/league reference used to turn the realized event rate into a centered team contribution.
    pub reference_per90: f64,
    /// Log-goal-intensity contribution for one reference-normalized unit above/below baseline.
    pub attack_log_weight: f64,
    pub defense_log_weight: f64,
    /// Sensitivity of this specific metric to the direct opponent interaction graph.
    pub matchup_sensitivity: f64,
    /// Sensitivity to historical/coordinated teammate relationships.
    pub teammate_sensitivity: f64,
    /// Perfect role fit is neutral; imperfect fit is penalized by this coefficient.
    pub role_fit_sensitivity: f64,
    /// Fatigue is a rate penalty, separate from lineup selection probability.
    pub fatigue_sensitivity: f64,
}

impl PlayerMetricImpactModel {
    pub fn from_stat_calibration(
        input: PlayerStatSimulationInput,
        reference_per90: f64,
        attack_log_weight: f64,
        defense_log_weight: f64,
    ) -> Option<Self> {
        let value = Self {
            player: input.player,
            metric: input.metric,
            family: input.family,
            mean_per90: input.mean_per90,
            variance_per90: input.variance_per90,
            epistemic_sigma: input.epistemic_sigma,
            reference_per90,
            attack_log_weight,
            defense_log_weight,
            matchup_sensitivity: 1.0,
            teammate_sensitivity: 1.0,
            role_fit_sensitivity: 0.5,
            fatigue_sensitivity: 0.5,
        };
        value.valid().then_some(value)
    }

    fn valid(self) -> bool {
        non_negative(self.mean_per90)
            && non_negative(self.variance_per90)
            && non_negative(self.epistemic_sigma)
            && self.reference_per90.is_finite()
            && self.reference_per90 > 0.0
            && self.attack_log_weight.is_finite()
            && self.defense_log_weight.is_finite()
            && self.matchup_sensitivity.is_finite()
            && self.teammate_sensitivity.is_finite()
            && self.role_fit_sensitivity.is_finite()
            && self.role_fit_sensitivity >= 0.0
            && self.fatigue_sensitivity.is_finite()
            && self.fatigue_sensitivity >= 0.0
    }
}

/// Latent FM/state contribution that survives even when event data for a player is sparse. The
/// caller calibrates the source-neutral attack/defence indices from role-specific ability vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerTeamImpactProfile {
    pub player: PlayerId,
    pub attack_strength: f64,
    pub defense_strength: f64,
    pub attack_reference: f64,
    pub defense_reference: f64,
    pub attack_log_weight: f64,
    pub defense_log_weight: f64,
}

impl PlayerTeamImpactProfile {
    fn valid(self) -> bool {
        non_negative(self.attack_strength)
            && non_negative(self.defense_strength)
            && self.attack_reference.is_finite()
            && self.attack_reference > 0.0
            && self.defense_reference.is_finite()
            && self.defense_reference > 0.0
            && self.attack_log_weight.is_finite()
            && self.defense_log_weight.is_finite()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamMetricModifier {
    pub metric: MetricId,
    /// Tactical/system multiplier on this team's own players' production.
    pub production_multiplier: f64,
    /// Tactical/system multiplier on the opponent's production of the same metric.
    pub opponent_suppression_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeamMatchContext {
    pub club: ClubId,
    /// Residual team attack/defence state not already represented by individual player processes.
    pub attack_log_adjustment: f64,
    pub defense_log_adjustment: f64,
    pub metric_modifiers: Vec<TeamMetricModifier>,
}

impl TeamMatchContext {
    fn production_multiplier(&self, metric: MetricId) -> f64 {
        self.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| value.production_multiplier).product()
    }

    fn suppression_multiplier(&self, metric: MetricId) -> f64 {
        self.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| value.opponent_suppression_multiplier).product()
    }

    fn valid(&self) -> bool {
        self.attack_log_adjustment.is_finite()
            && self.defense_log_adjustment.is_finite()
            && self.metric_modifiers.iter().all(|value| {
                value.production_multiplier.is_finite()
                    && value.production_multiplier > 0.0
                    && value.opponent_suppression_multiplier.is_finite()
                    && value.opponent_suppression_multiplier > 0.0
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricEnvironmentModifier {
    pub metric: MetricId,
    pub home_multiplier: f64,
    pub away_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchEnvironment {
    /// Venue/rest/travel/weather/referee residuals can enter here after causal feature construction.
    pub home_attack_log_adjustment: f64,
    pub away_attack_log_adjustment: f64,
    pub metric_modifiers: Vec<MetricEnvironmentModifier>,
}

impl MatchEnvironment {
    fn metric_multiplier(&self, metric: MetricId, is_home: bool) -> f64 {
        self.metric_modifiers.iter().filter(|value| value.metric == metric).map(|value| {
            if is_home { value.home_multiplier } else { value.away_multiplier }
        }).product()
    }

    fn valid(&self) -> bool {
        self.home_attack_log_adjustment.is_finite()
            && self.away_attack_log_adjustment.is_finite()
            && self.metric_modifiers.iter().all(|value| {
                value.home_multiplier.is_finite() && value.home_multiplier > 0.0
                    && value.away_multiplier.is_finite() && value.away_multiplier > 0.0
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoalProcessParameters {
    pub goals_per_team_match: f64,
    pub home_advantage_multiplier: f64,
    pub path_log_rate_sigma: f64,
    /// Shared-tempo correlation between per-path home/away log goal rates. Zero keeps the
    /// legacy independent sampling bit-identical; nonzero is opt-in pending OOS evidence.
    pub path_rate_correlation: f64,
    /// Prevents one bad calibration coefficient from creating absurd exponential intensities.
    pub max_abs_log_adjustment: f64,
    /// Score-state feedback is applied over equal segments; six corresponds to 15-minute blocks.
    pub segments: u8,
    pub trailing_attack_log_boost: f64,
    pub leading_attack_log_suppression: f64,
}

impl GoalProcessParameters {
    pub fn valid(self) -> bool {
        self.goals_per_team_match.is_finite()
            && self.goals_per_team_match > 0.0
            && self.home_advantage_multiplier.is_finite()
            && self.home_advantage_multiplier > 0.0
            && non_negative(self.path_log_rate_sigma)
            && self.path_rate_correlation.is_finite()
            && (-0.5..=0.9).contains(&self.path_rate_correlation)
            && self.max_abs_log_adjustment.is_finite()
            && self.max_abs_log_adjustment > 0.0
            && self.segments > 0
            && self.trailing_attack_log_boost.is_finite()
            && self.trailing_attack_log_boost >= 0.0
            && self.leading_attack_log_suppression.is_finite()
            && self.leading_attack_log_suppression >= 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecificMatchSimulationInput {
    pub match_id: MatchId,
    pub home: ClubId,
    pub away: ClubId,
    pub home_lineup: LineupProjectionInput,
    pub away_lineup: LineupProjectionInput,
    pub player_states: Vec<PlayerMatchupState>,
    pub metric_models: Vec<PlayerMetricImpactModel>,
    pub latent_team_impacts: Vec<PlayerTeamImpactProfile>,
    pub matchup_rules: Vec<MatchupRule>,
    pub synergies: Vec<PairwiseSynergy>,
    pub home_context: TeamMatchContext,
    pub away_context: TeamMatchContext,
    pub environment: MatchEnvironment,
    pub goal_process: GoalProcessParameters,
    pub matchup_spatial_sigma: f64,
    pub max_player_rate_log_adjustment: f64,
}

impl SpecificMatchSimulationInput {
    pub fn validate(&self) -> bool {
        if self.home == self.away
            || self.home_lineup.club != self.home
            || self.away_lineup.club != self.away
            || self.home_context.club != self.home
            || self.away_context.club != self.away
            || !self.home_lineup.validate()
            || !self.away_lineup.validate()
            || !self.home_context.valid()
            || !self.away_context.valid()
            || !self.environment.valid()
            || !self.goal_process.valid()
            || !self.matchup_spatial_sigma.is_finite()
            || self.matchup_spatial_sigma <= 0.0
            || !self.max_player_rate_log_adjustment.is_finite()
            || self.max_player_rate_log_adjustment <= 0.0
            || self.metric_models.iter().any(|value| !value.valid())
            || self.latent_team_impacts.iter().any(|value| !value.valid())
            || self.synergies.iter().any(|value| !value.strength.is_finite())
        {
            return false;
        }
        let mut metric_keys = BTreeSet::new();
        if !self.metric_models.iter().all(|value| metric_keys.insert((value.player, value.metric))) { return false; }
        let mut impact_players = BTreeSet::new();
        if !self.latent_team_impacts.iter().all(|value| impact_players.insert(value.player)) { return false; }
        let state_players: BTreeSet<PlayerId> = self.player_states.iter().map(|value| value.player).collect();
        let required = self.home_lineup.candidates.iter().chain(self.away_lineup.candidates.iter()).map(|value| value.player);
        required.into_iter().all(|player| state_players.contains(&player))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerMetricMatchForecast {
    pub player: PlayerId,
    pub metric: MetricId,
    pub start_probability: Probability,
    /// Includes zero on paths where the player is not in the projected starting XI.
    pub unconditional_total: DistributionSummary,
    pub conditional_on_start_total: Option<DistributionSummary>,
    pub conditional_on_start_per90: Option<DistributionSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerMatchInfluenceSummary {
    pub player: PlayerId,
    pub start_probability: Probability,
    pub expected_minutes_unconditional: f64,
    /// Direct expected contribution to the team's log scoring/defensive intensity.
    pub expected_attack_log_contribution: f64,
    pub expected_defense_log_contribution: f64,
    /// Mean event-rate modifiers conditional on modeled starter metrics.
    pub mean_matchup_log_modifier: f64,
    pub mean_teammate_log_modifier: f64,
    pub mean_role_log_modifier: f64,
    pub mean_fatigue_log_modifier: f64,
    pub mean_system_environment_log_modifier: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecificMatchForecast {
    pub outcome: MatchOutcomeForecast,
    pub home_lineup: LineupProjection,
    pub away_lineup: LineupProjection,
    pub player_metrics: Vec<PlayerMetricMatchForecast>,
    pub influences: Vec<PlayerMatchInfluenceSummary>,
    pub paths: u32,
}

#[derive(Default)]
struct InfluenceAccumulator {
    starts: u32,
    minutes: f64,
    attack: f64,
    defense: f64,
    matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    modifier_observations: u32,
}

#[derive(Default)]
struct MetricSamples {
    unconditional: Vec<f64>,
    conditional: Vec<f64>,
    conditional_rates: Vec<f64>,
}

struct PathMetricValue {
    player: PlayerId,
    metric: MetricId,
    total: f64,
    per90: f64,
}

struct PathPlayerContribution {
    player: PlayerId,
    attack: f64,
    defense: f64,
    matchup: f64,
    teammate: f64,
    role: f64,
    fatigue: f64,
    system: f64,
    modifier_observations: u32,
}

struct PathTeamOutput {
    attack: f64,
    defense: f64,
    metrics: Vec<PathMetricValue>,
    players: Vec<PathPlayerContribution>,
}

/// Full specific-match Monte Carlo. Every path samples the uncertain starting XIs first, then builds
/// metric-specific player↔player interactions, samples individual player events, aggregates those
/// events into team attack/defence state, and only then generates the score path.
pub fn simulate_specific_match(
    input: &SpecificMatchSimulationInput,
    paths: u32,
    seed: u64,
) -> Option<SpecificMatchForecast> {
    if paths == 0 || !input.validate() { return None; }
    let mut rng = Rng64::new(seed);
    let mut home_goals = Vec::with_capacity(paths as usize);
    let mut away_goals = Vec::with_capacity(paths as usize);
    let mut home_wins = 0_u32;
    let mut draws = 0_u32;
    let mut away_wins = 0_u32;
    let mut influence = BTreeMap::<PlayerId, InfluenceAccumulator>::new();
    let mut samples = BTreeMap::<(PlayerId, MetricId), MetricSamples>::new();
    for model in &input.metric_models { samples.entry((model.player, model.metric)).or_default(); }

    for _ in 0..paths {
        let home_lineup = sample_lineup(&input.home_lineup, &mut rng)?;
        let away_lineup = sample_lineup(&input.away_lineup, &mut rng)?;
        for starter in home_lineup.starters.iter().chain(away_lineup.starters.iter()) {
            let entry = influence.entry(starter.player).or_default();
            entry.starts = entry.starts.saturating_add(1);
            entry.minutes += starter.minutes;
        }

        let home_output = simulate_team_path(input, &home_lineup, &away_lineup, true, &mut rng)?;
        let away_output = simulate_team_path(input, &away_lineup, &home_lineup, false, &mut rng)?;
        let mut path_metric_values = BTreeMap::<(PlayerId, MetricId), (f64, f64)>::new();
        for value in home_output.metrics.iter().chain(away_output.metrics.iter()) {
            path_metric_values.insert((value.player, value.metric), (value.total, value.per90));
        }
        for ((player, metric), accumulator) in &mut samples {
            if let Some((total, rate)) = path_metric_values.get(&(*player, *metric)) {
                accumulator.unconditional.push(*total);
                accumulator.conditional.push(*total);
                accumulator.conditional_rates.push(*rate);
            } else {
                accumulator.unconditional.push(0.0);
            }
        }
        for value in home_output.players.iter().chain(away_output.players.iter()) {
            let entry = influence.entry(value.player).or_default();
            entry.attack += value.attack;
            entry.defense += value.defense;
            entry.matchup += value.matchup;
            entry.teammate += value.teammate;
            entry.role += value.role;
            entry.fatigue += value.fatigue;
            entry.system += value.system;
            entry.modifier_observations = entry.modifier_observations.saturating_add(value.modifier_observations);
        }

        let home_adjustment = (
            input.environment.home_attack_log_adjustment
                + input.home_context.attack_log_adjustment
                - input.away_context.defense_log_adjustment
                + home_output.attack
                - away_output.defense
        ).clamp(-input.goal_process.max_abs_log_adjustment, input.goal_process.max_abs_log_adjustment);
        let away_adjustment = (
            input.environment.away_attack_log_adjustment
                + input.away_context.attack_log_adjustment
                - input.home_context.defense_log_adjustment
                + away_output.attack
                - home_output.defense
        ).clamp(-input.goal_process.max_abs_log_adjustment, input.goal_process.max_abs_log_adjustment);
        let home_mean = input.goal_process.goals_per_team_match * input.goal_process.home_advantage_multiplier * home_adjustment.exp();
        let away_mean = input.goal_process.goals_per_team_match * away_adjustment.exp();
        let (home_path_rate, away_path_rate) = sample_correlated_log_rates(
            home_mean,
            away_mean,
            input.goal_process.path_log_rate_sigma,
            input.goal_process.path_rate_correlation,
            &mut rng,
        );
        let (home, away) = simulate_score_state(home_path_rate, away_path_rate, input.goal_process, &mut rng);
        home_goals.push(f64::from(home));
        away_goals.push(f64::from(away));
        match home.cmp(&away) {
            std::cmp::Ordering::Greater => home_wins = home_wins.saturating_add(1),
            std::cmp::Ordering::Equal => draws = draws.saturating_add(1),
            std::cmp::Ordering::Less => away_wins = away_wins.saturating_add(1),
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

    let home_lineup = project_lineup(&input.home_lineup, paths, seed ^ 0x484f_4d45_4c49_4e45)?;
    let away_lineup = project_lineup(&input.away_lineup, paths, seed ^ 0x4157_4159_4c49_4e45)?;
    let player_metrics = samples.into_iter().map(|((player, metric), value)| {
        let start_probability = influence.get(&player).map(|entry| Probability::clamped(entry.starts as f32 / denominator)).unwrap_or(Probability::ZERO);
        Some(PlayerMetricMatchForecast {
            player,
            metric,
            start_probability,
            unconditional_total: summarize(&value.unconditional)?,
            conditional_on_start_total: summarize(&value.conditional),
            conditional_on_start_per90: summarize(&value.conditional_rates),
        })
    }).collect::<Option<Vec<_>>>()?;
    let influences = influence.into_iter().map(|(player, value)| {
        let modifier_denominator = f64::from(value.modifier_observations.max(1));
        PlayerMatchInfluenceSummary {
            player,
            start_probability: Probability::clamped(value.starts as f32 / denominator),
            expected_minutes_unconditional: value.minutes / f64::from(paths),
            expected_attack_log_contribution: value.attack / f64::from(paths),
            expected_defense_log_contribution: value.defense / f64::from(paths),
            mean_matchup_log_modifier: value.matchup / modifier_denominator,
            mean_teammate_log_modifier: value.teammate / modifier_denominator,
            mean_role_log_modifier: value.role / modifier_denominator,
            mean_fatigue_log_modifier: value.fatigue / modifier_denominator,
            mean_system_environment_log_modifier: value.system / modifier_denominator,
        }
    }).collect();

    Some(SpecificMatchForecast { outcome, home_lineup, away_lineup, player_metrics, influences, paths })
}

fn simulate_team_path(
    input: &SpecificMatchSimulationInput,
    own: &SampledLineup,
    opponent: &SampledLineup,
    is_home: bool,
    rng: &mut Rng64,
) -> Option<PathTeamOutput> {
    let (own_context, opponent_context) = if is_home {
        (&input.home_context, &input.away_context)
    } else {
        (&input.away_context, &input.home_context)
    };
    let impact_map: BTreeMap<PlayerId, PlayerTeamImpactProfile> = input.latent_team_impacts.iter().copied().map(|value| (value.player, value)).collect();
    let mut player_map = BTreeMap::<PlayerId, PathPlayerContribution>::new();
    let mut attack = 0.0;
    let mut defense = 0.0;
    let mut metrics = Vec::new();

    for starter in &own.starters {
        let mut player_attack = 0.0;
        let mut player_defense = 0.0;
        if let Some(profile) = impact_map.get(&starter.player) {
            let exposure = starter.minutes / 90.0;
            let attack_contribution = profile.attack_log_weight * (profile.attack_strength / profile.attack_reference - 1.0) * exposure;
            let defense_contribution = profile.defense_log_weight * (profile.defense_strength / profile.defense_reference - 1.0) * exposure;
            attack += attack_contribution;
            defense += defense_contribution;
            player_attack += attack_contribution;
            player_defense += defense_contribution;
        }
        player_map.insert(starter.player, PathPlayerContribution {
            player: starter.player,
            attack: player_attack,
            defense: player_defense,
            matchup: 0.0,
            teammate: 0.0,
            role: 0.0,
            fatigue: 0.0,
            system: 0.0,
            modifier_observations: 0,
        });
    }

    for model in input.metric_models.iter().filter(|model| own.starter(model.player).is_some()) {
        let starter = own.starter(model.player)?;
        let interaction = player_metric_interaction_context(
            model.player,
            model.metric,
            own,
            opponent,
            &input.player_states,
            &input.matchup_rules,
            &input.synergies,
            input.matchup_spatial_sigma,
        )?;
        let system_multiplier = own_context.production_multiplier(model.metric)
            * opponent_context.suppression_multiplier(model.metric)
            * input.environment.metric_multiplier(model.metric, is_home);
        if !system_multiplier.is_finite() || system_multiplier <= 0.0 { return None; }
        let matchup_log = model.matchup_sensitivity * interaction.opponent_advantage;
        let teammate_log = model.teammate_sensitivity * interaction.teammate_support;
        let role_log = model.role_fit_sensitivity * (starter.role_fit - 1.0);
        let fatigue_log = -model.fatigue_sensitivity * starter.fatigue;
        let system_log = system_multiplier.ln();
        let rate_log_adjustment = (matchup_log + teammate_log + role_log + fatigue_log + system_log)
            .clamp(-input.max_player_rate_log_adjustment, input.max_player_rate_log_adjustment);
        let multiplier = rate_log_adjustment.exp();
        let mean = model.mean_per90 * multiplier;
        let latent = sample_non_negative_normal(mean, model.epistemic_sigma * multiplier, rng);
        let realized_rate = sample_non_negative_normal(latent, model.variance_per90.sqrt() * multiplier, rng);
        let exposure = starter.minutes / 90.0;
        let total = match model.family {
            StatFamily::ContinuousNonNegative => realized_rate * exposure,
            StatFamily::Count => f64::from(poisson(realized_rate * exposure, rng)),
        };
        let realized_per90 = match model.family {
            StatFamily::ContinuousNonNegative => realized_rate,
            StatFamily::Count => if exposure > 0.0 { total / exposure } else { 0.0 },
        };
        let centered = (realized_per90 / model.reference_per90 - 1.0) * exposure;
        let attack_contribution = model.attack_log_weight * centered;
        let defense_contribution = model.defense_log_weight * centered;
        attack += attack_contribution;
        defense += defense_contribution;
        metrics.push(PathMetricValue { player: model.player, metric: model.metric, total, per90: realized_per90 });
        let player = player_map.get_mut(&model.player)?;
        player.attack += attack_contribution;
        player.defense += defense_contribution;
        player.matchup += matchup_log;
        player.teammate += teammate_log;
        player.role += role_log;
        player.fatigue += fatigue_log;
        player.system += system_log;
        player.modifier_observations = player.modifier_observations.saturating_add(1);
    }

    Some(PathTeamOutput { attack, defense, metrics, players: player_map.into_values().collect() })
}

/// Score feedback is endogenous: once a team leads/trails, subsequent segment intensities change.
/// Red cards/substitutions can be added as additional state transitions without changing the
/// lineup/matchup/player-event layers above this primitive.
fn simulate_score_state(home_rate: f64, away_rate: f64, parameters: GoalProcessParameters, rng: &mut Rng64) -> (u32, u32) {
    let segment_share = 1.0 / f64::from(parameters.segments);
    let mut home_goals = 0_u32;
    let mut away_goals = 0_u32;
    for _ in 0..parameters.segments {
        let (home_state, away_state) = match home_goals.cmp(&away_goals) {
            std::cmp::Ordering::Less => (parameters.trailing_attack_log_boost, -parameters.leading_attack_log_suppression),
            std::cmp::Ordering::Greater => (-parameters.leading_attack_log_suppression, parameters.trailing_attack_log_boost),
            std::cmp::Ordering::Equal => (0.0, 0.0),
        };
        home_goals = home_goals.saturating_add(poisson(home_rate * home_state.exp() * segment_share, rng));
        away_goals = away_goals.saturating_add(poisson(away_rate * away_state.exp() * segment_share, rng));
    }
    (home_goals, away_goals)
}

fn sample_non_negative_normal(mean: f64, sigma: f64, rng: &mut Rng64) -> f64 {
    if sigma <= f64::EPSILON { mean.max(0.0) } else { (mean + sigma * rng.normal()).max(0.0) }
}

fn sample_log_rate(mean: f64, log_sigma: f64, rng: &mut Rng64) -> f64 {
    if log_sigma <= f64::EPSILON { return mean.max(f64::MIN_POSITIVE); }
    (mean.ln() - 0.5 * log_sigma.powi(2) + log_sigma * rng.normal()).exp()
}

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
    let home_log = home_mean.ln() - 0.5 * sigma.powi(2)
        + shared_scale * shared
        + idiosyncratic_scale * home_idiosyncratic;
    let away_log = away_mean.ln() - 0.5 * sigma.powi(2)
        + rho.signum() * shared_scale * shared
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
    let variance = if sorted.len() > 1 {
        sorted.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (sorted.len() - 1) as f64
    } else { 0.0 };
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
    if lower == upper { sorted[lower] } else {
        let fraction = position - lower as f64;
        sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
    }
}

fn non_negative(value: f64) -> bool { value.is_finite() && value >= 0.0 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineup::{
        FormationPrior, FormationTemplate, LineupCandidate, LineupSamplingParameters, RoleFit, TacticalSlot,
    };
    use crate::matchup::AbilityDimension;
    use footy_core::RoleId;
    use footy_state::AbilityVector;

    fn formation() -> FormationTemplate {
        FormationTemplate {
            id: 433,
            slots: (0..11).map(|slot| TacticalSlot {
                slot,
                role: RoleId(slot as u16 + 1),
                x: if slot == 0 { 0.05 } else { 0.2 + 0.06 * f64::from(slot) },
                y: f64::from(slot % 5) / 4.0,
            }).collect(),
        }
    }

    fn lineup(club: u32, offset: u64) -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(club),
            formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (0..14).map(|index| {
                let player = offset + index;
                LineupCandidate {
                    player: PlayerId(player), start_probability: if index < 11 { 0.85 } else { 0.2 }, availability_probability: 1.0,
                    ability_index: if club == 1 { 14.0 } else { 11.0 }, fatigue: 0.0, expected_minutes_if_start: 82.0,
                    minutes_sigma: 2.0, substitute_probability: 0.1,
                    role_fits: (1..=11).map(|role| RoleFit { role: RoleId(role), fit: if role == index as u16 + 1 { 1.0 } else { 0.2 } }).collect(),
                }
            }).collect(),
            locked_starters: Vec::new(),
            parameters: LineupSamplingParameters::default(),
        }
    }

    fn ability(value: f32) -> AbilityVector {
        AbilityVector { technical: value, ball_progression: value, creativity: value, finishing: value, defending: value, aerial: value, physical: value, decision_making: value, mentality: value, goalkeeping: value }
    }

    fn input() -> SpecificMatchSimulationInput {
        let home = lineup(1, 1);
        let away = lineup(2, 101);
        let all_players: Vec<PlayerId> = home.candidates.iter().chain(away.candidates.iter()).map(|value| value.player).collect();
        let metric = MetricId(1);
        SpecificMatchSimulationInput {
            match_id: MatchId(99), home: ClubId(1), away: ClubId(2), home_lineup: home, away_lineup: away,
            player_states: all_players.iter().map(|player| PlayerMatchupState { player: *player, ability: ability(if player.0 < 100 { 14.0 } else { 11.0 }) }).collect(),
            metric_models: all_players.iter().map(|player| PlayerMetricImpactModel {
                player: *player, metric, family: StatFamily::ContinuousNonNegative,
                mean_per90: if player.0 < 100 { 1.1 } else { 0.9 }, variance_per90: 0.04, epistemic_sigma: 0.03,
                reference_per90: 1.0, attack_log_weight: 0.015, defense_log_weight: 0.0,
                matchup_sensitivity: 0.2, teammate_sensitivity: 0.1, role_fit_sensitivity: 0.2, fatigue_sensitivity: 0.2,
            }).collect(),
            latent_team_impacts: all_players.iter().map(|player| PlayerTeamImpactProfile {
                player: *player, attack_strength: if player.0 < 100 { 1.1 } else { 0.9 }, defense_strength: 1.0,
                attack_reference: 1.0, defense_reference: 1.0, attack_log_weight: 0.01, defense_log_weight: 0.01,
            }).collect(),
            matchup_rules: vec![MatchupRule { metric: Some(metric), attacker_dimension: AbilityDimension::Technical, defender_dimension: AbilityDimension::Defending, weight: 1.0, scale: 20.0 }],
            synergies: Vec::new(),
            home_context: TeamMatchContext { club: ClubId(1), attack_log_adjustment: 0.0, defense_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            away_context: TeamMatchContext { club: ClubId(2), attack_log_adjustment: 0.0, defense_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            environment: MatchEnvironment { home_attack_log_adjustment: 0.0, away_attack_log_adjustment: 0.0, metric_modifiers: Vec::new() },
            goal_process: GoalProcessParameters {
                goals_per_team_match: 1.3, home_advantage_multiplier: 1.08, path_log_rate_sigma: 0.08,
                path_rate_correlation: 0.0,
                max_abs_log_adjustment: 1.5, segments: 6, trailing_attack_log_boost: 0.08, leading_attack_log_suppression: 0.05,
            },
            matchup_spatial_sigma: 0.25,
            max_player_rate_log_adjustment: 1.0,
        }
    }

    #[test]
    fn specific_match_simulation_is_reproducible() {
        let first = simulate_specific_match(&input(), 500, 42).unwrap();
        let second = simulate_specific_match(&input(), 500, 42).unwrap();
        assert_eq!(first, second);
        let sum = first.outcome.home_win.get() + first.outcome.draw.get() + first.outcome.away_win.get();
        assert!((sum - 1.0).abs() < 1e-6);
        assert_eq!(first.home_lineup.paths, 500);
    }

    #[test]
    fn stronger_home_player_state_reaches_result_layer() {
        let value = simulate_specific_match(&input(), 2_000, 7).unwrap();
        assert!(value.outcome.home_win.get() > value.outcome.away_win.get());
        assert!(value.influences.iter().any(|entry| entry.player.0 < 100 && entry.expected_attack_log_contribution > 0.0));
    }
}
