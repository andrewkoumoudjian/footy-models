#![forbid(unsafe_code)]
//! Standalone research CLI for the mathematical shot-to-goal match engine.
use footy_match::{TeamGoalModel, TeamGoalsObservation};
use footy_matchforecast::math_engine::{simulate, Candidate, MatchParameters};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

fn required<'a>(row: &'a Value, key: &str) -> Result<&'a Value, String> {
    row.get(key).ok_or_else(|| format!("missing field {key}"))
}
fn string<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    required(row, key)?
        .as_str()
        .ok_or_else(|| format!("{key} is not a string"))
}
fn number(row: &Value, key: &str) -> Result<f64, String> {
    let value = required(row, key)?
        .as_f64()
        .ok_or_else(|| format!("{key} is not numeric"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{key} is not finite"))
    }
}
fn integer(row: &Value, key: &str) -> Result<i32, String> {
    let value = required(row, key)?
        .as_i64()
        .ok_or_else(|| format!("{key} is not an integer"))?;
    i32::try_from(value).map_err(|_| format!("{key} outside day range"))
}
fn arr<'a>(row: &'a Value, key: &str) -> Result<&'a [Value], String> {
    required(row, key)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{key} not an array"))
}
fn opt_numeric(row: &Value, key: &str) -> Result<Option<f64>, String> {
    if required(row, key)?.is_null() {
        Ok(None)
    } else {
        number(row, key).map(Some)
    }
}
fn candidates(v: &[Value]) -> Result<Vec<Candidate>, String> {
    v.iter()
        .map(|p| {
            Ok(Candidate {
                id: required(p, "id")?.as_u64().ok_or("invalid player id")?,
                club: string(p, "club")?.to_owned(),
                minutes_prior: number(p, "minutes_prior")?,
                shots_per90: number(p, "shots_per90")?,
                xg_per90: number(p, "xg_per90")?,
                is_goalkeeper: required(p, "is_goalkeeper")?
                    .as_bool()
                    .ok_or("invalid goalkeeper flag")?,
                fm_source: string(p, "fm_source")?.to_owned(),
                fm_finishing: opt_numeric(p, "fm_finishing")?,
            })
        })
        .collect()
}

#[derive(Clone)]
struct ObservedMatch {
    date: i32,
    home: String,
    away: String,
    hg: u8,
    ag: u8,
    hs: Option<f64>,
    as_: Option<f64>,
}

fn history(rows: &[Value], cutoff: i32) -> Result<Vec<ObservedMatch>, String> {
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        let day = integer(row, "day")?;
        if day >= cutoff {
            return Err("historical source contains a future/same-fixture date".into());
        }
        let hg = number(row, "home_goals")?;
        let ag = number(row, "away_goals")?;
        if hg.fract() != 0.0 || ag.fract() != 0.0 || hg > 255.0 || ag > 255.0 {
            return Err("invalid football scores".into());
        }
        output.push(ObservedMatch {
            date: day,
            home: string(row, "home")?.to_owned(),
            away: string(row, "away")?.to_owned(),
            hg: hg as u8,
            ag: ag as u8,
            hs: opt_numeric(row, "home_shots")?,
            as_: opt_numeric(row, "away_shots")?,
        });
    }
    Ok(output)
}

/// Gamma-Poisson posterior means for goals-independent shot intensities.
/// Symmetric home attack/opponent-away defence shrink toward historical league
/// averages with 12 pseudo-fixtures apiece. No target-match stats are consulted.
fn shot_anchor(matches: &[ObservedMatch], home: &str, away: &str) -> [f64; 2] {
    let mut league = [(0.0, 0.0); 2];
    for m in matches {
        if let Some(s) = m.hs {
            league[0].0 += s;
            league[0].1 += 1.0;
        }
        if let Some(s) = m.as_ {
            league[1].0 += s;
            league[1].1 += 1.0;
        }
    }
    let global = [
        if league[0].1 > 0.0 {
            league[0].0 / league[0].1
        } else {
            13.0
        },
        if league[1].1 > 0.0 {
            league[1].0 / league[1].1
        } else {
            11.0
        },
    ];
    let mut sums = [(0.0, 0.0); 4];
    for m in matches {
        if m.home == home {
            if let Some(s) = m.hs {
                sums[0].0 += s;
                sums[0].1 += 1.0;
            }
        }
        if m.away == away {
            if let Some(s) = m.hs {
                sums[1].0 += s;
                sums[1].1 += 1.0;
            }
        }
        if m.away == away {
            if let Some(s) = m.as_ {
                sums[2].0 += s;
                sums[2].1 += 1.0;
            }
        }
        if m.home == home {
            if let Some(s) = m.as_ {
                sums[3].0 += s;
                sums[3].1 += 1.0;
            }
        }
    }
    let posterior = |idx: usize, prior: f64| (sums[idx].0 + 12.0 * prior) / (sums[idx].1 + 12.0);
    [
        (0.5 * posterior(0, global[0]) + 0.5 * posterior(1, global[0])).max(1.0),
        (0.5 * posterior(2, global[1]) + 0.5 * posterior(3, global[1])).max(1.0),
    ]
}

