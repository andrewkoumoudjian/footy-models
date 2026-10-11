//! Leakage-aware research pilot: train attention on strictly as-of candidate sets.
//! Realized match minutes enter the TARGET exposure, never player-set features.
#[path = "../set_transformer.rs"]
mod set_transformer;

use candle_core::{Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW};
use serde::{Deserialize, Serialize};
use set_transformer::PlayerSetTransformer;
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader};

const MAX_CANDIDATES: usize = 50;
const HIDDEN_WIDTH: usize = 16;

#[derive(Clone, Deserialize)]
struct Candidate {
    player_id: u64,
    features: Vec<f32>,
    asof_shots_per90: f64,
    fm_verified: bool,
    label: Option<Observed>,
}

#[derive(Clone, Deserialize)]
struct Observed {
    shots: u32,
    minutes: f64,
}

#[derive(Clone, Deserialize)]
struct Fixture {
    fixture_id: u64,
    date: String,
    split: String,
    candidates: Vec<Candidate>,
}

struct Batch {
    features: Tensor,
    candidates: Tensor,
    shots: Tensor,
    exposure: Tensor,
    labeled: Tensor,
    n_labeled: usize,
}

#[derive(Serialize)]
struct Prediction {
    split: String,
    date: String,
    match_id: u64,
    player_id: u64,
    shots: u32,
    actual_minutes: f64,
    baseline_expected_shots: f64,
    candle_expected_shots: f64,
}

fn read_fixture_groups(path: &str) -> Result<Vec<Fixture>, Box<dyn Error>> {
    let input = BufReader::new(fs::File::open(path)?);
    let mut fixtures = Vec::new();
    let mut ids = BTreeSet::new();
    for line in input.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let record: Fixture = serde_json::from_str(&line)?;
        if !ids.insert(record.fixture_id)
            || record.candidates.is_empty()
            || record.candidates.len() > MAX_CANDIDATES
            || !["train", "validation", "test"].contains(&record.split.as_str())
        {
            return Err("invalid fixture or candidate limit".into());
        }
        let mut player_ids = BTreeSet::new();
        for candidate in &record.candidates {
            if !player_ids.insert(candidate.player_id)
                || candidate.features.is_empty()
                || !candidate.features.iter().all(|x| x.is_finite())
                || !candidate.asof_shots_per90.is_finite()
                || candidate.asof_shots_per90 <= 0.0
            {
                return Err("invalid or duplicate candidate".into());
            }
            if let Some(obs) = &candidate.label {
                if !obs.minutes.is_finite() || obs.minutes <= 0.0 || obs.minutes > 130.0 {
                    return Err("invalid played minutes".into());
                }
            }
        }
        fixtures.push(record);
    }
    if fixtures.is_empty() {
        return Err("no groups".into());
    }
    let width = fixtures[0].candidates[0].features.len();
    if fixtures
        .iter()
        .flat_map(|f| &f.candidates)
        .any(|c| c.features.len() != width)
    {
        return Err("inconsistent feature widths".into());
    }
    let range = |split: &str| -> Result<(String, String), Box<dyn Error>> {
        let days: Vec<_> = fixtures
            .iter()
            .filter(|f| f.split == split)
            .map(|f| f.date.clone())
            .collect();
        Ok((
            days.iter().min().ok_or("missing split")?.clone(),
            days.iter().max().ok_or("missing split")?.clone(),
        ))
    };
    if !(range("train")?.1 < range("validation")?.0 && range("validation")?.1 < range("test")?.0) {
        return Err("overlapping chronological split dates".into());
    }
    Ok(fixtures)
}

