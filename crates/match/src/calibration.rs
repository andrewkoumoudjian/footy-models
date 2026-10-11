use crate::simulation::PlayerTeamImpactProfile;
use footy_state::{AbilityVector, PlayerState};

/// One strictly out-of-sample match result used to calibrate the probability surface emitted by
/// the causal simulator. `actual` is ordered home/draw/away as 0/1/2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutcomeCalibrationSample {
    pub probabilities: [f64; 3],
    pub actual: usize,
}

/// Small post-hoc calibrator for the simulator's H/D/A distribution.
///
/// Temperature corrects global over/under-confidence while a single draw-logit offset corrects a
/// systematic draw prior miss. It never sees team/player features and therefore cannot become a
/// second result model; it only reshapes probabilities already produced by the causal match graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutcomeCalibrator {
    pub temperature: f64,
    pub draw_logit_bias: f64,
}

impl Default for OutcomeCalibrator {
    fn default() -> Self {
        Self { temperature: 1.0, draw_logit_bias: 0.0 }
    }
}

impl OutcomeCalibrator {
    pub fn apply(self, probabilities: [f64; 3]) -> Option<[f64; 3]> {
        if !self.temperature.is_finite()
            || self.temperature <= 0.0
            || !self.draw_logit_bias.is_finite()
            || !valid_probabilities(probabilities)
        {
            return None;
        }
        let mut logits = [0.0_f64; 3];
        for index in 0..3 {
            logits[index] = probabilities[index].max(1e-12).ln() / self.temperature;
        }
        logits[1] += self.draw_logit_bias;
        let max_logit = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mut weights = [0.0_f64; 3];
        for index in 0..3 {
            weights[index] = (logits[index] - max_logit).exp();
        }
        let denominator = weights.iter().sum::<f64>();
        if !denominator.is_finite() || denominator <= 0.0 {
            return None;
        }
        Some(weights.map(|weight| weight / denominator))
    }

    /// Deterministic grid fit minimizing multiclass log loss. The grid deliberately stays small:
    /// these parameters are calibration corrections, not another predictive model.
    pub fn fit(samples: &[OutcomeCalibrationSample]) -> Option<Self> {
        if samples.is_empty()
            || samples.iter().any(|sample| sample.actual >= 3 || !valid_probabilities(sample.probabilities))
        {
            return None;
        }

        let mut best = Self::default();
        let mut best_loss = mean_log_loss(samples, best)?;
        // 0.75..=3.00 in 0.05 steps; -1.00..=1.50 in 0.05 steps.
        for temperature_step in 15..=60 {
            let temperature = f64::from(temperature_step) / 20.0;
            for bias_step in -20..=30 {
                let candidate = Self {
                    temperature,
                    draw_logit_bias: f64::from(bias_step) / 20.0,
                };
                let loss = mean_log_loss(samples, candidate)?;
                // Prefer the lower loss; exact ties prefer the correction closest to identity.
                let candidate_distance = (candidate.temperature - 1.0).abs() + candidate.draw_logit_bias.abs();
                let best_distance = (best.temperature - 1.0).abs() + best.draw_logit_bias.abs();
                if loss < best_loss - 1e-12
                    || ((loss - best_loss).abs() <= 1e-12 && candidate_distance < best_distance)
                {
                    best = candidate;
                    best_loss = loss;
                }
            }
        }
        Some(best)
    }
}

fn valid_probabilities(probabilities: [f64; 3]) -> bool {
    probabilities.iter().all(|value| value.is_finite() && *value >= 0.0)
        && probabilities.iter().sum::<f64>() > 0.0
}

