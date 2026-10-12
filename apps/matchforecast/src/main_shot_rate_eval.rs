#![forbid(unsafe_code)]
//! Source-separated as-of player-shot forecast evaluation. This is an
//! upstream challenger, and does not alter match simulation probabilities.
use footy_forecast::{ExposurePoissonShotModel, ObservedShotCount, Rng64, ShotRateFeatures, ShotRole};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

#[derive(Debug)]
struct Row {
    source: String,
    day: String,
    features: ShotRateFeatures,
    club_fixtures: f64,
    prior_starts: f64,
    prior_appearances: f64,
    prior_start_minutes: f64,
    prior_sub_appearances: f64,
    prior_sub_minutes: f64,
    appeared: bool,
    minutes: f64,
    shots: Option<u32>,
}

fn role(text: &str) -> ShotRole {
    let lower = text.to_ascii_lowercase();
    let p = lower.as_str();
    if p == "g" || p.contains("goalkeeper") || p == "keeper" { ShotRole::Goalkeeper }
    else if p == "d" || p.contains("back") || p.contains("defen") { ShotRole::Defender }
    else if p == "f" || p.contains("forward") || p.contains("striker") || p.contains("wing") && !p.contains("back") { ShotRole::Forward }
    else if p == "m" || p.contains("midfield") { ShotRole::Midfielder }
    else { ShotRole::Unknown }
}

fn read_rows(path: &str) -> Result<Vec<Row>, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = content.lines();
    let keys: Vec<&str> = lines.next().ok_or("empty panel")?.split('\t').collect();
    let mut columns = BTreeMap::new();
    for (index, key) in keys.iter().enumerate() { columns.insert(*key, index); }
    for key in ["source", "league", "date", "match_id", "player_id", "position", "latest_history_date",
        "prior_observed_shots", "prior_shot_exposure_minutes", "club_fixtures", "prior_starts",
        "prior_appearances", "prior_start_minutes", "prior_sub_appearances", "prior_sub_minutes",
        "appeared", "started", "minutes", "shots", "shot_observed", "event_exposure_minutes"] {
        if !columns.contains_key(key) { return Err(format!("missing required column {key}")); }
    }
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (number, line) in lines.enumerate() {
        if line.is_empty() { continue; }
        let cells: Vec<_> = line.split('\t').collect();
        let get = |key: &str| -> Result<&str, String> {
            cells.get(columns[key]).copied().ok_or_else(|| format!("line {} missing {key}", number + 2))
        };
        let number_value = |key: &str| -> Result<f64, String> {
            let x = get(key)?.parse::<f64>().map_err(|_| format!("line {} invalid {key}", number + 2))?;
            if !x.is_finite() || x < 0.0 { return Err(format!("line {} invalid nonnegative {key}", number + 2)); }
            Ok(x)
        };
        let binary = |key: &str| -> Result<bool, String> {
            match get(key)? { "0" => Ok(false), "1" => Ok(true), _ => Err(format!("invalid {key} binary label")) }
        };
        let source = format!("{}/{}", get("source")?, get("league")?);
        let day = get("date")?.to_owned();
        if get("latest_history_date")? >= day.as_str() { return Err("lookahead in panel".into()); }
        let match_id = get("match_id")?.to_owned();
        let person = get("player_id")?.to_owned();
        if !seen.insert((source.clone(), match_id.clone(), person.clone())) {
            return Err(format!("duplicate candidate {match_id}:{person}"));
        }
        let started = binary("started")?;
        let appeared = binary("appeared")?;
        let shot_observed = binary("shot_observed")?;
        let minutes = number_value("minutes")?;
        if started && !appeared || appeared && minutes <= 0.0 || !appeared && minutes > 0.0 {
            return Err("incoherent participation label".into());
        }
        let shots = if shot_observed {
            let raw = get("shots")?;
            let shots = raw.parse::<u32>().map_err(|_| format!("invalid observed shot count {raw}"))?;
            if !appeared || get("event_exposure_minutes")?.parse::<f64>().ok() != Some(minutes) {
                return Err("shot observed without exactly matching minutes".into());
            }
            Some(shots)
        } else {
            if !get("shots")?.is_empty() || !get("event_exposure_minutes")?.is_empty() {
                return Err("unobserved shot count contains a proxy".into());
            }
            None
        };
        let features = ShotRateFeatures {
            role: role(get("position")?),
            prior_shots: number_value("prior_observed_shots")?,
            prior_minutes: number_value("prior_shot_exposure_minutes")?,
        };
        if !features.validate() { return Err("invalid as-of shot exposure".into()); }
        result.push(Row {
            source, day, features,
            club_fixtures: number_value("club_fixtures")?,
            prior_starts: number_value("prior_starts")?,
            prior_appearances: number_value("prior_appearances")?,
            prior_start_minutes: number_value("prior_start_minutes")?,
            prior_sub_appearances: number_value("prior_sub_appearances")?,
            prior_sub_minutes: number_value("prior_sub_minutes")?,
            appeared, minutes, shots,
        });
    }
    Ok(result)
}

