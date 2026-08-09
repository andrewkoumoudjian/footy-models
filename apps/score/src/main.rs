#![forbid(unsafe_code)]

use footy_artifacts::{
    read_feature_matrix, read_linear_price_model, write_prediction_batch,
    FundamentalPredictionRow, PredictionBatchArtifact,
};
use footy_core::Date;
use footy_dataset::{materialize_scoring_features, read_canonical_dataset};
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("batch") => batch(&args[1..]),
        Some("canonical") => canonical(&args[1..]),
        _ => Err("usage:\n  footy-score batch --model model.bin --features matrix.bin --output predictions.bin\n  footy-score canonical --model model.bin --canonical canonical.bin --as-of YYYY-MM-DD --output predictions.bin".into()),
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
        rows.push(FundamentalPredictionRow { player: row.player, as_of: row.as_of, value: model.predict_distribution(&row.features)? });
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
    print_summary(&artifact, output);
    Ok(())
}

fn canonical(args: &[String]) -> Result<(), Box<dyn Error>> {
    let model_path = required_value(args, "--model")?;
    let canonical_path = required_value(args, "--canonical")?;
    let as_of = parse_date(required_value(args, "--as-of")?)?;
    let output = required_value(args, "--output")?;
    let model = read_linear_price_model(model_path)?;
    if as_of <= model.calibration_cutoff {
        return Err("canonical scoring date must be strictly after model calibration cutoff".into());
    }
    let dataset = read_canonical_dataset(canonical_path)?;
    let materialized = materialize_scoring_features(
        &dataset,
        as_of,
        model.metadata.feature_set_version,
        Some(&model.feature_ids),
    )?;
    if materialized.rows.is_empty() { return Err("canonical scoring produced no player states".into()); }
    if materialized.feature_ids != model.feature_ids { return Err("canonical feature projection does not match model ordering".into()); }
    let mut rows = Vec::with_capacity(materialized.rows.len());
    for row in materialized.rows {
        rows.push(FundamentalPredictionRow { player: row.player, as_of: row.as_of, value: model.predict_distribution(&row.features)? });
    }
    let artifact = PredictionBatchArtifact {
        scoring_dataset: dataset.dataset_version,
        source_manifest_checksum: dataset.source_manifest_checksum,
        training_dataset: model.metadata.dataset_version,
        feature_set_version: model.metadata.feature_set_version,
        model_version: model.metadata.model_version,
        calibration_version: model.calibration_version,
        trained_through: model.metadata.trained_through,
        calibration_cutoff: model.calibration_cutoff,
        rows,
    };
    write_prediction_batch(output, &artifact)?;
    print_summary(&artifact, output);
    Ok(())
}

fn print_summary(artifact: &PredictionBatchArtifact, output: &str) {
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
}

fn parse_date(value: &str) -> Result<Date, Box<dyn Error>> {
    let mut parts = value.split('-');
    let year = parts.next().ok_or("missing year")?.parse::<i32>()?;
    let month = parts.next().ok_or("missing month")?.parse::<u8>()?;
    let day = parts.next().ok_or("missing day")?.parse::<u8>()?;
    if parts.next().is_some() { return Err("date must be YYYY-MM-DD".into()); }
    Ok(Date::from_ymd(year, month, day)?)
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    let index = args.iter().position(|arg| arg == flag).ok_or_else(|| format!("missing {flag}"))?;
    args.get(index + 1).map(String::as_str).ok_or_else(|| format!("missing value for {flag}").into())
}
