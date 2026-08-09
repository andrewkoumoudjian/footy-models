#![forbid(unsafe_code)]

use footy_artifacts::{
    read_feature_matrix, read_linear_price_model, write_prediction_batch,
    FundamentalPredictionRow, PredictionBatchArtifact,
};
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("batch") => batch(&args[1..]),
        _ => Err("usage: footy-score batch --model model.bin --features matrix.bin --output predictions.bin".into()),
    }
}

fn batch(args: &[String]) -> Result<(), Box<dyn Error>> {
    let model_path = required_value(args, "--model")?;
    let features_path = required_value(args, "--features")?;
    let output = required_value(args, "--output")?;
    let model = read_linear_price_model(model_path)?;
    let matrix = read_feature_matrix(features_path)?;
    if matrix.feature_set_version != model.metadata.feature_set_version || matrix.feature_ids != model.feature_ids {
        return Err("feature/model version or ordering mismatch".into());
    }
    if matrix.rows.is_empty() { return Err("scoring matrix contains no rows".into()); }

    let mut rows = Vec::with_capacity(matrix.rows.len());
    for row in &matrix.rows {
        if row.as_of <= model.calibration_cutoff {
            return Err(format!("scoring row for player {} is not strictly after model calibration cutoff", row.player.0).into());
        }
        rows.push(FundamentalPredictionRow {
            player: row.player,
            as_of: row.as_of,
            value: model.predict_distribution(&row.features)?,
        });
    }
    let artifact = PredictionBatchArtifact {
        scoring_dataset: matrix.dataset_version,
        source_manifest_checksum: matrix.source_manifest_checksum,
        training_dataset: model.metadata.dataset_version,
        feature_set_version: model.metadata.feature_set_version,
        model_version: model.metadata.model_version,
        calibration_version: model.calibration_version,
        trained_through: model.metadata.trained_through,
        calibration_cutoff: model.calibration_cutoff,
        rows,
    };
    write_prediction_batch(output, &artifact)?;
    println!(
        "scored rows={} scoring_dataset={} training_dataset={} feature_set={} model={} calibration={} output={}",
        artifact.rows.len(),
        artifact.scoring_dataset.0,
        artifact.training_dataset.0,
        artifact.feature_set_version.0,
        artifact.model_version.0,
        artifact.calibration_version.0,
        output
    );
    Ok(())
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    let index = args.iter().position(|arg| arg == flag).ok_or_else(|| format!("missing {flag}"))?;
    args.get(index + 1).map(String::as_str).ok_or_else(|| format!("missing value for {flag}").into())
}
