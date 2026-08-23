use crate::Rng64;
use footy_core::{ClubId, MatchId, MetricId, PlayerId, Probability};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatFamily {
    ContinuousNonNegative,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistributionSummary {
    pub p10: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub mean: f64,
    pub standard_error: f64,
    pub paths: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerStatSimulationInput {
    pub player: PlayerId,
    pub metric: MetricId,
    pub family: StatFamily,
    /// Expected metric production per 90 minutes at the forecast origin.
    pub mean_per90: f64,
    /// Match-to-match variance of the per-90 rate.
    pub variance_per90: f64,
    /// Expected total minutes over the simulated horizon.
    pub expected_minutes: f64,
    /// Uncertainty in total minutes over the horizon.
    pub minutes_sigma: f64,
    /// Standard deviation of uncertainty in the latent true rate itself.
    pub epistemic_sigma: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerStatForecast {
    pub player: PlayerId,
    pub metric: MetricId,
    pub total: DistributionSummary,
    pub per90: DistributionSummary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamStatSimulationInput {
    pub club: ClubId,
    pub metric: MetricId,
    pub family: StatFamily,
    pub mean_per_match: f64,
    pub variance_per_match: f64,
    pub expected_matches: u16,
    pub epistemic_sigma: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamStatForecast {
    pub club: ClubId,
    pub metric: MetricId,
    pub total: DistributionSummary,
    pub per_match: DistributionSummary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchSimulationInput {
    pub match_id: MatchId,
    pub home: ClubId,
    pub away: ClubId,
    /// Expected home goals after team strength, venue, availability, and opponent context.
    pub home_goal_rate: f64,
    /// Expected away goals after team strength, venue, availability, and opponent context.
    pub away_goal_rate: f64,
    /// Log-scale uncertainty in the home scoring intensity.
    pub home_rate_sigma: f64,
    /// Log-scale uncertainty in the away scoring intensity.
    pub away_rate_sigma: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchOutcomeForecast {
    pub match_id: MatchId,
    pub home: ClubId,
    pub away: ClubId,
    pub home_win: Probability,
    pub draw: Probability,
    pub away_win: Probability,
    pub home_goals: DistributionSummary,
    pub away_goals: DistributionSummary,
    pub home_expected_points: f64,
    pub away_expected_points: f64,
    pub paths: u32,
}

/// Forecast an individual player's counting or continuous statistic with the same deterministic
/// Monte Carlo primitive used by valuation. Rate uncertainty and playing-time uncertainty are
/// sampled separately so a bench-risk forecast widens for the right reason.
pub fn simulate_player_stat(
    input: PlayerStatSimulationInput,
    paths: u32,
    seed: u64,
) -> Option<PlayerStatForecast> {
    validate_paths(paths)?;
    if !valid_non_negative(input.mean_per90)
        || !valid_non_negative(input.variance_per90)
        || !valid_non_negative(input.expected_minutes)
        || !valid_non_negative(input.minutes_sigma)
        || !valid_non_negative(input.epistemic_sigma)
    {
        return None;
    }

    let mut rng = Rng64::new(seed);
    let mut totals = Vec::with_capacity(paths as usize);
    let mut rates = Vec::with_capacity(paths as usize);
    for _ in 0..paths {
        let latent_rate = sample_non_negative_normal(input.mean_per90, input.epistemic_sigma, &mut rng);
        let realized_rate = sample_non_negative_normal(latent_rate, input.variance_per90.sqrt(), &mut rng);
        let minutes = sample_non_negative_normal(input.expected_minutes, input.minutes_sigma, &mut rng);
        let exposure = minutes / 90.0;
        let total = match input.family {
            StatFamily::ContinuousNonNegative => realized_rate * exposure,
            StatFamily::Count => f64::from(poisson(realized_rate * exposure, &mut rng)),
        };
        totals.push(total);
        rates.push(realized_rate);
    }

    Some(PlayerStatForecast {
        player: input.player,
        metric: input.metric,
        total: summarize(&totals)?,
        per90: summarize(&rates)?,
    })
}

/// Forecast a team's aggregate statistic over a known/expected number of fixtures.
pub fn simulate_team_stat(
    input: TeamStatSimulationInput,
    paths: u32,
    seed: u64,
) -> Option<TeamStatForecast> {
    validate_paths(paths)?;
    if input.expected_matches == 0
        || !valid_non_negative(input.mean_per_match)
        || !valid_non_negative(input.variance_per_match)
        || !valid_non_negative(input.epistemic_sigma)
    {
        return None;
    }

    let mut rng = Rng64::new(seed);
    let mut totals = Vec::with_capacity(paths as usize);
    let mut per_match = Vec::with_capacity(paths as usize);
    for _ in 0..paths {
        let latent_rate = sample_non_negative_normal(input.mean_per_match, input.epistemic_sigma, &mut rng);
        let mut total = 0.0;
        for _ in 0..input.expected_matches {
            let realized = sample_non_negative_normal(latent_rate, input.variance_per_match.sqrt(), &mut rng);
            total += match input.family {
                StatFamily::ContinuousNonNegative => realized,
                StatFamily::Count => f64::from(poisson(realized, &mut rng)),
            };
        }
        totals.push(total);
        per_match.push(total / f64::from(input.expected_matches));
    }

    Some(TeamStatForecast {
        club: input.club,
        metric: input.metric,
        total: summarize(&totals)?,
        per_match: summarize(&per_match)?,
    })
}

/// Simulate a match result from uncertain scoring intensities. The intensities are sampled on a
/// log scale and goals are sampled conditionally as Poisson counts, which gives a simple,
/// reproducible first model while allowing a richer strength model to supply the rates later.
pub fn simulate_match(
    input: MatchSimulationInput,
    paths: u32,
    seed: u64,
) -> Option<MatchOutcomeForecast> {
    validate_paths(paths)?;
    if !input.home_goal_rate.is_finite() || input.home_goal_rate <= 0.0
        || !input.away_goal_rate.is_finite() || input.away_goal_rate <= 0.0
        || !valid_non_negative(input.home_rate_sigma)
        || !valid_non_negative(input.away_rate_sigma)
    {
        return None;
    }

    let mut rng = Rng64::new(seed);
    let mut home_goals = Vec::with_capacity(paths as usize);
    let mut away_goals = Vec::with_capacity(paths as usize);
    let mut home_wins = 0_u32;
    let mut draws = 0_u32;
    let mut away_wins = 0_u32;

    for _ in 0..paths {
        let home_rate = sample_log_rate(input.home_goal_rate, input.home_rate_sigma, &mut rng);
        let away_rate = sample_log_rate(input.away_goal_rate, input.away_rate_sigma, &mut rng);
        let home = poisson(home_rate, &mut rng);
        let away = poisson(away_rate, &mut rng);
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
    Some(MatchOutcomeForecast {
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
    })
}

fn validate_paths(paths: u32) -> Option<()> { (paths > 0).then_some(()) }

fn valid_non_negative(value: f64) -> bool { value.is_finite() && value >= 0.0 }

fn sample_non_negative_normal(mean: f64, sigma: f64, rng: &mut Rng64) -> f64 {
    if sigma <= f64::EPSILON { mean.max(0.0) } else { (mean + sigma * rng.normal()).max(0.0) }
}

fn sample_log_rate(mean: f64, log_sigma: f64, rng: &mut Rng64) -> f64 {
    if log_sigma <= f64::EPSILON { return mean; }
    // Shift the log mean so E[lambda] remains approximately `mean` after lognormal sampling.
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
    // Normal approximation is stable and much cheaper for high-volume count metrics such as passes.
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
        p10: sample_quantile(&sorted, 0.10),
        p25: sample_quantile(&sorted, 0.25),
        p50: sample_quantile(&sorted, 0.50),
        p75: sample_quantile(&sorted, 0.75),
        p90: sample_quantile(&sorted, 0.90),
        mean,
        standard_error: (variance / sorted.len() as f64).sqrt(),
        paths: sorted.len().min(u32::MAX as usize) as u32,
    })
}

fn sample_quantile(sorted: &[f64], probability: f64) -> f64 {
    if sorted.len() == 1 { return sorted[0]; }
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper { sorted[lower] } else {
        let fraction = position - lower as f64;
        sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_stat_simulation_is_reproducible() {
        let input = PlayerStatSimulationInput { player: PlayerId(1), metric: MetricId(2), family: StatFamily::Count, mean_per90: 0.5, variance_per90: 0.04, expected_minutes: 1800.0, minutes_sigma: 120.0, epistemic_sigma: 0.05 };
        let first = simulate_player_stat(input, 1_000, 7).unwrap();
        let second = simulate_player_stat(input, 1_000, 7).unwrap();
        assert_eq!(first, second);
        assert!(first.total.p90 >= first.total.p10);
    }

    #[test]
    fn team_stat_simulation_reports_distribution() {
        let input = TeamStatSimulationInput { club: ClubId(1), metric: MetricId(3), family: StatFamily::ContinuousNonNegative, mean_per_match: 1.6, variance_per_match: 0.4, expected_matches: 20, epistemic_sigma: 0.1 };
        let forecast = simulate_team_stat(input, 500, 9).unwrap();
        assert!(forecast.total.mean > 0.0);
        assert_eq!(forecast.total.paths, 500);
    }

    #[test]
    fn balanced_match_probabilities_sum_to_one() {
        let input = MatchSimulationInput { match_id: MatchId(1), home: ClubId(1), away: ClubId(2), home_goal_rate: 1.4, away_goal_rate: 1.2, home_rate_sigma: 0.15, away_rate_sigma: 0.15 };
        let forecast = simulate_match(input, 10_000, 42).unwrap();
        let probability = forecast.home_win.get() + forecast.draw.get() + forecast.away_win.get();
        assert!((probability - 1.0).abs() < 1e-6);
        assert!(forecast.home_expected_points >= 0.0 && forecast.home_expected_points <= 3.0);
    }
}
