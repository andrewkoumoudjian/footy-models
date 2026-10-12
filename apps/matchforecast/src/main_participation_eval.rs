#![forbid(unsafe_code)]
//! Chronological, source-stratified participation evaluation on canonical
//! pre-match rows, independent of the stochastic match engine.
use footy_forecast::{
    NextMatchParticipationModel, PlayerParticipationFeatures,
    PlayerParticipationPrediction, PlayerParticipationTrainingExample,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

#[derive(Clone)]
struct Example {
    source: String,
    date: String,
    match_id: String,
    player_id: String,
    features: PlayerParticipationFeatures,
    target: PlayerParticipationTrainingExample,
}

fn parse_records(path: &str) -> Result<Vec<Example>, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = content.lines();
    let header: Vec<&str> = lines.next().ok_or("empty input TSV")?.split('\t').collect();
    let index = |key: &str| -> Result<usize, String> {
        header.iter().position(|col| *col == key).ok_or_else(|| format!("missing {key}"))
    };
    let keys = [
        "source", "league", "date", "match_id", "player_id", "club_fixtures",
        "prior_listings", "prior_appearances", "prior_starts", "prior_minutes",
        "prior_start_minutes", "days_since_last_appearance", "days_since_last_listed",
        "appeared", "started", "minutes", "latest_history_date",
    ];
    let mut columns = BTreeMap::new();
    for key in keys { columns.insert(key, index(key)?); }
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (number, line) in lines.enumerate() {
        if line.trim().is_empty() { continue; }
        let cells: Vec<_> = line.split('\t').collect();
        let get = |key: &str| -> Result<&str, String> {
            cells.get(columns[key]).copied().ok_or_else(|| format!("line {} missing {key}", number + 2))
        };
        let value = |key: &str| -> Result<f64, String> {
            let raw = get(key)?;
            let v: f64 = raw.parse().map_err(|_| format!("line {} invalid {key}={raw}", number + 2))?;
            if !v.is_finite() { return Err(format!("nonfinite {key}")); }
            Ok(v)
        };
        let source = get("source")?.to_owned();
        let league = get("league")?.to_owned();
        let date = get("date")?.to_owned();
        let match_id = get("match_id")?.to_owned();
        let player_id = get("player_id")?.to_owned();
        if get("latest_history_date")? >= date.as_str() {
            return Err(format!("lookahead in {source}:{match_id}:{player_id}"));
        }
        if !seen.insert((source.clone(), match_id.clone(), player_id.clone())) {
            return Err(format!("duplicate candidate {source}:{match_id}:{player_id}"));
        }
        let appeared = value("appeared")?;
        let started = value("started")?;
        if !matches!(appeared, 0.0 | 1.0) || !matches!(started, 0.0 | 1.0) {
            return Err(format!("bad binary targets in {source}:{match_id}"));
        }
        let features = PlayerParticipationFeatures {
            club_fixtures: value("club_fixtures")?,
            prior_listings: value("prior_listings")?,
            prior_appearances: value("prior_appearances")?,
            prior_starts: value("prior_starts")?,
            prior_minutes: value("prior_minutes")?,
            prior_start_minutes: value("prior_start_minutes")?,
            days_since_last_appearance: value("days_since_last_appearance")?,
            days_since_last_listed: value("days_since_last_listed")?,
        };
        let target = PlayerParticipationTrainingExample {
            features, appeared: appeared > 0.0, started: started > 0.0,
            minutes: value("minutes")?,
        };
        if target.minutes < 0.0 || target.started && (!target.appeared || target.minutes <= 0.0) {
            return Err(format!("invalid labels in {source}:{match_id}"));
        }
        result.push(Example {
            source: format!("{source}/{league}"), date, match_id, player_id, features, target,
        });
    }
    if result.is_empty() { return Err("no participation training examples".into()); }
    result.sort_by(|a, b| (&a.source, &a.date, &a.match_id, &a.player_id)
                   .cmp(&(&b.source, &b.date, &b.match_id, &b.player_id)));
    Ok(result)
}

