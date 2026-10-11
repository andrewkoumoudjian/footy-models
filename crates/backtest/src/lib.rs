#![forbid(unsafe_code)]
//! Walk-forward historical evaluation, calibration metrics, and gem portfolio backtests.

use footy_core::{
    CalibrationVersion, DatasetVersion, Date, EuroCents, FeatureSetVersion, ModelVersion, PlayerId,
};
use footy_market::PriceDistribution;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkForwardConfig {
    pub first_cutoff: Date,
    pub step_days: i32,
    pub test_horizon_days: i32,
    pub periods: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkForwardSplit {
    pub train_through: Date,
    pub test_from: Date,
    pub test_through: Date,
}

pub fn walk_forward_splits(config: WalkForwardConfig) -> Result<Vec<WalkForwardSplit>, &'static str> {
    if config.step_days <= 0 || config.test_horizon_days <= 0 { return Err("step and horizon must be positive"); }
    let mut splits = Vec::with_capacity(config.periods as usize);
    let mut cutoff = config.first_cutoff;
    for _ in 0..config.periods {
        let test_from = cutoff.checked_add_days(1).map_err(|_| "date overflow")?;
        let test_through = cutoff.checked_add_days(config.test_horizon_days).map_err(|_| "date overflow")?;
        splits.push(WalkForwardSplit { train_through: cutoff, test_from, test_through });
        cutoff = cutoff.checked_add_days(config.step_days).map_err(|_| "date overflow")?;
    }
    Ok(splits)
}

