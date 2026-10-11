use footy_models::{FitError, RidgeOls};

/// Source-neutral, strictly pre-match summary used to forecast a player's next-match metric rate.
/// Callers construct these features from observations available before the target fixture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerRateFeatures {
    pub long_run_per90: f64,
    pub recent_per90: f64,
    pub trend_per_match: f64,
    pub variance_per90: f64,
    pub prior_minutes: f64,
    pub prior_appearances: f64,
}

impl PlayerRateFeatures {
    fn model_row(self) -> Result<Vec<f64>, FitError> {
        let non_negative = [
            self.long_run_per90,
            self.recent_per90,
            self.variance_per90,
            self.prior_minutes,
            self.prior_appearances,
        ];
        if non_negative.iter().any(|value| !value.is_finite() || *value < 0.0)
            || !self.trend_per_match.is_finite()
        {
            return Err(FitError::NonFiniteInput);
        }
        Ok(vec![
            self.long_run_per90.ln_1p(),
            self.recent_per90.ln_1p(),
            self.trend_per_match,
            self.variance_per90.ln_1p(),
            self.prior_minutes.ln_1p(),
            self.prior_appearances.ln_1p(),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerRateTrainingExample {
    pub features: PlayerRateFeatures,
    pub target_per90: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerRatePrediction {
    pub mean_per90: f64,
    /// Match-to-match residual variance around the learned conditional mean.
    pub variance_per90: f64,
    /// Uncertainty in the latent rate, widened for players with little prior playing time.
    pub epistemic_sigma: f64,
}

/// Dependency-light next-match rate model. The response is fitted in `ln(1 + rate)` space to keep
/// heavy-tailed football counting metrics stable; scoring maps back to a non-negative rate.
#[derive(Debug, Clone, PartialEq)]
pub struct NextMatchPlayerRateModel {
    fit: RidgeOls,
    residual_variance_per90: f64,
    residual_sigma_per90: f64,
    observations: u32,
}

impl NextMatchPlayerRateModel {
    pub fn fit(examples: &[PlayerRateTrainingExample], ridge: f64) -> Result<Self, FitError> {
        if examples.is_empty() {
            return Err(FitError::EmptyData);
        }
        if !ridge.is_finite() || ridge < 0.0 {
            return Err(FitError::InvalidParameter("ridge must be finite and non-negative"));
        }
        let mut rows = Vec::with_capacity(examples.len());
        let mut targets = Vec::with_capacity(examples.len());
        for example in examples {
            if !example.target_per90.is_finite() || example.target_per90 < 0.0 {
                return Err(FitError::NonFiniteInput);
            }
            rows.push(example.features.model_row()?);
            targets.push(example.target_per90.ln_1p());
        }
        let fit = RidgeOls::fit(&rows, &targets, ridge)?;
        let mut squared_error = 0.0;
        for (row, example) in rows.iter().zip(examples) {
            let prediction = rate_from_log_prediction(fit.predict(row)?)?;
            squared_error += (example.target_per90 - prediction).powi(2);
        }
        let denominator = examples.len().saturating_sub(1).max(1) as f64;
        let residual_variance_per90 = (squared_error / denominator).max(0.0);
        let residual_sigma_per90 = residual_variance_per90.sqrt();
        Ok(Self {
            fit,
            residual_variance_per90,
            residual_sigma_per90,
            observations: examples.len().min(u32::MAX as usize) as u32,
        })
    }

    pub fn predict(&self, features: PlayerRateFeatures) -> Result<PlayerRatePrediction, FitError> {
        let row = features.model_row()?;
        let mean_per90 = rate_from_log_prediction(self.fit.predict(&row)?)?;
        let training_term = 1.0 / f64::from(self.observations.max(1));
        let player_term = 1.0 / (features.prior_appearances + 1.0);
        let epistemic_sigma = self.residual_sigma_per90 * (training_term + player_term).sqrt();
        Ok(PlayerRatePrediction {
            mean_per90,
            variance_per90: self.residual_variance_per90,
            epistemic_sigma,
        })
    }

    pub const fn observations(&self) -> u32 {
        self.observations
    }
}

fn rate_from_log_prediction(value: f64) -> Result<f64, FitError> {
    if !value.is_finite() {
        return Err(FitError::NonFiniteInput);
    }
    // A pathological small-sample coefficient vector must not be allowed to overflow scoring.
    // exp(12)-1 is already far outside plausible football per-90 rates, so this is a numerical
    // guard rather than a meaningful football cap; fixture-specific consumers can shrink harder.
    Ok(value.clamp(-20.0, 12.0).exp_m1().max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(level: f64, appearances: f64) -> PlayerRateFeatures {
        PlayerRateFeatures {
            long_run_per90: level,
            recent_per90: level * 1.1,
            trend_per_match: 0.03 * level,
            variance_per90: 0.15 + 0.05 * level,
            prior_minutes: appearances * 75.0,
            prior_appearances: appearances,
        }
    }

    #[test]
    fn next_match_rate_fit_is_deterministic_and_orders_signal() {
        let examples: Vec<PlayerRateTrainingExample> = (0..80)
            .map(|index| {
                let level = 0.2 + (index % 20) as f64 * 0.15;
                PlayerRateTrainingExample {
                    features: features(level, 4.0 + (index % 12) as f64),
                    target_per90: 0.15 + 0.85 * level + (index % 3) as f64 * 0.02,
                }
            })
            .collect();
        let first = NextMatchPlayerRateModel::fit(&examples, 1.0).unwrap();
        let second = NextMatchPlayerRateModel::fit(&examples, 1.0).unwrap();
        assert_eq!(first, second);
        let low = first.predict(features(0.5, 10.0)).unwrap();
        let high = first.predict(features(2.0, 10.0)).unwrap();
        assert!(high.mean_per90 > low.mean_per90);
        assert!(high.variance_per90.is_finite());
    }

    #[test]
    fn sparse_player_keeps_more_epistemic_uncertainty() {
        let examples: Vec<PlayerRateTrainingExample> = (0..40)
            .map(|index| {
                let level = 0.4 + (index % 10) as f64 * 0.1;
                PlayerRateTrainingExample {
                    features: features(level, 8.0),
                    target_per90: level + (index % 4) as f64 * 0.04,
                }
            })
            .collect();
        let model = NextMatchPlayerRateModel::fit(&examples, 1.0).unwrap();
        let sparse = model.predict(features(1.0, 2.0)).unwrap();
        let established = model.predict(features(1.0, 30.0)).unwrap();
        assert!(sparse.epistemic_sigma > established.epistemic_sigma);
    }
}