fn participation(row: &Row) -> (f64, f64, f64, f64) {
    let p_start = ((row.prior_starts + 1.0) / (row.club_fixtures + 2.0)).clamp(0.01, 0.99);
    let p_appear = ((row.prior_appearances + 1.5) / (row.club_fixtures + 2.0))
        .max(p_start).clamp(0.01, 0.995);
    let start_minutes = ((row.prior_start_minutes + 152.0) / (row.prior_starts + 2.0)).clamp(45.0, 90.0);
    let sub_minutes = ((row.prior_sub_minutes + 2.0 * 25.0) / (row.prior_sub_appearances + 2.0)).clamp(5.0, 70.0);
    (p_start, p_appear, start_minutes, sub_minutes)
}

fn poisson_mass(k: u32, mean: f64) -> f64 {
    if mean <= 0.0 { return f64::from(k == 0); }
    let factorial_log: f64 = (2..=k).map(|j| f64::from(j).ln()).sum();
    (f64::from(k) * mean.ln() - mean - factorial_log).exp()
}

#[derive(Default, Clone)]
struct Scores {
    n: usize,
    appeared: usize,
    count_log_loss: f64,
    occurrence_brier: f64,
    count_mae: f64,
    oracle_minutes_count_log_loss: f64,
}

impl Scores {
    fn record(&mut self, row: &Row, rate: f64) {
        // If the player did not appear, full-match shot count is verified zero
        // from the lineup label, not from a missing player-events API cell.
        let Some(k) = (if row.appeared { row.shots } else { Some(0) }) else { return; };
        let (p_start, p_appear, start_minutes, sub_minutes) = participation(row);
        let p_sub = p_appear - p_start;
        let start_lambda = rate * start_minutes / 90.0;
        let sub_lambda = rate * sub_minutes / 90.0;
        let mass = (if k == 0 { 1.0 - p_appear } else { 0.0 })
            + p_start * poisson_mass(k, start_lambda)
            + p_sub * poisson_mass(k, sub_lambda);
        let p_any = p_start * (1.0 - (-start_lambda).exp()) + p_sub * (1.0 - (-sub_lambda).exp());
        let mean = p_start * start_lambda + p_sub * sub_lambda;
        self.n += 1;
        self.appeared += usize::from(row.appeared);
        self.count_log_loss -= mass.max(1e-15).ln();
        self.occurrence_brier += (p_any - f64::from(k > 0)).powi(2);
        self.count_mae += (mean - f64::from(k)).abs();
        if row.appeared {
            self.oracle_minutes_count_log_loss -= poisson_mass(k, rate * row.minutes / 90.0).max(1e-15).ln();
        }
    }
    fn add(&mut self, rhs: &Self) {
        self.n += rhs.n;
        self.appeared += rhs.appeared;
        self.count_log_loss += rhs.count_log_loss;
        self.occurrence_brier += rhs.occurrence_brier;
        self.count_mae += rhs.count_mae;
        self.oracle_minutes_count_log_loss += rhs.oracle_minutes_count_log_loss;
    }
    fn result(&self) -> Value {
        json!({"candidate_rows": self.n, "appearing_with_shot_labels": self.appeared,
            "count_log_loss": self.count_log_loss / self.n.max(1) as f64,
            "any_shot_brier": self.occurrence_brier / self.n.max(1) as f64,
            "shot_count_mae": self.count_mae / self.n.max(1) as f64,
            "conditional_given_actual_minutes_log_loss_diagnostic_only":
                self.oracle_minutes_count_log_loss / self.appeared.max(1) as f64})
    }
}

fn score(rows: &[&Row], model: &ExposurePoissonShotModel) -> Result<Value, String> {
    let mut groups: BTreeMap<&str, [Scores; 3]> = BTreeMap::new();
    let mut totals: [Scores; 3] = std::array::from_fn(|_| Scores::default());
    for row in rows {
        let prior_rate = row.features.empirical_per90().ok_or("invalid posterior rate")?;
        let rates = [row.features.role.prior_per90(), prior_rate,
            model.rate_per90(row.features).ok_or("invalid Poisson model rate")?];
        let g = groups.entry(&row.day).or_insert_with(|| std::array::from_fn(|_| Scores::default()));
        for index in 0..3 {
            g[index].record(row, rates[index]);
            totals[index].record(row, rates[index]);
        }
    }
    let blocks: Vec<_> = groups.into_values().collect();
    let mut rng = Rng64::new(20261011);
    let mut intervals = [Vec::new(), Vec::new()];
    for _ in 0..1000 {
        let mut sums: [Scores; 3] = std::array::from_fn(|_| Scores::default());
        for _ in 0..blocks.len() {
            let index = ((rng.uniform() * blocks.len() as f64) as usize).min(blocks.len() - 1);
            for j in 0..3 { sums[j].add(&blocks[index][j]); }
        }
        let n = sums[0].n.max(1) as f64;
        intervals[0].push((sums[1].count_log_loss - sums[0].count_log_loss) / n);
        intervals[1].push((sums[2].count_log_loss - sums[1].count_log_loss) / n);
    }
    for v in &mut intervals { v.sort_by(f64::total_cmp); }
    Ok(json!({
        "distinct_matchdays": blocks.len(),
        "role_only_reference": totals[0].result(),
        "empirical_gamma_poisson": totals[1].result(),
        "fitted_exposure_offset_poisson": totals[2].result(),
        "eb_minus_role_count_log_loss": (totals[1].count_log_loss - totals[0].count_log_loss) / totals[0].n.max(1) as f64,
        "glm_minus_eb_count_log_loss": (totals[2].count_log_loss - totals[1].count_log_loss) / totals[0].n.max(1) as f64,
        "eb_minus_role_95pc_matchday_bootstrap": [intervals[0][25], intervals[0][974]],
        "glm_minus_eb_95pc_matchday_bootstrap": [intervals[1][25], intervals[1][974]],
        "glm_significantly_beats_eb": intervals[1][974] < 0.0,
    }))
}

