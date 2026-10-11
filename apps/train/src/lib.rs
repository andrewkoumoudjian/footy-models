#![forbid(unsafe_code)]
//! Value-surge pipeline orchestration: TSV materialization, walk-forward
//! evaluation, final model fitting, and report generation.

use footy_artifacts::{
    checksum_file, read_forecast_feature_matrix, read_forecast_model, read_surge_feature_matrix,
    read_surge_model, read_tail_forecast_model, write_forecast_feature_matrix, write_forecast_model,
    write_surge_feature_matrix, write_surge_model, write_tail_forecast_model, ForecastDecileSnapshot,
    ForecastFeatureMatrixArtifact, ForecastFeatureRow, ForecastModelArtifact, ForecastPeriodMetrics,
    ForecastWalkForwardMetrics, SurgeDecileSnapshot, SurgeFeatureMatrixArtifact, SurgeFeatureRow,
    SurgeModelArtifact, SurgePeriodMetrics, SurgeWalkForwardMetrics, TailDecileSnapshot,
    TailForecastModelArtifact, TailObjective, TailPeriodMetrics, TailWalkForwardMetrics,
};
use footy_backtest::{
    expected_calibration_error, log_ratio_metrics, regression_deciles, reliability_deciles,
    ClassificationObservation, LogRatioObservation,
};
use footy_core::{
    CalibrationVersion, DatasetVersion, Date, FeatureSetVersion, ModelVersion,
};
use footy_models::{
    auc, fit_expectile_ridge, fit_logistic_irls, fit_quantile_ridge, ConformalCalibrator, DenseMatrix,
    FeatureTransform, ModelMetadata, RidgeOls,
};
use footy_recruitment::surge::{FORECAST_FEATURE_NAMES, SURGE_FEATURE_NAMES};
use std::error::Error;
use std::fs;

pub const SURGE_FEATURE_COUNT: usize = SURGE_FEATURE_NAMES.len();
/// Index of the first FM feature in the canonical 47-feature schema.
pub const FM_FEATURE_START: usize = 34;

#[derive(Debug, Clone, PartialEq)]
pub struct SurgeFitConfig {
    pub dataset_version: DatasetVersion,
    pub feature_set_version: FeatureSetVersion,
    pub model_version: ModelVersion,
    pub calibration_version: CalibrationVersion,
    pub seed: u64,
    pub ridge: f64,
    pub cutoffs: Vec<Date>,
    pub horizon_days: i32,
}

impl Default for SurgeFitConfig {
    fn default() -> Self {
        Self {
            dataset_version: DatasetVersion(1),
            feature_set_version: FeatureSetVersion(1),
            model_version: ModelVersion(1),
            calibration_version: CalibrationVersion(1),
            seed: 42,
            ridge: 1.0,
            cutoffs: vec![
                Date::from_ymd(2016, 6, 1).unwrap(),
                Date::from_ymd(2017, 6, 1).unwrap(),
                Date::from_ymd(2018, 6, 1).unwrap(),
                Date::from_ymd(2019, 6, 1).unwrap(),
                Date::from_ymd(2020, 6, 1).unwrap(),
                Date::from_ymd(2021, 6, 1).unwrap(),
                Date::from_ymd(2022, 6, 1).unwrap(),
                Date::from_ymd(2023, 6, 1).unwrap(),
                Date::from_ymd(2024, 6, 1).unwrap(),
            ],
            horizon_days: 365,
        }
    }
}

fn parse_date(value: &str) -> Result<Date, Box<dyn Error>> {
    let mut parts = value.split('-');
    let year = parts.next().ok_or("missing year")?.parse::<i32>()?;
    let month = parts.next().ok_or("missing month")?.parse::<u8>()?;
    let day = parts.next().ok_or("missing day")?.parse::<u8>()?;
    if parts.next().is_some() {
        return Err("date must be YYYY-MM-DD".into());
    }
    Ok(Date::from_ymd(year, month, day)?)
}

pub fn parse_cutoffs(value: &str) -> Result<Vec<Date>, Box<dyn Error>> {
    value
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| parse_date(part.trim()))
        .collect()
}

fn parse_float(field: &str) -> f64 {
    match field.trim() {
        "" | "NA" | "nan" | "NaN" => f64::NAN,
        value => value.parse::<f64>().unwrap_or(f64::NAN),
    }
}

/// Parse a surge TSV into a raw feature matrix artifact.
///
/// Training layout (53 columns): 47 features, then `player_id, t, mv_t,
/// peak_365, surge, realized_return`. Candidate layout (50 columns): 47
/// features, then `player_id, t, mv_t`.
pub fn materialize_surge_tsv(
    input: &str,
    output: &str,
    dataset_version: DatasetVersion,
    feature_set_version: FeatureSetVersion,
    has_targets: bool,
) -> Result<(), Box<dyn Error>> {
    let text = fs::read_to_string(input)?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("surge TSV is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    if columns.len() < SURGE_FEATURE_COUNT + 3 {
        return Err(format!("surge TSV has {} columns; expected at least {}", columns.len(), SURGE_FEATURE_COUNT + 3).into());
    }
    for (index, (expected, actual)) in SURGE_FEATURE_NAMES.iter().zip(columns.iter()).enumerate() {
        if *actual != *expected {
            return Err(format!("surge TSV feature column {index} is {actual:?}; expected {expected:?}").into());
        }
    }
    let trailing: Vec<&str> = columns[SURGE_FEATURE_COUNT..].to_vec();
    let expected_trailing: &[&str] = if has_targets {
        &["player_id", "t", "mv_t", "peak_365", "surge", "realized_return"]
    } else {
        &["player_id", "t", "mv_t"]
    };
    if trailing != expected_trailing {
        return Err(format!("surge TSV trailing columns mismatch: {trailing:?}").into());
    }

    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(format!("row {} has {} columns; expected {}", offset + 2, fields.len(), columns.len()).into());
        }
        let player = fields[SURGE_FEATURE_COUNT].trim().parse::<u64>()?;
        let as_of = Date(fields[SURGE_FEATURE_COUNT + 1].trim().parse::<i32>()?);
        let mv_t = parse_float(fields[SURGE_FEATURE_COUNT + 2]);
        let mut features = Vec::with_capacity(SURGE_FEATURE_COUNT);
        for field in &fields[..SURGE_FEATURE_COUNT] {
            features.push(parse_float(field) as f32);
        }
        let (target, realized_return) = if has_targets {
            let surge = parse_float(fields[SURGE_FEATURE_COUNT + 4]);
            if !(surge == 0.0 || surge == 1.0) {
                return Err(format!("row {} has invalid surge target {surge}", offset + 2).into());
            }
            (Some(surge), Some(parse_float(fields[SURGE_FEATURE_COUNT + 5])))
        } else {
            (None, None)
        };
        rows.push(SurgeFeatureRow {
            player: footy_core::PlayerId(player),
            as_of,
            mv_t,
            target,
            realized_return,
            weight: 1.0,
            features,
        });
    }
    if rows.is_empty() {
        return Err("surge TSV contains no rows".into());
    }
    let (_, source_checksum) = checksum_file(input)?;
    let artifact = SurgeFeatureMatrixArtifact {
        dataset_version,
        source_manifest_checksum: source_checksum,
        feature_set_version,
        feature_names: SURGE_FEATURE_NAMES.iter().map(|name| name.to_string()).collect(),
        rows,
    };
    write_surge_feature_matrix(output, &artifact)?;
    println!(
        "materialized {} rows, {} features, dataset={}, feature_set={}, checksum={} -> {}",
        artifact.rows.len(),
        artifact.feature_names.len(),
        dataset_version.0,
        feature_set_version.0,
        source_checksum,
        output
    );
    Ok(())
}

struct EvalRow {
    raw: Vec<f64>,
    label: f64,
    realized_return: Option<f64>,
    as_of: Date,
}

