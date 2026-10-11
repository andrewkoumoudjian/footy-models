//! Research-only, CPU-only FM26 player-shot residual model using Candle.
//!
//! Counts are observed SofaScore totalShots; realized minutes are an evaluation *exposure*,
//! never a forecast-time feature. The strictly earlier-match prior is supplied by the dated
//! Python materializer. This binary does not modify any production forecast.

use candle_core::{Device, Tensor, Var};
use candle_nn::{AdamW, Optimizer, ParamsAdamW};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader};

#[derive(Deserialize)]
struct Row {
    split: String,
    date: String,
    match_id: u64,
    player_id: u64,
    shots: u32,
    minutes: f64,
    prior_rate: f64,
    features: Vec<f64>,
}

struct Batch {
    features: Tensor,
    expected: Tensor,
    offset: Tensor,
}

#[derive(Debug, Serialize)]
struct Scores {
    examples: usize,
    matches: usize,
    poisson_log_loss: f64,
    shot_occurrence_brier: f64,
    mean_expected_shots: f64,
    mean_observed_shots: f64,
}

#[derive(Serialize)]
struct ModelWeights {
    input_features: usize,
    hidden: usize,
    activation: &'static str,
    output_clamp: &'static str,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
}

#[derive(Serialize)]
struct PredictionRecord {
    split: String,
    date: String,
    match_id: u64,
    player_id: u64,
    shots: u32,
    actual_minutes: f64,
    baseline_expected_shots: f64,
    candle_expected_shots: f64,
}

#[derive(Serialize)]
struct Report {
    experiment: &'static str,
    training_examples: usize,
    validation_examples: usize,
    test_examples: usize,
    best_epoch: usize,
    selected_by: &'static str,
    hyperparameters: serde_json::Value,
    split_windows: serde_json::Value,
    baseline: serde_json::Value,
    candle: serde_json::Value,
    deltas_candidate_minus_baseline: serde_json::Value,
    promotion: &'static str,
    caveats: &'static [&'static str],
}

struct Model {
    w1: Var,
    b1: Var,
    w2: Var,
    b2: Var,
    input: usize,
    hidden: usize,
}

impl Model {
    fn new(input: usize, hidden: usize, device: &Device) -> candle_core::Result<Self> {
        // Stable deterministic initialization independent of Candle global RNG and hardware.
        let init = |n: usize, stride: f64, amplitude: f64| -> Vec<f32> {
            (0..n)
                .map(|i| (amplitude * ((i as f64 + 1.0) * stride).sin()) as f32)
                .collect()
        };
        Ok(Self {
            w1: Var::from_tensor(&Tensor::from_vec(
                init(input * hidden, 9.193, 0.10),
                (input, hidden),
                device,
            )?)?,
            b1: Var::from_tensor(&Tensor::zeros(
                (1, hidden),
                candle_core::DType::F32,
                device,
            )?)?,
            w2: Var::from_tensor(&Tensor::from_vec(
                init(hidden, 0.381, 0.025),
                (hidden, 1),
                device,
            )?)?,
            b2: Var::from_tensor(&Tensor::zeros((1, 1), candle_core::DType::F32, device)?)?,
            input,
            hidden,
        })
    }

    fn residual(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        // Bounded log-rate multiplier; no unbounded path rates on out-of-domain players.
        let hidden = x
            .matmul(self.w1.as_tensor())?
            .broadcast_add(self.b1.as_tensor())?
            .relu()?;
        let log_rate = hidden
            .matmul(self.w2.as_tensor())?
            .broadcast_add(self.b2.as_tensor())?;
        log_rate.tanh()?.affine(1.2, 0.0)
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            self.w1.clone(),
            self.b1.clone(),
            self.w2.clone(),
            self.b2.clone(),
        ]
    }

    fn weights(&self) -> candle_core::Result<ModelWeights> {
        fn flattened(var: &Var) -> candle_core::Result<Vec<f32>> {
            var.flatten_all()?.to_vec1::<f32>()
        }
        Ok(ModelWeights {
            input_features: self.input,
            hidden: self.hidden,
            activation: "relu",
            output_clamp: "1.2*tanh(raw)",
            w1: flattened(&self.w1)?,
            b1: flattened(&self.b1)?,
            w2: flattened(&self.w2)?,
            b2: flattened(&self.b2)?,
        })
    }
}

