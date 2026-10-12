//! Loading for the fitted pooled-shot heads artifact.
//!
//! The artifact is produced by `scripts/fit_unified_forecast_heads.py` and
//! consumed either by `scripts/apply_pooled_correction.py` (offline adapter
//! for frozen bundles) or directly by the math engine binary via
//! `--heads-artifact` together with per-player `pooled_features` vectors
//! (universal bundles from `scripts/build_universal_bundle.py`). JSON parsing
//! lives here in the app layer; the numeric contract lives in
//! `footy_forecast::PooledShotCorrection`.
use footy_forecast::{PooledEventRate, PooledShotCorrection};

pub struct HeadsArtifact {
    pub feature_names: Vec<String>,
    pub model: PooledShotCorrection,
    /// Adopted defensive head (tackles). Gated challengers (recoveries,
    /// conversion) are parsed nowhere: absence from the engine is the gate.
    pub tackle_feature_names: Option<Vec<String>>,
    pub tackle_model: Option<PooledEventRate>,
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
    let mut artifact = HeadsArtifact {
        feature_names: names,
        model,
        tackle_feature_names: None,
        tackle_model: None,
    };
    if let Some(tackles) = value.get("tackles") {
        let tackle_names = tackles
            .get("feature_names")
            .and_then(|names| names.as_array())
            .ok_or("tackles head lacks feature_names")?
            .iter()
            .map(|name| {
                name.as_str()
                    .ok_or_else(|| "non-string tackle feature name".to_owned())
                    .map(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tackle_coefficients = tackles.get("coefficients").ok_or("tackles head lacks coefficients")?;
        if tackle_coefficients.is_null() {
            return Err("tackles head selected identity; no residual to apply".into());
        }
        let tackle_coefficients = tackle_coefficients
            .as_array()
            .ok_or("tackles coefficients are not an array")?
            .iter()
            .map(|value| value.as_f64().ok_or_else(|| "non-numeric tackle coefficient".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        artifact.tackle_model = Some(PooledEventRate::from_parts(
            tackle_names.clone(),
            tackle_coefficients,
        )?);
        artifact.tackle_feature_names = Some(tackle_names);
    }
    Ok(artifact)
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

    const TACKLE_ARTIFACT: &str = r#"{
        "schema": "unified_epl_mls_forecast_heads_v1",
        "feature_names_shots": ["league_mls"],
        "shots": {"best": "0.1", "coefficients": [0.1]},
        "tackles": {"best": "1.0", "feature_names": ["league_mls", "fm_Tackling_norm"],
                    "coefficients": [0.05, 0.12]}
    }"#;

    #[test]
    fn tackle_head_loads_and_applies() {
        let heads = load_heads_artifact_json(TACKLE_ARTIFACT).unwrap();
        let names = heads.tackle_feature_names.unwrap();
        assert_eq!(names, vec!["league_mls", "fm_Tackling_norm"]);
        let model = heads.tackle_model.unwrap();
        let got = model.correct(2.0, &[1.0, 0.5]).unwrap();
        assert!((got - 2.0 * (0.05 + 0.06_f64).exp()).abs() < 1e-12);
    }

    #[test]
    fn shots_artifact_without_tackles_loads_clean() {
        let heads = load_heads_artifact_json(ARTIFACT).unwrap();
        assert!(heads.tackle_model.is_none());
    }
}

/// FM26 squad attack/defense prior indices keyed by canonical club name.
pub fn load_fm_prior_indices_json(text: &str) -> Result<std::collections::BTreeMap<String, (f64, f64)>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("invalid FM priors JSON: {error}"))?;
    if value.get("schema").and_then(|schema| schema.as_str()) != Some("fm26_team_prior_indices_v1") {
        return Err("unsupported FM prior indices schema".into());
    }
    let map = value
        .get("priors")
        .and_then(|priors| priors.as_object())
        .ok_or("FM priors lack priors object")?;
    let mut out = std::collections::BTreeMap::new();
    for (club, entry) in map {
        let attack = entry.get("attack").and_then(|v| v.as_f64()).ok_or_else(|| {
            format!("FM prior lacks attack for {club}")
        })?;
        let defense = entry.get("defense").and_then(|v| v.as_f64()).ok_or_else(|| {
            format!("FM prior lacks defense for {club}")
        })?;
        if !attack.is_finite() || !defense.is_finite() || attack.abs() > 5.0 || defense.abs() > 5.0 {
            return Err(format!("FM prior outside plausible range for {club}"));
        }
        out.insert(club.clone(), (attack, defense));
    }
    if out.is_empty() {
        return Err("no FM club priors".into());
    }
    Ok(out)
}

#[cfg(test)]
mod prior_tests {
    use super::*;

    #[test]
    fn prior_loader_validates_schema_and_range() {
        let good = r#"{"schema": "fm26_team_prior_indices_v1",
            "priors": {"Arsenal": {"attack": 1.39, "defense": 1.43}}}"#;
        let priors = load_fm_prior_indices_json(good).unwrap();
        assert_eq!(priors["Arsenal"], (1.39, 1.43));
        assert!(load_fm_prior_indices_json("{}").is_err());
        let wild = r#"{"schema": "fm26_team_prior_indices_v1",
            "priors": {"Arsenal": {"attack": 99.0, "defense": 0.0}}}"#;
        assert!(load_fm_prior_indices_json(wild).is_err());
    }
}