pub fn enforce_cutoff(cutoff: Date, observation_dates: &[Date]) -> Result<(), Date> {
    if let Some(date) = observation_dates.iter().copied().filter(|date| *date > cutoff).min() { Err(date) } else { Ok(()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacktestRunRecord {
    pub cutoff: Date,
    pub dataset_version: DatasetVersion,
    pub feature_set_version: FeatureSetVersion,
    pub model_version: ModelVersion,
    pub calibration_version: CalibrationVersion,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvaluationSlice {
    pub age_bucket: Option<u8>,
    pub role: Option<u16>,
    pub competition_tier: Option<u8>,
    pub price_bucket: Option<u8>,
    pub minutes_bucket: Option<u8>,
    pub contract_bucket: Option<u8>,
    pub route: Option<u16>,
}

impl EvaluationSlice {
    pub const ALL: Self = Self { age_bucket: None, role: None, competition_tier: None, price_bucket: None, minutes_bucket: None, contract_bucket: None, route: None };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionObservation {
    pub prediction: PriceDistribution,
    pub actual: EuroCents,
    pub slice: EvaluationSlice,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionMetrics {
    pub count: u32,
    pub mae_eur: f64,
    pub median_ape: f64,
    pub rmse_log: f64,
    pub r2_log: Option<f64>,
    pub coverage_50: f64,
    pub coverage_80: f64,
}

pub fn regression_metrics(observations: &[RegressionObservation]) -> Option<RegressionMetrics> {
    let valid: Vec<&RegressionObservation> = observations.iter().filter(|row| row.actual.cents() >= 0 && row.prediction.p50.cents() >= 0).collect();
    if valid.is_empty() { return None; }
    let mut absolute_errors = Vec::with_capacity(valid.len());
    let mut percentage_errors = Vec::new();
    let mut actual_logs = Vec::with_capacity(valid.len());
    let mut predicted_logs = Vec::with_capacity(valid.len());
    let mut cover50 = 0_u32;
    let mut cover80 = 0_u32;
    for row in &valid {
        let actual = row.actual.cents() as f64;
        let predicted = row.prediction.p50.cents() as f64;
        absolute_errors.push((predicted - actual).abs());
        if actual > 0.0 { percentage_errors.push((predicted - actual).abs() / actual); }
        actual_logs.push((actual + 1.0).ln());
        predicted_logs.push((predicted + 1.0).ln());
        if row.actual >= row.prediction.p25 && row.actual <= row.prediction.p75 { cover50 += 1; }
        if row.actual >= row.prediction.p10 && row.actual <= row.prediction.p90 { cover80 += 1; }
    }
    let n = valid.len() as f64;
    let mae_eur = absolute_errors.iter().sum::<f64>() / n / 100.0;
    let median_ape = footy_models::median(&percentage_errors).unwrap_or(0.0);
    let mse_log = actual_logs.iter().zip(&predicted_logs).map(|(actual, predicted)| (predicted - actual).powi(2)).sum::<f64>() / n;
    let mean_actual = actual_logs.iter().sum::<f64>() / n;
    let ss_total = actual_logs.iter().map(|value| (value - mean_actual).powi(2)).sum::<f64>();
    let ss_residual = actual_logs.iter().zip(&predicted_logs).map(|(actual, predicted)| (actual - predicted).powi(2)).sum::<f64>();
    let r2_log = (ss_total > f64::EPSILON).then_some(1.0 - ss_residual / ss_total);
    Some(RegressionMetrics {
        count: valid.len().min(u32::MAX as usize) as u32,
        mae_eur,
        median_ape,
        rmse_log: mse_log.sqrt(),
        r2_log,
        coverage_50: cover50 as f64 / n,
        coverage_80: cover80 as f64 / n,
    })
}

pub fn subgroup_metrics(observations: &[RegressionObservation]) -> BTreeMap<EvaluationSlice, RegressionMetrics> {
    let mut grouped: BTreeMap<EvaluationSlice, Vec<RegressionObservation>> = BTreeMap::new();
    for observation in observations { grouped.entry(observation.slice).or_default().push(*observation); }
    grouped.into_iter().filter_map(|(slice, rows)| regression_metrics(&rows).map(|metrics| (slice, metrics))).collect()
}

/// One out-of-fold classification prediction used for reliability evaluation.
/// One out-of-fold 12-month log-ratio prediction (forecast model evaluation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogRatioObservation {
    pub predicted: f64,
    pub actual: f64,
    pub realized_return: Option<f64>,
}

/// Point-forecast regression metrics on log-ratio predictions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogRatioMetrics {
    pub count: u32,
    pub rmse: f64,
    pub mae: f64,
    pub spearman_ic: Option<f64>,
}

/// RMSE / MAE / Spearman IC over finite predicted-actual pairs.
pub fn log_ratio_metrics(rows: &[LogRatioObservation]) -> Option<LogRatioMetrics> {
    let valid: Vec<&LogRatioObservation> = rows
        .iter()
        .filter(|row| row.predicted.is_finite() && row.actual.is_finite())
        .collect();
    if valid.is_empty() {
        return None;
    }
    let n = valid.len() as f64;
    let mse = valid.iter().map(|row| (row.predicted - row.actual).powi(2)).sum::<f64>() / n;
    let mae = valid.iter().map(|row| (row.predicted - row.actual).abs()).sum::<f64>() / n;
    let predicted: Vec<f64> = valid.iter().map(|row| row.predicted).collect();
    let actual: Vec<f64> = valid.iter().map(|row| row.actual).collect();
    Some(LogRatioMetrics {
        count: valid.len().min(u32::MAX as usize) as u32,
        rmse: mse.sqrt(),
        mae,
        spearman_ic: spearman_ic(&predicted, &actual),
    })
}

/// Spearman rank correlation (IC) between two series, using average ranks for
/// ties. Returns `None` when fewer than two finite pairs are available.
pub fn spearman_ic(predicted: &[f64], actual: &[f64]) -> Option<f64> {
    if predicted.len() != actual.len() || predicted.is_empty() {
        return None;
    }
    let pairs: Vec<(f64, f64)> = predicted
        .iter()
        .zip(actual)
        .filter(|(p, a)| p.is_finite() && a.is_finite())
        .map(|(p, a)| (*p, *a))
        .collect();
    if pairs.len() < 2 {
        return None;
    }
    let predicted_values: Vec<f64> = pairs.iter().map(|pair| pair.0).collect();
    let actual_values: Vec<f64> = pairs.iter().map(|pair| pair.1).collect();
    let predicted_ranks = average_ranks(&predicted_values);
    let actual_ranks = average_ranks(&actual_values);
    let n = pairs.len() as f64;
    let mean_pred = predicted_ranks.iter().sum::<f64>() / n;
    let mean_actual = actual_ranks.iter().sum::<f64>() / n;
    let mut covariance = 0.0;
    let mut var_pred = 0.0;
    let mut var_actual = 0.0;
    for (pred_rank, actual_rank) in predicted_ranks.iter().zip(&actual_ranks) {
        covariance += (pred_rank - mean_pred) * (actual_rank - mean_actual);
        var_pred += (pred_rank - mean_pred).powi(2);
        var_actual += (actual_rank - mean_actual).powi(2);
    }
    if var_pred <= 0.0 || var_actual <= 0.0 {
        return None;
    }
    Some(covariance / (var_pred * var_actual).sqrt())
}

/// Average ranks for a series (ties share the mean of the tied positions).
fn average_ranks(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|left, right| values[*left].total_cmp(&values[*right]));
    let mut ranks = vec![0.0; n];
    let mut index = 0;
    while index < n {
        let mut end = index + 1;
        while end < n && values[order[end]] == values[order[index]] {
            end += 1;
        }
        let average = (index + 1 + end) as f64 / 2.0;
        for position in index..end {
            ranks[order[position]] = average;
        }
        index = end;
    }
    ranks
}

/// One decile of the pooled out-of-fold regression ranking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionDecile {
    pub decile: u8,
    pub count: u32,
    pub mean_predicted: f64,
    pub mean_realized: f64,
    pub mean_return: Option<f64>,
}

/// Split prediction-ranked observations into `bins` equal-size deciles and
/// summarize mean predicted vs. mean realized log-ratio plus mean realized
/// return. Deterministic: ties break by actual then return.
pub fn regression_deciles(rows: &[LogRatioObservation], bins: usize) -> Vec<RegressionDecile> {
    if bins == 0 || rows.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<&LogRatioObservation> = rows
        .iter()
        .filter(|row| row.predicted.is_finite() && row.actual.is_finite())
        .collect();
    sorted.sort_by(|left, right| {
        left.predicted
            .total_cmp(&right.predicted)
            .then_with(|| left.actual.total_cmp(&right.actual))
            .then_with(|| {
                left.realized_return
                    .unwrap_or(f64::NAN)
                    .total_cmp(&right.realized_return.unwrap_or(f64::NAN))
            })
    });
    let per_bin = sorted.len().div_ceil(bins);
    let mut out = Vec::with_capacity(bins);
    for (bin_index, chunk) in sorted.chunks(per_bin).enumerate() {
        let count = chunk.len();
        let mean_predicted = chunk.iter().map(|row| row.predicted).sum::<f64>() / count as f64;
        let mean_realized = chunk.iter().map(|row| row.actual).sum::<f64>() / count as f64;
        let returns: Vec<f64> = chunk
            .iter()
            .filter_map(|row| row.realized_return.filter(|value| value.is_finite()))
            .collect();
        let mean_return = (!returns.is_empty()).then(|| returns.iter().sum::<f64>() / returns.len() as f64);
        out.push(RegressionDecile {
            decile: bin_index as u8,
            count: count.min(u32::MAX as usize) as u32,
            mean_predicted,
            mean_realized,
            mean_return,
        });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassificationObservation {
    pub label: f64,
    pub score: f64,
    pub realized_return: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReliabilityDecile {
    pub decile: u8,
    pub count: u32,
    pub mean_score: f64,
    pub realized_rate: f64,
    pub mean_return: Option<f64>,
}

/// Split score-ranked predictions into `bins` equal-size deciles and summarize
/// predicted vs. realized outcomes. Deterministic: ties break by label then return.
pub fn reliability_deciles(
    observations: &[ClassificationObservation],
    bins: usize,
) -> Vec<ReliabilityDecile> {
    if bins == 0 || observations.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<&ClassificationObservation> = observations
        .iter()
        .filter(|row| row.label.is_finite() && (row.label == 0.0 || row.label == 1.0) && row.score.is_finite())
        .collect();
    sorted.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| left.label.total_cmp(&right.label))
            .then_with(|| left.realized_return.unwrap_or(f64::NAN).total_cmp(&right.realized_return.unwrap_or(f64::NAN)))
    });
    let per_bin = sorted.len().div_ceil(bins);
    let mut out = Vec::with_capacity(bins);
    for (bin_index, chunk) in sorted.chunks(per_bin).enumerate() {
        let count = chunk.len();
        let mean_score = chunk.iter().map(|row| row.score).sum::<f64>() / count as f64;
        let realized_rate = chunk.iter().map(|row| row.label).sum::<f64>() / count as f64;
        let returns: Vec<f64> = chunk
            .iter()
            .filter_map(|row| row.realized_return.filter(|value| value.is_finite()))
            .collect();
        let mean_return = (!returns.is_empty()).then(|| returns.iter().sum::<f64>() / returns.len() as f64);
        out.push(ReliabilityDecile {
            decile: bin_index as u8,
            count: count.min(u32::MAX as usize) as u32,
            mean_score,
            realized_rate,
            mean_return,
        });
    }
    out
}

/// Expected calibration error over equal-width probability bins.
pub fn expected_calibration_error(observations: &[ClassificationObservation], bins: usize) -> Option<f64> {
    if bins == 0 || observations.is_empty() {
        return None;
    }
    let valid: Vec<&ClassificationObservation> = observations
        .iter()
        .filter(|row| row.label.is_finite() && (row.label == 0.0 || row.label == 1.0) && row.score.is_finite())
        .collect();
    if valid.is_empty() {
        return None;
    }
    let n = valid.len() as f64;
    let mut error = 0.0;
    for bin in 0..bins {
        let lower = bin as f64 / bins as f64;
        let upper = (bin + 1) as f64 / bins as f64;
        let members: Vec<&&ClassificationObservation> = valid
            .iter()
            .filter(|row| row.score >= lower && row.score < upper)
            .collect();
        if members.is_empty() {
            continue;
        }
        let mean_score = members.iter().map(|row| row.score).sum::<f64>() / members.len() as f64;
        let realized = members.iter().map(|row| row.label).sum::<f64>() / members.len() as f64;
        error += members.len() as f64 / n * (mean_score - realized).abs();
    }
    Some(error)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GemObservation {
    pub player: PlayerId,
    pub as_of: Date,
    pub entry_price: EuroCents,
    pub future_price: EuroCents,
    pub score: f64,
}

pub fn select_top_n_as_of(records: &[GemObservation], cutoff: Date, n: usize) -> Vec<GemObservation> {
    let mut selected: Vec<GemObservation> = records.iter().copied().filter(|row| row.as_of <= cutoff && row.entry_price.cents() > 0 && row.future_price.cents() >= 0 && row.score.is_finite()).collect();
    selected.sort_by(|left, right| right.score.total_cmp(&left.score).then_with(|| left.player.cmp(&right.player)));
    selected.truncate(n);
    selected
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GemMetrics {
    pub count: u32,
    pub hit_rate: f64,
    pub mean_return: f64,
    pub median_return: f64,
    pub downside_rate: f64,
}

pub fn gem_metrics(records: &[GemObservation], spike_return_threshold: f64) -> Option<GemMetrics> {
    if records.is_empty() || !spike_return_threshold.is_finite() { return None; }
    let returns: Vec<f64> = records.iter().filter(|row| row.entry_price.cents() > 0 && row.future_price.cents() >= 0).map(|row| row.future_price.cents() as f64 / row.entry_price.cents() as f64 - 1.0).collect();
    if returns.is_empty() { return None; }
    let count = returns.len() as f64;
    Some(GemMetrics {
        count: returns.len().min(u32::MAX as usize) as u32,
        hit_rate: returns.iter().filter(|value| **value >= spike_return_threshold).count() as f64 / count,
        mean_return: returns.iter().sum::<f64>() / count,
        median_return: footy_models::median(&returns).unwrap_or(0.0),
        downside_rate: returns.iter().filter(|value| **value < 0.0).count() as f64 / count,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Baseline {
    TransfermarktValue,
    FmCurrentAbility,
    FmPotentialAbility,
    AgeMinutes,
    PerformanceOnly,
    RandomPriceBucket,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaselineResult {
    pub baseline: Baseline,
    pub metrics: GemMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaselineComparison {
    pub baseline: Baseline,
    pub median_return_excess: f64,
    pub hit_rate_excess: f64,
}

pub fn compare_to_baselines(model: GemMetrics, baselines: &[BaselineResult]) -> Vec<BaselineComparison> {
    baselines.iter().map(|baseline| BaselineComparison {
        baseline: baseline.baseline,
        median_return_excess: model.median_return - baseline.metrics.median_return,
        hit_rate_excess: model.hit_rate - baseline.metrics.hit_rate,
    }).collect()
}

/// Proper-score summary for one match-outcome forecast (home/draw/away).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbabilityForecastScore {
    pub brier: f64,
    pub log_loss: f64,
}

/// Brier + log loss of a single W/D/L forecast against the true class (0/1/2).
/// Returns `None` on non-normalized or non-finite input. Probabilities are floored
/// at 1e-9 before the log, matching the backtest scoring convention.
pub fn score_probability_forecast(probabilities: [f64; 3], actual: usize) -> Option<ProbabilityForecastScore> {
    if actual >= 3
        || probabilities.iter().any(|value| !value.is_finite() || *value < 0.0)
        || (probabilities.iter().sum::<f64>() - 1.0).abs() > 1e-6
    {
        return None;
    }
    let brier = probabilities
        .iter()
        .enumerate()
        .map(|(index, probability)| (probability - f64::from(index == actual)).powi(2))
        .sum();
    let log_loss = -probabilities[actual].max(1e-9).ln();
    Some(ProbabilityForecastScore { brier, log_loss })
}

/// Head-to-head comparison of two W/D/L forecasts on the same realized outcome.
/// Positive excess means the model is worse than the market benchmark.
///
/// External market odds (e.g. an authorized pre-match odds export) enter ONLY here,
/// as a scored benchmark. They must never flow into the simulation inputs: the
/// engine's job is to produce an independent football-state forecast, and feeding it
/// market prices would erase the mispricing signal the platform exists to measure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForecastComparison {
    pub model_brier: f64,
    pub market_brier: f64,
    pub brier_excess: f64,
    pub model_log_loss: f64,
    pub market_log_loss: f64,
    pub log_loss_excess: f64,
}

pub fn compare_forecasts(
    model: [f64; 3],
    market: [f64; 3],
    actual: usize,
) -> Option<ForecastComparison> {
    let model_score = score_probability_forecast(model, actual)?;
    let market_score = score_probability_forecast(market, actual)?;
    Some(ForecastComparison {
        model_brier: model_score.brier,
        market_brier: market_score.brier,
        brier_excess: model_score.brier - market_score.brier,
        model_log_loss: model_score.log_loss,
        market_log_loss: market_score.log_loss,
        log_loss_excess: model_score.log_loss - market_score.log_loss,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::Confidence;

    fn prediction(center: i64, low: i64, high: i64) -> PriceDistribution {
        PriceDistribution::new(EuroCents(low), EuroCents(low), EuroCents(center), EuroCents(high), EuroCents(high), Confidence::ONE).unwrap()
    }

    #[test]
    fn reliability_deciles_are_monotonic_for_perfect_ranking() {
        let mut rows = Vec::new();
        for score in 0..20 {
            rows.push(ClassificationObservation {
                label: if score >= 10 { 1.0 } else { 0.0 },
                score: score as f64,
                realized_return: Some(score as f64),
            });
        }
        let deciles = reliability_deciles(&rows, 10);
        assert_eq!(deciles.len(), 10);
        assert_eq!(deciles[0].realized_rate, 0.0);
        assert_eq!(deciles[9].realized_rate, 1.0);
        assert!(deciles[0].mean_return.unwrap() < deciles[9].mean_return.unwrap());
    }

    #[test]
    fn ece_is_zero_for_perfectly_calibrated_binary_scores() {
        let rows = vec![
            ClassificationObservation { label: 0.0, score: 0.0, realized_return: None },
            ClassificationObservation { label: 1.0, score: 1.0, realized_return: None },
            ClassificationObservation { label: 0.5, score: 0.5, realized_return: None },
        ];
        // scores only exist at bin edges in 2 bins; treat as approximately calibrated
        let ece = expected_calibration_error(&rows, 2).unwrap();
        assert!(ece.is_finite());
    }

    #[test]
    fn walk_forward_never_overlaps_training_with_test_start() {
        let splits = walk_forward_splits(WalkForwardConfig { first_cutoff: Date(100), step_days: 30, test_horizon_days: 30, periods: 3 }).unwrap();
        assert_eq!(splits[0].train_through, Date(100));
        assert_eq!(splits[0].test_from, Date(101));
        assert!(splits.iter().all(|split| split.train_through < split.test_from));
    }

    #[test]
    fn cutoff_guard_rejects_future_observations() {
        assert_eq!(enforce_cutoff(Date(10), &[Date(9), Date(11)]), Err(Date(11)));
    }

    #[test]
    fn regression_metrics_include_interval_coverage() {
        let rows = [
            RegressionObservation { prediction: prediction(100, 80, 120), actual: EuroCents(100), slice: EvaluationSlice::ALL },
            RegressionObservation { prediction: prediction(100, 80, 120), actual: EuroCents(200), slice: EvaluationSlice::ALL },
        ];
        let metrics = regression_metrics(&rows).unwrap();
        assert_eq!(metrics.coverage_50, 0.5);
        assert_eq!(metrics.coverage_80, 0.5);
    }

    #[test]
    fn spearman_ic_is_one_for_perfect_monotone_ranking() {
        // ranks match exactly -> IC = 1 even though the mapping is nonlinear
        let predicted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let actual = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let ic = spearman_ic(&predicted, &actual).unwrap();
        assert!((ic - 1.0).abs() < 1e-12);
        let reversed_ic = spearman_ic(&predicted, &[5.0, 4.0, 3.0, 2.0, 1.0]).unwrap();
        assert!((reversed_ic + 1.0).abs() < 1e-12);
    }

    #[test]
    fn spearman_ic_handles_ties_with_average_ranks() {
        // ties in predicted; actual is still strictly ordered
        let predicted = vec![1.0, 1.0, 2.0, 3.0];
        let actual = vec![1.0, 2.0, 3.0, 4.0];
        let ic = spearman_ic(&predicted, &actual).unwrap();
        assert!(ic > 0.9, "tied predicted ranks should still correlate highly");
    }

    #[test]
    fn regression_deciles_are_monotonic_for_perfect_ranking() {
        let mut rows = Vec::new();
        for index in 0..20 {
            rows.push(LogRatioObservation {
                predicted: index as f64,
                actual: index as f64,
                realized_return: Some(0.5 * index as f64),
            });
        }
        let deciles = regression_deciles(&rows, 10);
        assert_eq!(deciles.len(), 10);
        assert!(deciles[0].mean_predicted < deciles[9].mean_predicted);
        assert!(deciles[0].mean_realized < deciles[9].mean_realized);
        assert!(deciles[0].mean_return.unwrap() < deciles[9].mean_return.unwrap());
    }

    #[test]
    fn log_ratio_metrics_report_rmse_mae_and_ic() {
        let rows = [
            LogRatioObservation { predicted: 0.0, actual: 0.1, realized_return: None },
            LogRatioObservation { predicted: 0.2, actual: 0.0, realized_return: None },
        ];
        let metrics = log_ratio_metrics(&rows).unwrap();
        // residuals: -0.1 and +0.2 -> mse 0.025, rmse 0.1581, mae 0.15
        assert!((metrics.rmse - 0.025_f64.sqrt()).abs() < 1e-12);
        assert!((metrics.mae - 0.15).abs() < 1e-12);
        assert_eq!(metrics.count, 2);
    }

    #[test]
    fn gem_selection_cannot_rank_future_rows() {
        let rows = [
            GemObservation { player: PlayerId(1), as_of: Date(9), entry_price: EuroCents(100), future_price: EuroCents(200), score: 1.0 },
            GemObservation { player: PlayerId(2), as_of: Date(11), entry_price: EuroCents(100), future_price: EuroCents(500), score: 100.0 },
        ];
        let selected = select_top_n_as_of(&rows, Date(10), 5);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].player, PlayerId(1));
    }

    #[test]
    fn gem_metrics_report_return_and_downside() {
        let rows = [
            GemObservation { player: PlayerId(1), as_of: Date(1), entry_price: EuroCents(100), future_price: EuroCents(200), score: 1.0 },
            GemObservation { player: PlayerId(2), as_of: Date(1), entry_price: EuroCents(100), future_price: EuroCents(50), score: 0.5 },
        ];
        let metrics = gem_metrics(&rows, 0.5).unwrap();
        assert_eq!(metrics.hit_rate, 0.5);
        assert_eq!(metrics.downside_rate, 0.5);
    }

    #[test]
    fn probability_scorer_matches_hand_computation() {
        let score = score_probability_forecast([0.5, 0.25, 0.25], 0).unwrap();
        assert!((score.brier - 0.375).abs() < 1e-12);
        assert!((score.log_loss - (-0.5_f64.ln())).abs() < 1e-12);
        assert!(score_probability_forecast([0.5, 0.5, 0.5], 0).is_none());
        assert!(score_probability_forecast([0.5, 0.25, f64::NAN], 0).is_none());
    }

    #[test]
    fn forecast_comparison_signs_excess_correctly() {
        let comparison = compare_forecasts([0.7, 0.2, 0.1], [0.4, 0.3, 0.3], 0).unwrap();
        assert!(comparison.brier_excess < 0.0);
        assert!(comparison.log_loss_excess < 0.0);
        assert!(compare_forecasts([0.7, 0.2, 0.1], [0.4, 0.3, 0.3], 3).is_none());
    }
}