fn empirical_prior(features: PlayerParticipationFeatures) -> PlayerParticipationPrediction {
    // Exactly causal smoothing, no outcomes from the target match. Keeps the
    // comparison reproducible, unlike a tuned or hindsight constant baseline.
    let fixtures = features.club_fixtures;
    let start_probability = ((features.prior_starts + 1.0) / (fixtures + 2.0)).clamp(0.01, 0.99);
    let appearance_probability = ((features.prior_appearances + 1.5) / (fixtures + 2.0))
        .max(start_probability).clamp(0.01, 0.995);
    let expected_minutes_if_start = ((features.prior_start_minutes + 76.0 * 2.0)
        / (features.prior_starts + 2.0)).clamp(45.0, 90.0);
    PlayerParticipationPrediction {
        start_probability, appearance_probability, expected_minutes_if_start,
        minutes_sigma_if_start: 15.0,
    }
}

#[derive(Default, Clone)]
struct Scores {
    players: usize,
    starters: usize,
    appearances: usize,
    start_brier: f64,
    appearance_brier: f64,
    start_log_loss: f64,
    appearance_log_loss: f64,
    starter_minutes_absolute_error: f64,
}

impl Scores {
    fn add(&mut self, label: &PlayerParticipationTrainingExample, prediction: PlayerParticipationPrediction) {
        let squared = |p: f64, y: bool| (p - f64::from(y)).powi(2);
        let log_loss = |p: f64, y: bool| {
            let p = p.clamp(1e-9, 1.0 - 1e-9);
            if y { -p.ln() } else { -(1.0 - p).ln() }
        };
        self.players += 1;
        self.starters += usize::from(label.started);
        self.appearances += usize::from(label.appeared);
        self.start_brier += squared(prediction.start_probability, label.started);
        self.appearance_brier += squared(prediction.appearance_probability, label.appeared);
        self.start_log_loss += log_loss(prediction.start_probability, label.started);
        self.appearance_log_loss += log_loss(prediction.appearance_probability, label.appeared);
        if label.started {
            self.starter_minutes_absolute_error +=
                (prediction.expected_minutes_if_start - label.minutes).abs();
        }
    }
    fn merge(&mut self, other: &Self) {
        self.players += other.players;
        self.starters += other.starters;
        self.appearances += other.appearances;
        self.start_brier += other.start_brier;
        self.appearance_brier += other.appearance_brier;
        self.start_log_loss += other.start_log_loss;
        self.appearance_log_loss += other.appearance_log_loss;
        self.starter_minutes_absolute_error += other.starter_minutes_absolute_error;
    }
    fn output(&self) -> Value {
        let n = self.players.max(1) as f64;
        json!({
            "candidate_rows": self.players, "actual_starts": self.starters,
            "actual_appearances": self.appearances,
            "start_brier": self.start_brier / n,
            "appearance_brier": self.appearance_brier / n,
            "start_log_loss": self.start_log_loss / n,
            "appearance_log_loss": self.appearance_log_loss / n,
            "conditional_starter_minutes_mae": self.starter_minutes_absolute_error / self.starters.max(1) as f64,
        })
    }
}

fn score_group(rows: &[&Example], model: &NextMatchParticipationModel) -> Result<Value, String> {
    let mut empirical = Scores::default();
    let mut fitted = Scores::default();
    let mut day_groups: BTreeMap<&str, (Scores, Scores)> = BTreeMap::new();
    for row in rows {
        let prediction = model.predict(row.features).map_err(|error| format!("model predict: {error:?}"))?;
        if prediction.start_probability > prediction.appearance_probability + 1e-9 {
            return Err("impossible predicted appearance < start".into());
        }
        let prior = empirical_prior(row.features);
        empirical.add(&row.target, prior);
        fitted.add(&row.target, prediction);
        let group = day_groups.entry(&row.date).or_default();
        group.0.add(&row.target, prior);
        group.1.add(&row.target, prediction);
    }
    // Matchday-cluster paired bootstrap: resample whole matchdays, not the
    // strongly dependent eleven player rows within each match.
    let groups: Vec<_> = day_groups.into_values().collect();
    let mut differences = Vec::new();
    let mut rng = footy_forecast::Rng64::new(20261011);
    for _ in 0..1000 {
        let mut old = Scores::default();
        let mut new = Scores::default();
        for _ in 0..groups.len() {
            let index = ((rng.uniform() * groups.len() as f64) as usize).min(groups.len() - 1);
            old.merge(&groups[index].0);
            new.merge(&groups[index].1);
        }
        differences.push((new.start_brier - old.start_brier) / new.players.max(1) as f64);
    }
    differences.sort_by(f64::total_cmp);
    Ok(json!({
        "independent_matchdays": groups.len(),
        "baseline": empirical.output(), "model": fitted.output(),
        "start_brier_model_minus_prior": (fitted.start_brier - empirical.start_brier) / fitted.players.max(1) as f64,
        "start_brier_difference_95pc_matchday_bootstrap": [differences[25], differences[974]],
        "beats_prior_significantly": differences[974] < 0.0,
    }))
}