fn args() -> Result<(String, String, MatchParameters), String> {
    let mut argv = std::env::args().skip(1);
    let input=argv.next().ok_or("usage: footy-matchforecast-math INPUT.json OUTPUT.json [--paths N] [--seed N] [--red-per90 P] [--score-effect B] [--man-advantage B] [--fm-effect B]")?;
    let output = argv.next().ok_or("missing output JSON path")?;
    let mut p = MatchParameters::default();
    while let Some(flag) = argv.next() {
        let value = argv
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--paths" => p.paths = value.parse().map_err(|_| "invalid --paths")?,
            "--seed" => p.seed = value.parse().map_err(|_| "invalid --seed")?,
            "--red-per90" => {
                p.red_card_probability_per90 = value.parse().map_err(|_| "invalid --red-per90")?
            }
            "--score-effect" => {
                p.trailing_shot_log_effect = value.parse().map_err(|_| "invalid --score-effect")?
            }
            "--man-advantage" => {
                p.numerical_advantage_log_effect =
                    value.parse().map_err(|_| "invalid --man-advantage")?
            }
            "--fm-effect" => {
                p.fm_finishing_log_effect = value.parse().map_err(|_| "invalid --fm-effect")?
            }
            _ => return Err(format!("unknown argument {flag}")),
        }
    }
    Ok((input, output, p))
}

