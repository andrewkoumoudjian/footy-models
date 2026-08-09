#![forbid(unsafe_code)]

use footy_artifacts::{
    checksum_file, read_dataset_manifest, read_feature_matrix, write_feature_matrix,
    write_linear_price_model, FeatureMatrixArtifact, FeatureRow, LinearPriceModelArtifact,
};
use footy_core::{
    CalibrationVersion, DatasetVersion, Date, FeatureId, FeatureSetVersion, ModelVersion, PlayerId,
};
use footy_dataset::{
    materialize_training_features, read_canonical_dataset, MaterializationConfig,
};
use footy_models::{fit_weighted_linear, ConformalCalibrator, DenseMatrix, ModelMetadata};
use std::env;
use std::error::Error;
use std::fs;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("materialize") => materialize(&args[1..]),
        Some("materialize-canonical") => materialize_canonical(&args[1..]),
        Some("fit") => fit(&args[1..]),
        _ => Err(usage().into()),
    }
}

fn usage() -> &'static str {
    "usage:\n  footy-train materialize --input rows.tsv --dataset-manifest manifest.bin --dataset-version N --feature-set-version N --output matrix.bin\n  footy-train materialize-canonical --canonical canonical.bin --feature-set-version N [--through YYYY-MM-DD] --output matrix.bin\n  footy-train fit --input matrix.bin --dataset-version N --feature-set-version N --training-cutoff YYYY-MM-DD --calibration-cutoff YYYY-MM-DD --model-version N --calibration-version N --ridge R --seed N --output model.bin\nmaterialize TSV columns: player_id, as_of, target_log_price, weight, feature:<id>..."
}

fn materialize(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let manifest_path = required_value(args, "--dataset-manifest")?;
    let dataset_version = DatasetVersion(required_u64(args, "--dataset-version")?);
    let feature_set_version = FeatureSetVersion(required_u64(args, "--feature-set-version")?);
    let output = required_value(args, "--output")?;

    let manifest = read_dataset_manifest(manifest_path)?;
    if !manifest.complete || manifest.dataset_version != dataset_version {
        return Err("dataset manifest is incomplete or version-mismatched".into());
    }
    let (_, source_manifest_checksum) = checksum_file(manifest_path)?;
    let text = fs::read_to_string(input)?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("feature TSV is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    if columns.len() < 5 || columns[0..4] != ["player_id", "as_of", "target_log_price", "weight"] {
        return Err("feature TSV must begin with player_id, as_of, target_log_price, weight".into());
    }
    let mut feature_ids = Vec::with_capacity(columns.len() - 4);
    for column in &columns[4..] {
        let raw = column.strip_prefix("feature:").ok_or("feature columns must be named feature:<id>")?;
        feature_ids.push(FeatureId(raw.parse::<u32>()?));
    }

    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(format!("row {} has {} columns; expected {}", offset + 2, fields.len(), columns.len()).into());
        }
        let player = PlayerId(fields[0].trim().parse::<u64>()?);
        let as_of = parse_date(fields[1].trim())?;
        let target_log_price = if fields[2].trim().is_empty() { None } else { Some(fields[2].trim().parse::<f64>()?) };
        let weight = fields[3].trim().parse::<f64>()?;
        let mut features = Vec::with_capacity(feature_ids.len());
        for field in &fields[4..] { features.push(field.trim().parse::<f32>()?); }
        rows.push(FeatureRow { player, as_of, features, target_log_price, weight });
    }
    if rows.is_empty() { return Err("feature TSV contains no rows".into()); }

    let artifact = FeatureMatrixArtifact { dataset_version, source_manifest_checksum, feature_set_version, feature_ids, rows };
    write_feature_matrix(output, &artifact)?;
    println!("materialized dataset={} feature_set={} features={} rows={} manifest_checksum={} output={}", dataset_version.0, feature_set_version.0, artifact.feature_ids.len(), artifact.rows.len(), source_manifest_checksum, output);
    Ok(())
}

fn materialize_canonical(args: &[String]) -> Result<(), Box<dyn Error>> {
    let canonical_path = required_value(args, "--canonical")?;
    let feature_set_version = FeatureSetVersion(required_u64(args, "--feature-set-version")?);
    let through = optional_value(args, "--through").map(parse_date).transpose()?;
    let output = required_value(args, "--output")?;
    let dataset = read_canonical_dataset(canonical_path)?;
    let materialized = materialize_training_features(
        &dataset,
        MaterializationConfig { feature_set_version, through, ..MaterializationConfig::default() },
    )?;
    if materialized.rows.is_empty() {
        return Err("canonical materialization produced no trainable price rows".into());
    }
    let artifact = FeatureMatrixArtifact {
        dataset_version: dataset.dataset_version,
        source_manifest_checksum: dataset.source_manifest_checksum,
        feature_set_version,
        feature_ids: materialized.feature_ids,
        rows: materialized.rows.into_iter().map(|row| FeatureRow { player: row.player, as_of: row.as_of, features: row.features, target_log_price: row.target_log_price, weight: row.weight }).collect(),
    };
    write_feature_matrix(output, &artifact)?;
    println!(
        "materialized_canonical dataset={} feature_set={} features={} rows={} actual_targets={} calibrated_targets={} skipped_no_state={} skipped_uncalibrated_tm={} output={}",
        artifact.dataset_version.0,
        artifact.feature_set_version.0,
        artifact.feature_ids.len(),
        artifact.rows.len(),
        materialized.actual_target_rows,
        materialized.calibrated_target_rows,
        materialized.skipped_targets_without_state,
        materialized.skipped_uncalibrated_market_values,
        output
    );
    Ok(())
}