fn evaluate(records: &[Example]) -> Result<Value, String> {
    let mut report = BTreeMap::<String, Value>::new();
    let mut by_source: BTreeMap<&str, Vec<&Example>> = BTreeMap::new();
    for example in records { by_source.entry(&example.source).or_default().push(example); }
    for (source, rows) in by_source {
        let mut dates: Vec<&str> = rows.iter().map(|r| r.date.as_str()).collect();
        dates.sort(); dates.dedup();
        if dates.len() < 12 { return Err(format!("{source}: only {} independent days", dates.len())); }
        let first_validation = dates[dates.len() * 3 / 5];
        let first_test = dates[dates.len() * 4 / 5];
        if first_validation >= first_test { return Err("collapsed chronological split".into()); }
        let fit_rows: Vec<_> = rows.iter().filter(|r| r.date.as_str() < first_validation).collect();
        let validation: Vec<_> = rows.iter().filter(|r| r.date.as_str() >= first_validation && r.date.as_str() < first_test)
            .copied().collect();
        let test: Vec<_> = rows.iter().filter(|r| r.date.as_str() >= first_test).copied().collect();
        if fit_rows.is_empty() || validation.is_empty() || test.is_empty() {
            return Err(format!("{source}: missing one chronological split"));
        }
        let examples: Vec<_> = fit_rows.iter().map(|row| row.target).collect();
        let model = NextMatchParticipationModel::fit(&examples, 2.0, 8.0)
            .map_err(|err| format!("{source}: participation fit failed: {err:?}"))?;
        report.insert(source.to_string(), json!({
            "fit_rows": fit_rows.len(),
            "fit_last_day": fit_rows.last().map(|r| r.date.as_str()),
            "validation_from": first_validation,
            "test_from": first_test,
            "no_lookahead": fit_rows.iter().all(|r| r.date.as_str() < first_validation),
            "validation": score_group(&validation, &model)?,
            "untouched_test": score_group(&test, &model)?,
        }));
    }
    Ok(json!({
        "schema_version": 1,
        "study": "one fixed regularized Rust participation model versus causal empirical Bayes priors",
        "model": "NextMatchParticipationModel::fit(logistic_l2=2, minutes_ridge=8)",
        "selection_policy": "research_only; neither validation nor inspected final test promotes live parameters",
        "splitting": "per source, distinct dates: first 60% fit, next 20% validation, last 20% test",
        "bootstrap": "1000 fixed-seed resamples of whole source matchdays",
        "sources": report,
    }))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 4 || args[0] != "--input" || args[2] != "--out" {
        return Err("usage: footy-participation-eval --input path/to/participation.tsv --out report.json".into());
    }
    let rows = parse_records(&args[1])?;
    let report = evaluate(&rows)?;
    let path = std::path::Path::new(&args[3]);
    if path.exists() { return Err("refusing to overwrite an earlier participation evaluation".into()); }
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    fs::write(path, serde_json::to_string_pretty(&report).map_err(|e| e.to_string())? + "\n")
        .map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
    Ok(())
}

fn main() {
    if let Err(error) = run() { eprintln!("participation evaluation: {error}"); std::process::exit(1); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empirical_prior_is_causal_and_structurally_coherent() {
        let features = PlayerParticipationFeatures {
            club_fixtures: 6.0, prior_listings: 5.0, prior_appearances: 3.0,
            prior_starts: 2.0, prior_minutes: 200.0, prior_start_minutes: 155.0,
            days_since_last_appearance: 3.0, days_since_last_listed: 3.0,
        };
        let p = empirical_prior(features);
        assert!(p.start_probability <= p.appearance_probability);
        assert!(p.expected_minutes_if_start > 45.0);
    }
}