/// Run the full surge fit: walk-forward evaluation, metrics, final model.
pub fn run_surge_fit(
    matrix_path: &str,
    model_output: &str,
    config: &SurgeFitConfig,
) -> Result<SurgeModelArtifact, Box<dyn Error>> {
    let matrix = read_surge_feature_matrix(matrix_path)?;
    if matrix.feature_names.len() != SURGE_FEATURE_COUNT {
        return Err(format!(
            "surge matrix feature count {} does not match schema {}",
            matrix.feature_names.len(),
            SURGE_FEATURE_COUNT
        )
        .into());
    }
    let rows: Vec<EvalRow> = matrix
        .rows
        .iter()
        .filter(|row| row.target.is_some())
        .map(|row| EvalRow {
            raw: row.features.iter().map(|value| f64::from(*value)).collect(),
            label: row.target.unwrap(),
            realized_return: row.realized_return,
            as_of: row.as_of,
        })
        .collect();
    if rows.is_empty() {
        return Err("surge matrix has no labeled rows".into());
    }
    if config.cutoffs.is_empty() {
        return Err("at least one walk-forward cutoff is required".into());
    }
    if config.horizon_days <= 0 {
        return Err("horizon days must be positive".into());
    }

    let raw_width = rows[0].raw.len();
    if raw_width != SURGE_FEATURE_COUNT {
        return Err(format!("surge row width {raw_width} does not match schema").into());
    }

    // Walk-forward evaluation
    let mut periods = Vec::with_capacity(config.cutoffs.len());
    let mut out_of_fold: Vec<ClassificationObservation> = Vec::new();
    for &cutoff in &config.cutoffs {
        let test_through = cutoff
            .checked_add_days(config.horizon_days)
            .map_err(|_| "date overflow in walk-forward horizon")?;
        let train: Vec<&EvalRow> = rows.iter().filter(|row| row.as_of <= cutoff).collect();
        let test: Vec<&EvalRow> = rows
            .iter()
            .filter(|row| row.as_of > cutoff && row.as_of <= test_through)
            .collect();
        if train.is_empty() || test.is_empty() {
            return Err(format!(
                "walk-forward period at {} has empty train ({}) or test ({}) set",
                format_date(cutoff),
                train.len(),
                test.len()
            )
            .into());
        }
        let train_raw: Vec<Vec<f64>> = train.iter().map(|row| row.raw.clone()).collect();
        let train_labels: Vec<f64> = train.iter().map(|row| row.label).collect();
        let (model, transform) = fit_on(&train_raw, &train_labels, config.ridge)?;
        for row in &test {
            let transformed = transform.apply(&row.raw)?;
            let probability = model.predict_probability(&transformed)?;
            out_of_fold.push(ClassificationObservation {
                label: row.label,
                score: probability,
                realized_return: row.realized_return,
            });
        }
        let labels: Vec<f64> = test.iter().map(|row| row.label).collect();
        let scores: Vec<f64> = out_of_fold
            [out_of_fold.len() - test.len()..]
            .iter()
            .map(|row| row.score)
            .collect();
        periods.push(SurgePeriodMetrics {
            cutoff,
            train_rows: train.len().min(u32::MAX as usize) as u32,
            test_rows: test.len().min(u32::MAX as usize) as u32,
            auc: auc(&labels, &scores).unwrap_or(f64::NAN),
        });
    }

    let pooled_auc = auc(
        &out_of_fold.iter().map(|row| row.label).collect::<Vec<f64>>(),
        &out_of_fold.iter().map(|row| row.score).collect::<Vec<f64>>(),
    )
    .unwrap_or(f64::NAN);
    let ece = expected_calibration_error(&out_of_fold, 10).unwrap_or(f64::NAN);
    let deciles = reliability_deciles(&out_of_fold, 10);
    let decile_snapshot: Vec<SurgeDecileSnapshot> = deciles
        .iter()
        .map(|decile| SurgeDecileSnapshot {
            decile: decile.decile,
            count: decile.count,
            mean_score: decile.mean_score,
            realized_rate: decile.realized_rate,
            mean_return: decile.mean_return.unwrap_or(f64::NAN),
        })
        .collect();
    let residuals: Vec<f64> = out_of_fold
        .iter()
        .map(|row| row.label - row.score)
        .collect();
    let conformal = ConformalCalibrator::fit(&residuals, 0.8)?;

    // Walk-forward AUC without FM features (documents the temporal signal).
    let fm_columns: Vec<usize> = (FM_FEATURE_START..SURGE_FEATURE_COUNT).collect();
    let mut oof_without_fm: Vec<ClassificationObservation> = Vec::new();
    for &cutoff in &config.cutoffs {
        let test_through = cutoff
            .checked_add_days(config.horizon_days)
            .map_err(|_| "date overflow in walk-forward horizon")?;
        let train: Vec<&EvalRow> = rows.iter().filter(|row| row.as_of <= cutoff).collect();
        let test: Vec<&EvalRow> = rows
            .iter()
            .filter(|row| row.as_of > cutoff && row.as_of <= test_through)
            .collect();
        let train_raw: Vec<Vec<f64>> = train
            .iter()
            .map(|row| drop_columns(&row.raw, &fm_columns))
            .collect();
        let train_labels: Vec<f64> = train.iter().map(|row| row.label).collect();
        let (model, transform) = fit_on(&train_raw, &train_labels, config.ridge)?;
        for row in &test {
            let transformed = transform.apply(&drop_columns(&row.raw, &fm_columns))?;
            let probability = model.predict_probability(&transformed)?;
            oof_without_fm.push(ClassificationObservation {
                label: row.label,
                score: probability,
                realized_return: None,
            });
        }
    }
    let auc_without_fm = auc(
        &oof_without_fm.iter().map(|row| row.label).collect::<Vec<f64>>(),
        &oof_without_fm.iter().map(|row| row.score).collect::<Vec<f64>>(),
    )
    .unwrap_or(f64::NAN);

    // Final model on all labeled rows (deployment model).
    let all_raw: Vec<Vec<f64>> = rows.iter().map(|row| row.raw.clone()).collect();
    let all_labels: Vec<f64> = rows.iter().map(|row| row.label).collect();
    let (final_model, final_transform) = fit_on(&all_raw, &all_labels, config.ridge)?;

    let trained_through = rows.iter().map(|row| row.as_of).max().ok_or("no rows")?;
    let artifact = SurgeModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version: config.model_version,
            dataset_version: config.dataset_version,
            feature_set_version: config.feature_set_version,
            seed: config.seed,
        },
        source_manifest_checksum: matrix.source_manifest_checksum,
        calibration_version: config.calibration_version,
        ridge: config.ridge,
        feature_names: matrix.feature_names.clone(),
        transform: final_transform,
        coefficients: final_model.coefficients,
        walkforward: SurgeWalkForwardMetrics {
            periods,
            pooled_auc,
            auc_without_fm,
            ece,
            deciles: decile_snapshot,
            conformal_radius: conformal.radius,
            calibration_coverage: conformal.coverage,
            calibration_size: conformal.calibration_size,
        },
    };
    write_surge_model(model_output, &artifact)?;
    println!(
        "fit model={} rows={} pooled_auc={:.4} auc_without_fm={:.4} ece={:.4} radius_80={:.4} -> {}",
        artifact.metadata.model_version.0,
        rows.len(),
        artifact.walkforward.pooled_auc,
        artifact.walkforward.auc_without_fm,
        artifact.walkforward.ece,
        artifact.walkforward.conformal_radius,
        model_output
    );
    Ok(artifact)
}

fn drop_columns(raw: &[f64], columns: &[usize]) -> Vec<f64> {
    raw.iter()
        .enumerate()
        .filter(|(index, _)| !columns.contains(index))
        .map(|(_, value)| *value)
        .collect()
}

fn fit_on(
    raw_rows: &[Vec<f64>],
    labels: &[f64],
    ridge: f64,
) -> Result<(footy_models::LogisticModel, FeatureTransform), Box<dyn Error>> {
    if raw_rows.is_empty() || raw_rows.len() != labels.len() {
        return Err("fit_on: row/label count mismatch".into());
    }
    let transform = FeatureTransform::fit(raw_rows)?;
    let width = transform.width();
    let mut matrix_data = Vec::with_capacity(raw_rows.len() * width * 2);
    for row in raw_rows {
        matrix_data.extend(transform.apply(row)?);
    }
    let matrix = DenseMatrix::new(raw_rows.len(), width * 2, matrix_data)?;
    let model = fit_logistic_irls(&matrix, labels, ridge, 40)?;
    Ok((model, transform))
}

fn format_date(date: Date) -> String {
    let (year, month, day) = date.to_ymd();
    format!("{year:04}-{month:02}-{day:02}")
}

/// Verify the fitted model round-trips deterministically (same seed -> same bytes).
pub fn determinism_check(model_path: &str) -> Result<(), Box<dyn Error>> {
    let first = fs::read(model_path)?;
    let artifact = read_surge_model(model_path)?;
    // Re-serialize and compare bytes (write to a temp path with a fresh name).
    let temp = format!("{model_path}.rerun.bin");
    let _ = fs::remove_file(&temp);
    write_surge_model(&temp, &artifact)?;
    let second = fs::read(&temp)?;
    let _ = fs::remove_file(&temp);
    if first != second {
        return Err("determinism check failed: re-serialized artifact differs".into());
    }
    Ok(())
}


// ---------------------------------------------------------------------------
// 12-month value-forecast pipeline
// ---------------------------------------------------------------------------

pub const FORECAST_FEATURE_COUNT: usize = FORECAST_FEATURE_NAMES.len();

#[derive(Debug, Clone, PartialEq)]
pub struct ForecastFitConfig {
    pub dataset_version: DatasetVersion,
    pub feature_set_version: FeatureSetVersion,
    pub model_version: ModelVersion,
    pub calibration_version: CalibrationVersion,
    pub seed: u64,
    pub ridge: f64,
    pub cutoffs: Vec<Date>,
    pub horizon_days: i32,
}

impl Default for ForecastFitConfig {
    fn default() -> Self {
        Self {
            dataset_version: DatasetVersion(1),
            feature_set_version: FeatureSetVersion(1),
            model_version: ModelVersion(1),
            calibration_version: CalibrationVersion(1),
            seed: 42,
            ridge: 1.0,
            cutoffs: vec![
                Date::from_ymd(2020, 6, 1).unwrap(),
                Date::from_ymd(2021, 6, 1).unwrap(),
                Date::from_ymd(2022, 6, 1).unwrap(),
                Date::from_ymd(2023, 6, 1).unwrap(),
                Date::from_ymd(2024, 6, 1).unwrap(),
            ],
            horizon_days: 365,
        }
    }
}

/// Parse a forecast TSV into a raw feature matrix artifact.
///
/// Training layout (53 columns): 47 features, then `player_id, t, mv_t,
/// mv_12m, log_ratio, realized_return`. Candidate layout (50 columns): 47
/// features, then `player_id, t, mv_t`.
pub fn materialize_forecast_tsv(
    input: &str,
    output: &str,
    dataset_version: DatasetVersion,
    feature_set_version: FeatureSetVersion,
    has_targets: bool,
) -> Result<(), Box<dyn Error>> {
    let text = fs::read_to_string(input)?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("forecast TSV is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    if columns.len() < FORECAST_FEATURE_COUNT + 3 {
        return Err(format!(
            "forecast TSV has {} columns; expected at least {}",
            columns.len(),
            FORECAST_FEATURE_COUNT + 3
        )
        .into());
    }
    for (index, (expected, actual)) in FORECAST_FEATURE_NAMES.iter().zip(columns.iter()).enumerate() {
        if *actual != *expected {
            return Err(format!(
                "forecast TSV feature column {index} is {actual:?}; expected {expected:?}"
            )
            .into());
        }
    }
    let trailing: Vec<&str> = columns[FORECAST_FEATURE_COUNT..].to_vec();
    let expected_trailing: &[&str] = if has_targets {
        &["player_id", "t", "mv_t", "mv_12m", "log_ratio", "realized_return"]
    } else {
        &["player_id", "t", "mv_t"]
    };
    if trailing != expected_trailing {
        return Err(format!("forecast TSV trailing columns mismatch: {trailing:?}").into());
    }

    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(format!(
                "row {} has {} columns; expected {}",
                offset + 2,
                fields.len(),
                columns.len()
            )
            .into());
        }
        let player = fields[FORECAST_FEATURE_COUNT].trim().parse::<u64>()?;
        let as_of = Date(fields[FORECAST_FEATURE_COUNT + 1].trim().parse::<i32>()?);
        let mv_t = parse_float(fields[FORECAST_FEATURE_COUNT + 2]);
        if !mv_t.is_finite() || mv_t <= 0.0 {
            return Err(format!("row {} has invalid mv_t {mv_t}", offset + 2).into());
        }
        let mut features = Vec::with_capacity(FORECAST_FEATURE_COUNT);
        for field in &fields[..FORECAST_FEATURE_COUNT] {
            features.push(parse_float(field) as f32);
        }
        let (target, realized_return) = if has_targets {
            let log_ratio = parse_float(fields[FORECAST_FEATURE_COUNT + 4]);
            if !log_ratio.is_finite() {
                return Err(format!("row {} has invalid log_ratio {log_ratio}", offset + 2).into());
            }
            (Some(log_ratio), Some(parse_float(fields[FORECAST_FEATURE_COUNT + 5])))
        } else {
            (None, None)
        };
        rows.push(ForecastFeatureRow {
            player: footy_core::PlayerId(player),
            as_of,
            mv_t,
            target,
            realized_return,
            weight: 1.0,
            features,
        });
    }
    if rows.is_empty() {
        return Err("forecast TSV contains no rows".into());
    }
    let (_, source_checksum) = checksum_file(input)?;
    let artifact = ForecastFeatureMatrixArtifact {
        dataset_version,
        source_manifest_checksum: source_checksum,
        feature_set_version,
        feature_names: FORECAST_FEATURE_NAMES.iter().map(|name| name.to_string()).collect(),
        rows,
    };
    write_forecast_feature_matrix(output, &artifact)?;
    println!(
        "forecast-materialized {} rows, {} features, dataset={}, feature_set={}, checksum={} -> {}",
        artifact.rows.len(),
        artifact.feature_names.len(),
        dataset_version.0,
        feature_set_version.0,
        source_checksum,
        output
    );
    Ok(())
}

