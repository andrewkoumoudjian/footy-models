#![forbid(unsafe_code)]
//! Rolling-origin out-of-sample scoring for the production team-goal model.
//!
//! Reads dated EPL results plus Championship priors from CSV/TSV, refits
//! [`footy_match::TeamGoalModel`] at each monthly cutoff using strictly prior
//! matches, and scores WDL/scoreline proper losses against the
//! home-advantage-only baseline. Thin orchestration: scoring lives in
//! `footy_match::oos`.
use footy_match::{rolling_origin, RollingConfig, RollingReport, TeamMatch};
use serde_json::json;
use std::fs;

const ALIASES: &[(&str, &str)] = &[
    ("Nottingham Forest", "Nott'm Forest"),
    ("West Ham United", "West Ham"),
    ("Wolverhampton Wanderers", "Wolves"),
    ("Leicester City", "Leicester"),
    ("Leeds United", "Leeds"),
    ("Ipswich Town", "Ipswich"),
    ("Tottenham Hotspur", "Tottenham"),
];
/// Proleptic Gregorian ordinal of 1970-01-01 (Python `date(1970,1,1).toordinal()`),
/// the epoch `results.tsv` day offsets count from.
const UNIX_EPOCH_ORDINAL: i32 = 719163;

fn canonical(name: &str) -> String {
    let trimmed = name.trim();
    for (raw, canonical) in ALIASES {
        if trimmed.eq_ignore_ascii_case(raw) {
            return canonical.to_string();
        }
    }
    trimmed.to_owned()
}

/// Days since the unix epoch for a `YYYY-MM-DD` date; std only.
fn unix_day(text: &str) -> Result<i32, String> {
    let parts: Vec<&str> = text.trim().split('-').collect();
    if parts.len() != 3 {
        return Err(format!("invalid date {text}"));
    }
    let year: i64 = parts[0].parse().map_err(|_| format!("invalid date {text}"))?;
    let month: i64 = parts[1].parse().map_err(|_| format!("invalid date {text}"))?;
    let day: i64 = parts[2].parse().map_err(|_| format!("invalid date {text}"))?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("invalid date {text}"));
    }
    let adjusted = if month <= 2 { year - 1 } else { year };
    let era = adjusted.div_euclid(400);
    let yoe = adjusted.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok((era * 146097 + doe - 719468) as i32)
}

fn goals(text: &str, what: &str) -> Result<u8, String> {
    let value: f64 = text.trim().parse().map_err(|_| format!("invalid {what} {text}"))?;
    if value.fract() != 0.0 || value < 0.0 || value > 30.0 {
        return Err(format!("invalid {what} {text}"));
    }
    Ok(value as u8)
}

fn read_asof_csv(path: &str) -> Result<Vec<TeamMatch>, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = content.lines();
    let header: Vec<String> =
        footy_matchforecast::driver::csv_fields::parse_csv_fields(
            lines.next().ok_or("empty results CSV")?,
            path,
            1,
        )
        .ok_or("malformed results CSV header")?;
    let column = |name: &str| {
        header
            .iter()
            .position(|key| key.trim() == name)
            .ok_or_else(|| format!("results CSV lacks {name}"))
    };
    let (date, home, away, hg, ag) = (
        column("date")?,
        column("home")?,
        column("away")?,
        column("home_goals")?,
        column("away_goals")?,
    );
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (number, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<String> = footy_matchforecast::driver::csv_fields::parse_csv_fields(
            line,
            path,
            number + 2,
        )
        .ok_or(format!("malformed results CSV line {}", number + 2))?;
        let get = |index: usize| cells.get(index).map(String::as_str).unwrap_or("").trim();
        let game = TeamMatch {
            day: unix_day(get(date))?,
            home: canonical(get(home)),
            away: canonical(get(away)),
            home_goals: goals(get(hg), "home goals")?,
            away_goals: goals(get(ag), "away goals")?,
        };
        if !game.valid() {
            return Err(format!("line {} invalid fixture identity", number + 2));
        }
        if !seen.insert((game.day, game.home.clone(), game.away.clone())) {
            return Err(format!("line {} duplicate fixture", number + 2));
        }
        out.push(game);
    }
    out.sort_by_key(|game| game.day);
    Ok(out)
}

