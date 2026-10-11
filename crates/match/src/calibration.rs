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
    ///
    /// A small-sample shrinkage penalty toward identity is included: on `n` samples the
    /// penalized objective is `mean_log_loss + 2 * ((T - 1)^2 + bias^2) / n`, so a
    /// two-parameter correction must earn roughly its AIC optimism before moving away from
    /// the raw simulator output. Without this, tiny calibration windows (e.g. 6-30
    /// fixtures) routinely select extreme temperatures that hurt the next window out of
    /// sample. The penalty vanishes as `n` grows, leaving large-sample fits unchanged.
    pub fn fit(samples: &[OutcomeCalibrationSample]) -> Option<Self> {
        if samples.is_empty()
            || samples.iter().any(|sample| sample.actual >= 3 || !valid_probabilities(sample.probabilities))
        {
            return None;
        }

        let penalty_scale = 2.0 / samples.len() as f64;
        let penalized = |candidate: Self| -> Option<f64> {
            let loss = mean_log_loss(samples, candidate)?;
            let distance = (candidate.temperature - 1.0).powi(2) + candidate.draw_logit_bias.powi(2);
            Some(loss + penalty_scale * distance)
        };
        let mut best = Self::default();
        let mut best_loss = penalized(best)?;
        // 0.75..=3.00 in 0.05 steps; -1.00..=1.50 in 0.05 steps.
        for temperature_step in 15..=60 {
            let temperature = f64::from(temperature_step) / 20.0;
            for bias_step in -20..=30 {
                let candidate = Self {
                    temperature,
                    draw_logit_bias: f64::from(bias_step) / 20.0,
                };
                let loss = penalized(candidate)?;
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

/// Dixon–Coles (1997) low-score dependence multiplier for one exact scoreline.
///
/// Independent Poisson systematically misprices 0-0, 1-0, 0-1 and 1-1; the correction
/// reallocates mass among exactly those four cells while preserving the marginals
/// of an *independent Poisson base*. This does NOT preserve marginals of an
/// arbitrary correlated empirical path table. Sign convention: negative `rho` (the empirically typical case,
/// around -0.03) boosts both draws and suppresses both 1-0/0-1 results. `rho = 0` is the
/// identity. Returns `None` when inputs are non-finite or `rho` would flip a cell sign.
pub fn dixon_coles_tau(
    home_goals: u32,
    away_goals: u32,
    home_mean: f64,
    away_mean: f64,
    rho: f64,
) -> Option<f64> {
    if !home_mean.is_finite() || !away_mean.is_finite() || !rho.is_finite()
        || home_mean <= 0.0
        || away_mean <= 0.0
    {
        return None;
    }
    let tau = match (home_goals, away_goals) {
        (0, 0) => 1.0 - home_mean * away_mean * rho,
        (1, 0) => 1.0 + away_mean * rho,
        (0, 1) => 1.0 + home_mean * rho,
        (1, 1) => 1.0 - rho,
        _ => 1.0,
    };
    (tau.is_finite() && tau > 0.0).then_some(tau)
}

/// Validity of `rho` against both goal means: every affected cell multiplier must stay
/// positive. The feasible interval moves with the means, so callers must check per fixture.
pub fn dixon_coles_valid_rho(home_mean: f64, away_mean: f64, rho: f64) -> bool {
    [(0, 0), (1, 0), (0, 1), (1, 1)]
        .iter()
        .all(|(home, away)| dixon_coles_tau(*home, *away, home_mean, away_mean, rho).is_some())
}

/// Apply the Dixon–Coles correction to a joint scoreline table and renormalize.
///
/// `cells` holds `(home_goals, away_goals, probability)` triples such as the engine's
/// exported path-observed scoreline distribution; probabilities must be non-negative.
/// Applied to a non-Poisson path table, the normalization may change team-goal
/// marginals, W/D/L probabilities and overs/unders. Treat the result as a
/// separate diagnostic candidate, not a coherent reweighting of all path outputs.
/// Output triples are in the same order and sum to one. `rho = 0` reproduces the input
/// shares exactly (up to renormalization, the identity when input already sums to one).
/// Returns `None` on invalid input or an infeasible `rho` for these means.
pub fn apply_dixon_coles(
    cells: &[(u32, u32, f64)],
    home_mean: f64,
    away_mean: f64,
    rho: f64,
) -> Option<Vec<(u32, u32, f64)>> {
    if cells.is_empty()
        || cells.iter().any(|(_, _, probability)| !probability.is_finite() || *probability < 0.0)
        || !dixon_coles_valid_rho(home_mean, away_mean, rho)
    {
        return None;
    }
    let mut adjusted = Vec::with_capacity(cells.len());
    for (home, away, probability) in cells {
        let tau = dixon_coles_tau(*home, *away, home_mean, away_mean, rho)?;
        adjusted.push((*home, *away, probability * tau));
    }
    let total: f64 = adjusted.iter().map(|(_, _, probability)| probability).sum();
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    for (_, _, probability) in &mut adjusted {
        *probability /= total;
    }
    Some(adjusted)
}

/// One realized scoreline with the pre-match Poisson means used for Dixon–Coles fitting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DixonColesSample {
    pub home_mean: f64,
    pub away_mean: f64,
    pub home_goals: u32,
    pub away_goals: u32,
}

fn dixon_coles_cell_log_loss(
    sample: DixonColesSample,
    rho: f64,
    max_goals: u32,
) -> Option<f64> {
    if !dixon_coles_valid_rho(sample.home_mean, sample.away_mean, rho) {
        return None;
    }
    let mut total = 0.0;
    let mut target = 0.0;
    for home in 0..=max_goals {
        let home_probability = poisson_probability(sample.home_mean, home)?;
        for away in 0..=max_goals {
            let tau = dixon_coles_tau(home, away, sample.home_mean, sample.away_mean, rho)?;
            let probability = home_probability * poisson_probability(sample.away_mean, away)? * tau;
            total += probability;
            if home == sample.home_goals && away == sample.away_goals {
                target = probability;
            }
        }
    }
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    // Renormalize over the truncated grid so the floor, not the tail, absorbs truncation.
    Some(-(target / total).max(1e-9).ln())
}

fn poisson_probability(mean: f64, goals: u32) -> Option<f64> {
    if !mean.is_finite() || mean <= 0.0 {
        return None;
    }
    let mut probability = (-mean).exp();
    for k in 1..=goals {
        probability *= mean / f64::from(k);
        if !probability.is_finite() {
            return None;
        }
    }
    Some(probability)
}

/// Deterministic shrunk grid fit of the Dixon–Coles `rho` minimizing exact-score log loss.
///
/// Grid is -0.100..=0.050 in 0.005 steps (31 candidates): the correction is a local
/// low-score adjustment, not a whole-matrix dependence model, so wide search is
/// overfitting surface. Objective adds `2 * (rho / 0.05)^2 / n` toward zero — same
/// AIC-style shrinkage philosophy as [`OutcomeCalibrator::fit`]. Returns 0.0 (identity)
/// when no candidate beats it. `None` on empty/invalid samples.
pub fn fit_dixon_coles_rho(samples: &[DixonColesSample]) -> Option<f64> {
    if samples.is_empty()
        || samples.iter().any(|sample| {
            !sample.home_mean.is_finite()
                || !sample.away_mean.is_finite()
                || sample.home_mean <= 0.0
                || sample.away_mean <= 0.0
        })
    {
        return None;
    }
    // Truncation must cover every observed scoreline with headroom for the tail.
    let max_goals = samples
        .iter()
        .map(|sample| sample.home_goals.max(sample.away_goals))
        .max()
        .unwrap_or(0)
        .max(6);
    let penalty_scale = 2.0 / samples.len() as f64;
    let penalized = |rho: f64| -> Option<f64> {
        let mut total = 0.0;
        for sample in samples {
            total += dixon_coles_cell_log_loss(*sample, rho, max_goals)?;
        }
        let loss = total / samples.len() as f64;
        Some(loss + penalty_scale * (rho / 0.05).powi(2))
    };
    let mut best = 0.0_f64;
    let mut best_loss = penalized(0.0)?;
    let mut rho_milli = -100;
    while rho_milli <= 50 {
        let candidate = f64::from(rho_milli) / 1000.0;
        if let Some(loss) = penalized(candidate) {
            if loss < best_loss - 1e-12
                || ((loss - best_loss).abs() <= 1e-12 && candidate.abs() < best.abs())
            {
                best = candidate;
                best_loss = loss;
            }
        }
        rho_milli += 5;
    }
    Some(best)
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

    #[test]
    fn shrinkage_holds_near_identity_on_half_a_dozen_noisy_samples() {
        // Six fixtures cannot identify two calibration parameters: a couple of mild
        // surprises must not select an extreme temperature.
        let samples = [
            OutcomeCalibrationSample { probabilities: [0.50, 0.25, 0.25], actual: 2 },
            OutcomeCalibrationSample { probabilities: [0.45, 0.25, 0.30], actual: 0 },
            OutcomeCalibrationSample { probabilities: [0.30, 0.30, 0.40], actual: 1 },
            OutcomeCalibrationSample { probabilities: [0.55, 0.20, 0.25], actual: 0 },
            OutcomeCalibrationSample { probabilities: [0.25, 0.25, 0.50], actual: 2 },
            OutcomeCalibrationSample { probabilities: [0.40, 0.30, 0.30], actual: 1 },
        ];
        let fitted = OutcomeCalibrator::fit(&samples).unwrap();
        let distance = (fitted.temperature - 1.0).abs() + fitted.draw_logit_bias.abs();
        assert!(distance < 0.6, "small-sample fit must stay near identity, got {fitted:?}");
    }

    #[test]
    fn shrinkage_still_corrects_persistent_draw_bias_at_sample() {
        // Thirty fixtures of systematic draw underpricing must still move the bias.
        let mut samples = Vec::new();
        for index in 0..30 {
            let probabilities = if index % 3 == 2 {
                [0.25, 0.20, 0.55]
            } else {
                [0.60, 0.15, 0.25]
            };
            let actual = if index % 2 == 0 { 1 } else { index % 3 };
            samples.push(OutcomeCalibrationSample { probabilities, actual });
        }
        let fitted = OutcomeCalibrator::fit(&samples).unwrap();
        assert!(fitted.draw_logit_bias > 0.0, "persistent draw miss must still add draw mass, got {fitted:?}");
    }

    fn analytic_poisson_grid(home_mean: f64, away_mean: f64, max_goals: u32) -> Vec<(u32, u32, f64)> {
        let mut cells = Vec::new();
        for home in 0..=max_goals {
            for away in 0..=max_goals {
                cells.push((home, away, poisson_probability(home_mean, home).unwrap() * poisson_probability(away_mean, away).unwrap()));
            }
        }
        cells
    }

    fn marginals(cells: &[(u32, u32, f64)], max_goals: u32) -> (Vec<f64>, Vec<f64>) {
        let mut home = vec![0.0; max_goals as usize + 1];
        let mut away = vec![0.0; max_goals as usize + 1];
        for (h, a, p) in cells {
            home[*h as usize] += p;
            away[*a as usize] += p;
        }
        (home, away)
    }

    #[test]
    fn dixon_coles_matches_textbook_low_score_adjustment() {
        // Worked example μh=1.70, μa=1.10, ρ=-0.03: draws boosted, 1-0/0-1 suppressed.
        assert!((dixon_coles_tau(0, 0, 1.7, 1.1, -0.03).unwrap() - 1.0561).abs() < 1e-12);
        assert!((dixon_coles_tau(1, 0, 1.7, 1.1, -0.03).unwrap() - 0.967).abs() < 1e-12);
        assert!((dixon_coles_tau(0, 1, 1.7, 1.1, -0.03).unwrap() - 0.949).abs() < 1e-12);
        assert!((dixon_coles_tau(1, 1, 1.7, 1.1, -0.03).unwrap() - 1.03).abs() < 1e-12);
        assert_eq!(dixon_coles_tau(2, 2, 1.7, 1.1, -0.03).unwrap(), 1.0);
        assert_eq!(dixon_coles_tau(0, 0, 1.7, 1.1, 0.0).unwrap(), 1.0);
    }

    #[test]
    fn dixon_coles_preserves_mass_and_marginals_on_full_grid() {
        let grid = analytic_poisson_grid(1.7, 1.1, 15);
        let raw_total: f64 = grid.iter().map(|(_, _, p)| p).sum();
        let normalized: Vec<(u32, u32, f64)> =
            grid.iter().map(|(h, a, p)| (*h, *a, p / raw_total)).collect();
        let adjusted = apply_dixon_coles(&normalized, 1.7, 1.1, -0.03).unwrap();
        let total: f64 = adjusted.iter().map(|(_, _, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-12, "renormalized table must sum to one");
        // The four corrected cells sit far from the truncation boundary, so marginal
        // preservation (exact on the infinite grid) must hold to numerical precision.
        let (raw_home, raw_away) = marginals(&normalized, 15);
        let (adj_home, adj_away) = marginals(&adjusted, 15);
        for (raw, adj) in raw_home.iter().zip(&adj_home).chain(raw_away.iter().zip(&adj_away)) {
            assert!((raw - adj).abs() < 1e-12, "Poisson marginals must survive the correction");
        }
        // Draw mass must rise under negative rho.
        let draw = |cells: &[(u32, u32, f64)]| cells.iter().filter(|(h, a, _)| h == a).map(|(_, _, p)| p).sum::<f64>();
        assert!(draw(&adjusted) > draw(&grid));
    }

    #[test]
    fn dixon_coles_can_change_marginals_of_empirical_path_table() {
        // Valid but non-Poisson 2-by-2 path table: the classic marginal
        // preservation theorem does not apply to this distribution.
        let raw = vec![(0, 0, 0.4), (1, 0, 0.1), (0, 1, 0.1), (1, 1, 0.4)];
        let adjusted = apply_dixon_coles(&raw, 0.5, 0.5, -0.1).unwrap();
        let original_zero_home = raw.iter().filter(|(h, _, _)| *h == 0)
            .map(|(_, _, p)| p).sum::<f64>();
        let adjusted_zero_home = adjusted.iter().filter(|(h, _, _)| *h == 0)
            .map(|(_, _, p)| p).sum::<f64>();
        assert!((original_zero_home - 0.5).abs() < 1e-12);
        assert!((adjusted_zero_home - (0.505 / 1.04)).abs() < 1e-12);
        assert!((original_zero_home - adjusted_zero_home).abs() > 0.01);
    }

    #[test]
    fn dixon_coles_rejects_sign_flipping_rho() {
        assert!(!dixon_coles_valid_rho(1.5, 1.2, -1.0));
        assert!(!dixon_coles_valid_rho(1.5, 1.2, 1.0));
        assert!(dixon_coles_valid_rho(1.5, 1.2, -0.03));
        assert!(dixon_coles_valid_rho(1.5, 1.2, 0.0));
        assert!(apply_dixon_coles(&[(0, 0, 1.0)], 1.5, 1.2, -1.0).is_none());
        assert!(apply_dixon_coles(&[], 1.5, 1.2, -0.03).is_none());
    }

    #[test]
    fn dixon_coles_fit_recovers_draw_heavy_history() {
        // Twenty 1-1s at means predicting ~11% each plus ten decisive results: the only
        // way to explain the draw excess inside this family is negative rho.
        let mut samples = Vec::new();
        for _ in 0..20 {
            samples.push(DixonColesSample { home_mean: 1.4, away_mean: 1.1, home_goals: 1, away_goals: 1 });
        }
        for index in 0..10 {
            samples.push(DixonColesSample {
                home_mean: 1.4,
                away_mean: 1.1,
                home_goals: if index % 2 == 0 { 2 } else { 0 },
                away_goals: if index % 2 == 0 { 0 } else { 2 },
            });
        }
        let first = fit_dixon_coles_rho(&samples).unwrap();
        let second = fit_dixon_coles_rho(&samples).unwrap();
        assert_eq!(first, second, "rho fit must be deterministic");
        assert!(first < 0.0, "draw-heavy history must select negative rho, got {first}");
        assert!(fit_dixon_coles_rho(&[]).is_none());
    }

    #[test]
    fn dixon_coles_fit_stays_at_zero_without_draw_signal() {
        // Six decisive results at proportionate means: no evidence for any correction.
        let samples = [
            DixonColesSample { home_mean: 1.6, away_mean: 1.0, home_goals: 2, away_goals: 0 },
            DixonColesSample { home_mean: 1.6, away_mean: 1.0, home_goals: 1, away_goals: 0 },
            DixonColesSample { home_mean: 1.0, away_mean: 1.6, home_goals: 0, away_goals: 2 },
            DixonColesSample { home_mean: 1.0, away_mean: 1.6, home_goals: 0, away_goals: 1 },
            DixonColesSample { home_mean: 1.3, away_mean: 1.3, home_goals: 2, away_goals: 1 },
            DixonColesSample { home_mean: 1.3, away_mean: 1.3, home_goals: 1, away_goals: 2 },
        ];
        assert_eq!(fit_dixon_coles_rho(&samples).unwrap(), 0.0);
    }
}