fn fit(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let output = required_value(args, "--output")?;
    let expected_dataset = DatasetVersion(required_u64(args, "--dataset-version")?);
    let expected_feature_set = FeatureSetVersion(required_u64(args, "--feature-set-version")?);
    let training_cutoff = parse_date(required_value(args, "--training-cutoff")?)?;
    let calibration_cutoff = parse_date(required_value(args, "--calibration-cutoff")?)?;
    let model_version = ModelVersion(required_u64(args, "--model-version")?);
    let calibration_version = CalibrationVersion(required_u64(args, "--calibration-version")?);
    let ridge = required_value(args, "--ridge")?.parse::<f64>()?;
    let seed = required_u64(args, "--seed")?;
    if calibration_cutoff <= training_cutoff { return Err("calibration cutoff must follow training cutoff".into()); }

    let matrix = read_feature_matrix(input)?;
    if matrix.dataset_version != expected_dataset || matrix.feature_set_version != expected_feature_set {
        return Err("explicit dataset/feature version does not match matrix artifact".into());
    }
    if matrix.rows.iter().any(|row| row.as_of > calibration_cutoff) {
        return Err("feature matrix contains rows beyond the calibration cutoff".into());
    }

    let mut train_features = Vec::new();
    let mut train_targets = Vec::new();
    let mut train_weights = Vec::new();
    let mut calibration_rows = Vec::new();
    for row in &matrix.rows {
        let target = row.target_log_price.ok_or("training/calibration matrix contains a missing target")?;
        if row.as_of <= training_cutoff {
            train_features.extend(row.features.iter().map(|value| f64::from(*value)));
            train_targets.push(target);
            train_weights.push(row.weight);
        } else {
            calibration_rows.push(row);
        }
    }
    if train_targets.is_empty() || calibration_rows.is_empty() {
        return Err("training and calibration partitions must both be non-empty".into());
    }
    let x = DenseMatrix::new(train_targets.len(), matrix.feature_ids.len(), train_features)?;
    let model = fit_weighted_linear(&x, &train_targets, Some(&train_weights), ridge)?;

    let mut calibration_residuals = Vec::with_capacity(calibration_rows.len());
    for row in calibration_rows {
        let features: Vec<f64> = row.features.iter().map(|value| f64::from(*value)).collect();
        let predicted = model.predict(&features)?;
        calibration_residuals.push(row.target_log_price.ok_or("missing calibration target")? - predicted);
    }
    let p50_calibrator = ConformalCalibrator::fit(&calibration_residuals, 0.50)?;
    let p80_calibrator = ConformalCalibrator::fit(&calibration_residuals, 0.80)?;
    let artifact = LinearPriceModelArtifact {
        metadata: ModelMetadata { trained_through: training_cutoff, model_version, dataset_version: matrix.dataset_version, feature_set_version: matrix.feature_set_version, seed },
        source_manifest_checksum: matrix.source_manifest_checksum,
        calibration_version,
        calibration_cutoff,
        ridge,
        feature_ids: matrix.feature_ids,
        model,
        p50_calibrator,
        p80_calibrator,
    };
    write_linear_price_model(output, &artifact)?;
    println!("trained dataset={} feature_set={} model={} calibration={} training_rows={} calibration_rows={} cutoff={} calibration_cutoff={} output={}", artifact.metadata.dataset_version.0, artifact.metadata.feature_set_version.0, artifact.metadata.model_version.0, artifact.calibration_version.0, train_targets.len(), calibration_residuals.len(), format_date(training_cutoff), format_date(calibration_cutoff), output);
    Ok(())
}

fn parse_date(value: &str) -> Result<Date, Box<dyn Error>> {
    let mut parts = value.split('-');
    let year = parts.next().ok_or("missing year")?.parse::<i32>()?;
    let month = parts.next().ok_or("missing month")?.parse::<u8>()?;
    let day = parts.next().ok_or("missing day")?.parse::<u8>()?;
    if parts.next().is_some() { return Err("date must be YYYY-MM-DD".into()); }
    Ok(Date::from_ymd(year, month, day)?)
}

fn format_date(date: Date) -> String {
    let (year, month, day) = date.to_ymd();
    format!("{year:04}-{month:02}-{day:02}")
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    optional_value(args, flag).ok_or_else(|| format!("missing {flag}").into())
}

fn optional_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1).map(String::as_str)
}

fn required_u64(args: &[String], flag: &str) -> Result<u64, Box<dyn Error>> { Ok(required_value(args, flag)?.parse::<u64>()?) }
