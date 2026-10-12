//! Pooled player-agnostic count-rate correction for non-shot events.
//!
//! Same mathematics as the shot head (`PooledShotCorrection`): `corrected =
//! baseline * exp(clamp(dot(features, beta), -3, 3))`. A separate type so
//! defensive (or other event) heads carry their own feature contract and
//! provenance instead of overloading the shot correction. JSON parsing stays
//! in the calling app; invalid inputs yield errors or `None` so callers fall
//! back with explicit missing coverage. Defensive rates have no team-level
//! totals to preserve (tackles do not enter the score process); the engine
//! multiplies the corrected per-90 rate by simulated minutes.

/// Pooled correction for one defensive event family (e.g. tackles).
#[derive(Debug, Clone, PartialEq)]
pub struct PooledEventRate {
    feature_names: Vec<String>,
    coefficients: Vec<f64>,
}

impl PooledEventRate {
    /// Maximum absolute residual logit; matches the Python fit/predict clip.
    pub const LOGIT_CLIP: f64 = 3.0;

    pub fn from_parts(feature_names: Vec<String>, coefficients: Vec<f64>) -> Result<Self, String> {
        if feature_names.is_empty() || feature_names.iter().any(|name| name.is_empty()) {
            return Err("event-rate correction needs non-empty feature names".into());
        }
        if coefficients.len() != feature_names.len()
            || !coefficients.iter().all(|value| value.is_finite())
        {
            return Err("coefficient vector does not match feature contract".into());
        }
        if feature_names.iter().any(|name| {
            matches!(
                name.as_str(),
                "player_id" | "espn_id" | "fm_uid" | "canonical_player_id"
            )
        }) {
            return Err("correction must stay player-agnostic".into());
        }
        if feature_names.iter().collect::<std::collections::BTreeSet<_>>().len()
            != feature_names.len()
        {
            return Err("duplicate event-rate feature names".into());
        }
        Ok(Self { feature_names, coefficients })
    }

    pub fn feature_names(&self) -> &[String] {
        &self.feature_names
    }

    /// Corrected per-90 rate. `features` must follow [`Self::feature_names`].
    pub fn correct(&self, baseline_per90: f64, features: &[f64]) -> Option<f64> {
        if !baseline_per90.is_finite() || baseline_per90 <= 0.0 {
            return None;
        }
        if features.len() != self.coefficients.len()
            || !features.iter().all(|value| value.is_finite())
        {
            return None;
        }
        let dot: f64 = features.iter().zip(&self.coefficients).map(|(x, b)| x * b).sum();
        if !dot.is_finite() {
            return None;
        }
        Some(baseline_per90 * dot.clamp(-Self::LOGIT_CLIP, Self::LOGIT_CLIP).exp())
    }

    /// Expected event count for simulated minutes at a corrected per-90 rate.
    pub fn expected_for_minutes(rate_per90: f64, minutes: f64) -> Option<f64> {
        if !rate_per90.is_finite()
            || rate_per90 < 0.0
            || !minutes.is_finite()
            || !(0.0..=130.0).contains(&minutes)
        {
            return None;
        }
        Some(rate_per90 * minutes / 90.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> PooledEventRate {
        PooledEventRate::from_parts(
            vec!["league_mls".to_owned(), "fm_Tackling_norm".to_owned()],
            vec![0.05, 0.12],
        )
        .unwrap()
    }

    #[test]
    fn golden_vector_matches_python_prediction() {
        let model = model();
        let got = model.correct(2.0, &[1.0, 0.5]).unwrap();
        let expected = 2.0 * (0.05 + 0.5 * 0.12_f64).exp();
        assert!((got - expected).abs() < 1e-12);
    }

    #[test]
    fn minutes_scale_linearly_and_reject_garbage() {
        assert_eq!(PooledEventRate::expected_for_minutes(2.0, 90.0).unwrap(), 2.0);
        assert_eq!(PooledEventRate::expected_for_minutes(2.0, 45.0).unwrap(), 1.0);
        assert!(PooledEventRate::expected_for_minutes(-1.0, 90.0).is_none());
        assert!(PooledEventRate::expected_for_minutes(2.0, 200.0).is_none());
        assert!(PooledEventRate::expected_for_minutes(f64::NAN, 90.0).is_none());
    }

    #[test]
    fn contract_gates_reject() {
        assert!(PooledEventRate::from_parts(vec![], vec![]).is_err());
        assert!(PooledEventRate::from_parts(vec!["a".to_owned()], vec![]).is_err());
        assert!(PooledEventRate::from_parts(vec!["fm_uid".to_owned()], vec![0.1]).is_err());
        assert!(PooledEventRate::from_parts(
            vec!["a".to_owned(), "a".to_owned()],
            vec![0.1, 0.2]
        )
        .is_err());
    }
}