fn read_rows(path: &str) -> Result<Vec<Row>, Box<dyn Error>> {
    let input = BufReader::new(fs::File::open(path)?);
    let mut rows: Vec<Row> = Vec::new();
    let mut keys = BTreeSet::new();
    for line in input.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let row: Row = serde_json::from_str(&line)?;
        if row.features.is_empty()
            || !row.features.iter().all(|x| x.is_finite())
            || !row.prior_rate.is_finite()
            || row.prior_rate <= 0.0
            || !row.minutes.is_finite()
            || !(0.0..=130.0).contains(&row.minutes)
            || row.minutes == 0.0
            || !["train", "validation", "test"].contains(&row.split.as_str())
            || !keys.insert((row.match_id, row.player_id))
        {
            return Err("invalid/duplicate dated match row".into());
        }
        if !rows.is_empty() && row.features.len() != rows[0].features.len() {
            return Err("inconsistent feature width".into());
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err("dataset contains no rows".into());
    }
    let boundaries = |split: &str| -> Result<(String, String), Box<dyn Error>> {
        let mut iter = rows
            .iter()
            .filter(|r| r.split == split)
            .map(|r| r.date.clone());
        let first = iter.next().ok_or("missing chronological split")?;
        let (mut lo, mut hi) = (first.clone(), first);
        for date in iter {
            if date < lo {
                lo = date.clone();
            }
            if date > hi {
                hi = date;
            }
        }
        Ok((lo, hi))
    };
    let train = boundaries("train")?;
    let val = boundaries("validation")?;
    let test = boundaries("test")?;
    if !(train.1 < val.0 && val.1 < test.0) {
        return Err("train/validation/test dates overlap".into());
    }
    Ok(rows)
}

fn batch(rows: &[&Row], device: &Device) -> candle_core::Result<Batch> {
    let n = rows.len();
    let dim = rows[0].features.len();
    let feature_values: Vec<f32> = rows
        .iter()
        .flat_map(|r| r.features.iter().map(|x| *x as f32))
        .collect();
    let shots: Vec<f32> = rows.iter().map(|r| r.shots as f32).collect();
    let offset: Vec<f32> = rows
        .iter()
        .map(|r| (r.prior_rate * r.minutes / 90.0) as f32)
        .collect();
    Ok(Batch {
        features: Tensor::from_vec(feature_values, (n, dim), device)?,
        expected: Tensor::from_vec(shots, (n, 1), device)?,
        offset: Tensor::from_vec(offset, (n, 1), device)?,
    })
}

fn loss(model: &Model, batch: &Batch) -> candle_core::Result<Tensor> {
    let log_mult = model.residual(&batch.features)?;
    let mu = batch.offset.broadcast_mul(&log_mult.exp()?)?;
    let negative_log_likelihood = (&mu - &batch.expected.broadcast_mul(&log_mult)?)?;
    let penalty = log_mult
        .sqr()?
        .sum_all()?
        .affine(0.03 / batch.expected.dims()[0] as f64, 0.0)?;
    negative_log_likelihood.mean_all()? + penalty
}

fn score(rows: &[&Row], log_multipliers: Option<&[f32]>) -> Result<Scores, Box<dyn Error>> {
    let mut total_loss = 0.0;
    let mut total_brier = 0.0;
    let mut expected_shots = 0.0;
    let mut actual_shots = 0.0;
    let mut matches = BTreeSet::new();
    for (i, row) in rows.iter().enumerate() {
        let multiplier = log_multipliers.map_or(0.0, |p| p[i] as f64);
        let mean = (row.prior_rate * row.minutes / 90.0 * multiplier.exp()).max(1e-9);
        let shots = f64::from(row.shots);
        let ln_factorial: f64 = (1..=row.shots).map(|k| f64::from(k).ln()).sum();
        total_loss += mean - shots * mean.ln() + ln_factorial;
        let occurrence = -(-mean).exp_m1();
        total_brier += (occurrence - if row.shots > 0 { 1.0 } else { 0.0 }).powi(2);
        expected_shots += mean;
        actual_shots += shots;
        matches.insert(row.match_id);
    }
    let n = rows.len() as f64;
    Ok(Scores {
        examples: rows.len(),
        matches: matches.len(),
        poisson_log_loss: total_loss / n,
        shot_occurrence_brier: total_brier / n,
        mean_expected_shots: expected_shots / n,
        mean_observed_shots: actual_shots / n,
    })
}

fn predictions(model: &Model, data: &Batch) -> candle_core::Result<Vec<f32>> {
    model
        .residual(&data.features)?
        .flatten_all()?
        .to_vec1::<f32>()
}