fn evaluate(rows: &[Row]) -> Result<Value, String> {
    let mut sources: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for row in rows { sources.entry(&row.source).or_default().push(row); }
    let mut output = BTreeMap::new();
    for (source, records) in sources {
        let mut dates: Vec<_> = records.iter().map(|r| r.day.as_str()).collect();
        dates.sort(); dates.dedup();
        if dates.len() < 12 { return Err(format!("insufficient matchdays in {source}")); }
        let validation_start = dates[dates.len() * 3 / 5];
        let test_start = dates[dates.len() * 4 / 5];
        let training: Vec<_> = records.iter().copied().filter(|r| r.day.as_str() < validation_start
            && r.appeared && r.shots.is_some()).map(|r| ObservedShotCount {
            features: r.features, minutes: r.minutes, shots: r.shots.unwrap_or(0),
        }).collect();
        let validation: Vec<_> = records.iter().copied().filter(|r| r.day.as_str() >= validation_start
            && r.day.as_str() < test_start).collect();
        let test: Vec<_> = records.iter().copied().filter(|r| r.day.as_str() >= test_start).collect();
        let fit = ExposurePoissonShotModel::fit(&training, 10.0)
            .ok_or_else(|| format!("failed to fit exposure-Poisson on {source}"))?;
        output.insert(source.to_owned(), json!({
            "train_observed_appearance_shot_rows": training.len(),
            "validation_start_date": validation_start, "test_start_date": test_start,
            "validation": score(&validation, &fit)?,
            "untouched_test": score(&test, &fit)?,
        }));
    }
    Ok(json!({
        "schema_version": 1, "status": "research_only",
        "rates": "role prior / 5-match Gamma-Poisson posterior / Poisson regression with actual-minute offset on training only",
        "prediction": "three-way zero-start-sub mixture; participation probabilities and expected starter/bench minutes use previous club appearances only",
        "splits": "source-separated chronological distinct dates 60/20/20; no same-day training features",
        "bootstrap": "1000 fixed-seed resamples of entire matchdays",
        "diagnostic_warning": "conditional_given_actual_minutes uses post-match exposure for scoring only and is not a deployable pre-match forecast",
        "release": "no production changes and no parameter selection using inspected final-test results",
        "sources": output,
    }))
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let run = || -> Result<(), String> {
        if args.len() != 4 || args[0] != "--input" || args[2] != "--out" {
            return Err("usage: footy-shot-rate-eval --input participation.tsv --out fresh_report.json".into());
        }
        let path = std::path::Path::new(&args[3]);
        if path.exists() { return Err("refusing to overwrite a previous experiment".into()); }
        let rows = read_rows(&args[1])?;
        let output = evaluate(&rows)?;
        if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        fs::write(path, serde_json::to_string_pretty(&output).map_err(|e| e.to_string())? + "\n")
            .map_err(|e| e.to_string())?;
        println!("{}", serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?);
        Ok(())
    };
    if let Err(error) = run() { eprintln!("shot-rate evaluation: {error}"); std::process::exit(1); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forward_and_goalkeeper_positions_parse() {
        assert_eq!(role("Center Forward"), ShotRole::Forward);
        assert_eq!(role("Right Wing"), ShotRole::Forward);
        assert_eq!(role("Goalkeeper"), ShotRole::Goalkeeper);
        assert_eq!(role("M"), ShotRole::Midfielder);
        assert_eq!(role("Right Wing Back"), ShotRole::Defender);
    }
    #[test]
    fn nonappearance_mass_is_always_nonnegative() {
        let p = 0.35 * poisson_mass(0, 0.3) + 0.2 * poisson_mass(0, 0.1) + 0.45;
        assert!(p > 0.45 && p < 1.0);
    }
}
