use crate::simulation::PlayerTeamImpactProfile;
use footy_state::{AbilityVector, PlayerState};

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
}
