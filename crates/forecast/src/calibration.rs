use crate::{
    MatchSimulationInput, PlayerStatSimulationInput, StatFamily, TeamStatSimulationInput,
};
use footy_core::{ClubId, MatchId, MetricId};
use footy_performance::{PerformanceWindow, TeamPerformanceWindow};

/// Empirical-Bayes prior used to stabilize a sporting statistic when the observed window is short.
/// `strength` is measured in effective observations, so a strength of 5 means the prior carries
/// the same weight as five observed matches/rates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatPrior {
    pub mean: f64,
    pub variance: f64,
    pub strength: f64,
}

impl StatPrior {
    pub fn new(mean: f64, variance: f64, strength: f64) -> Option<Self> {
        (mean.is_finite()
            && mean >= 0.0
            && variance.is_finite()
            && variance >= 0.0
            && strength.is_finite()
            && strength >= 0.0)
            .then_some(Self { mean, variance, strength })
    }
}

/// Convert a causal player performance window into Monte Carlo parameters. The observed EWMA rate
/// is shrunk toward a supplied peer/league prior, while aleatoric match variance and epistemic
/// uncertainty remain separate inputs to the simulator.
pub fn calibrate_player_stat(
    window: &PerformanceWindow,
    metric: MetricId,
    family: StatFamily,
    prior: StatPrior,
    expected_minutes: f64,
    minutes_sigma: f64,
) -> Option<PlayerStatSimulationInput> {
    if !expected_minutes.is_finite() || expected_minutes < 0.0 || !minutes_sigma.is_finite() || minutes_sigma < 0.0 {
        return None;
    }
    let observed = window.metric(metric)?;
    let observed_mean = observed.ewma_per90.or(observed.per90)?;
    if !observed_mean.is_finite() || observed_mean < 0.0 { return None; }
    let n = f64::from(observed.observations);
    let mean_per90 = shrink(observed_mean, prior.mean, n, prior.strength)?;
    let variance_per90 = shrink(observed.match_rate_variance.unwrap_or(prior.variance), prior.variance, n, prior.strength)?;
    let epistemic_sigma = posterior_standard_error(variance_per90, n + prior.strength);
    Some(PlayerStatSimulationInput {
        player: window.player,
        metric,
        family,
        mean_per90,
        variance_per90,
        expected_minutes,
        minutes_sigma,
        epistemic_sigma,
    })
}

/// Convert a causal team window into Monte Carlo parameters using the same shrinkage rule as
/// individual player statistics.
pub fn calibrate_team_stat(
    window: &TeamPerformanceWindow,
    metric: MetricId,
    family: StatFamily,
    prior: StatPrior,
    expected_matches: u16,
) -> Option<TeamStatSimulationInput> {
    if expected_matches == 0 { return None; }
    let observed = window.metric(metric)?;
    if !observed.ewma_per_match.is_finite() || observed.ewma_per_match < 0.0 { return None; }
    let n = f64::from(observed.observations);
    let mean_per_match = shrink(observed.ewma_per_match, prior.mean, n, prior.strength)?;
    let variance_per_match = shrink(observed.match_variance.unwrap_or(prior.variance), prior.variance, n, prior.strength)?;
    let epistemic_sigma = posterior_standard_error(variance_per_match, n + prior.strength);
    Some(TeamStatSimulationInput {
        club: window.club,
        metric,
        family,
        mean_per_match,
        variance_per_match,
        expected_matches,
        epistemic_sigma,
    })
}

/// League/context prior for turning observed team form into scoring intensities. The caller owns
/// the league baseline instead of baking a Premier-League-like constant into the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchGoalPrior {
    pub goals_per_team_match: f64,
    pub strength_matches: f64,
    pub home_advantage_multiplier: f64,
    pub log_rate_sigma_floor: f64,
}

impl MatchGoalPrior {
    pub fn new(
        goals_per_team_match: f64,
        strength_matches: f64,
        home_advantage_multiplier: f64,
        log_rate_sigma_floor: f64,
    ) -> Option<Self> {
        (goals_per_team_match.is_finite()
            && goals_per_team_match > 0.0
            && strength_matches.is_finite()
            && strength_matches > 0.0
            && home_advantage_multiplier.is_finite()
            && home_advantage_multiplier > 0.0
            && log_rate_sigma_floor.is_finite()
            && log_rate_sigma_floor >= 0.0)
            .then_some(Self {
                goals_per_team_match,
                strength_matches,
                home_advantage_multiplier,
                log_rate_sigma_floor,
            })
    }
}

