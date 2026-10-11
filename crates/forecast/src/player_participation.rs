use footy_models::{fit_logistic_irls, DenseMatrix, FeatureTransform, FitError, LogisticModel, RidgeOls};

/// Strictly pre-match selection state for one player at one club.
///
/// Counts must refer only to club fixtures for which player-level selection coverage exists. A
/// missing lineup source is not a non-appearance and must never increment `club_fixtures`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerParticipationFeatures {
    pub club_fixtures: f64,
    pub prior_listings: f64,
    pub prior_appearances: f64,
    pub prior_starts: f64,
    pub prior_minutes: f64,
    pub prior_start_minutes: f64,
    pub days_since_last_appearance: f64,
    pub days_since_last_listed: f64,
}

impl PlayerParticipationFeatures {
    fn model_row(self) -> Result<Vec<f64>, FitError> {
        let values = [
            self.club_fixtures,
            self.prior_listings,
            self.prior_appearances,
            self.prior_starts,
            self.prior_minutes,
            self.prior_start_minutes,
            self.days_since_last_appearance,
            self.days_since_last_listed,
        ];
        if values.iter().any(|value| !value.is_finite() || *value < 0.0)
            || self.prior_listings > self.club_fixtures + 1e-9
            || self.prior_appearances > self.prior_listings + 1e-9
            || self.prior_starts > self.prior_appearances + 1e-9
        {
            return Err(FitError::NonFiniteInput);
        }
        let fixtures = self.club_fixtures.max(1.0);
        let appearances = self.prior_appearances.max(1.0);
        let starts = self.prior_starts.max(1.0);
        Ok(vec![
            self.prior_listings / fixtures,
            self.prior_appearances / fixtures,
            self.prior_starts / fixtures,
            self.prior_minutes / appearances / 90.0,
            self.prior_start_minutes / starts / 90.0,
            self.club_fixtures.ln_1p(),
            self.prior_listings.ln_1p(),
            self.prior_appearances.ln_1p(),
            self.days_since_last_appearance.ln_1p(),
            self.days_since_last_listed.ln_1p(),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerParticipationTrainingExample {
    pub features: PlayerParticipationFeatures,
    pub appeared: bool,
    pub started: bool,
    /// Realized minutes in the target match. Zero for a non-appearance.
    pub minutes: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerParticipationPrediction {
    pub start_probability: f64,
    pub appearance_probability: f64,
    pub expected_minutes_if_start: f64,
    pub minutes_sigma_if_start: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NextMatchParticipationModel {
    transform: FeatureTransform,
    appearance: LogisticModel,
    start: LogisticModel,
    start_minutes: RidgeOls,
    start_minutes_residual_sigma: f64,
    observations: u32,
}

impl NextMatchParticipationModel {
    pub fn fit(
        examples: &[PlayerParticipationTrainingExample],
        logistic_l2: f64,
        minutes_ridge: f64,
    ) -> Result<Self, FitError> {
        if examples.is_empty() {
            return Err(FitError::EmptyData);
        }
        if !logistic_l2.is_finite() || logistic_l2 < 0.0
            || !minutes_ridge.is_finite() || minutes_ridge < 0.0
        {
            return Err(FitError::InvalidParameter("regularization must be finite and non-negative"));
        }

        let mut raw_rows = Vec::with_capacity(examples.len());
        let mut appearance_targets = Vec::with_capacity(examples.len());
        let mut start_targets = Vec::with_capacity(examples.len());
        let mut start_rows = Vec::new();
        let mut start_minutes = Vec::new();
        for example in examples {
            if !example.minutes.is_finite() || example.minutes < 0.0
                || example.started && !example.appeared
                || example.started && example.minutes <= 0.0
            {
                return Err(FitError::NonFiniteInput);
            }
            let row = example.features.model_row()?;
            raw_rows.push(row.clone());
            appearance_targets.push(if example.appeared { 1.0 } else { 0.0 });
            start_targets.push(if example.started { 1.0 } else { 0.0 });
            if example.started {
                start_rows.push(row);
                start_minutes.push(example.minutes.clamp(1.0, 90.0));
            }
        }
        if !has_both_classes(&appearance_targets) || !has_both_classes(&start_targets) || start_rows.len() < 8 {
            return Err(FitError::InvalidParameter("participation fit needs both classes and starter-minute examples"));
        }

        let transform = FeatureTransform::fit(&raw_rows)?;
        let width = transform.width() * 2;
        let mut transformed = Vec::with_capacity(raw_rows.len() * width);
        for row in &raw_rows {
            transformed.extend(transform.apply(row)?);
        }
        let x = DenseMatrix::new(raw_rows.len(), width, transformed)?;
        let appearance = fit_logistic_irls(&x, &appearance_targets, logistic_l2, 50)?;
        let start = fit_logistic_irls(&x, &start_targets, logistic_l2, 50)?;
        let start_minutes = RidgeOls::fit(&start_rows, &start_minutes, minutes_ridge)?;

        let mut squared_error = 0.0;
        let mut starters = 0usize;
        for example in examples.iter().filter(|example| example.started) {
            let prediction = start_minutes.predict(&example.features.model_row()?)?.clamp(1.0, 90.0);
            squared_error += (prediction - example.minutes).powi(2);
            starters += 1;
        }
        let denominator = starters.saturating_sub(1).max(1) as f64;
        let start_minutes_residual_sigma = (squared_error / denominator).sqrt().clamp(3.0, 30.0);

        Ok(Self {
            transform,
            appearance,
            start,
            start_minutes,
            start_minutes_residual_sigma,
            observations: examples.len().min(u32::MAX as usize) as u32,
        })
    }

    pub fn predict(&self, features: PlayerParticipationFeatures) -> Result<PlayerParticipationPrediction, FitError> {
        let row = features.model_row()?;
        let transformed = self.transform.apply(&row)?;
        let start_probability = self.start.predict_probability(&transformed)?.clamp(0.0, 1.0);
        let raw_appearance = self.appearance.predict_probability(&transformed)?.clamp(0.0, 1.0);
        // Starting implies appearing; enforce that structural relation even when separately fitted
        // logistic heads cross because of finite-sample noise.
        let appearance_probability = raw_appearance.max(start_probability);
        let expected_minutes_if_start = self.start_minutes.predict(&row)?.clamp(45.0, 90.0);
        let sparse_scale = (1.0 + 4.0 / (features.prior_starts + 1.0)).sqrt();
        let minutes_sigma_if_start = (self.start_minutes_residual_sigma * sparse_scale).clamp(5.0, 30.0);
        Ok(PlayerParticipationPrediction {
            start_probability,
            appearance_probability,
            expected_minutes_if_start,
            minutes_sigma_if_start,
        })
    }

    pub const fn observations(&self) -> u32 { self.observations }
}

fn has_both_classes(values: &[f64]) -> bool {
    values.iter().any(|value| *value <= 0.0) && values.iter().any(|value| *value >= 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(appearances: f64, starts: f64, days: f64) -> PlayerParticipationFeatures {
        PlayerParticipationFeatures {
            club_fixtures: 12.0,
            prior_listings: 11.0,
            prior_appearances: appearances,
            prior_starts: starts,
            prior_minutes: appearances * 60.0,
            prior_start_minutes: starts * 76.0,
            days_since_last_appearance: days,
            days_since_last_listed: days.min(5.0),
        }
    }

    #[test]
    fn deterministic_fit_orders_established_and_fringe_players() {
        let mut examples = Vec::new();
        for index in 0..80 {
            let established = index % 2 == 0;
            let started = established && index % 4 != 0;
            let appeared = started || if established { index % 7 != 0 } else { index % 5 == 0 };
            examples.push(PlayerParticipationTrainingExample {
                features: if established { features(10.0, 8.0, 4.0) } else { features(3.0, 1.0, 18.0) },
                appeared,
                started,
                minutes: if started { 74.0 + (index % 5) as f64 } else if appeared { 22.0 } else { 0.0 },
            });
        }
        let first = NextMatchParticipationModel::fit(&examples, 1.0, 4.0).unwrap();
        let second = NextMatchParticipationModel::fit(&examples, 1.0, 4.0).unwrap();
        assert_eq!(first, second);
        let regular = first.predict(features(10.0, 8.0, 4.0)).unwrap();
        let fringe = first.predict(features(3.0, 1.0, 18.0)).unwrap();
        assert!(regular.start_probability > fringe.start_probability);
        assert!(regular.appearance_probability >= regular.start_probability);
        assert!(fringe.appearance_probability >= fringe.start_probability);
    }

    #[test]
    fn sparse_starter_has_wider_minutes_uncertainty() {
        let mut examples = Vec::new();
        for index in 0..60 {
            let started = index % 3 == 0;
            let appeared = started || index % 4 != 0;
            examples.push(PlayerParticipationTrainingExample {
                features: features(8.0, 4.0, 7.0),
                appeared,
                started,
                minutes: if started { 68.0 + (index % 12) as f64 } else if appeared { 20.0 } else { 0.0 },
            });
        }
        let model = NextMatchParticipationModel::fit(&examples, 2.0, 4.0).unwrap();
        let sparse = model.predict(features(3.0, 1.0, 7.0)).unwrap();
        let established = model.predict(features(11.0, 9.0, 7.0)).unwrap();
        assert!(sparse.minutes_sigma_if_start > established.minutes_sigma_if_start);
    }
}