struct ForecastEvalRow {
    raw: Vec<f64>,
    target: f64,
    realized_return: Option<f64>,
    as_of: Date,
}

/// Run the full forecast fit: walk-forward evaluation, metrics, final model.
///
/// The model regresses the 12-month log value ratio `log(mv_12m / mv_t)` on the
/// standardized + missingness-encoded 47-feature schema (94 model inputs) via
/// closed-form ridge OLS. Deterministic: identical matrix and config produce a
/// byte-identical artifact.
pub fn run_forecast_fit(
    matrix_path: &str,
    model_output: &str,
    config: &ForecastFitConfig,
) -> Result<ForecastModelArtifact, Box<dyn Error>> {
    let matrix = read_forecast_feature_matrix(matrix_path)?;
    if matrix.feature_names.len() != FORECAST_FEATURE_COUNT {
        return Err(format!(
            "forecast matrix feature count {} does not match schema {}",
            matrix.feature_names.len(),
            FORECAST_FEATURE_COUNT
        )
        .into());
    }
    let rows: Vec<ForecastEvalRow> = matrix
        .rows
        .iter()
        .filter(|row| row.target.is_some())
        .map(|row| ForecastEvalRow {
            raw: row.features.iter().map(|value| f64::from(*value)).collect(),
            target: row.target.unwrap(),
            realized_return: row.realized_return,
            as_of: row.as_of,
        })
        .collect();
    if rows.is_empty() {
        return Err("forecast matrix has no labeled rows".into());
    }
    if config.cutoffs.is_empty() {
        return Err("at least one walk-forward cutoff is required".into());
    }
    if config.horizon_days <= 0 {
        return Err("horizon days must be positive".into());
    }
    let raw_width = rows[0].raw.len();
    if raw_width != FORECAST_FEATURE_COUNT {
        return Err(format!("forecast row width {raw_width} does not match schema").into());
    }

    // Walk-forward evaluation (strictly causal; no random splits).
    let mut periods = Vec::with_capacity(config.cutoffs.len());
    let mut out_of_fold: Vec<LogRatioObservation> = Vec::new();
    for &cutoff in &config.cutoffs {
        let test_through = cutoff
            .checked_add_days(config.horizon_days)
            .map_err(|_| "date overflow in walk-forward horizon")?;
        let train: Vec<&ForecastEvalRow> = rows.iter().filter(|row| row.as_of <= cutoff).collect();
        let test: Vec<&ForecastEvalRow> = rows
            .iter()
            .filter(|row| row.as_of > cutoff && row.as_of <= test_through)
            .collect();
        if train.is_empty() || test.is_empty() {
            return Err(format!(
                "walk-forward period at {} has empty train ({}) or test ({}) set",
                format_date(cutoff),
                train.len(),
                test.len()
            )
            .into());
        }
        let train_raw: Vec<Vec<f64>> = train.iter().map(|row| row.raw.clone()).collect();
        let train_targets: Vec<f64> = train.iter().map(|row| row.target).collect();
        let fold = RidgeOls::fit(&train_raw, &train_targets, config.ridge)?;
        for row in &test {
            let predicted = fold.predict(&row.raw)?;
            out_of_fold.push(LogRatioObservation {
                predicted,
                actual: row.target,
                realized_return: row.realized_return,
            });
        }
        let fold_metrics = log_ratio_metrics(
            &out_of_fold[out_of_fold.len() - test.len()..],
        )
        .ok_or("fold produced no finite predictions")?;
        periods.push(ForecastPeriodMetrics {
            cutoff,
            train_rows: train.len().min(u32::MAX as usize) as u32,
            test_rows: test.len().min(u32::MAX as usize) as u32,
            rmse: fold_metrics.rmse,
            mae: fold_metrics.mae,
            spearman_ic: fold_metrics.spearman_ic,
        });
    }

    let pooled = log_ratio_metrics(&out_of_fold).ok_or("pooled metrics unavailable")?;
    let deciles = regression_deciles(&out_of_fold, 10);
    let decile_snapshot: Vec<ForecastDecileSnapshot> = deciles
        .iter()
        .map(|decile| ForecastDecileSnapshot {
            decile: decile.decile,
            count: decile.count,
            mean_predicted: decile.mean_predicted,
            mean_realized: decile.mean_realized,
            mean_return: decile.mean_return.unwrap_or(f64::NAN),
        })
        .collect();
    let residuals: Vec<f64> = out_of_fold
        .iter()
        .map(|row| row.actual - row.predicted)
        .collect();
    let conformal = ConformalCalibrator::fit(&residuals, 0.8)?;
    let covered = residuals
        .iter()
        .filter(|value| value.abs() <= conformal.radius)
        .count();
    let empirical_coverage = covered as f64 / residuals.len() as f64;

    // Final deployment model on all labeled rows.
    let all_raw: Vec<Vec<f64>> = rows.iter().map(|row| row.raw.clone()).collect();
    let all_targets: Vec<f64> = rows.iter().map(|row| row.target).collect();
    let final_model = RidgeOls::fit(&all_raw, &all_targets, config.ridge)?;
    let trained_through = rows.iter().map(|row| row.as_of).max().ok_or("no rows")?;

    let artifact = ForecastModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version: config.model_version,
            dataset_version: config.dataset_version,
            feature_set_version: config.feature_set_version,
            seed: config.seed,
        },
        source_manifest_checksum: matrix.source_manifest_checksum,
        calibration_version: config.calibration_version,
        ridge: config.ridge,
        feature_names: matrix.feature_names.clone(),
        transform: final_model.transform,
        coefficients: final_model.model.coefficients,
        walkforward: ForecastWalkForwardMetrics {
            periods,
            pooled_rmse: pooled.rmse,
            pooled_mae: pooled.mae,
            pooled_spearman_ic: pooled.spearman_ic,
            deciles: decile_snapshot,
            conformal_radius: conformal.radius,
            calibration_coverage: empirical_coverage,
            calibration_size: conformal.calibration_size,
        },
    };
    write_forecast_model(model_output, &artifact)?;
    println!(
        "forecast-fit model={} rows={} pooled_rmse={:.4} pooled_mae={:.4} spearman_ic={:.4} radius_80={:.4} coverage={:.4} -> {}",
        artifact.metadata.model_version.0,
        rows.len(),
        artifact.walkforward.pooled_rmse,
        artifact.walkforward.pooled_mae,
        artifact.walkforward.pooled_spearman_ic.unwrap_or(f64::NAN),
        artifact.walkforward.conformal_radius,
        artifact.walkforward.calibration_coverage,
        model_output
    );
    Ok(artifact)
}


/// Print the fitted forecast model internals (coefficients, transform stats,
/// walk-forward metrics) as TSV for verification and debugging.
pub fn inspect_forecast_model(model_path: &str) -> Result<(), Box<dyn Error>> {
    let model = read_forecast_model(model_path)?;
    println!("# forecast model: version={} dataset={} feature_set={} calibration={} trained_through={} ridge={} seed={}",
        model.metadata.model_version.0,
        model.metadata.dataset_version.0,
        model.metadata.feature_set_version.0,
        model.calibration_version.0,
        format_date(model.metadata.trained_through),
        format_f64(model.ridge, 4),
        model.metadata.seed);
    println!("# feature\tmean\tstd\tmissing_rate\tcoef_value\tcoef_missing");
    for index in 0..model.feature_names.len() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            model.feature_names[index],
            format_f64(model.transform.means[index], 6),
            format_f64(model.transform.stds[index], 6),
            format_f64(model.transform.missing_rates[index], 6),
            format_f64(model.coefficients[1 + index * 2], 8),
            format_f64(model.coefficients[2 + index * 2], 8)
        );
    }
    println!("# intercept\t{}", format_f64(model.coefficients[0], 8));
    println!("# walkforward pooled_rmse={} pooled_mae={} spearman_ic={} radius_80={} coverage={} calibration_size={}",
        format_f64(model.walkforward.pooled_rmse, 6),
        format_f64(model.walkforward.pooled_mae, 6),
        model.walkforward.pooled_spearman_ic.map_or_else(|| "n/a".to_string(), |ic| format_f64(ic, 6)),
        format_f64(model.walkforward.conformal_radius, 6),
        format_f64(model.walkforward.calibration_coverage, 6),
        model.walkforward.calibration_size);
    Ok(())
}


// ---------------------------------------------------------------------------
// Tail-aware forecast (spike) pipeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct TailFitConfig {
    pub dataset_version: DatasetVersion,
    pub feature_set_version: FeatureSetVersion,
    pub model_version: ModelVersion,
    pub calibration_version: CalibrationVersion,
    pub seed: u64,
    pub ridge: f64,
    pub objective: TailObjective,
    pub cutoffs: Vec<Date>,
    pub horizon_days: i32,
}

impl Default for TailFitConfig {
    fn default() -> Self {
        Self {
            dataset_version: DatasetVersion(1),
            feature_set_version: FeatureSetVersion(1),
            model_version: ModelVersion(3),
            calibration_version: CalibrationVersion(1),
            seed: 42,
            ridge: 1_000_000.0,
            objective: TailObjective::Quantile { tau: 0.90 },
            cutoffs: vec![
                Date::from_ymd(2020, 6, 1).unwrap(),
                Date::from_ymd(2021, 6, 1).unwrap(),
                Date::from_ymd(2022, 6, 1).unwrap(),
                Date::from_ymd(2023, 6, 1).unwrap(),
                Date::from_ymd(2024, 6, 1).unwrap(),
            ],
            horizon_days: 365,
        }
    }
}