fn batch(groups: &[&Fixture], device: &Device) -> candle_core::Result<Batch> {
    let n = groups.len();
    let width = groups[0].candidates[0].features.len();
    let mut x = vec![0.0f32; n * MAX_CANDIDATES * width];
    let mut candidate_mask = vec![0.0f32; n * MAX_CANDIDATES];
    let mut shot_count = vec![0.0f32; n * MAX_CANDIDATES];
    let mut exposure = vec![0.0f32; n * MAX_CANDIDATES];
    let mut label_mask = vec![0.0f32; n * MAX_CANDIDATES];
    let mut n_labeled = 0;
    for (i, fixture) in groups.iter().enumerate() {
        for (j, candidate) in fixture.candidates.iter().enumerate() {
            let index = i * MAX_CANDIDATES + j;
            x[index * width..(index + 1) * width].copy_from_slice(&candidate.features);
            candidate_mask[index] = 1.0;
            if candidate.fm_verified {
                if let Some(label) = &candidate.label {
                    shot_count[index] = label.shots as f32;
                    exposure[index] = (candidate.asof_shots_per90 * label.minutes / 90.0) as f32;
                    label_mask[index] = 1.0;
                    n_labeled += 1;
                }
            }
        }
    }
    if n_labeled == 0 {
        return Err(candle_core::Error::Msg(
            "no matched supervised labels".into(),
        ));
    }
    Ok(Batch {
        features: Tensor::from_vec(x, (n, MAX_CANDIDATES, width), device)?,
        candidates: Tensor::from_vec(candidate_mask, (n, MAX_CANDIDATES), device)?,
        shots: Tensor::from_vec(shot_count, (n, MAX_CANDIDATES, 1), device)?,
        exposure: Tensor::from_vec(exposure, (n, MAX_CANDIDATES, 1), device)?,
        labeled: Tensor::from_vec(label_mask, (n, MAX_CANDIDATES, 1), device)?,
        n_labeled,
    })
}

fn loss(model: &PlayerSetTransformer, batch: &Batch) -> candle_core::Result<Tensor> {
    let correction = model.forward(&batch.features, &batch.candidates)?;
    let mu = batch.exposure.broadcast_mul(&correction.exp()?)?;
    let neglog_no_constant = (&mu - batch.shots.broadcast_mul(&correction)?)?;
    let masked = neglog_no_constant.broadcast_mul(&batch.labeled)?;
    let prior = correction.sqr()?.broadcast_mul(&batch.labeled)?;
    (masked.sum_all()? + prior.sum_all()?.affine(0.03, 0.0)?)?
        .affine(1.0 / batch.n_labeled as f64, 0.0)
}

