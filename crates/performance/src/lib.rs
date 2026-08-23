#![forbid(unsafe_code)]
//! Causal performance aggregation and robust normalization primitives.

mod team;
pub use team::*;

use footy_core::{ClubId, CompetitionId, Date, MetricId, ObservationState, PlayerId, RoleId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricValue {
    pub metric: MetricId,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerMatch {
    pub player: PlayerId,
    pub club: ClubId,
    pub competition: CompetitionId,
    pub date: Date,
    pub minutes: u16,
    pub metrics: Vec<MetricValue>,
    pub observation_state: ObservationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerGroupKey {
    pub role: RoleId,
    pub age_bucket: u8,
    pub competition_tier: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowMetric {
    pub metric: MetricId,
    pub total: f64,
    pub per90: Option<f64>,
    pub ewma_per90: Option<f64>,
    pub trend_per90: Option<f64>,
    pub match_rate_variance: Option<f64>,
    pub observations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageSummary {
    pub matches: u32,
    pub minutes: u32,
    pub metric_observations: u32,
    pub low_sample: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceWindow {
    pub player: PlayerId,
    pub as_of: Date,
    pub horizon_days: u16,
    pub metrics: Vec<WindowMetric>,
    pub coverage: CoverageSummary,
}

impl PerformanceWindow {
    pub fn metric(&self, metric: MetricId) -> Option<&WindowMetric> {
        self.metrics.iter().find(|value| value.metric == metric)
    }
}

#[derive(Default)]
struct Accumulator {
    total: f64,
    weighted_total: f64,
    weighted_minutes: f64,
    recent_total: f64,
    recent_minutes: u64,
    prior_total: f64,
    prior_minutes: u64,
    rates: Vec<f64>,
    observations: u32,
}

/// Build a rolling window using observations on or before `as_of` only.
pub fn build_window(
    matches: &[PlayerMatch],
    player: PlayerId,
    as_of: Date,
    horizon_days: u16,
    half_life_days: f64,
    low_sample_minutes: u32,
) -> PerformanceWindow {
    let horizon = i64::from(horizon_days.max(1));
    let half_life = if half_life_days.is_finite() && half_life_days > 0.0 { half_life_days } else { horizon as f64 / 2.0 };
    let split = horizon / 2;
    let mut accumulators: HashMap<MetricId, Accumulator> = HashMap::new();
    let mut match_count = 0_u32;
    let mut minutes = 0_u64;
    let mut metric_observations = 0_u32;

    for observation in matches {
        if observation.player != player {
            continue;
        }
        let age_days = as_of.days_since(observation.date);
        if age_days < 0 || age_days >= horizon {
            continue;
        }
        match_count = match_count.saturating_add(1);
        minutes = minutes.saturating_add(u64::from(observation.minutes));
        let decay = (-std::f64::consts::LN_2 * age_days as f64 / half_life).exp();

        for metric in &observation.metrics {
            if !metric.value.is_finite() {
                continue;
            }
            metric_observations = metric_observations.saturating_add(1);
            let entry = accumulators.entry(metric.metric).or_default();
            entry.total += metric.value;
            entry.observations = entry.observations.saturating_add(1);
            if observation.minutes > 0 {
                entry.weighted_total += metric.value * decay;
                entry.weighted_minutes += f64::from(observation.minutes) * decay;
                entry.rates.push(metric.value * 90.0 / f64::from(observation.minutes));
                if age_days < split {
                    entry.recent_total += metric.value;
                    entry.recent_minutes = entry.recent_minutes.saturating_add(u64::from(observation.minutes));
                } else {
                    entry.prior_total += metric.value;
                    entry.prior_minutes = entry.prior_minutes.saturating_add(u64::from(observation.minutes));
                }
            }
        }
    }

    let capped_minutes = minutes.min(u64::from(u32::MAX)) as u32;
    let mut metrics: Vec<WindowMetric> = accumulators
        .into_iter()
        .map(|(metric, entry)| {
            let per90 = (minutes > 0).then(|| entry.total * 90.0 / minutes as f64);
            let ewma_per90 = (entry.weighted_minutes > 0.0).then(|| entry.weighted_total * 90.0 / entry.weighted_minutes);
            let recent = (entry.recent_minutes > 0).then(|| entry.recent_total * 90.0 / entry.recent_minutes as f64);
            let prior = (entry.prior_minutes > 0).then(|| entry.prior_total * 90.0 / entry.prior_minutes as f64);
            let trend_per90 = match (recent, prior) { (Some(a), Some(b)) => Some(a - b), _ => None };
            let match_rate_variance = variance(&entry.rates);
            WindowMetric { metric, total: entry.total, per90, ewma_per90, trend_per90, match_rate_variance, observations: entry.observations }
        })
        .collect();
    metrics.sort_by_key(|metric| metric.metric);

    PerformanceWindow {
        player,
        as_of,
        horizon_days,
        metrics,
        coverage: CoverageSummary {
            matches: match_count,
            minutes: capped_minutes,
            metric_observations,
            low_sample: capped_minutes < low_sample_minutes,
        },
    }
}

pub fn quantile(values: &[f64], probability: f64) -> Option<f64> {
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
        return None;
    }
    let mut finite: Vec<f64> = values.iter().copied().filter(|value| value.is_finite()).collect();
    if finite.is_empty() {
        return None;
    }
    finite.sort_by(f64::total_cmp);
    if finite.len() == 1 {
        return Some(finite[0]);
    }
    let position = probability * (finite.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        Some(finite[lower])
    } else {
        let fraction = position - lower as f64;
        Some(finite[lower] * (1.0 - fraction) + finite[upper] * fraction)
    }
}

pub fn median(values: &[f64]) -> Option<f64> {
    quantile(values, 0.5)
}

pub fn mad(values: &[f64]) -> Option<f64> {
    let center = median(values)?;
    let deviations: Vec<f64> = values.iter().copied().filter(|value| value.is_finite()).map(|value| (value - center).abs()).collect();
    median(&deviations)
}

pub fn robust_z_score(value: f64, peer_values: &[f64]) -> Option<f64> {
    let center = median(peer_values)?;
    let scale = 1.4826 * mad(peer_values)?;
    if !value.is_finite() || scale <= f64::EPSILON {
        None
    } else {
        Some((value - center) / scale)
    }
}

fn variance(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    Some(values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TranslationObservation {
    pub metric: MetricId,
    pub from: CompetitionId,
    pub to: CompetitionId,
    pub before_per90: f64,
    pub after_per90: f64,
    pub observed_on: Date,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TranslationFactor {
    pub metric: MetricId,
    pub from: CompetitionId,
    pub to: CompetitionId,
    pub factor: f64,
    pub observations: u32,
}

/// Fit a causal, robust league translation ratio and shrink it toward `parent_factor`.
pub fn fit_translation(
    observations: &[TranslationObservation],
    metric: MetricId,
    from: CompetitionId,
    to: CompetitionId,
    cutoff: Date,
    parent_factor: f64,
    shrinkage_k: f64,
) -> Option<TranslationFactor> {
    if !parent_factor.is_finite() || parent_factor <= 0.0 {
        return None;
    }
    let logs: Vec<f64> = observations
        .iter()
        .filter(|observation| observation.metric == metric && observation.from == from && observation.to == to && observation.observed_on <= cutoff)
        .filter_map(|observation| {
            (observation.before_per90.is_finite() && observation.after_per90.is_finite() && observation.before_per90 > 0.0 && observation.after_per90 > 0.0)
                .then(|| (observation.after_per90 / observation.before_per90).ln())
        })
        .collect();
    let local = median(&logs)?;
    let n = logs.len() as f64;
    let k = if shrinkage_k.is_finite() && shrinkage_k >= 0.0 { shrinkage_k } else { 0.0 };
    let weight = if n + k > 0.0 { n / (n + k) } else { 0.0 };
    let shrunk = weight * local + (1.0 - weight) * parent_factor.ln();
    Some(TranslationFactor { metric, from, to, factor: shrunk.exp(), observations: logs.len().min(u32::MAX as usize) as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_window_never_uses_future_matches() {
        let matches = vec![
            PlayerMatch { player: PlayerId(1), club: ClubId(1), competition: CompetitionId(1), date: Date(10), minutes: 90, metrics: vec![MetricValue { metric: MetricId(1), value: 1.0 }], observation_state: ObservationState::Measured },
            PlayerMatch { player: PlayerId(1), club: ClubId(1), competition: CompetitionId(1), date: Date(12), minutes: 90, metrics: vec![MetricValue { metric: MetricId(1), value: 99.0 }], observation_state: ObservationState::Measured },
        ];
        let window = build_window(&matches, PlayerId(1), Date(10), 30, 15.0, 180);
        assert_eq!(window.metric(MetricId(1)).unwrap().total, 1.0);
    }

    #[test]
    fn missing_metric_is_not_synthesized_as_zero() {
        let matches = vec![PlayerMatch { player: PlayerId(1), club: ClubId(1), competition: CompetitionId(1), date: Date(10), minutes: 90, metrics: Vec::new(), observation_state: ObservationState::SourceUnavailable }];
        let window = build_window(&matches, PlayerId(1), Date(10), 30, 15.0, 180);
        assert!(window.metrics.is_empty());
    }

    #[test]
    fn robust_z_score_resists_outlier_location_shift() {
        let peers = [1.0, 1.1, 0.9, 1.05, 30.0];
        let score = robust_z_score(1.2, &peers).unwrap();
        assert!(score > 1.0);
    }

    #[test]
    fn translation_respects_cutoff() {
        let observations = [
            TranslationObservation { metric: MetricId(1), from: CompetitionId(1), to: CompetitionId(2), before_per90: 1.0, after_per90: 0.8, observed_on: Date(5) },
            TranslationObservation { metric: MetricId(1), from: CompetitionId(1), to: CompetitionId(2), before_per90: 1.0, after_per90: 10.0, observed_on: Date(50) },
        ];
        let fit = fit_translation(&observations, MetricId(1), CompetitionId(1), CompetitionId(2), Date(10), 1.0, 0.0).unwrap();
        assert!((fit.factor - 0.8).abs() < 1e-9);
    }
}