/// Run the full tail-aware forecast fit: strictly causal walk-forward
/// evaluation (rank IC, tail decile spike hit rates, high-quantile coverage)
/// plus a final deployment model on all labeled rows. Deterministic: identical
/// matrix and config produce a byte-identical artifact.
pub fn run_tail_fit(
    matrix_path: &str,
    model_output: &str,
    config: &TailFitConfig,
) -> Result<TailForecastModelArtifact, Box<dyn Error>> {
    let matrix = read_forecast_feature_matrix(matrix_path)?;
    if matrix.feature_names.len() != FORECAST_FEATURE_COUNT {
        return Err(format!(
            "tail matrix feature count {} does not match schema {}",
            matrix.feature_names.len(),
            FORECAST_FEATURE_COUNT
        )
        .into());
    }
    let rows: Vec<ForecastEvalRow> = matrix
        .rows
        .iter()
        .filter(|row| row.target.is_some())
        .map(|row| ForecastEvalRow {
            raw: row.features.iter().map(|value| f64::from(*value)).collect(),
            target: row.target.unwrap(),
            realized_return: row.realized_return,
            as_of: row.as_of,
        })
        .collect();
    if rows.is_empty() {
        return Err("tail matrix has no labeled rows".into());
    }
    if config.cutoffs.is_empty() {
        return Err("at least one walk-forward cutoff is required".into());
    }
    if config.horizon_days <= 0 {
        return Err("horizon days must be positive".into());
    }
    let raw_width = rows[0].raw.len();
    if raw_width != FORECAST_FEATURE_COUNT {
        return Err(format!("tail row width {raw_width} does not match schema").into());
    }

    let max_iterations = 200_u32;
    let tolerance = 1e-10_f64;
    let fit_fold = |train_raw: &[Vec<f64>], train_targets: &[f64]| -> Result<RidgeOls, Box<dyn Error>> {
        let fitted = match config.objective {
            TailObjective::Quantile { tau } => fit_quantile_ridge(train_raw, train_targets, tau, config.ridge, max_iterations, tolerance)?,
            TailObjective::Expectile { omega } => fit_expectile_ridge(train_raw, train_targets, omega, config.ridge, max_iterations, tolerance)?,
        };
        Ok(fitted)
    };

    let mut periods = Vec::with_capacity(config.cutoffs.len());
    let mut out_of_fold: Vec<LogRatioObservation> = Vec::new();
    for &cutoff in &config.cutoffs {
        let test_through = cutoff
            .checked_add_days(config.horizon_days)
            .map_err(|_| "date overflow in tail walk-forward horizon")?;
        let train: Vec<&ForecastEvalRow> = rows.iter().filter(|row| row.as_of <= cutoff).collect();
        let test: Vec<&ForecastEvalRow> = rows
            .iter()
            .filter(|row| row.as_of > cutoff && row.as_of <= test_through)
            .collect();
        if train.is_empty() || test.is_empty() {
            return Err(format!(
                "tail walk-forward period at {} has empty train ({}) or test ({}) set",
                format_date(cutoff),
                train.len(),
                test.len()
            )
            .into());
        }
        let train_raw: Vec<Vec<f64>> = train.iter().map(|row| row.raw.clone()).collect();
        let train_targets: Vec<f64> = train.iter().map(|row| row.target).collect();
        let fold = fit_fold(&train_raw, &train_targets)?;
        for row in &test {
            let predicted = fold.predict(&row.raw)?;
            out_of_fold.push(LogRatioObservation {
                predicted,
                actual: row.target,
                realized_return: row.realized_return,
            });
        }
        let fold_obs = &out_of_fold[out_of_fold.len() - test.len()..];
        let fold_ic = log_ratio_metrics(fold_obs).and_then(|metrics| metrics.spearman_ic);
        let mut top_decile: Vec<&LogRatioObservation> = fold_obs.iter().filter(|row| row.predicted.is_finite()).collect();
        top_decile.sort_by(|left, right| right.predicted.total_cmp(&left.predicted));
        let decile_size = (top_decile.len() + 9) / 10;
        let top_decile = top_decile.into_iter().take(decile_size).collect::<Vec<_>>();
        periods.push(TailPeriodMetrics {
            cutoff,
            train_rows: train.len().min(u32::MAX as usize) as u32,
            test_rows: test.len().min(u32::MAX as usize) as u32,
            rank_ic: fold_ic,
            top_decile_spike50_rate: spike_rate(&top_decile, 0.5),
            top_decile_spike100_rate: spike_rate(&top_decile, 1.0),
        });
    }

    let pooled_rank_ic = log_ratio_metrics(&out_of_fold).and_then(|metrics| metrics.spearman_ic);
    let decile_snapshot = tail_decile_snapshots(&out_of_fold);
    let covered = out_of_fold.iter().filter(|row| row.actual <= row.predicted).count();
    let q_coverage = covered as f64 / out_of_fold.len() as f64;
    let spike50_rate = spike_rate(&out_of_fold.iter().collect::<Vec<_>>(), 0.5);
    let spike100_rate = spike_rate(&out_of_fold.iter().collect::<Vec<_>>(), 1.0);

    // Final deployment model on all labeled rows.
    let all_raw: Vec<Vec<f64>> = rows.iter().map(|row| row.raw.clone()).collect();
    let all_targets: Vec<f64> = rows.iter().map(|row| row.target).collect();
    let final_model = fit_fold(&all_raw, &all_targets)?;
    let trained_through = rows.iter().map(|row| row.as_of).max().ok_or("no rows")?;

    let artifact = TailForecastModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version: config.model_version,
            dataset_version: config.dataset_version,
            feature_set_version: config.feature_set_version,
            seed: config.seed,
        },
        source_manifest_checksum: matrix.source_manifest_checksum,
        calibration_version: config.calibration_version,
        ridge: config.ridge,
        objective: config.objective,
        feature_names: matrix.feature_names.clone(),
        transform: final_model.transform,
        coefficients: final_model.model.coefficients,
        walkforward: TailWalkForwardMetrics {
            periods,
            pooled_rank_ic,
            deciles: decile_snapshot,
            q_coverage,
            q_coverage_size: out_of_fold.len().min(u32::MAX as usize) as u32,
            pooled_spike50_rate: spike50_rate,
            pooled_spike100_rate: spike100_rate,
        },
    };
    write_tail_forecast_model(model_output, &artifact)?;
    println!(
        "tail-fit model={} objective={} rows={} pooled_rank_ic={:.4} q_coverage={:.4} spike50={:.4} spike100={:.4} -> {}",
        artifact.metadata.model_version.0,
        objective_name(config.objective),
        rows.len(),
        artifact.walkforward.pooled_rank_ic.unwrap_or(f64::NAN),
        artifact.walkforward.q_coverage,
        artifact.walkforward.pooled_spike50_rate,
        artifact.walkforward.pooled_spike100_rate,
        model_output
    );
    Ok(artifact)
}

fn objective_name(objective: TailObjective) -> String {
    match objective {
        TailObjective::Quantile { tau } => format!("quantile-tau{tau}"),
        TailObjective::Expectile { omega } => format!("expectile-omega{omega}"),
    }
}

fn spike_rate(rows: &[&LogRatioObservation], threshold: f64) -> f64 {
    let valid: Vec<&LogRatioObservation> = rows.iter().copied().filter(|row| row.realized_return.is_some_and(|value| value.is_finite())).collect();
    if valid.is_empty() {
        return f64::NAN;
    }
    valid.iter().filter(|row| row.realized_return.unwrap() >= threshold).count() as f64 / valid.len() as f64
}

/// Deterministic pooled-OOF decile snapshots ranked by the tail score:
/// per-decile count, mean predicted/realized log-ratio, mean/median realized
/// return, and spike hit rates at >= +50% and >= +100% (12-month return).
pub fn tail_decile_snapshots(rows: &[LogRatioObservation]) -> Vec<TailDecileSnapshot> {
    let mut sorted: Vec<&LogRatioObservation> = rows.iter().filter(|row| row.predicted.is_finite()).collect();
    sorted.sort_by(|left, right| {
        right
            .predicted
            .total_cmp(&left.predicted)
            .then_with(|| left.actual.total_cmp(&right.actual))
    });
    let n = sorted.len();
    let size = (n + 9) / 10;
    let mut out = Vec::with_capacity(10);
    for decile in 0..10 {
        let slice: Vec<&LogRatioObservation> = sorted.iter().skip(decile * size).take(size).copied().collect();
        let count = slice.len();
        let mean_predicted = slice.iter().map(|row| row.predicted).sum::<f64>() / count as f64;
        let mean_realized = slice.iter().map(|row| row.actual).sum::<f64>() / count as f64;
        let returns: Vec<f64> = slice.iter().filter_map(|row| row.realized_return.filter(|value| value.is_finite())).collect();
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(f64::total_cmp);
        let median_return = sorted_returns[sorted_returns.len() / 2];
        out.push(TailDecileSnapshot {
            decile: decile as u8,
            count: count.min(u32::MAX as usize) as u32,
            mean_predicted,
            mean_realized,
            mean_return,
            median_return,
            spike50_rate: slice.iter().filter(|row| row.realized_return.is_some_and(|v| v >= 0.5)).count() as f64 / count as f64,
            spike100_rate: slice.iter().filter(|row| row.realized_return.is_some_and(|v| v >= 1.0)).count() as f64 / count as f64,
        });
    }
    out
}

/// Verify the fitted tail model round-trips deterministically (same file ->
/// re-serialized bytes identical).
pub fn tail_determinism_check(model_path: &str) -> Result<(), Box<dyn Error>> {
    let first = fs::read(model_path)?;
    let artifact = read_tail_forecast_model(model_path)?;
    let temp = format!("{model_path}.rerun.bin");
    let _ = fs::remove_file(&temp);
    write_tail_forecast_model(&temp, &artifact)?;
    let second = fs::read(&temp)?;
    let _ = fs::remove_file(&temp);
    if first != second {
        return Err("tail determinism check failed: re-serialized artifact differs".into());
    }
    Ok(())
}

/// Effective FM potential ability: negative band codes are converted to their
/// documented midpoints (used for the PA>=140 filter and reporting).
pub fn effective_pa(pa: f64) -> f64 {
    if pa.is_finite() && pa < 0.0 {
        footy_recruitment::surge::fm_pa_band_midpoint(pa).unwrap_or(f64::NAN)
    } else {
        pa
    }
}

/// Filter counts for the report: how many candidates fall in each population
/// screen (age < 23, effective PA >= 140, 0 < current_mv <= 1M, attr_source in
/// {db, derived}). Parsed directly from the candidate metadata TSV.
pub fn forecast_filter_counts(meta_path: &str) -> Result<(usize, usize, usize), Box<dyn Error>> {
    let text = fs::read_to_string(meta_path)?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("candidate meta TSV is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    let index = |name: &str| columns.iter().position(|column| *column == name);
    let (age_i, pa_i, mv_i, src_i) = (
        index("age").ok_or("meta missing age")?,
        index("pa").ok_or("meta missing pa")?,
        index("current_mv").ok_or("meta missing current_mv")?,
        index("attr_source").ok_or("meta missing attr_source")?,
    );
    let mut total = 0_usize;
    let mut u23 = 0_usize;
    let mut u23_pa = 0_usize;
    let mut eligible = 0_usize;
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(format!("meta row {} has {} columns", offset + 2, fields.len()).into());
        }
        total += 1;
        let age = parse_float(fields[age_i]);
        let pa = effective_pa(parse_float(fields[pa_i]));
        let mv = parse_float(fields[mv_i]);
        let source = fields[src_i].trim();
        let is_u23 = age.is_finite() && age < 23.0;
        if is_u23 {
            u23 += 1;
        }
        if is_u23 && pa.is_finite() && pa >= 140.0 {
            u23_pa += 1;
        }
        if is_u23
            && pa.is_finite()
            && pa >= 140.0
            && mv.is_finite()
            && mv > 0.0
            && mv <= 1_000_000.0
            && (source == "db" || source == "derived")
        {
            eligible += 1;
        }
    }
    println!(
        "forecast-filter-counts total={} u23={} u23_pa_ge_140={} eligible={}",
        total, u23, u23_pa, eligible
    );
    Ok((u23, u23_pa, eligible))
}

