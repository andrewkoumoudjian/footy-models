#![forbid(unsafe_code)]
//! Source-neutral, causal player-state representation for valuation and forecasting.

use footy_core::{Confidence, Date, FeatureSetVersion, MetricId, ObservationState, PlayerId, RoleId};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AbilityVector {
    pub technical: f32,
    pub ball_progression: f32,
    pub creativity: f32,
    pub finishing: f32,
    pub defending: f32,
    pub aerial: f32,
    pub physical: f32,
    pub decision_making: f32,
    pub mentality: f32,
    pub goalkeeping: f32,
}

impl AbilityVector {
    pub fn components(self) -> [f32; 10] {
        [self.technical, self.ball_progression, self.creativity, self.finishing, self.defending, self.aerial, self.physical, self.decision_making, self.mentality, self.goalkeeping]
    }

    pub fn from_components(values: [f32; 10]) -> Self {
        Self { technical: values[0], ball_progression: values[1], creativity: values[2], finishing: values[3], defending: values[4], aerial: values[5], physical: values[6], decision_making: values[7], mentality: values[8], goalkeeping: values[9] }
    }

    pub fn mean(self) -> f32 {
        self.components().iter().sum::<f32>() / 10.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DevelopmentVector {
    pub ability_velocity_per_year: f32,
    pub ability_acceleration_per_year2: f32,
    pub breadth: f32,
    pub consistency: f32,
    pub ca_realization: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilitySnapshot {
    pub date: Date,
    pub ability: AbilityVector,
}

pub fn derive_development(samples: &[AbilitySnapshot], current_ability: Option<u16>, potential_ability: Option<u16>) -> DevelopmentVector {
    let mut ordered = samples.to_vec();
    ordered.sort_by_key(|sample| sample.date);
    let realization = match (current_ability, potential_ability) {
        (Some(current), Some(potential)) if potential > 0 => Some((current as f32 / potential as f32).clamp(0.0, 2.0)),
        _ => None,
    };
    if ordered.len() < 2 {
        return DevelopmentVector { ca_realization: realization, ..DevelopmentVector::default() };
    }

    let latest = ordered[ordered.len() - 1];
    let previous = ordered[ordered.len() - 2];
    let years = (latest.date.days_since(previous.date) as f32 / 365.2425).max(1.0 / 365.2425);
    let latest_delta = subtract(latest.ability, previous.ability);
    let velocity = latest_delta.mean() / years;
    let breadth = latest_delta.components().iter().filter(|value| **value > 0.0).count() as f32 / 10.0;

    let mut improving = 0_u32;
    let mut intervals = 0_u32;
    for pair in ordered.windows(2) {
        if pair[1].date <= pair[0].date { continue; }
        intervals += 1;
        if pair[1].ability.mean() > pair[0].ability.mean() { improving += 1; }
    }
    let consistency = if intervals > 0 { improving as f32 / intervals as f32 } else { 0.0 };

    let acceleration = if ordered.len() >= 3 {
        let older = ordered[ordered.len() - 3];
        let prior_years = (previous.date.days_since(older.date) as f32 / 365.2425).max(1.0 / 365.2425);
        let prior_velocity = subtract(previous.ability, older.ability).mean() / prior_years;
        (velocity - prior_velocity) / years
    } else { 0.0 };

    DevelopmentVector { ability_velocity_per_year: velocity, ability_acceleration_per_year2: acceleration, breadth, consistency, ca_realization: realization }
}

fn subtract(left: AbilityVector, right: AbilityVector) -> AbilityVector {
    let a = left.components();
    let b = right.components();
    let mut values = [0.0; 10];
    for index in 0..10 { values[index] = a[index] - b[index]; }
    AbilityVector::from_components(values)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoleScore {
    pub role: RoleId,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoleVector {
    pub definition_version: u64,
    pub scores: Vec<RoleScore>,
}

impl RoleVector {
    pub fn score(&self, role: RoleId) -> Option<f32> {
        self.scores.iter().find(|value| value.role == role).map(|value| value.score)
    }

    pub fn normalized(mut self) -> Self {
        let norm = self.scores.iter().map(|value| value.score.max(0.0).powi(2)).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut self.scores { value.score = value.score.max(0.0) / norm; }
        }
        self.scores.sort_by_key(|value| value.role);
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UsageVector {
    pub minutes_30: u32,
    pub minutes_90: u32,
    pub minutes_365: u32,
    pub starts_365: u16,
    pub availability_ratio: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PsychologyMeasure {
    pub value: f32,
    pub confidence: Confidence,
}

impl Default for PsychologyMeasure {
    fn default() -> Self { Self { value: 0.0, confidence: Confidence::ZERO } }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PsychologyVector {
    pub professionalism: PsychologyMeasure,
    pub ambition: PsychologyMeasure,
    pub consistency: PsychologyMeasure,
    pub pressure: PsychologyMeasure,
    pub adaptability: PsychologyMeasure,
    pub leadership: PsychologyMeasure,
    pub teamwork: PsychologyMeasure,
    pub discipline: PsychologyMeasure,
}

impl PsychologyVector {
    pub fn weighted_index(&self, weights: [f32; 8]) -> Option<(f32, f32)> {
        let measures = [self.professionalism, self.ambition, self.consistency, self.pressure, self.adaptability, self.leadership, self.teamwork, self.discipline];
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        let mut confidence_weight = 0.0;
        for (measure, weight) in measures.iter().zip(weights) {
            let effective = weight.max(0.0) * measure.confidence.get();
            numerator += measure.value * effective;
            denominator += effective;
            confidence_weight += weight.max(0.0);
        }
        if denominator == 0.0 { None } else {
            let confidence = if confidence_weight > 0.0 { (denominator / confidence_weight).clamp(0.0, 1.0) } else { 0.0 };
            Some((numerator / denominator, confidence))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformanceFeature {
    pub metric: MetricId,
    pub value: f32,
    pub state: ObservationState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceVector {
    pub metrics: Vec<PerformanceFeature>,
    pub minutes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateLineage {
    pub fm_snapshot_date: Option<Date>,
    pub performance_as_of: Option<Date>,
    pub feature_set_version: FeatureSetVersion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub player: PlayerId,
    pub as_of: Date,
    pub age_years: f32,
    pub ability: AbilityVector,
    pub development: DevelopmentVector,
    pub performance: PerformanceVector,
    pub usage: UsageVector,
    pub competition_strength: f32,
    pub roles: RoleVector,
    pub psychology: Option<PsychologyVector>,
    pub lineage: StateLineage,
}

impl PlayerState {
    pub fn new(
        player: PlayerId,
        as_of: Date,
        birth_date: Date,
        ability: AbilityVector,
        development: DevelopmentVector,
        performance: PerformanceVector,
        usage: UsageVector,
        competition_strength: f32,
        roles: RoleVector,
        psychology: Option<PsychologyVector>,
        lineage: StateLineage,
    ) -> Result<Self, &'static str> {
        if birth_date > as_of { return Err("birth date cannot be after state date"); }
        if !competition_strength.is_finite() || competition_strength <= 0.0 { return Err("competition strength must be positive and finite"); }
        if lineage.fm_snapshot_date.is_some_and(|date| date > as_of) || lineage.performance_as_of.is_some_and(|date| date > as_of) {
            return Err("state lineage cannot reference future observations");
        }
        Ok(Self { player, as_of, age_years: age_years(birth_date, as_of), ability, development, performance, usage, competition_strength, roles: roles.normalized(), psychology, lineage })
    }
}

pub fn age_years(birth_date: Date, as_of: Date) -> f32 {
    (as_of.days_since(birth_date) as f32 / 365.2425).max(0.0)
}

pub fn capability_gap(expected_performance: f32, observed_performance: f32) -> Option<f32> {
    (expected_performance.is_finite() && observed_performance.is_finite()).then_some(expected_performance - observed_performance)
}

pub fn opportunity_suppression(expected_minutes: f32, actual_minutes: f32) -> Option<f32> {
    if !expected_minutes.is_finite() || !actual_minutes.is_finite() || expected_minutes <= 0.0 || actual_minutes < 0.0 {
        return None;
    }
    Some(((expected_minutes - actual_minutes) / expected_minutes).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ability(value: f32) -> AbilityVector { AbilityVector::from_components([value; 10]) }

    #[test]
    fn development_uses_only_ordered_historical_snapshots() {
        let development = derive_development(&[
            AbilitySnapshot { date: Date(0), ability: ability(10.0) },
            AbilitySnapshot { date: Date(365), ability: ability(12.0) },
            AbilitySnapshot { date: Date(730), ability: ability(15.0) },
        ], Some(120), Some(180));
        assert!(development.ability_velocity_per_year > 2.9);
        assert_eq!(development.breadth, 1.0);
        assert_eq!(development.consistency, 1.0);
    }

    #[test]
    fn player_state_rejects_future_lineage() {
        let result = PlayerState::new(
            PlayerId(1), Date(10), Date(0), ability(1.0), DevelopmentVector::default(),
            PerformanceVector { metrics: Vec::new(), minutes: 0 }, UsageVector::default(), 1.0,
            RoleVector { definition_version: 1, scores: Vec::new() }, None,
            StateLineage { fm_snapshot_date: Some(Date(11)), performance_as_of: None, feature_set_version: FeatureSetVersion(1) },
        );
        assert!(result.is_err());
    }

    #[test]
    fn opportunity_suppression_is_bounded() {
        assert_eq!(opportunity_suppression(900.0, 450.0), Some(0.5));
        assert_eq!(opportunity_suppression(900.0, 1_000.0), Some(0.0));
    }
}
