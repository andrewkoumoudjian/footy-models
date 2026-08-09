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

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::Confidence;

    fn prediction(center: i64, low: i64, high: i64) -> PriceDistribution {
        PriceDistribution::new(EuroCents(low), EuroCents(low), EuroCents(center), EuroCents(high), EuroCents(high), Confidence::ONE).unwrap()
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
}