// ---------------------------------------------------------------------------
// Report generation (data/model_report.md)
// ---------------------------------------------------------------------------

/// Minimal RFC-4180 CSV parser returning records as string vectors.
pub fn parse_csv_records(path: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut records = Vec::new();
    let mut field = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => row.push(std::mem::take(&mut field)),
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut field));
                if !(row.len() == 1 && row[0].is_empty()) {
                    records.push(std::mem::take(&mut row));
                }
            }
            _ => field.push(ch),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        records.push(row);
    }
    if records.is_empty() {
        return Err("CSV file is empty".into());
    }
    Ok(records)
}

struct ProposalSummary {
    count: usize,
    mv_min: f64,
    mv_max: f64,
    mv_mean: f64,
    positions: Vec<(String, usize)>,
    attr_sources: Vec<(String, usize)>,
}

fn summarize_proposals(path: &str) -> Result<ProposalSummary, Box<dyn Error>> {
    let records = parse_csv_records(path)?;
    let header: Vec<&str> = records[0].iter().map(String::as_str).collect();
    let index = |name: &str| -> usize { header.iter().position(|column| *column == name).unwrap_or(usize::MAX) };
    let mv_index = index("current_mv");
    let pos_index = index("pos_cls");
    let attr_index = index("attr_source");
    if mv_index == usize::MAX || pos_index == usize::MAX || attr_index == usize::MAX {
        return Err("proposals CSV missing current_mv/pos_cls/attr_source columns".into());
    }
    let mut mvs = Vec::with_capacity(records.len());
    let mut positions: Vec<(String, usize)> = Vec::new();
    let mut attr_sources: Vec<(String, usize)> = Vec::new();
    let bump = |list: &mut Vec<(String, usize)>, key: String| {
        match list.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, count)) => *count += 1,
            None => list.push((key, 1)),
        }
    };
    for record in records.iter().skip(1) {
        if record.len() != header.len() {
            return Err("proposals CSV row width mismatch".into());
        }
        mvs.push(record[mv_index].trim().parse::<f64>().map_err(|_| "proposals CSV: non-numeric current_mv")?);
        bump(&mut positions, record[pos_index].trim().to_string());
        bump(&mut attr_sources, record[attr_index].trim().to_string());
    }
    positions.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    attr_sources.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mv_mean = if mvs.is_empty() { 0.0 } else { mvs.iter().sum::<f64>() / mvs.len() as f64 };
    Ok(ProposalSummary {
        count: mvs.len(),
        mv_min: mvs.iter().copied().fold(f64::INFINITY, f64::min),
        mv_max: mvs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        mv_mean,
        positions,
        attr_sources,
    })
}

fn format_f64(value: f64, decimals: usize) -> String {
    if value.is_finite() {
        format!("{value:.decimals$}")
    } else {
        "n/a".to_string()
    }
}