fn export_predictions(rows: &[&Row], log_multipliers: &[f32], output: &mut Vec<PredictionRecord>) {
    for (row, residual) in rows.iter().zip(log_multipliers.iter()) {
        let baseline = row.prior_rate * row.minutes / 90.0;
        output.push(PredictionRecord {
            split: row.split.clone(),
            date: row.date.clone(),
            match_id: row.match_id,
            player_id: row.player_id,
            shots: row.shots,
            actual_minutes: row.minutes,
            baseline_expected_shots: baseline,
            candle_expected_shots: baseline * f64::from(*residual).exp(),
        });
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let input = args
        .first()
        .ok_or("usage: footy-candle-mls-shots input.jsonl output-dir")?;
    let output = args
        .get(1)
        .ok_or("usage: footy-candle-mls-shots input.jsonl output-dir")?;
    let rows = read_rows(input)?;
    let select = |split: &str| -> Vec<&Row> { rows.iter().filter(|r| r.split == split).collect() };
    let train = select("train");
    let validation = select("validation");
    let test = select("test");
    let baseline_val = score(&validation, None)?;
    let baseline_test = score(&test, None)?;
    let device = Device::Cpu;
    let train_batch = batch(&train, &device)?;
    let val_batch = batch(&validation, &device)?;
    let test_batch = batch(&test, &device)?;
    let model = Model::new(train[0].features.len(), 16, &device)?;
    let mut optimizer = AdamW::new(
        model.vars(),
        ParamsAdamW {
            lr: 0.003,
            weight_decay: 0.05,
            ..Default::default()
        },
    )?;

    // Select epoch exclusively using earlier validation; test labels aren't read for tuning.
    let mut best_loss = f64::INFINITY;
    let mut best_weights = model.weights()?;
    let mut best_epoch = 0;
    let mut stalled = 0;
    for epoch in 1..=160 {
        optimizer.backward_step(&loss(&model, &train_batch)?)?;
        let val_pred = predictions(&model, &val_batch)?;
        let val_score = score(&validation, Some(&val_pred))?;
        if val_score.poisson_log_loss < best_loss - 1e-6 {
            best_loss = val_score.poisson_log_loss;
            best_weights = model.weights()?;
            best_epoch = epoch;
            stalled = 0;
        } else {
            stalled += 1;
            if stalled >= 15 {
                break;
            }
        }
    }

    // Restore best validation weights before final untouched test evaluation.
    model.w1.set(&Tensor::from_vec(
        best_weights.w1.clone(),
        (model.input, model.hidden),
        &device,
    )?)?;
    model.b1.set(&Tensor::from_vec(
        best_weights.b1.clone(),
        (1, model.hidden),
        &device,
    )?)?;
    model.w2.set(&Tensor::from_vec(
        best_weights.w2.clone(),
        (model.hidden, 1),
        &device,
    )?)?;
    model
        .b2
        .set(&Tensor::from_vec(best_weights.b2.clone(), (1, 1), &device)?)?;
    let val_pred = predictions(&model, &val_batch)?;
    let test_pred = predictions(&model, &test_batch)?;
    let ml_val = score(&validation, Some(&val_pred))?;
    let ml_test = score(&test, Some(&test_pred))?;
    fs::create_dir_all(output)?;
    fs::write(
        format!("{output}/weights.json"),
        serde_json::to_vec_pretty(&best_weights)?,
    )?;
    let mut per_player = Vec::new();
    export_predictions(&validation, &val_pred, &mut per_player);
    export_predictions(&test, &test_pred, &mut per_player);
    fs::write(
        format!("{output}/predictions.json"),
        serde_json::to_vec_pretty(&per_player)?,
    )?;
    let report = Report {
        experiment: "fm26_mls_candle_player_shots_v1",
        training_examples: train.len(),
        validation_examples: validation.len(),
        test_examples: test.len(),
        best_epoch,
        selected_by: "minimum validation Poisson log loss, early stop patience 15",
        hyperparameters: serde_json::json!({
            "framework": "Candle 0.11.0 CPU",
            "hidden": 16, "learning_rate": 0.003, "adamw_decay": 0.05,
            "log_rate_regularization": 0.03, "max_epochs": 160,
            "log_residual_bound": 1.2, "train_batch": "full-batch",
            "initialization": "deterministic sin", "seed": null
        }),
        split_windows: serde_json::json!({
            "train": ["2026-04-01", "2026-06-01"],
            "validation": ["2026-06-01", "2026-08-01"],
            "test": ["2026-08-01", "2026-09-01"]
        }),
        baseline: serde_json::json!({"validation": baseline_val, "test": baseline_test}),
        candle: serde_json::json!({"validation": ml_val, "test": ml_test}),
        deltas_candidate_minus_baseline: serde_json::json!({
            "validation_poisson_log_loss": ml_val.poisson_log_loss - baseline_val.poisson_log_loss,
            "test_poisson_log_loss": ml_test.poisson_log_loss - baseline_test.poisson_log_loss,
            "validation_occurrence_brier": ml_val.shot_occurrence_brier - baseline_val.shot_occurrence_brier,
            "test_occurrence_brier": ml_test.shot_occurrence_brier - baseline_test.shot_occurrence_brier
        }),
        promotion: "RESEARCH_ONLY_NO_AUTOMATIC_SIMULATOR_CHANGE",
        caveats: &[
            "FM26 March 2026 snapshot: only fixtures in April-August 2026 used as labels",
            "FM attributes explicitly attr_source=db; name+DOB exact join, no fuzzy substitutions",
            "Shots conditioned on observed minutes; not a fully pre-match minutes forecast",
            "Few matches and repeated players; held-out estimates have large uncertainty",
            "No direct WDL, exact-score, or within-match engine OOS evidence",
            "Purely diagnostic: no path engine parameter has been promoted",
        ],
    };
    fs::write(
        format!("{output}/report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
