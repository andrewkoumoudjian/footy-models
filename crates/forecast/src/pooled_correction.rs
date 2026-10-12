//! Versioned pooled ridge-Poisson shot-rate correction (player-agnostic).
//!
//! Mirrors `scripts/fit_unified_forecast_heads.py` (shots head) exactly:
//! `corrected_per90 = baseline_per90 * exp(clamp(dot(features, beta), -3, 3))`.
//! Feature order is defined by the fitted artifact; player identity never
//! enters the numeric vector. JSON parsing stays in the calling app (this
//! crate takes already-parsed parts); anything failing validation yields an
//! error or `None` so callers fall back to the engine's research prior with
//! explicit missing coverage.

/// Pooled correction built from a `unified_epl_mls_forecast_heads_v1` artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct PooledShotCorrection {
    feature_names: Vec<String>,
    coefficients: Vec<f64>,
}

impl PooledShotCorrection {
    /// Maximum absolute residual logit; matches the Python fit/predict clip.
    pub const LOGIT_CLIP: f64 = 3.0;

    pub fn from_parts(feature_names: Vec<String>, coefficients: Vec<f64>) -> Result<Self, String> {
        if feature_names.is_empty() || feature_names.iter().any(|name| name.is_empty()) {
            return Err("correction needs non-empty feature names".into());
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
            return Err("duplicate correction feature names".into());
        }
        Ok(Self { feature_names, coefficients })
    }

    pub fn feature_names(&self) -> &[String] {
        &self.feature_names
    }

    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> PooledShotCorrection {
        PooledShotCorrection::from_parts(
            vec!["league_mls".to_owned(), "age_years_norm".to_owned()],
            vec![0.1348066168780403, 0.03159050958852127],
        )
        .unwrap()
    }

    #[test]
    fn golden_vector_matches_python_prediction() {
        // Python: baseline * exp(clip(0.1348066168780403*1.0 + 0.03159050958852127*(-0.5), -3, 3)).
        let model = model();
        let got = model.correct(1.2, &[1.0, -0.5]).unwrap();
        let expected = 1.2 * (0.1348066168780403_f64 - 0.5 * 0.03159050958852127_f64).exp();
        assert!((got - expected).abs() < 1e-12);
    }

    #[test]
    fn residual_logit_clips_both_sides() {
        let model = model();
        assert_eq!(model.correct(1.0, &[100.0, 100.0]).unwrap(), 3.0_f64.exp());
        assert_eq!(model.correct(1.0, &[-100.0, -100.0]).unwrap(), (-3.0_f64).exp());
    }

    #[test]
    fn invalid_inputs_yield_missing_not_panic() {
        let model = model();
        assert!(model.correct(0.0, &[1.0, 0.0]).is_none());
        assert!(model.correct(-1.0, &[1.0, 0.0]).is_none());
        assert!(model.correct(f64::NAN, &[1.0, 0.0]).is_none());
        assert!(model.correct(1.0, &[1.0]).is_none());
        assert!(model.correct(1.0, &[1.0, f64::INFINITY]).is_none());
    }

    #[test]
    fn contract_gates_reject() {
        assert!(PooledShotCorrection::from_parts(vec![], vec![]).is_err());
        assert!(PooledShotCorrection::from_parts(vec!["a".to_owned()], vec![]).is_err());
        assert!(PooledShotCorrection::from_parts(
            vec!["a".to_owned()],
            vec![f64::NAN]
        )
        .is_err());
        assert!(PooledShotCorrection::from_parts(
            vec!["fm_uid".to_owned()],
            vec![0.1]
        )
        .is_err());
        assert!(PooledShotCorrection::from_parts(
            vec!["a".to_owned(), "a".to_owned()],
            vec![0.1, 0.2]
        )
        .is_err());
    }
}