fn score(
    fixtures: &[&Fixture],
    model: Option<&PlayerSetTransformer>,
    device: &Device,
) -> Result<(f64, Vec<Prediction>), Box<dyn Error>> {
    let batch = batch(fixtures, device)?;
    let correction = match model {
        Some(model) => model
            .forward(&batch.features, &batch.candidates)?
            .flatten_all()?
            .to_vec1::<f32>()?,
        None => vec![0.0f32; fixtures.len() * MAX_CANDIDATES],
    };
    let mut total = 0.0;
    let mut rows = Vec::new();
    for (i, fixture) in fixtures.iter().enumerate() {
        for (j, c) in fixture.candidates.iter().enumerate() {
            if !c.fm_verified {
                continue;
            }
            let Some(label) = &c.label else { continue };
            let baseline = c.asof_shots_per90 * label.minutes / 90.0;
            let mean = baseline * f64::from(correction[i * MAX_CANDIDATES + j]).exp();
            let ln_factorial: f64 = (1..=label.shots).map(|k| f64::from(k).ln()).sum();
            total += mean - f64::from(label.shots) * mean.ln() + ln_factorial;
            rows.push(Prediction {
                split: fixture.split.clone(),
                date: fixture.date.clone(),
                match_id: fixture.fixture_id,
                player_id: c.player_id,
                shots: label.shots,
                actual_minutes: label.minutes,
                baseline_expected_shots: baseline,
                candle_expected_shots: mean,
            });
        }
    }
    Ok((total / rows.len() as f64, rows))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let data = args
        .first()
        .ok_or("usage: set_attention_pilot data.jsonl output-dir")?;
    let output = args
        .get(1)
        .ok_or("usage: set_attention_pilot data.jsonl output-dir")?;
    let fixtures = read_fixture_groups(data)?;
    let select =
        |split: &str| -> Vec<&Fixture> { fixtures.iter().filter(|f| f.split == split).collect() };
    let train = select("train");
    let validation = select("validation");
    let test = select("test");
    let device = Device::Cpu;
    let train_batch = batch(&train, &device)?;
    let validation_batch = batch(&validation, &device)?;
    let model =
        PlayerSetTransformer::new(train[0].candidates[0].features.len(), HIDDEN_WIDTH, &device)?;
    let mut optimizer = AdamW::new(
        model.parameters(),
        ParamsAdamW {
            lr: 0.001,
            weight_decay: 0.05,
            ..Default::default()
        },
    )?;
    let mut best_loss = f64::INFINITY;
    let mut best_epoch = 0;
    let mut checkpoint = Vec::new();
    let mut no_improvement = 0;
    for epoch in 1..=50 {
        optimizer.backward_step(&loss(&model, &train_batch)?)?;
        let val_loss = score(&validation, Some(&model), &device)?.0;
        if val_loss + 1e-7 < best_loss {
            best_loss = val_loss;
            best_epoch = epoch;
            checkpoint = model
                .parameters()
                .into_iter()
                .map(|v| {
                    Ok::<_, candle_core::Error>((
                        v.flatten_all()?.to_vec1::<f32>()?,
                        v.dims().to_vec(),
                    ))
                })
                .collect::<candle_core::Result<Vec<_>>>()?;
            no_improvement = 0;
        } else {
            no_improvement += 1;
            if no_improvement >= 8 {
                break;
            }
        }
    }
    for (parameter, (values, shape)) in model.parameters().into_iter().zip(checkpoint.iter()) {
        parameter.set(&Tensor::from_vec(values.clone(), shape.clone(), &device)?)?;
    }
    let (val_prior, _) = score(&validation, None, &device)?;
    let (val_attn, mut predictions) = score(&validation, Some(&model), &device)?;
    let (test_prior, _) = score(&test, None, &device)?;
    let (test_attn, test_rows) = score(&test, Some(&model), &device)?;
    predictions.extend(test_rows);
    fs::create_dir_all(output)?;
    fs::write(
        format!("{output}/predictions.json"),
        serde_json::to_vec_pretty(&predictions)?,
    )?;
    let result = serde_json::json!({
        "experiment": "fm26_asof_candidate_attention_v1",
        "selection": "minimum JULY validation Poisson log loss",
        "test_disclosure": "August test was previously inspected for an MLP; exploratory only",
        "architecture": {"one_head": true, "hidden": HIDDEN_WIDTH,
            "position_encoding": false, "candidate_mask": true},
        "counts": {"train_fixtures": train.len(),
            "validation_fixtures": validation.len(), "test_fixtures": test.len(),
            "train_labels": train_batch.n_labeled,
            "validation_labels": validation_batch.n_labeled},
        "best_epoch": best_epoch,
        "validation": {"baseline_poisson_ll": val_prior,
            "attention_poisson_ll": val_attn, "delta": val_attn - val_prior},
        "test": {"baseline_poisson_ll": test_prior,
            "attention_poisson_ll": test_attn, "delta": test_attn - test_prior},
        "gate": "research_only_no_production_promotion",
        "limitations": [
            "As-of candidates inferred from previous matches, not official squads.",
            "True minutes are evaluation exposure, not an input to attention.",
            "Only exact FM name+DOB joined players score supervised events.",
            "Few matched fixtures, same-player/club dependence, exploratory test."
        ]
    });
    fs::write(
        format!("{output}/report.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