/// Seed match Monte Carlo directly from causal team form. Attack and defensive rates are
/// independently shrunk to the supplied league prior, then combined geometrically so neither a
/// team's attack nor its opponent's defence dominates the forecast.
pub fn calibrate_match_from_form(
    match_id: MatchId,
    home: &TeamPerformanceWindow,
    away: &TeamPerformanceWindow,
    prior: MatchGoalPrior,
) -> Option<MatchSimulationInput> {
    if home.club == away.club { return None; }
    let home_attack = smoothed_goal_rate(home.form.goals_for, home.form.matches, prior)?;
    let home_defence = smoothed_goal_rate(home.form.goals_against, home.form.matches, prior)?;
    let away_attack = smoothed_goal_rate(away.form.goals_for, away.form.matches, prior)?;
    let away_defence = smoothed_goal_rate(away.form.goals_against, away.form.matches, prior)?;

    let home_goal_rate = (home_attack * away_defence).sqrt() * prior.home_advantage_multiplier;
    let away_goal_rate = (away_attack * home_defence).sqrt();
    let home_rate_sigma = log_rate_uncertainty(
        home_goal_rate,
        f64::from(home.form.matches.min(away.form.matches)) + prior.strength_matches,
        prior.log_rate_sigma_floor,
    );
    let away_rate_sigma = log_rate_uncertainty(
        away_goal_rate,
        f64::from(home.form.matches.min(away.form.matches)) + prior.strength_matches,
        prior.log_rate_sigma_floor,
    );

    Some(MatchSimulationInput {
        match_id,
        home: home.club,
        away: away.club,
        home_goal_rate,
        away_goal_rate,
        home_rate_sigma,
        away_rate_sigma,
    })
}

/// Convenience for callers that have canonical club IDs but construct team windows elsewhere.
pub fn validate_match_clubs(home: ClubId, away: ClubId) -> bool { home != away }

fn shrink(observed: f64, prior: f64, observations: f64, prior_strength: f64) -> Option<f64> {
    if !observed.is_finite()
        || observed < 0.0
        || !prior.is_finite()
        || prior < 0.0
        || !observations.is_finite()
        || observations < 0.0
        || !prior_strength.is_finite()
        || prior_strength < 0.0
    {
        return None;
    }
    let denominator = observations + prior_strength;
    if denominator <= f64::EPSILON { Some(observed) }
    else { Some((observed * observations + prior * prior_strength) / denominator) }
}

fn posterior_standard_error(variance: f64, effective_observations: f64) -> f64 {
    if variance <= 0.0 || effective_observations <= f64::EPSILON { 0.0 }
    else { (variance / effective_observations).sqrt() }
}

fn smoothed_goal_rate(goals: u32, matches: u32, prior: MatchGoalPrior) -> Option<f64> {
    let denominator = f64::from(matches) + prior.strength_matches;
    if denominator <= f64::EPSILON { return None; }
    Some((f64::from(goals) + prior.goals_per_team_match * prior.strength_matches) / denominator)
}

fn log_rate_uncertainty(rate: f64, effective_matches: f64, floor: f64) -> f64 {
    if rate <= f64::EPSILON || effective_matches <= f64::EPSILON { return floor; }
    // Poisson mean uncertainty transformed to an equivalent lognormal sigma.
    let variance = rate / effective_matches;
    let sigma = (1.0 + variance / rate.powi(2)).ln().sqrt();
    sigma.max(floor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::{ClubId, Date, MetricId, PlayerId};
    use footy_performance::{CoverageSummary, PerformanceWindow, TeamForm, TeamPerformanceWindow, TeamWindowMetric, WindowMetric};

    #[test]
    fn player_calibration_shrinks_short_samples() {
        let window = PerformanceWindow {
            player: PlayerId(1), as_of: Date(10), horizon_days: 90,
            metrics: vec![WindowMetric { metric: MetricId(1), total: 4.0, per90: Some(2.0), ewma_per90: Some(2.0), trend_per90: None, match_rate_variance: Some(1.0), observations: 2 }],
            coverage: CoverageSummary { matches: 2, minutes: 180, metric_observations: 2, low_sample: true },
        };
        let prior = StatPrior::new(1.0, 0.5, 4.0).unwrap();
        let input = calibrate_player_stat(&window, MetricId(1), StatFamily::Count, prior, 900.0, 90.0).unwrap();
        assert!(input.mean_per90 > 1.0 && input.mean_per90 < 2.0);
        assert!(input.epistemic_sigma > 0.0);
    }

    fn team(club: u32, matches: u32, goals_for: u32, goals_against: u32) -> TeamPerformanceWindow {
        TeamPerformanceWindow {
            club: ClubId(club), as_of: Date(10), horizon_days: 365, metrics: vec![TeamWindowMetric { metric: MetricId(1), total: 10.0, per_match: 1.0, ewma_per_match: 1.0, trend_per_match: None, match_variance: Some(0.2), observations: matches }],
            form: TeamForm { matches, wins: 0, draws: 0, losses: 0, goals_for, goals_against, points: 0 },
        }
    }

    #[test]
    fn team_calibration_uses_same_prior_mechanism() {
        let window = team(1, 10, 15, 10);
        let prior = StatPrior::new(0.8, 0.4, 5.0).unwrap();
        let input = calibrate_team_stat(&window, MetricId(1), StatFamily::ContinuousNonNegative, prior, 20).unwrap();
        assert!(input.mean_per_match > 0.8 && input.mean_per_match < 1.0);
    }

    #[test]
    fn match_calibration_adds_home_advantage_after_shrinkage() {
        let home = team(1, 8, 16, 8);
        let away = team(2, 8, 8, 16);
        let prior = MatchGoalPrior::new(1.3, 5.0, 1.1, 0.05).unwrap();
        let input = calibrate_match_from_form(MatchId(7), &home, &away, prior).unwrap();
        assert!(input.home_goal_rate > input.away_goal_rate);
        assert!(input.home_rate_sigma >= 0.05);
    }
}
