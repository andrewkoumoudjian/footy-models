//! Rolling-origin out-of-sample scoring for the team-goal model.
//!
//! Source-neutral: callers supply dated match slices with canonical club
//! names. Every forecast for a fixture uses only matches with strictly
//! earlier days. The home-advantage-only independent-Poisson baseline is the
//! parsimonious comparator; the fitted [`TeamGoalModel`](crate::TeamGoalModel)
//! must beat it on proper scores to justify its attack/defense parameters.
//! Resampling for confidence intervals uses the repository's deterministic
//! `Rng64` with a fixed seed.

use crate::{scoreline_pmf, TeamGoalModel, TeamGoalsObservation};
use footy_forecast::Rng64;
use std::collections::BTreeMap;

fn poisson_mass(k: u8, mu: f64) -> f64 {
    let mu = mu.clamp(0.15, 5.0);
    let mut log_factorial = 0.0;
    for j in 2..=u32::from(k) {
        log_factorial += f64::from(j).ln();
    }
    (f64::from(k) * mu.ln() - mu - log_factorial).exp()
}

/// One dated, scored fixture. Goals above 12 per side are folded consistently
/// with the 13x13 scoreline grid.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamMatch {
    pub day: i32,
    pub home: String,
    pub away: String,
    pub home_goals: u8,
    pub away_goals: u8,
}

impl TeamMatch {
    pub fn valid(&self) -> bool {
        !self.home.is_empty() && self.home != self.away
    }

