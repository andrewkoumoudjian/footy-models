use crate::{MetricValue, quantile};
use footy_core::{ClubId, CompetitionId, Date, MatchId, MetricId, ObservationState};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct TeamMatch {
    pub match_id: MatchId,
    pub club: ClubId,
    pub opponent: ClubId,
    pub competition: CompetitionId,
    pub date: Date,
    pub is_home: bool,
    pub goals_for: Option<u8>,
    pub goals_against: Option<u8>,
    pub metrics: Vec<MetricValue>,
    pub observation_state: ObservationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOutcome {
    Win,
    Draw,
    Loss,
}

impl TeamMatch {
    pub fn outcome(&self) -> Option<MatchOutcome> {
        match (self.goals_for, self.goals_against) {
            (Some(for_goals), Some(against_goals)) if for_goals > against_goals => Some(MatchOutcome::Win),
            (Some(for_goals), Some(against_goals)) if for_goals == against_goals => Some(MatchOutcome::Draw),
            (Some(_), Some(_)) => Some(MatchOutcome::Loss),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamWindowMetric {
    pub metric: MetricId,
    pub total: f64,
    pub per_match: f64,
    pub ewma_per_match: f64,
    pub trend_per_match: Option<f64>,
    pub match_variance: Option<f64>,
    pub observations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamForm {
    pub matches: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub goals_for: u32,
    pub goals_against: u32,
    pub points: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeamPerformanceWindow {
    pub club: ClubId,
    pub as_of: Date,
    pub horizon_days: u16,
    pub metrics: Vec<TeamWindowMetric>,
    pub form: TeamForm,
}

impl TeamPerformanceWindow {
    pub fn metric(&self, metric: MetricId) -> Option<&TeamWindowMetric> {
        self.metrics.iter().find(|value| value.metric == metric)
    }
}

#[derive(Default)]
struct TeamAccumulator {
    total: f64,
    weighted_total: f64,
    weighted_matches: f64,
    recent_total: f64,
    recent_matches: u32,
    prior_total: f64,
    prior_matches: u32,
    values: Vec<f64>,
    observations: u32,
}

/// Build a causal team form/statistics window from source-neutral match observations.
/// Missing metrics are not synthesized, and future matches are ignored.
pub fn build_team_window(
    matches: &[TeamMatch],
    club: ClubId,
    as_of: Date,
    horizon_days: u16,
    half_life_days: f64,
) -> TeamPerformanceWindow {
    let horizon = i64::from(horizon_days.max(1));
    let half_life = if half_life_days.is_finite() && half_life_days > 0.0 {
        half_life_days
    } else {
        horizon as f64 / 2.0
    };
    let split = horizon / 2;
    let mut accumulators = HashMap::<MetricId, TeamAccumulator>::new();
    let mut form = TeamForm { matches: 0, wins: 0, draws: 0, losses: 0, goals_for: 0, goals_against: 0, points: 0 };

    for observation in matches {
        if observation.club != club { continue; }
        let age_days = as_of.days_since(observation.date);
        if age_days < 0 || age_days >= horizon { continue; }
        let decay = (-std::f64::consts::LN_2 * age_days as f64 / half_life).exp();

        if let Some(outcome) = observation.outcome() {
            form.matches = form.matches.saturating_add(1);
            form.goals_for = form.goals_for.saturating_add(u32::from(observation.goals_for.unwrap_or(0)));
            form.goals_against = form.goals_against.saturating_add(u32::from(observation.goals_against.unwrap_or(0)));
            match outcome {
                MatchOutcome::Win => { form.wins = form.wins.saturating_add(1); form.points = form.points.saturating_add(3); }
                MatchOutcome::Draw => { form.draws = form.draws.saturating_add(1); form.points = form.points.saturating_add(1); }
                MatchOutcome::Loss => form.losses = form.losses.saturating_add(1),
            }
        }

        for metric in &observation.metrics {
            if !metric.value.is_finite() { continue; }
            let entry = accumulators.entry(metric.metric).or_default();
            entry.total += metric.value;
            entry.weighted_total += metric.value * decay;
            entry.weighted_matches += decay;
            entry.values.push(metric.value);
            entry.observations = entry.observations.saturating_add(1);
            if age_days < split {
                entry.recent_total += metric.value;
                entry.recent_matches = entry.recent_matches.saturating_add(1);
            } else {
                entry.prior_total += metric.value;
                entry.prior_matches = entry.prior_matches.saturating_add(1);
            }
        }
    }

    let mut metrics: Vec<TeamWindowMetric> = accumulators
        .into_iter()
        .filter_map(|(metric, entry)| {
            if entry.observations == 0 { return None; }
            let per_match = entry.total / f64::from(entry.observations);
            let ewma_per_match = if entry.weighted_matches > 0.0 { entry.weighted_total / entry.weighted_matches } else { per_match };
            let trend_per_match = if entry.recent_matches > 0 && entry.prior_matches > 0 {
                Some(entry.recent_total / f64::from(entry.recent_matches) - entry.prior_total / f64::from(entry.prior_matches))
            } else { None };
            Some(TeamWindowMetric {
                metric,
                total: entry.total,
                per_match,
                ewma_per_match,
                trend_per_match,
                match_variance: sample_variance(&entry.values),
                observations: entry.observations,
            })
        })
        .collect();
    metrics.sort_by_key(|value| value.metric);
    TeamPerformanceWindow { club, as_of, horizon_days, metrics, form }
}

/// Quantile of a team's metric across matches in a causal window. This is useful for scenario
/// calibration without pretending a heavy-tailed match statistic is Gaussian.
pub fn team_metric_quantile(
    matches: &[TeamMatch],
    club: ClubId,
    metric: MetricId,
    as_of: Date,
    horizon_days: u16,
    probability: f64,
) -> Option<f64> {
    let horizon = i64::from(horizon_days.max(1));
    let values: Vec<f64> = matches
        .iter()
        .filter(|value| value.club == club)
        .filter(|value| {
            let age = as_of.days_since(value.date);
            age >= 0 && age < horizon
        })
        .flat_map(|value| value.metrics.iter())
        .filter(|value| value.metric == metric && value.value.is_finite())
        .map(|value| value.value)
        .collect();
    quantile(&values, probability)
}

fn sample_variance(values: &[f64]) -> Option<f64> {
    if values.len() < 2 { return None; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    Some(values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn match_row(date: i32, goals_for: u8, goals_against: u8, metric: f64) -> TeamMatch {
        TeamMatch {
            match_id: MatchId(date as u64 + 100), club: ClubId(1), opponent: ClubId(2), competition: CompetitionId(1), date: Date(date), is_home: true,
            goals_for: Some(goals_for), goals_against: Some(goals_against), metrics: vec![MetricValue { metric: MetricId(1), value: metric }], observation_state: ObservationState::Measured,
        }
    }

    #[test]
    fn team_window_is_causal_and_computes_form() {
        let matches = [match_row(5, 2, 0, 10.0), match_row(8, 1, 1, 20.0), match_row(12, 9, 0, 999.0)];
        let window = build_team_window(&matches, ClubId(1), Date(10), 30, 15.0);
        assert_eq!(window.form.matches, 2);
        assert_eq!(window.form.points, 4);
        assert_eq!(window.metric(MetricId(1)).unwrap().total, 30.0);
    }

    #[test]
    fn team_window_does_not_turn_missing_into_zero() {
        let row = TeamMatch { match_id: MatchId(1), club: ClubId(1), opponent: ClubId(2), competition: CompetitionId(1), date: Date(5), is_home: false, goals_for: None, goals_against: None, metrics: Vec::new(), observation_state: ObservationState::SourceUnavailable };
        let window = build_team_window(&[row], ClubId(1), Date(10), 30, 15.0);
        assert!(window.metrics.is_empty());
        assert_eq!(window.form.matches, 0);
    }
}