fn run() -> Result<(), String> {
    let (input, output, params) = args()?;
    let data: Value = serde_json::from_slice(&fs::read(&input).map_err(|e| e.to_string())?)
        .map_err(|e| format!("parse {input}: {e}"))?;
    if string(&data, "schema")? != "math_shot_goal_v1" {
        return Err("unsupported schema".into());
    }
    let fixture_rows = arr(&data, "fixtures")?;
    if fixture_rows.is_empty() {
        return Err("no fixtures".into());
    }
    let first_kickoff = fixture_rows
        .iter()
        .map(|f| integer(f, "day"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .ok_or("no dated fixtures")?;
    let observed = history(arr(&data, "history")?, first_kickoff)?;
    let championship = history(arr(&data, "championship")?, first_kickoff)?;
    if observed.len() < 30 {
        return Err("insufficient historical matches".into());
    }
    let players = candidates(arr(&data, "players")?)?;
    let mut unique = BTreeMap::new();
    for candidate in &players {
        if unique.insert(candidate.id, &candidate.club).is_some() {
            return Err(format!("duplicate player {}", candidate.id));
        }
    }
    let mut forecasts = Vec::new();
    for fixture in fixture_rows {
        let day = integer(fixture, "day")?;
        let home = string(fixture, "home")?;
        let away = string(fixture, "away")?;
        if home == away {
            return Err("invalid home/away identity".into());
        }
        let prior: Vec<ObservedMatch> = observed.iter().filter(|m| m.date < day).cloned().collect();
        let epl: Vec<TeamGoalsObservation> = prior
            .iter()
            .map(|m| TeamGoalsObservation {
                day: m.date,
                home: m.home.clone(),
                away: m.away.clone(),
                home_goals: m.hg,
                away_goals: m.ag,
            })
            .collect();
        let e1: Vec<TeamGoalsObservation> = championship
            .iter()
            .filter(|m| m.date < day)
            .map(|m| TeamGoalsObservation {
                day: m.date,
                home: m.home.clone(),
                away: m.away.clone(),
                home_goals: m.hg,
                away_goals: m.ag,
            })
            .collect();
        let fitted = TeamGoalModel::fit(&epl, &e1, day)
            .ok_or_else(|| format!("cannot fit chronological team model for {home} vs {away}"))?;
        let team_goals = fitted.forecast(home, away);
        let goals = [team_goals.home_goals, team_goals.away_goals];
        let mut shots = shot_anchor(&prior, home, away);
        for side in 0..2 {
            shots[side] = shots[side].max(goals[side] * 2.0);
        }
        let home_pool: Vec<Candidate> =
            players.iter().filter(|p| p.club == home).cloned().collect();
        let away_pool: Vec<Candidate> =
            players.iter().filter(|p| p.club == away).cloned().collect();
        let mut per_fixture = params;
        // Stable independent fixture streams; output doesn't depend on fixture order.
        per_fixture.seed = params.seed
            ^ ((day as u64).wrapping_mul(0x9e3779b97f4a7c15))
            ^ home
                .bytes()
                .chain(away.bytes())
                .fold(0_u64, |h, b| h.wrapping_mul(131).wrapping_add(u64::from(b)));
        let result = simulate(&home_pool, &away_pool, goals, shots, per_fixture)?;
        let scorer_rows: Vec<Value> = result
            .players
            .iter()
            .map(|(id, counts)| {
                json!({
                    "id":id,"expected_shots":counts.shots as f64/result.paths as f64,
                    "expected_goals":counts.goals as f64/result.paths as f64,
                    "p_start":counts.starts as f64/result.paths as f64,
                })
            })
            .collect();
        let score_rows: Vec<Value> = result
            .scorelines
            .iter()
            .map(|((h, a), n)| {
                json!({
                    "home_goals":h,"away_goals":a,"probability":*n as f64/result.paths as f64,
                })
            })
            .collect();
        forecasts.push(json!({
            "date":string(fixture,"date")?,"home":home,"away":away,
            "fit_matches":prior.len(),"fit_championship_matches":e1.len(),
            "team_goal_anchor":goals,"team_shot_anchor":shots,
            "home_win":result.home_win,"draw":result.draw,"away_win":result.away_win,
            "expected_home_goals":result.expected_home_goals,"expected_away_goals":result.expected_away_goals,
            "expected_home_shots":result.expected_home_shots,"expected_away_shots":result.expected_away_shots,
            "mean_red_cards":result.mean_red_cards,
            "home_window_shots":result.home_window_shots,"away_window_shots":result.away_window_shots,
            "home_window_goals":result.home_window_goals,"away_window_goals":result.away_window_goals,
            "scorelines":score_rows,"player_forecasts":scorer_rows,
            "paths":result.paths,"fixture_seed":per_fixture.seed,
        }));
    }
    let report = json!({
        "status":"research_only_unvalidated",
        "schema":"math_shot_goal_v1",
        "as_of":string(&data,"as_of")?,
        "fm_effective":string(&data,"fm_effective")?,
        "data_sources":required(&data,"sources")?,
        "source_counts":required(&data,"fm_source_counts")?,
        "paths":params.paths,"seed":params.seed,
        "parameters":{
            "red_probability_per90":params.red_card_probability_per90,
            "trailing_shot_log_effect":params.trailing_shot_log_effect,
            "numerical_advantage_log_effect":params.numerical_advantage_log_effect,
            "fm_finishing_log_effect":params.fm_finishing_log_effect,
        },
        "limitations":[
            "no completed substitutions or injuries; red cards optional, not independently outcome-calibrated",
            "source player shot/xG rates are FM-derived research priors, not observed current season event rates",
            "role-neutral ten-outfield exact-cardinality sample is not a positionally calibrated XI",
            "zero tactical/FM attribute effects by default pending new independent OOS evidence",
            "all probabilities are unvalidated simulation estimates; not betting odds",
        ],
        "fixtures":forecasts,
    });
    let dest = Path::new(&output);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        return Err(format!(
            "refusing to overwrite existing research snapshot: {output}"
        ));
    }
    fs::write(
        dest,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "saved {} strictly pre-match research forecasts to {output}",
        forecasts.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("math-match: {error}");
        std::process::exit(1);
    }
}