    fn outcome(&self) -> usize {
        match self.home_goals.cmp(&self.away_goals) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RollingConfig {
    /// First month-start (unix day) that is forecast, never fitted.
    pub first_forecast_day: i32,
    /// Fixtures before this day still supply history but are never scored.
    /// Used to restrict scoring to a post-vintage window (e.g. FM priors must
    /// not score fixtures predating the FM snapshot).
    pub score_from_day: i32,
    /// Refits happen at this cadence; forecasts use the latest prior fit.
    pub step_days: i32,
    /// Minimum strictly-prior matches required before the first forecast.
    pub min_history: usize,
    /// FM roster-prior coefficient applied through
    /// `TeamGoalModel::forecast_with_fm` (0.0 = strictly result-driven).
    pub fm_coefficient: f64,
    /// Per-club (attack, defense) FM prior indices; absent clubs use 0.0.
    pub fm_priors: BTreeMap<String, (f64, f64)>,
    /// Bootstrap draws for the paired log-loss interval.
    pub bootstrap_draws: usize,
    pub bootstrap_seed: u64,
}

impl Default for RollingConfig {
    fn default() -> Self {
        Self {
            first_forecast_day: 0,
            score_from_day: 0,
            step_days: 30,
            min_history: 30,
            fm_coefficient: 0.0,
            fm_priors: BTreeMap::new(),
            bootstrap_draws: 2500,
            bootstrap_seed: 20261012,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixtureScore {
    pub day: i32,
    pub wdl_log_loss_model: f64,
    pub wdl_log_loss_baseline: f64,
    pub wdl_brier_model: f64,
    pub wdl_brier_baseline: f64,
    pub scoreline_log_loss_model: f64,
    pub scoreline_log_loss_baseline: f64,
    pub home_win_prob_model: f64,
    pub home_win_prob_baseline: f64,
    pub home_win_outcome: f64,
    pub goals_abs_error_model: f64,
    pub total_goals: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReliabilityBin {
    pub bin: usize,
    pub n_model: usize,
    pub mean_pred_model: f64,
    pub mean_obs_model: f64,
    pub n_baseline: usize,
    pub mean_pred_baseline: f64,
    pub mean_obs_baseline: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RollingReport {
    pub fixtures: usize,
    pub skipped_new_club_months: usize,
    pub wdl_log_loss_model: f64,
    pub wdl_log_loss_baseline: f64,
    pub wdl_brier_model: f64,
    pub wdl_brier_baseline: f64,
    pub scoreline_log_loss_model: f64,
    pub scoreline_log_loss_baseline: f64,
    pub mean_abs_goals_error_model: f64,
    /// Paired (model minus baseline) WDL log-loss 95% interval from
    /// matchday-clustered resampling; negative favors the fitted model.
    pub wdl_delta_95pc: [f64; 2],
    pub matchday_clusters: usize,
    pub home_win_reliability: Vec<ReliabilityBin>,
    pub per_season: BTreeMap<i32, SeasonScores>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeasonScores {
    pub fixtures: usize,
    pub wdl_log_loss_model: f64,
    pub wdl_log_loss_baseline: f64,
}

fn days_to_year(day: i32) -> i32 {
    // Howard Hinnant's civil-from-days algorithm on unix days; std only.
    let z = i64::from(day) + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (era * 400 + yoe + i64::from(m <= 2)) as i32
}

fn score_one(model_mu: [f64; 2], base_mu: [f64; 2], game: &TeamMatch) -> FixtureScore {
    let model = crate::score_distribution(model_mu[0], model_mu[1]);
    let grid = scoreline_pmf(model_mu[0], model_mu[1]);
    let hi = usize::from(game.home_goals.min(12));
    let ai = usize::from(game.away_goals.min(12));
    let outcome = game.outcome();
    let brier = |probs: &[f64; 3]| {
        probs
            .iter()
            .enumerate()
            .map(|(k, p)| (p - f64::from(outcome == k)).powi(2))
            .sum::<f64>()
    };
    let base_outcomes = {
        let h: Vec<f64> = (0..13).map(|k| poisson_mass(k, base_mu[0])).collect();
        let a: Vec<f64> = (0..13).map(|k| poisson_mass(k, base_mu[1])).collect();
        let mut outcomes = [0.0; 3];
        let mut mass = 0.0;
        for (i, ph) in h.iter().enumerate() {
            for (j, pa) in a.iter().enumerate() {
                let p = ph * pa;
                mass += p;
                outcomes[if i > j { 0 } else if i == j { 1 } else { 2 }] += p;
            }
        }
        outcomes.map(|v| v / mass)
    };
    FixtureScore {
        day: game.day,
        wdl_log_loss_model: -model.outcomes[outcome].max(1e-12).ln(),
        wdl_log_loss_baseline: -base_outcomes[outcome].max(1e-12).ln(),
        wdl_brier_model: brier(&model.outcomes),
        wdl_brier_baseline: brier(&base_outcomes),
        scoreline_log_loss_model: -grid[hi][ai].max(1e-15).ln(),
        scoreline_log_loss_baseline: -(poisson_mass(game.home_goals.min(12), base_mu[0])
            * poisson_mass(game.away_goals.min(12), base_mu[1]))
        .max(1e-15)
        .ln(),
        home_win_prob_model: model.outcomes[0],
        home_win_prob_baseline: base_outcomes[0],
        home_win_outcome: f64::from(outcome == 0),
        goals_abs_error_model: (model.home_goals - f64::from(game.home_goals)).abs()
            + (model.away_goals - f64::from(game.away_goals)).abs(),
        total_goals: u32::from(game.home_goals) + u32::from(game.away_goals),
    }
}

/// Rolling-origin scoring. `matches` must be sorted by day; championship
/// priors (e.g. promoted clubs) enter only through strictly-prior rows.
pub fn rolling_origin(
    matches: &[TeamMatch],
    championship: &[TeamMatch],
    config: RollingConfig,
) -> Option<RollingReport> {
    if matches.len() < config.min_history
        || config.step_days <= 0
        || config.bootstrap_draws == 0
        || matches.iter().any(|game| !game.valid())
        || championship.iter().any(|game| !game.valid())
    {
        return None;
    }
    let mut cursor = config.first_forecast_day;
    let last_day = matches.iter().map(|game| game.day).max()?;
    let mut scores = Vec::new();
    let mut skipped = 0_usize;
    while cursor <= last_day {
        let prior: Vec<&TeamMatch> = matches.iter().filter(|game| game.day < cursor).collect();
        if prior.len() >= config.min_history {
            let epl: Vec<TeamGoalsObservation> = prior
                .iter()
                .map(|game| TeamGoalsObservation {
                    day: game.day,
                    home: game.home.clone(),
                    away: game.away.clone(),
                    home_goals: game.home_goals.min(12),
                    away_goals: game.away_goals.min(12),
                })
                .collect();
            let e1: Vec<TeamGoalsObservation> = championship
                .iter()
                .filter(|game| game.day < cursor)
                .map(|game| TeamGoalsObservation {
                    day: game.day,
                    home: game.home.clone(),
                    away: game.away.clone(),
                    home_goals: game.home_goals.min(12),
                    away_goals: game.away_goals.min(12),
                })
                .collect();
            // League home/away means from the same strictly-prior window.
            let (home_sum, away_sum) = prior.iter().fold((0.0, 0.0), |(h, a), game| {
                (h + f64::from(game.home_goals), a + f64::from(game.away_goals))
            });
            let base_mu = [
                (home_sum / prior.len() as f64).clamp(0.15, 5.0),
                (away_sum / prior.len() as f64).clamp(0.15, 5.0),
            ];
            if let Some(fitted) = TeamGoalModel::fit(&epl, &e1, cursor) {
                for game in matches.iter().filter(|game| {
                    game.day >= cursor
                        && game.day < cursor + config.step_days
                        && game.day >= config.score_from_day
                }) {
                    let (home_attack, home_defense) = config
                        .fm_priors
                        .get(&game.home)
                        .copied()
                        .unwrap_or((0.0, 0.0));
                    let (away_attack, away_defense) = config
                        .fm_priors
                        .get(&game.away)
                        .copied()
                        .unwrap_or((0.0, 0.0));
                    let forecast = fitted.forecast_with_fm(
                        &game.home,
                        &game.away,
                        home_attack,
                        home_defense,
                        away_attack,
                        away_defense,
                        config.fm_coefficient,
                    );
                    scores.push(score_one(
                        [forecast.home_goals, forecast.away_goals],
                        base_mu,
                        game,
                    ));
                }
            } else {
                skipped += matches
                    .iter()
                    .filter(|game| game.day >= cursor && game.day < cursor + config.step_days)
                    .count();
            }
        }
        cursor += config.step_days;
    }
    if scores.is_empty() {
        return None;
    }
    let n = scores.len() as f64;
    let mean = |f: fn(&FixtureScore) -> f64| scores.iter().map(f).sum::<f64>() / n;
    // Matchday-clustered paired bootstrap for the WDL log-loss delta.
    let mut clusters: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for (index, score) in scores.iter().enumerate() {
        clusters.entry(score.day).or_default().push(index);
    }
    let groups: Vec<Vec<usize>> = clusters.into_values().collect();
    let mut rng = Rng64::new(config.bootstrap_seed);
    let mut deltas = Vec::with_capacity(config.bootstrap_draws);
    for _ in 0..config.bootstrap_draws {
        let mut sum = 0.0;
        let mut count = 0_usize;
        for _ in 0..groups.len() {
            let pick = (rng.uniform() * groups.len() as f64) as usize;
            let group = &groups[pick.min(groups.len() - 1)];
            for &index in group {
                sum += scores[index].wdl_log_loss_model - scores[index].wdl_log_loss_baseline;
                count += 1;
            }
        }
        deltas.push(sum / count.max(1) as f64);
    }
    deltas.sort_by(f64::total_cmp);
    let quantile = |q: f64| {
        let index = ((q * config.bootstrap_draws as f64) as usize).min(config.bootstrap_draws - 1);
        deltas[index]
    };
    let mut reliability = Vec::new();
    for bin in 0..10_usize {
        let lo = bin as f64 / 10.0;
        let hi = (bin + 1) as f64 / 10.0;
        let in_bin = |p: f64| p > lo && p <= hi || (bin == 0 && p == 0.0);
        let mut bin_scores: Vec<&FixtureScore> = scores
            .iter()
            .filter(|score| in_bin(score.home_win_prob_model))
            .collect();
        let (n_model, mp_model, mo_model) = if bin_scores.is_empty() {
            (0, 0.0, 0.0)
        } else {
            (
                bin_scores.len(),
                bin_scores.iter().map(|s| s.home_win_prob_model).sum::<f64>()
                    / bin_scores.len() as f64,
                bin_scores.iter().map(|s| s.home_win_outcome).sum::<f64>()
                    / bin_scores.len() as f64,
            )
        };
        bin_scores = scores
            .iter()
            .filter(|score| in_bin(score.home_win_prob_baseline))
            .collect();
        let (n_base, mp_base, mo_base) = if bin_scores.is_empty() {
            (0, 0.0, 0.0)
        } else {
            (
                bin_scores.len(),
                bin_scores.iter().map(|s| s.home_win_prob_baseline).sum::<f64>()
                    / bin_scores.len() as f64,
                bin_scores.iter().map(|s| s.home_win_outcome).sum::<f64>()
                    / bin_scores.len() as f64,
            )
        };
        reliability.push(ReliabilityBin {
            bin,
            n_model,
            mean_pred_model: mp_model,
            mean_obs_model: mo_model,
            n_baseline: n_base,
            mean_pred_baseline: mp_base,
            mean_obs_baseline: mo_base,
        });
    }
    let mut per_season: BTreeMap<i32, SeasonScores> = BTreeMap::new();
    for score in &scores {
        let season = days_to_year(score.day);
        let entry = per_season.entry(season).or_default();
        entry.fixtures += 1;
        entry.wdl_log_loss_model += score.wdl_log_loss_model;
        entry.wdl_log_loss_baseline += score.wdl_log_loss_baseline;
    }
    for entry in per_season.values_mut() {
        entry.wdl_log_loss_model /= entry.fixtures as f64;
        entry.wdl_log_loss_baseline /= entry.fixtures as f64;
    }
    Some(RollingReport {
        fixtures: scores.len(),
        skipped_new_club_months: skipped,
        wdl_log_loss_model: mean(|s| s.wdl_log_loss_model),
        wdl_log_loss_baseline: mean(|s| s.wdl_log_loss_baseline),
        wdl_brier_model: mean(|s| s.wdl_brier_model),
        wdl_brier_baseline: mean(|s| s.wdl_brier_baseline),
        scoreline_log_loss_model: mean(|s| s.scoreline_log_loss_model),
        scoreline_log_loss_baseline: mean(|s| s.scoreline_log_loss_baseline),
        mean_abs_goals_error_model: mean(|s| s.goals_abs_error_model),
        wdl_delta_95pc: [quantile(0.025), quantile(0.975)],
        matchday_clusters: groups.len(),
        home_win_reliability: reliability,
        per_season,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn league(days: std::ops::Range<i32>, home_goals: u8, away_goals: u8) -> Vec<TeamMatch> {
        let mut out = Vec::new();
        for day in days {
            out.push(TeamMatch { day, home: "A".into(), away: "B".into(),
                                home_goals, away_goals });
            out.push(TeamMatch { day, home: "B".into(), away: "A".into(),
                                home_goals: away_goals, away_goals: home_goals });
        }
        out
    }

    #[test]
    fn rolling_report_is_deterministic_and_favors_signal() {
        let matches = league(0..120, 2, 1);
        let config = RollingConfig {
            first_forecast_day: 90,
            step_days: 10,
            min_history: 30,
            bootstrap_draws: 200,
            bootstrap_seed: 7,
            ..RollingConfig::default()
        };
        let first = rolling_origin(&matches, &[], config.clone()).unwrap();
        let second = rolling_origin(&matches, &[], config).unwrap();
        assert_eq!(first, second);
        assert!(first.fixtures > 0);
        assert!(first.wdl_log_loss_model <= first.wdl_log_loss_baseline + 0.05);
        assert!(first.wdl_delta_95pc[0] <= first.wdl_delta_95pc[1]);
        let mass: usize = first.home_win_reliability.iter().map(|b| b.n_model).sum();
        assert_eq!(mass, first.fixtures);
    }

    #[test]
    fn invalid_inputs_return_none() {
        let matches = league(0..40, 1, 1);
        assert!(rolling_origin(&[], &[], RollingConfig::default()).is_none());
        let mut bad = matches.clone();
        bad[0].away = "A".into();
        assert!(rolling_origin(&bad, &[], RollingConfig::default()).is_none());
    }

    #[test]
    fn score_from_day_restricts_scoring_not_history() {
        let matches = league(0..120, 2, 1);
        let full = rolling_origin(
            &matches,
            &[],
            RollingConfig {
                first_forecast_day: 90,
                step_days: 10,
                min_history: 30,
                bootstrap_draws: 50,
                bootstrap_seed: 7,
                ..RollingConfig::default()
            },
        )
        .unwrap();
        let gated = rolling_origin(
            &matches,
            &[],
            RollingConfig {
                first_forecast_day: 90,
                score_from_day: 100,
                step_days: 10,
                min_history: 30,
                bootstrap_draws: 50,
                bootstrap_seed: 7,
                ..RollingConfig::default()
            },
        )
        .unwrap();
        assert!(gated.fixtures < full.fixtures);
        assert!(gated.fixtures > 0);
    }

    #[test]
    fn zero_fm_coefficient_reproduces_result_driven_forecast() {
        let matches = league(0..120, 2, 1);
        let mut priors = BTreeMap::new();
        priors.insert("A".to_owned(), (1.5, -1.5));
        priors.insert("B".to_owned(), (-1.5, 1.5));
        let plain = rolling_origin(
            &matches,
            &[],
            RollingConfig {
                first_forecast_day: 90,
                step_days: 30,
                min_history: 30,
                bootstrap_draws: 50,
                bootstrap_seed: 7,
                ..RollingConfig::default()
            },
        )
        .unwrap();
        let zeroed = rolling_origin(
            &matches,
            &[],
            RollingConfig {
                first_forecast_day: 90,
                step_days: 30,
                min_history: 30,
                fm_coefficient: 0.0,
                fm_priors: priors,
                bootstrap_draws: 50,
                bootstrap_seed: 7,
                ..RollingConfig::default()
            },
        )
        .unwrap();
        assert_eq!(plain, zeroed);
    }

    #[test]
    fn year_mapping_covers_known_dates() {
        // 2026-10-10 is unix day 20736.
        assert_eq!(days_to_year(20736), 2026);
        assert_eq!(days_to_year(0), 1970);
    }
}
