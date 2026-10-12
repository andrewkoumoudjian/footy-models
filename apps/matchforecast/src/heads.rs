//! Loading for the fitted pooled-shot heads artifact.
//!
//! The artifact is produced by `scripts/fit_unified_forecast_heads.py` and
//! consumed either by `scripts/apply_pooled_correction.py` (offline adapter
//! for frozen bundles) or directly by the math engine binary via
//! `--heads-artifact` together with per-player `pooled_features` vectors
//! (universal bundles from `scripts/build_universal_bundle.py`). JSON parsing
//! lives here in the app layer; the numeric contract lives in
//! `footy_forecast::PooledShotCorrection`.
use footy_forecast::PooledShotCorrection;

pub struct HeadsArtifact {
    pub feature_names: Vec<String>,
    pub model: PooledShotCorrection,
}

pub fn load_heads_artifact_json(text: &str) -> Result<HeadsArtifact, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("invalid heads JSON: {error}"))?;
    let get = |key: &str| value.get(key).ok_or_else(|| format!("heads artifact lacks {key}"));
    if get("schema")?.as_str().ok_or("heads schema is not a string")?
        != "unified_epl_mls_forecast_heads_v1"
    {
        return Err("unsupported heads artifact schema".into());
    }
    let names = get("feature_names_shots")?
        .as_array()
        .ok_or("feature_names_shots is not an array")?
        .iter()
        .map(|name| {
            name.as_str()
                .ok_or_else(|| "non-string feature name".to_owned())
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shots = get("shots")?;
    let coefficients = shots.get("coefficients").ok_or("shots head lacks coefficients")?;
    if coefficients.is_null() {
        return Err("shots head selected identity; no residual to apply".into());
    }
    let coefficients = coefficients
        .as_array()
        .ok_or("shots coefficients are not an array")?
        .iter()
        .map(|value| value.as_f64().ok_or_else(|| "non-numeric coefficient".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let model = PooledShotCorrection::from_parts(names.clone(), coefficients)?;
    Ok(HeadsArtifact { feature_names: names, model })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARTIFACT: &str = r#"{
        "schema": "unified_epl_mls_forecast_heads_v1",
        "feature_names_shots": ["league_mls", "age_years_norm"],
        "shots": {"best": "0.1", "coefficients": [0.1348066168780403, 0.03159050958852127]}
    }"#;

    #[test]
    fn loader_applies_golden_correction() {
        let heads = load_heads_artifact_json(ARTIFACT).unwrap();
        assert_eq!(heads.feature_names, vec!["league_mls", "age_years_norm"]);
        let got = heads.model.correct(1.2, &[1.0, -0.5]).unwrap();
        let expected = 1.2 * (0.1348066168780403_f64 - 0.5 * 0.03159050958852127_f64).exp();
        assert!((got - expected).abs() < 1e-12);
    }

    #[test]
    fn loader_rejects_bad_artifacts() {
        assert!(load_heads_artifact_json("{}").is_err());
        assert!(load_heads_artifact_json(
            &ARTIFACT.replace("unified_epl_mls_forecast_heads_v1", "v2")
        )
        .is_err());
        assert!(load_heads_artifact_json(&ARTIFACT.replace(
            "[0.1348066168780403, 0.03159050958852127]",
            "null"
        ))
        .is_err());
        assert!(load_heads_artifact_json(
            &ARTIFACT.replace("\"age_years_norm\"", "\"fm_uid\"")
        )
        .is_err());
    }
}