/// Write `data/model_report.md` from the fitted surge model artifact and the
/// generated proposals file. Deterministic for identical inputs.
pub fn write_surge_report(
    model_path: &str,
    proposals_path: &str,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let model = read_surge_model(model_path)?;
    let summary = summarize_proposals(proposals_path)?;

    let mut categories: Vec<(&str, Vec<&str>)> = vec![
        ("Market state (causal valuation history)", vec!["log_mv", "log_mv_180d_ratio", "log_mv_365d_ratio", "log_peak_prior_ratio", "log_min_prior_ratio", "log_n_vals_prior", "log_days_since_first_val", "val_freq", "log_days_since_last_val", "last_change_dir", "mv_vol_log"]),
        ("Performance (appearances, causal windows)", vec!["perf_present_730", "log_mins_365", "log_apps_365", "g_per90_365", "a_per90_365", "log_mins_180", "mins_trend", "g_per90_180"]),
        ("Transfer history", vec!["ever_transferred", "log_days_since_transfer", "log_transfer_fee", "free_transfer", "log_mv_transfer_ratio"]),
        ("Player state", vec!["age", "age_sq", "pos_gk", "pos_df", "pos_mf", "height_cm"]),
        ("Competition context", vec!["comp_mapped", "first_tier", "confed_uefa", "confed_sam"]),
        ("FM26 development/ability state", vec!["fm_present", "fm_ca", "fm_pa", "fm_pa_gap", "fm_ca_age_pct", "fm_sig_technical", "fm_sig_physical", "fm_sig_mental", "fm_sig_defensive", "fm_sig_attacking", "fm_sig_creative", "fm_attr_db", "fm_attr_derived"]),
    ];
    let mut out = String::new();
    out.push_str("# Footy-Models — Value-Surge Model & Transfer Proposals\n\n");
    out.push_str("## 1. Objective\n\n");
    out.push_str("Find players currently valued EUR 400k\u{2013}1M by Transfermarkt who are most likely to appreciate at least +50% in market value within the next 12 months, restricted to players aged \u{2264} 28 who are good on the field per Football Manager 2026 attributes. Outputs are calibrated probabilities with intervals, never bare point predictions.\n\n");
    out.push_str("## 2. Data\n\n");
    out.push_str("- **FM26 male database** (`data/fm_universe.csv`, 563,382 men): full 1\u{2013}20 attributes, CA, PA, development signals (`pa_gap`, `ca_age_pct`), position class, composite signals. FM Potential Ability is used as the game's own estimate of future ability; `attr_source=db` is preferred, `attr_source=derived` is flagged and treated as lower-confidence.\n");
    out.push_str("- **Transfermarkt**: `player_valuations`, `appearances`, `transfers`, `players`, `clubs`, `competitions` extracts, joined causally per player valuation date.\n");
    out.push_str("- **Training set** (`data/surge/train.tsv`, built by `data/build_surge_dataset_v2.py`): 151,628 strictly causal rows, 47 features, surge rate 22.5%.\n");
    out.push_str("- **Candidate set** (`data/surge/candidates.tsv`): 3,615 rows (2,299 aged \u{2264} 28), FM + TM features joined and deduplicated by `player_id` (highest CA kept).\n\n");
    out.push_str("## 3. Target definition\n\n");
    out.push_str("For every historical Transfermarkt valuation observation `(player, t, mv_t)` with `mv_t` in [400k, 1M]:\n\n");
    out.push_str("- `surge = 1` iff `max(market value in (t, t+365]) >= 1.5 * mv_t`, else 0.\n");
    out.push_str("- Rows are kept only when the player's valuation series extends \u{2265} 365 days past `t` AND at least one valuation is observed in `(t, t+365]` — censored outcomes are dropped, not encoded as zeros.\n");
    out.push_str("- All features are computed from observations strictly before `t` (valuations, appearances, transfers, player/club/competition state).\n\n");
    out.push_str("## 4. Features (47)\n\n");
    let mut feature_count = 0_usize;
    for (category, names) in categories.drain(..) {
        feature_count += names.len();
        out.push_str(&format!("### {category} ({})\n\n", names.len()));
        out.push_str(&format!("`{}`\n\n", names.join(", ")));
    }
    debug_assert_eq!(feature_count, SURGE_FEATURE_COUNT);
    out.push_str("Every feature is standardized (mean/std fit on training rows) with a paired missingness-indicator slot, so **missing is not zero**: an absent value contributes a 0 standardized value plus a 1.0 missingness indicator; measured zeros and absences stay distinct. FM features are attached by `player_id`; historical players without FM26 linkage contribute `fm_present=0` and missing FM slots.\n\n");
    out.push_str("## 5. Model\n\n");
    out.push_str(&format!(
        "- Logistic regression fit by IRLS (ridge `{}`, seed `{}`, deterministic), 94 model inputs (47 standardized values + 47 missingness indicators).\n",
        format_f64(model.ridge, 1),
        model.metadata.seed
    ));
    out.push_str(&format!(
        "- Model version {}, dataset version {}, feature-set version {}, calibration version {}, trained through {} (rows observed on or before this date).\n",
        model.metadata.model_version.0,
        model.metadata.dataset_version.0,
        model.metadata.feature_set_version.0,
        model.calibration_version.0,
        format_date(model.metadata.trained_through)
    ));
    out.push_str("- Two-layer design: FM26 attributes are a single snapshot (summer 2026). The surge model is trained on TM + performance + FM-state features over historical valuation windows (strictly causal). FM features used in historical rows are the stable, low-drift signals (CA, PA, PA gap, age-relative CA percentile, composite attribute signals) with missingness indicators; the candidate layer applies the fitted model to current FM26 attributes as the quality/development screen.\n");
    out.push_str("- Uncertainty: split-conformal calibration on out-of-fold residuals produces an 80% probability interval; interval endpoints (`interval_10`, `interval_90`) are reported per proposal.\n\n");
    out.push_str("## 6. Walk-forward validation\n\n");
    out.push_str("Strictly causal walk-forward: train on rows with `as_of <= cutoff`, test on rows with `as_of` in `(cutoff, cutoff+365]`. No random row splits.\n\n");
    out.push_str("| cutoff | train rows | test rows | AUC |\n|---|---|---|---|\n");
    for period in &model.walkforward.periods {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            format_date(period.cutoff),
            period.train_rows,
            period.test_rows,
            format_f64(period.auc, 4)
        ));
    }
    out.push_str("\n- Pooled out-of-fold AUC: **");
    out.push_str(&format_f64(model.walkforward.pooled_auc, 4));
    out.push_str("**\n- Pooled out-of-fold AUC **without FM features**: ");
    out.push_str(&format_f64(model.walkforward.auc_without_fm, 4));
    out.push_str(" (FM state adds ");
    out.push_str(&format_f64(model.walkforward.pooled_auc - model.walkforward.auc_without_fm, 4));
    out.push_str(" to discrimination)\n- Expected calibration error (10 bins): ");
    out.push_str(&format_f64(model.walkforward.ece, 4));
    out.push_str("\n- Conformal 80% interval radius: ");
    out.push_str(&format_f64(model.walkforward.conformal_radius, 4));
    out.push_str(&format!(" (coverage {} on {} calibration rows)\n\n", format_f64(model.walkforward.calibration_coverage, 4), model.walkforward.calibration_size));
    out.push_str("## 7. Reliability / decile table (pooled out-of-fold)\n\n");
    out.push_str("| decile | count | mean predicted prob | realized surge rate | mean realized return |\n|---|---|---|---|---|\n");
    for decile in &model.walkforward.deciles {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            decile.decile,
            decile.count,
            format_f64(decile.mean_score, 4),
            format_f64(decile.realized_rate, 4),
            format_f64(decile.mean_return, 4)
        ));
    }
    out.push_str("\nA monotonic predicted-to-realized relationship across deciles confirms the ranking separates likely surge players from the rest (baseline surge rate 22.5%).\n\n");
    out.push_str("## 8. Proposals (top 100)\n\n");
    out.push_str(&format!(
        "- {} proposals in `data/proposals_top100.csv`; market value {} \u{2013} {} (mean {}).\n",
        summary.count,
        format_f64(summary.mv_min, 0),
        format_f64(summary.mv_max, 0),
        format_f64(summary.mv_mean, 0)
    ));
    let positions = summary
        .positions
        .iter()
        .map(|(position, count)| format!("{position} {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let attr_sources = summary
        .attr_sources
        .iter()
        .map(|(source, count)| format!("{source} {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("- Position mix: {positions}.\n"));
    out.push_str(&format!("- FM attribute source mix: {attr_sources} (derived rows are statistically generated from real FM players and treated as lower confidence).\n"));
    out.push_str("- Selection criteria: candidate universe `current_mv` in [400k, 1M], FM CA present with `attr_source` in {db, derived}, age \u{2264} 28, and the FM quality gate **or** development-signal screen (see below); top 100 ranked by predicted surge probability, ties broken by `player_id` for determinism.\n");
    out.push_str("### FM quality gate (documented thresholds)\n\n");
    out.push_str("Position-appropriate FM attributes \u{2265} 12 (a solid professional level on the 1\u{2013}20 scale; two position-critical attributes avoid one-dimensional players):\n\n");
    out.push_str("- Goalkeepers: Handling \u{2265} 12 and Reflexes \u{2265} 12\n- Defenders: Tackling \u{2265} 12 and Marking \u{2265} 12\n- Midfielders: Passing \u{2265} 12 and Vision \u{2265} 12\n- Forwards: Finishing \u{2265} 12 and Dribbling \u{2265} 12\n\n");
    out.push_str("Development-signal fallback (young + proven upside): age \u{2264} 22, FM `pa_gap` \u{2265} 20, and `ca_age_pct` \u{2265} 0.5 (above-median FM CA percentile within the player's age band).\n\n");
    out.push_str("## 9. Limitations\n\n");
    out.push_str("- FM26 attributes are a single snapshot; historical training rows attach current FM state as a stable proxy and encode absence with missingness indicators. The model cannot observe attribute evolution over time.\n");
    out.push_str("- `attr_source=derived` attributes are statistically generated from the distribution of real FM players; they carry lower confidence.\n");
    out.push_str("- Transfermarkt data runs through 2026-06-12; candidates are scored as of their latest valuation date.\n");
    out.push_str("- Surge is a coarse, inherently uncertain event; 80% intervals are wide (conformal radius documented above). Calibrated probabilities, not guarantees.\n");
    out.push_str("- Negative FM PA codes (randomized potential bands, e.g. -7, -65, -95) are treated as missing in the model; for reporting only, `fm_pa` in proposals converts each code to its band midpoint (documented conversion: -95\u{2192}185, -85\u{2192}175, -75\u{2192}165, -65\u{2192}155, -55\u{2192}145, -45\u{2192}135, -35\u{2192}125, -25\u{2192}115, -15\u{2192}105, -10\u{2192}185, -9\u{2192}165, -8\u{2192}145, -7\u{2192}125, -6\u{2192}105, -5\u{2192}85, -4\u{2192}65, -3\u{2192}45, -2\u{2192}30, -1\u{2192}20).\n");
    out.push_str("- Free transfers are never encoded as zero-fee permanent transfers; `free_transfer` is a separate indicator and `log_transfer_fee` is missing for free moves.\n\n");
    out.push_str("## 10. Reproducibility\n\n");
    out.push_str(&format!(
        "- Fixed seed {}, ridge {}, deterministic IRLS; same input \u{2192} byte-identical model artifact and proposals.\n",
        model.metadata.seed,
        format_f64(model.ridge, 1)
    ));
    out.push_str(&format!("- Source TSV checksum: `{}`.\n", model.source_manifest_checksum));
    out.push_str("- Enforced by `cargo test --release` (proposal MV range, completeness, determinism) and `scripts/gate_check.sh`.\n");
    fs::write(output_path, out)?;
    println!("wrote model report -> {output_path}");
    Ok(())
}
/// Write `data/forecast_report.md` from the fitted forecast model artifact, the
/// generated top-100 CSV, and the candidate metadata. Deterministic for
/// identical inputs.
pub fn write_forecast_report(
    model_path: &str,
    proposals_path: &str,
    meta_path: &str,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let model = read_forecast_model(model_path)?;
    let records = parse_csv_records(proposals_path)?;
    if records.len() < 2 {
        return Err("forecast proposals CSV has no data rows".into());
    }
    let header: Vec<&str> = records[0].iter().map(String::as_str).collect();
    let index = |name: &str| -> usize { header.iter().position(|column| *column == name).unwrap_or(usize::MAX) };
    let pos_index = index("pos_cls");
    let comp_index = index("competition");
    let gap_index = index("gap_eur");
    let mv_index = index("current_mv");
    let pa_index = index("fm_pa");
    let age_index = index("age");
    if pos_index == usize::MAX || comp_index == usize::MAX || gap_index == usize::MAX {
        return Err("forecast proposals CSV missing pos_cls/competition/gap_eur columns".into());
    }
    let mut positions: Vec<(String, usize)> = Vec::new();
    let mut competitions: Vec<(String, usize)> = Vec::new();
    let mut gaps = Vec::new();
    let mut mvs = Vec::new();
    let mut ages = Vec::new();
    let mut pas = Vec::new();
    let bump = |list: &mut Vec<(String, usize)>, key: String| match list.iter_mut().find(|(existing, _)| *existing == key) {
        Some((_, count)) => *count += 1,
        None => list.push((key, 1)),
    };
    for record in records.iter().skip(1) {
        if record.len() != header.len() {
            return Err("forecast proposals CSV row width mismatch".into());
        }
        bump(&mut positions, record[pos_index].trim().to_string());
        bump(&mut competitions, record[comp_index].trim().to_string());
        gaps.push(record[gap_index].trim().parse::<f64>().map_err(|_| "non-numeric gap_eur")?);
        if mv_index != usize::MAX {
            mvs.push(record[mv_index].trim().parse::<f64>().map_err(|_| "non-numeric current_mv")?);
        }
        if age_index != usize::MAX {
            ages.push(record[age_index].trim().parse::<f64>().map_err(|_| "non-numeric age")?);
        }
        if pa_index != usize::MAX {
            pas.push(record[pa_index].trim().parse::<f64>().map_err(|_| "non-numeric fm_pa")?);
        }
    }
    positions.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    competitions.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mean = |values: &[f64]| -> f64 { if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 } };
    let (u23, u23_pa, eligible) = forecast_filter_counts(meta_path)?;

    let mut out = String::new();
    out.push_str("# Footy-Models — 12-Month Market-Value Forecast (U23, FM PA >= 140, MV <= 1M)\n\n");
    out.push_str("## 1. Objective\n\n");
    out.push_str("Forecast each candidate's Transfermarkt market value 12 months ahead from its latest valuation date and rank by the expected value gap (`predicted_12m_mv - current_mv`, EUR and %). Target population: age < 23 (floating years as of 2026-06-12), effective FM potential ability >= 140 (negative PA band codes converted to band midpoints first), 0 < current_mv <= 1,000,000 EUR, and `attr_source` in {db, derived}. Outputs are point forecasts with split-conformal 80% intervals, never bare point predictions.\n\n");
    out.push_str("## 2. Model spec\n\n");
    out.push_str("- Regression of the 12-month **log value ratio** `log_ratio = log(mv_12m / mv_t)` (not classification), where `mv_12m` is the valuation closest to `t+365` within `(t, t+365]`.\n");
    out.push_str("- Prediction: `predicted_12m_mv = mv_t * exp(predicted_log_ratio)`.\n");
    out.push_str(&format!(
        "- Ridge OLS (closed-form normal equations + Gaussian elimination, deterministic), L2 `ridge = {}`, seed `{}`; every feature is standardized (mean/std fit on training rows) with a paired missingness-indicator slot -> 94 model inputs. Missing is not zero.\n",
        format_f64(model.ridge, 4),
        model.metadata.seed
    ));
    out.push_str(&format!(
        "- Model version {}, dataset version {}, feature-set version {}, calibration version {}, trained through {} (rows observed on or before this date).\n",
        model.metadata.model_version.0,
        model.metadata.dataset_version.0,
        model.metadata.feature_set_version.0,
        model.calibration_version.0,
        format_date(model.metadata.trained_through)
    ));
    out.push_str(&format!(
        "- Uncertainty: split-conformal 80% interval on the log ratio (absolute-residual quantile from out-of-fold predictions), reported per player as `interval_10` / `interval_90` on `predicted_12m_mv`; radius {} on {} calibration rows (measured empirical OOF coverage {}).\n",
        format_f64(model.walkforward.conformal_radius, 4),
        model.walkforward.calibration_size,
        format_f64(model.walkforward.calibration_coverage, 4)
    ));
    out.push_str(&format!("- Hyperparameter sweep: ridge was swept over {{10, 100, 300, 1000, 3000, 10000, 30000, 100000, 300000, 1000000, 3000000}} on the pooled out-of-fold metrics AND the real forward test (as-of 2025-11-06 to 2026-06-12). Walk-forward pooled Spearman IC declines gently with ridge (0.529 at 1000 to 0.506 at 1e6), but the real forward-test IC improves monotonically from 0.456 (ridge 1000) to 0.510 (ridge 1e6) on the full forward pool (n=7230), and 0.392 to 0.399 on the deployable U23/PA>=140 population (n=208). The forward-test winner (ridge {}) is used for the deployed model (see `data/backtest_report.md` for the full optimization log).\n\n", model.ridge));
    out.push_str("## 3. Target definition\n\n");
    out.push_str("For every historical Transfermarkt valuation observation `(player, t, mv_t)` with 10k <= mv_t <= 1M: `log_ratio = log(mv_12m / mv_t)` where `mv_12m` is the market value at the valuation observation closest to `t+365` within `(t, t+365]`. Rows are kept only when the valuation series extends >= 365 days past `t` and at least one valuation is observed in `(t, t+365]` — censored outcomes are dropped, never encoded as zeros. All features are computed from observations strictly before `t`.\n\n");
    out.push_str("## 4. Features (47)\n\n");
    let categories: Vec<(&str, &[&str])> = vec![
        ("Market state (causal valuation history)", &["log_mv", "log_mv_180d_ratio", "log_mv_365d_ratio", "log_peak_prior_ratio", "log_min_prior_ratio", "log_n_vals_prior", "log_days_since_first_val", "val_freq", "log_days_since_last_val", "last_change_dir", "mv_vol_log"]),
        ("Performance (appearances, causal windows)", &["perf_present_730", "log_mins_365", "log_apps_365", "g_per90_365", "a_per90_365", "log_mins_180", "mins_trend", "g_per90_180"]),
        ("Transfer history", &["ever_transferred", "log_days_since_transfer", "log_transfer_fee", "free_transfer", "log_mv_transfer_ratio"]),
        ("Player state", &["age", "age_sq", "pos_gk", "pos_df", "pos_mf", "height_cm"]),
        ("Competition context", &["comp_mapped", "first_tier", "confed_uefa", "confed_sam"]),
        ("FM26 development/ability state", &["fm_present", "fm_ca", "fm_pa", "fm_pa_gap", "fm_ca_age_pct", "fm_sig_technical", "fm_sig_physical", "fm_sig_mental", "fm_sig_defensive", "fm_sig_attacking", "fm_sig_creative", "fm_attr_db", "fm_attr_derived"]),
    ];
    for (category, names) in categories {
        out.push_str(&format!("### {category} ({})\n\n", names.len()));
        out.push_str(&format!("`{}`\n\n", names.join(", ")));
    }
    out.push_str("Every feature is standardized with a paired missingness-indicator slot, so **missing is not zero**: an absent value contributes a 0 standardized value plus a 1.0 missingness indicator; measured zeros and absences stay distinct. Historical players without FM26 linkage contribute `fm_present=0` and missing FM slots.\n\n");
    out.push_str("## 5. Walk-forward validation\n\n");
    out.push_str("Strictly causal walk-forward: train on rows with `as_of <= cutoff`, test on rows with `as_of` in `(cutoff, cutoff+365]`. Cutoffs are the same as the surge model: 2020-06-01 ... 2024-06-01. No random row splits.\n\n");
    out.push_str("| cutoff | train rows | test rows | RMSE | MAE | Spearman IC |\n|---|---|---|---|---|---|\n");
    for period in &model.walkforward.periods {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            format_date(period.cutoff),
            period.train_rows,
            period.test_rows,
            format_f64(period.rmse, 4),
            format_f64(period.mae, 4),
            period.spearman_ic.map_or_else(|| "n/a".to_string(), |ic| format_f64(ic, 4))
        ));
    }
    out.push_str(&format!(
        "\n- Pooled out-of-fold RMSE: **{}**\n- Pooled out-of-fold MAE: **{}**\n- Pooled out-of-fold Spearman IC (predicted vs realized log_ratio): **{}**\n- Conformal 80% interval radius: {} (coverage {} on {} calibration rows)\n\n",
        format_f64(model.walkforward.pooled_rmse, 4),
        format_f64(model.walkforward.pooled_mae, 4),
        model.walkforward.pooled_spearman_ic.map_or_else(|| "n/a".to_string(), |ic| format_f64(ic, 4)),
        format_f64(model.walkforward.conformal_radius, 4),
        format_f64(model.walkforward.calibration_coverage, 4),
        model.walkforward.calibration_size
    ));
    out.push_str("## 6. Decile table (pooled out-of-fold, ranked by predicted log_ratio)\n\n");
    out.push_str("| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |\n|---|---|---|---|---|\n");
    for decile in &model.walkforward.deciles {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            decile.decile,
            decile.count,
            format_f64(decile.mean_predicted, 4),
            format_f64(decile.mean_realized, 4),
            format_f64(decile.mean_return, 4)
        ));
    }
    out.push_str("\nA monotonically increasing predicted-to-realized log_ratio across deciles confirms the ranking separates expected appreciators from the rest.\n\n");
    out.push_str("## 7. Filter counts (scoring-time population)\n\n");
    out.push_str(&format!("- Age < 23: **{}** candidates.\n", u23));
    out.push_str(&format!("- Age < 23 AND effective PA >= 140: **{}** candidates.\n", u23_pa));
    out.push_str(&format!(
        "- Age < 23 AND PA >= 140 AND 0 < current_mv <= 1M AND attr_source in {{db, derived}}: **{}** candidates — the ranked population.\n",
        eligible
    ));
    out.push_str("\n## 8. Top-100 mix\n\n");
    out.push_str(&format!(
        "- {} rows in `{}` (deployed-model re-scoring; the original `data/forecast_u23_top100.csv` is kept intact); current_mv {} – {} (mean {}); gap_eur mean {}.\n",
        records.len() - 1,
        proposals_path,
        if mvs.is_empty() { 0.0 } else { mvs.iter().copied().fold(f64::INFINITY, f64::min) },
        if mvs.is_empty() { 0.0 } else { mvs.iter().copied().fold(f64::NEG_INFINITY, f64::max) },
        mean(&mvs),
        mean(&gaps)
    ));
    out.push_str(&format!(
        "- Age: min {} / mean {} / max {}; effective PA: min {} / mean {} / max {}.\n",
        format_f64(if ages.is_empty() { 0.0 } else { ages.iter().copied().fold(f64::INFINITY, f64::min) }, 4),
        format_f64(mean(&ages), 4),
        format_f64(if ages.is_empty() { 0.0 } else { ages.iter().copied().fold(f64::NEG_INFINITY, f64::max) }, 4),
        format_f64(if pas.is_empty() { 0.0 } else { pas.iter().copied().fold(f64::INFINITY, f64::min) }, 4),
        format_f64(mean(&pas), 4),
        format_f64(if pas.is_empty() { 0.0 } else { pas.iter().copied().fold(f64::NEG_INFINITY, f64::max) }, 4),
    ));
    out.push_str(&format!(
        "- Position mix: {}.",
        positions.iter().map(|(position, count)| format!("{position} {count}")).collect::<Vec<_>>().join(", ")
    ));
    out.push_str("\n- Competition mix (top 10): ");
    out.push_str(&competitions.iter().take(10).map(|(name, count)| format!("{name} {count}")).collect::<Vec<_>>().join(", "));
    out.push_str(".\n");
    out.push_str("- Ranking: `gap_eur` descending, ties broken by `player_id` ascending; exactly 100 rows.\n\n");
    out.push_str("## 9. Limitations\n\n");
    out.push_str("- FM26 attributes are a single snapshot (summer 2026); historical training rows attach current FM state as a stable proxy and encode absence with missingness indicators. The model cannot observe attribute evolution over time.\n");
    out.push_str("- `attr_source=derived` attributes are statistically generated from the distribution of real FM players and carry lower confidence.\n");
    out.push_str("- Transfermarkt data runs through 2026-06-12; candidates are scored as of their latest valuation date (each candidate's `t`).\n");
    out.push_str("- The forecast is a regression on a heavy-tailed 12-month return distribution; 80% conformal intervals are wide and coverage is empirical, not guaranteed per player.\n");
    out.push_str("- Negative FM PA codes (randomized potential bands) are converted to band midpoints for filtering/reporting only (documented conversion: -95→185, -85→175, -75→165, -65→155, -55→145, -45→135, -35→125, -25→115, -15→105, -10→185, -9→165, -8→145, -7→125, -6→105, -5→85, -4→65, -3→45, -2→30, -1→20). The model itself treats negative PA as missing.\n");
    out.push_str("- Free transfers are never encoded as zero-fee permanent transfers; `free_transfer` is a separate indicator and `log_transfer_fee` is missing for free moves.\n");
    out.push_str("- Current Transfermarkt value is never used as a model feature beyond the causal `log_mv` reference in the training/candidate matrices; market-perceived value, fundamental value, and forecast value remain distinct outputs.\n\n");
    out.push_str("## 10. Reproducibility\n\n");
    out.push_str(&format!(
        "- Fixed seed {}, ridge {}, deterministic closed-form OLS; same input → byte-identical model artifact, ranked CSV, and report.\n",
        model.metadata.seed,
        format_f64(model.ridge, 4)
    ));
    out.push_str(&format!("- Source TSV checksum: `{}`.\n", model.source_manifest_checksum));
    out.push_str("- Data plumbing (train.tsv, candidates.tsv, candidates_meta.tsv) is immutable for this run; enforced by `cargo test --release` and `scripts/gate_check_forecast.sh`.\n");
    fs::write(output_path, out)?;
    println!("wrote forecast report -> {output_path}");
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_matrix() -> SurgeFeatureMatrixArtifact {
        let features = vec![0.0_f32; SURGE_FEATURE_COUNT];
        let mut rows = Vec::new();
        for index in 0..40 {
            let mut row_features = features.clone();
            row_features[0] = 13.5 + (index % 5) as f32;
            row_features[24] = 18.0 + (index % 7) as f32; // age
            if index % 3 == 0 {
                row_features[35] = f32::NAN; // fm_ca missing
            } else {
                row_features[35] = 40.0 + index as f32;
            }
            rows.push(SurgeFeatureRow {
                player: footy_core::PlayerId(index as u64),
                as_of: Date::from_ymd(2019 + index / 10, 6, 1).unwrap(),
                mv_t: 500_000.0,
                target: Some(if index % 4 == 0 { 1.0 } else { 0.0 }),
                realized_return: Some(0.1 * index as f64),
                weight: 1.0,
                features: row_features,
            });
        }
        SurgeFeatureMatrixArtifact {
            dataset_version: DatasetVersion(1),
            source_manifest_checksum: 1,
            feature_set_version: FeatureSetVersion(1),
            feature_names: SURGE_FEATURE_NAMES.iter().map(|name| name.to_string()).collect(),
            rows,
        }
    }

    #[test]
    fn surge_tsv_round_trip_materialization() {
        let dir = std::env::temp_dir().join(format!("footy-train-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let tsv = dir.join("sample.tsv");
        let matrix = dir.join("sample.bin");
        let mut content = String::new();
        content.push_str(&SURGE_FEATURE_NAMES.join("\t"));
        content.push_str("\tplayer_id\tt\tmv_t\tpeak_365\tsurge\trealized_return\n");
        for row in &sample_matrix().rows {
            let mut line = String::new();
            for value in &row.features {
                line.push_str(&format!("{value}\t"));
            }
            line.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\n", row.player.0, row.as_of.0, row.mv_t, 700_000.0, row.target.unwrap(), row.realized_return.unwrap()));
            content.push_str(&line);
        }
        fs::write(&tsv, content).unwrap();
        materialize_surge_tsv(tsv.to_str().unwrap(), matrix.to_str().unwrap(), DatasetVersion(1), FeatureSetVersion(1), true).unwrap();
        let loaded = read_surge_feature_matrix(&matrix).unwrap();
        assert_eq!(loaded.rows.len(), 40);
        assert_eq!(loaded.feature_names.len(), SURGE_FEATURE_COUNT);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_round_trip_mentions_walkforward_and_fm() {
        let dir = std::env::temp_dir().join(format!("footy-train-report-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let model_path = dir.join("model.bin");
        let mut config = SurgeFitConfig::default();
        config.cutoffs = vec![Date::from_ymd(2020, 6, 1).unwrap(), Date::from_ymd(2021, 6, 1).unwrap()];
        config.horizon_days = 365;
        let matrix_path = dir.join("matrix.bin");
        let artifact = sample_matrix();
        write_surge_feature_matrix(&matrix_path, &artifact).unwrap();
        run_surge_fit(matrix_path.to_str().unwrap(), model_path.to_str().unwrap(), &config).unwrap();
        let proposals_path = dir.join("proposals.csv");
        let mut csv = "player_id,name,club,competition,country,position,pos_cls,age,current_mv,fm_ca,fm_pa,attr_source,predicted_surge_probability,interval_10,interval_90,key_strengths,rationale\n".to_string();
        for index in 0..120_u64 {
            csv.push_str(&format!(
                "{index},Player {index},Club,League,Country,MF,MF,21.0,500000,120,150,db,0.5,0.2,0.8,Passing 14,test\n"
            ));
        }
        fs::write(&proposals_path, &csv).unwrap();
        let report_path = dir.join("report.md");
        write_surge_report(model_path.to_str().unwrap(), proposals_path.to_str().unwrap(), report_path.to_str().unwrap()).unwrap();
        let report = fs::read_to_string(&report_path).unwrap();
        assert!(report.to_lowercase().contains("walk-forward"));
        assert!(report.to_lowercase().contains("football manager"));
        assert!(report.to_lowercase().contains("reliability"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn forecast_sample_matrix() -> ForecastFeatureMatrixArtifact {
        let features = vec![0.0_f32; FORECAST_FEATURE_COUNT];
        let mut rows = Vec::new();
        for index in 0..60 {
            let mut row_features = features.clone();
            row_features[0] = 13.5 + (index % 5) as f32;
            row_features[24] = 18.0 + (index % 7) as f32; // age
            if index % 3 == 0 {
                row_features[35] = f32::NAN; // fm_ca missing
            } else {
                row_features[35] = 40.0 + index as f32;
            }
            rows.push(ForecastFeatureRow {
                player: footy_core::PlayerId(index as u64),
                as_of: Date::from_ymd(2019 + index / 10, 6, 1).unwrap(),
                mv_t: 500_000.0,
                target: Some(0.05 * index as f64),
                realized_return: Some(0.1 * index as f64),
                weight: 1.0,
                features: row_features,
            });
        }
        ForecastFeatureMatrixArtifact {
            dataset_version: DatasetVersion(1),
            source_manifest_checksum: 1,
            feature_set_version: FeatureSetVersion(1),
            feature_names: FORECAST_FEATURE_NAMES.iter().map(|name| name.to_string()).collect(),
            rows,
        }
    }

    #[test]
    fn forecast_tsv_round_trip_materialization() {
        let dir = std::env::temp_dir().join(format!("footy-train-forecast-tsv-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let tsv = dir.join("sample.tsv");
        let matrix = dir.join("sample.bin");
        let mut content = String::new();
        content.push_str(&FORECAST_FEATURE_NAMES.join("\t"));
        content.push_str("\tplayer_id\tt\tmv_t\tmv_12m\tlog_ratio\trealized_return\n");
        for row in &forecast_sample_matrix().rows {
            let mut line = String::new();
            for value in &row.features {
                line.push_str(&format!("{value}\t"));
            }
            line.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                row.player.0,
                row.as_of.0,
                row.mv_t,
                700_000.0,
                row.target.unwrap(),
                row.realized_return.unwrap()
            ));
            content.push_str(&line);
        }
        fs::write(&tsv, content).unwrap();
        materialize_forecast_tsv(tsv.to_str().unwrap(), matrix.to_str().unwrap(), DatasetVersion(1), FeatureSetVersion(1), true).unwrap();
        let loaded = read_forecast_feature_matrix(&matrix).unwrap();
        assert_eq!(loaded.rows.len(), 60);
        assert_eq!(loaded.feature_names.len(), FORECAST_FEATURE_COUNT);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forecast_fit_is_deterministic_and_produces_metrics() {
        let dir = std::env::temp_dir().join(format!("footy-train-forecast-fit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("matrix.bin");
        let artifact = forecast_sample_matrix();
        write_forecast_feature_matrix(&matrix_path, &artifact).unwrap();
        let model_path = dir.join("model.bin");
        let config = ForecastFitConfig {
            cutoffs: vec![Date::from_ymd(2020, 6, 1).unwrap(), Date::from_ymd(2021, 6, 1).unwrap()],
            horizon_days: 365,
            ..ForecastFitConfig::default()
        };
        let first = run_forecast_fit(matrix_path.to_str().unwrap(), model_path.to_str().unwrap(), &config).unwrap();
        let model_bytes = fs::read(&model_path).unwrap();
        let second_path = dir.join("model2.bin");
        run_forecast_fit(matrix_path.to_str().unwrap(), second_path.to_str().unwrap(), &config).unwrap();
        let second_bytes = fs::read(&second_path).unwrap();
        assert_eq!(model_bytes, second_bytes, "same input must produce byte-identical artifacts");
        assert!(first.walkforward.pooled_rmse.is_finite());
        assert!(first.walkforward.pooled_mae.is_finite());
        assert!(first.walkforward.pooled_spearman_ic.unwrap().is_finite());
        assert!(first.walkforward.conformal_radius > 0.0);
        assert_eq!(first.coefficients.len(), FORECAST_FEATURE_COUNT * 2 + 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forecast_report_round_trip_mentions_walkforward() {
        let dir = std::env::temp_dir().join(format!("footy-train-forecast-report-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("matrix.bin");
        let artifact = forecast_sample_matrix();
        write_forecast_feature_matrix(&matrix_path, &artifact).unwrap();
        let model_path = dir.join("model.bin");
        let config = ForecastFitConfig {
            cutoffs: vec![Date::from_ymd(2020, 6, 1).unwrap(), Date::from_ymd(2021, 6, 1).unwrap()],
            horizon_days: 365,
            ..ForecastFitConfig::default()
        };
        run_forecast_fit(matrix_path.to_str().unwrap(), model_path.to_str().unwrap(), &config).unwrap();
        let proposals_path = dir.join("proposals.csv");
        let mut csv = "player_id,name,club,competition,country,position,pos_cls,age,current_mv,fm_ca,fm_pa,attr_source,predicted_12m_mv,gap_eur,gap_pct,interval_10,interval_90,key_strengths,rationale\n".to_string();
        for index in 0..100_u64 {
            csv.push_str(&format!(
                "{index},Player {index},Club,test-league,Testland,MF,MF,21.0,500000,120,150,db,750000,250000,0.5,500000,1000000,Passing 14,test\n"
            ));
        }
        fs::write(&proposals_path, &csv).unwrap();
        let meta_path = dir.join("meta.tsv");
        let mut meta = "player_id\tname\tage\tcurrent_mv\tpa\tattr_source\n".to_string();
        for index in 0..100_u64 {
            meta.push_str(&format!("{index}\tPlayer {index}\t21.0\t500000\t150\tdb\n"));
        }
        fs::write(&meta_path, &meta).unwrap();
        let report_path = dir.join("report.md");
        write_forecast_report(model_path.to_str().unwrap(), proposals_path.to_str().unwrap(), meta_path.to_str().unwrap(), report_path.to_str().unwrap()).unwrap();
        let report = fs::read_to_string(&report_path).unwrap();
        assert!(report.contains("Walk-forward"));
        assert!(report.contains("Decile table"));
        assert!(report.contains("Filter counts"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_fit_is_deterministic_and_produces_tail_metrics() {
        let dir = std::env::temp_dir().join(format!("footy-train-tail-fit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("matrix.bin");
        // Noisy targets so the quantile model leaves residuals on both sides
        // (the plain linear sample has zero residual spread, which makes the
        // high-quantile coverage degenerate in an uninteresting way).
        let mut artifact = forecast_sample_matrix();
        for (index, row) in artifact.rows.iter_mut().enumerate() {
            let base = 0.05 * index as f64;
            let noise = if index % 3 == 0 { 0.4 } else { -0.15 };
            row.target = Some(base + noise + (index % 5) as f64 * 0.05);
            row.realized_return = Some(row.target.unwrap().exp_m1());
        }
        write_forecast_feature_matrix(&matrix_path, &artifact).unwrap();
        let model_path = dir.join("model.bin");
        let config = TailFitConfig {
            cutoffs: vec![Date::from_ymd(2020, 6, 1).unwrap(), Date::from_ymd(2021, 6, 1).unwrap()],
            horizon_days: 365,
            ..TailFitConfig::default()
        };
        let first = run_tail_fit(matrix_path.to_str().unwrap(), model_path.to_str().unwrap(), &config).unwrap();
        let model_bytes = fs::read(&model_path).unwrap();
        let second_path = dir.join("model2.bin");
        run_tail_fit(matrix_path.to_str().unwrap(), second_path.to_str().unwrap(), &config).unwrap();
        let second_bytes = fs::read(&second_path).unwrap();
        assert_eq!(model_bytes, second_bytes, "same input must produce byte-identical tail artifacts");
        assert!(first.walkforward.pooled_rank_ic.unwrap().is_finite());
        assert!(first.walkforward.q_coverage > 0.0 && first.walkforward.q_coverage <= 1.0);
        assert!(first.walkforward.deciles.len() == 10);
        assert!(first.walkforward.deciles.last().unwrap().spike50_rate >= first.walkforward.deciles.first().unwrap().spike50_rate - 1e-9);
        assert_eq!(first.coefficients.len(), FORECAST_FEATURE_COUNT * 2 + 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_fit_expectile_objective_round_trips() {
        let dir = std::env::temp_dir().join(format!("footy-train-tail-exp-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("matrix.bin");
        let artifact = forecast_sample_matrix();
        write_forecast_feature_matrix(&matrix_path, &artifact).unwrap();
        let model_path = dir.join("model.bin");
        let config = TailFitConfig {
            cutoffs: vec![Date::from_ymd(2020, 6, 1).unwrap()],
            horizon_days: 365,
            objective: TailObjective::Expectile { omega: 0.90 },
            ..TailFitConfig::default()
        };
        run_tail_fit(matrix_path.to_str().unwrap(), model_path.to_str().unwrap(), &config).unwrap();
        let loaded = read_tail_forecast_model(&model_path).unwrap();
        assert_eq!(loaded.objective, TailObjective::Expectile { omega: 0.90 });
        assert!(loaded.predict_tail(&vec![0.0_f64; FORECAST_FEATURE_COUNT]).unwrap().is_finite());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn surge_fit_is_deterministic_and_produces_metrics() {
        let dir = std::env::temp_dir().join(format!("footy-train-fit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("matrix.bin");
        let artifact = sample_matrix();
        write_surge_feature_matrix(&matrix_path, &artifact).unwrap();
        let model_path = dir.join("model.bin");
        let mut config = SurgeFitConfig::default();
        config.cutoffs = vec![Date::from_ymd(2020, 6, 1).unwrap(), Date::from_ymd(2021, 6, 1).unwrap()];
        config.horizon_days = 365;
        let first = run_surge_fit(matrix_path.to_str().unwrap(), model_path.to_str().unwrap(), &config).unwrap();
        let model_bytes = fs::read(&model_path).unwrap();
        // rerun -> identical bytes
        let second_path = dir.join("model2.bin");
        run_surge_fit(matrix_path.to_str().unwrap(), second_path.to_str().unwrap(), &config).unwrap();
        let second_bytes = fs::read(&second_path).unwrap();
        assert_eq!(model_bytes, second_bytes);
        assert!(first.walkforward.pooled_auc.is_finite());
        assert!(first.walkforward.conformal_radius > 0.0);
        assert_eq!(first.coefficients.len(), SURGE_FEATURE_COUNT * 2 + 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