fn mean_log_loss(samples: &[OutcomeCalibrationSample], calibrator: OutcomeCalibrator) -> Option<f64> {
    let mut total = 0.0;
    for sample in samples {
        let calibrated = calibrator.apply(sample.probabilities)?;
        total -= calibrated[sample.actual].max(1e-12).ln();
    }
    Some(total / samples.len() as f64)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityImpactWeights {
    /// Order follows `AbilityVector::components()`.
    pub attack: [f64; 10],
    pub defense: [f64; 10],
    pub attack_reference: f64,
    pub defense_reference: f64,
    pub attack_log_weight: f64,
    pub defense_log_weight: f64,
}

impl AbilityImpactWeights {
    pub fn validate(self) -> bool {
        self.attack.iter().chain(self.defense.iter()).all(|value| value.is_finite() && *value >= 0.0)
            && self.attack.iter().any(|value| *value > 0.0)
            && self.defense.iter().any(|value| *value > 0.0)
            && self.attack_reference.is_finite()
            && self.attack_reference > 0.0
            && self.defense_reference.is_finite()
            && self.defense_reference > 0.0
            && self.attack_log_weight.is_finite()
            && self.defense_log_weight.is_finite()
    }
}

/// Convert the repository's FM-derived source-neutral ability vector into the latent player term
/// consumed by the specific-match simulator. Weight choice remains a calibrated/model-versioned
/// input rather than a hard-coded claim about which football attributes matter for every role.
pub fn latent_team_impact_from_state(
    state: &PlayerState,
    weights: AbilityImpactWeights,
) -> Option<PlayerTeamImpactProfile> {
    if !weights.validate() { return None; }
    let attack_strength = weighted_mean(state.ability, weights.attack)?;
    let defense_strength = weighted_mean(state.ability, weights.defense)?;
    Some(PlayerTeamImpactProfile {
        player: state.player,
        attack_strength,
        defense_strength,
        attack_reference: weights.attack_reference,
        defense_reference: weights.defense_reference,
        attack_log_weight: weights.attack_log_weight,
        defense_log_weight: weights.defense_log_weight,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FatigueLoadParameters {
    /// Recent-minute load at which the unrecovered component equals one unit.
    pub reference_minutes: f64,
    /// Exponential recovery half-life after the latest fixture.
    pub recovery_half_life_days: f64,
    /// Optional extra congestion term from fixtures inside the recent window.
    pub congestion_weight: f64,
    pub reference_fixtures: f64,
}

impl FatigueLoadParameters {
    pub fn validate(self) -> bool {
        self.reference_minutes.is_finite()
            && self.reference_minutes > 0.0
            && self.recovery_half_life_days.is_finite()
            && self.recovery_half_life_days > 0.0
            && self.congestion_weight.is_finite()
            && self.congestion_weight >= 0.0
            && self.reference_fixtures.is_finite()
            && self.reference_fixtures > 0.0
    }
}

/// A bounded, source-neutral fatigue primitive. It is intentionally transparent and replaceable:
/// recent load creates pressure, rest decays it exponentially, and fixture congestion is additive.
pub fn fatigue_from_load(
    recent_minutes: f64,
    recent_fixtures: f64,
    rest_days: f64,
    parameters: FatigueLoadParameters,
) -> Option<f64> {
    if !parameters.validate()
        || !recent_minutes.is_finite()
        || recent_minutes < 0.0
        || !recent_fixtures.is_finite()
        || recent_fixtures < 0.0
        || !rest_days.is_finite()
        || rest_days < 0.0
    {
        return None;
    }
    let load = recent_minutes / parameters.reference_minutes;
    let congestion = parameters.congestion_weight * recent_fixtures / parameters.reference_fixtures;
    let recovery = (-std::f64::consts::LN_2 * rest_days / parameters.recovery_half_life_days).exp();
    Some(((load + congestion) * recovery).clamp(0.0, 1.0))
}

fn weighted_mean(ability: AbilityVector, weights: [f64; 10]) -> Option<f64> {
    let components = ability.components();
    let denominator = weights.iter().sum::<f64>();
    if denominator <= f64::EPSILON { return None; }
    let numerator = components.iter().zip(weights).map(|(value, weight)| f64::from(*value) * weight).sum::<f64>();
    Some(numerator / denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::{Date, FeatureSetVersion, PlayerId};
    use footy_state::{DevelopmentVector, PerformanceVector, RoleVector, StateLineage, UsageVector};

    fn player() -> PlayerState {
        PlayerState::new(
            PlayerId(1), Date(100), Date(0), AbilityVector::from_components([10.0, 14.0, 16.0, 15.0, 8.0, 7.0, 12.0, 13.0, 11.0, 1.0]),
            DevelopmentVector::default(), PerformanceVector { metrics: Vec::new(), minutes: 0 }, UsageVector::default(), 1.0,
            RoleVector { definition_version: 1, scores: Vec::new() }, None,
            StateLineage { fm_snapshot_date: Some(Date(100)), performance_as_of: None, feature_set_version: FeatureSetVersion(1) },
        ).unwrap()
    }

    #[test]
    fn ability_calibration_keeps_attack_and_defence_separate() {
        let weights = AbilityImpactWeights {
            attack: [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0],
            defense: [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.5, 1.0, 1.0, 0.0],
            attack_reference: 10.0, defense_reference: 10.0, attack_log_weight: 0.1, defense_log_weight: 0.1,
        };
        let impact = latent_team_impact_from_state(&player(), weights).unwrap();
        assert!(impact.attack_strength > impact.defense_strength);
    }

    #[test]
    fn rest_decays_fatigue() {
        let parameters = FatigueLoadParameters { reference_minutes: 450.0, recovery_half_life_days: 3.0, congestion_weight: 0.2, reference_fixtures: 5.0 };
        let short_rest = fatigue_from_load(450.0, 5.0, 1.0, parameters).unwrap();
        let long_rest = fatigue_from_load(450.0, 5.0, 6.0, parameters).unwrap();
        assert!(short_rest > long_rest);
    }

    #[test]
    fn identity_outcome_calibration_preserves_probabilities() {
        let calibrated = OutcomeCalibrator::default().apply([0.55, 0.25, 0.20]).unwrap();
        for (actual, expected) in calibrated.into_iter().zip([0.55, 0.25, 0.20]) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn outcome_calibration_learns_missing_draw_mass() {
        let samples = [
            OutcomeCalibrationSample { probabilities: [0.65, 0.15, 0.20], actual: 1 },
            OutcomeCalibrationSample { probabilities: [0.20, 0.15, 0.65], actual: 1 },
            OutcomeCalibrationSample { probabilities: [0.60, 0.20, 0.20], actual: 1 },
            OutcomeCalibrationSample { probabilities: [0.20, 0.20, 0.60], actual: 1 },
            OutcomeCalibrationSample { probabilities: [0.55, 0.20, 0.25], actual: 0 },
            OutcomeCalibrationSample { probabilities: [0.25, 0.20, 0.55], actual: 2 },
        ];
        let first = OutcomeCalibrator::fit(&samples).unwrap();
        let second = OutcomeCalibrator::fit(&samples).unwrap();
        assert_eq!(first, second, "calibration fit must be deterministic");
        let raw = [0.60, 0.15, 0.25];
        let calibrated = first.apply(raw).unwrap();
        assert!(calibrated[1] > raw[1], "draw-underprediction history should add draw mass");
    }
}