fn read_championship_tsv(path: &str, epoch_ordinal: i32) -> Result<Vec<TeamMatch>, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = content.lines();
    let header: Vec<&str> = lines.next().ok_or("empty results TSV")?.split('\t').collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|key| key.trim() == name)
            .ok_or_else(|| format!("results TSV lacks {name}"))
    };
    let (day, division, home, away, hg, ag) = (
        column("day")?,
        column("division")?,
        column("home")?,
        column("away")?,
        column("home_goals")?,
        column("away_goals")?,
    );
    let mut out = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        let get = |index: usize| cells.get(index).copied().unwrap_or("").trim();
        if get(division) != "E1" {
            continue;
        }
        let offset: i32 = get(day).parse().map_err(|_| "invalid E1 day")?;
        out.push(TeamMatch {
            day: offset + epoch_ordinal,
            home: canonical(get(home)),
            away: canonical(get(away)),
            home_goals: goals(get(hg), "E1 home goals")?,
            away_goals: goals(get(ag), "E1 away goals")?,
        });
    }
    out.sort_by_key(|game| game.day);
    Ok(out)
}

fn report_json(report: &RollingReport) -> serde_json::Value {
    json!({
        "schema": "team_goals_rolling_origin_oos_v1",
        "status": "research_only",
        "fixtures_scored": report.fixtures,
        "skipped_months_without_fit": report.skipped_new_club_months,
        "matchday_clusters": report.matchday_clusters,
        "wdl_log_loss": {"model": report.wdl_log_loss_model,
                         "home_advantage_only": report.wdl_log_loss_baseline,
                         "delta": report.wdl_log_loss_model - report.wdl_log_loss_baseline},
        "wdl_brier": {"model": report.wdl_brier_model,
                      "home_advantage_only": report.wdl_brier_baseline,
                      "delta": report.wdl_brier_model - report.wdl_brier_baseline},
        "scoreline_log_loss": {"model": report.scoreline_log_loss_model,
                               "home_advantage_only": report.scoreline_log_loss_baseline,
                               "delta": report.scoreline_log_loss_model - report.scoreline_log_loss_baseline},
        "mean_abs_total_goals_error_model": report.mean_abs_goals_error_model,
        "wdl_log_loss_delta_95pc_matchday_bootstrap": report.wdl_delta_95pc,
        "home_win_reliability": report.home_win_reliability.iter().map(|bin| json!({
            "bin": bin.bin, "n_model": bin.n_model,
            "mean_pred_model": bin.mean_pred_model, "mean_obs_model": bin.mean_obs_model,
            "n_baseline": bin.n_baseline,
            "mean_pred_baseline": bin.mean_pred_baseline,
            "mean_obs_baseline": bin.mean_obs_baseline,
        })).collect::<Vec<_>>(),
        "per_season_wdl_log_loss": report.per_season.iter().map(|(season, scores)| json!({
            "season": season, "fixtures": scores.fixtures,
            "model": scores.wdl_log_loss_model,
            "home_advantage_only": scores.wdl_log_loss_baseline,
        })).collect::<Vec<_>>(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run = || -> Result<(), String> {
        if args.len() != 6 || args[0] != "--asof-csv" || args[2] != "--results-tsv" || args[4] != "--out" {
            return Err(
                "usage: footy-team-goals-oos --asof-csv ASOF.csv --results-tsv RESULTS.tsv --out REPORT.json"
                    .into(),
            );
        }
        let matches = read_asof_csv(&args[1])?;
        let championship = read_championship_tsv(&args[3], UNIX_EPOCH_ORDINAL)?;
        if matches.len() < 100 {
            return Err("insufficient dated results for rolling-origin scoring".into());
        }
        let first = matches.iter().map(|game| game.day).min().ok_or("no matches")?;
        let config = RollingConfig {
            // First forecast season: one full season of history is required
            // before the first monthly refit is trusted.
            first_forecast_day: first + 365,
            step_days: 30,
            min_history: 100,
            bootstrap_draws: 2500,
            bootstrap_seed: 20261012,
        };
        let report = rolling_origin(&matches, &championship, config)
            .ok_or("rolling-origin scoring produced no fixtures")?;
        let output = report_json(&report);
        let path = std::path::Path::new(&args[5]);
        if path.exists() {
            return Err("refusing to overwrite a previous experiment".into());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, serde_json::to_string_pretty(&output).map_err(|error| error.to_string())? + "\n")
            .map_err(|error| error.to_string())?;
        println!("{}", serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?);
        Ok(())
    };
    if let Err(error) = run() {
        eprintln!("team-goals-oos: {error}");
        std::process::exit(1);
    }
}
