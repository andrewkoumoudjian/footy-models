#![forbid(unsafe_code)]
//! Provider-boundary parsers for cached/open football statistics and match results.
//!
//! This crate deliberately owns JSON shape knowledge only. Canonical identity, feature
//! construction, modeling, and remote collection belong elsewhere.

use footy_core::{Date, ProvenanceId};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatsProvider {
    Sofascore,
    StatsBomb,
    OpenFootball,
    FootballDataOrg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchStatus {
    Scheduled,
    Live,
    Finished,
    Postponed,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceMetric {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStatSourceRecord {
    pub provider: StatsProvider,
    pub player_external_id: String,
    pub club_external_id: String,
    pub competition_external_id: String,
    pub season_external_id: String,
    pub match_external_id: String,
    pub date: Date,
    pub minutes: Option<u16>,
    pub metrics: Vec<SourceMetric>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeamStatSourceRecord {
    pub provider: StatsProvider,
    pub club_external_id: String,
    pub opponent_external_id: String,
    pub competition_external_id: String,
    pub season_external_id: String,
    pub match_external_id: String,
    pub date: Date,
    pub is_home: bool,
    pub metrics: Vec<SourceMetric>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResultSourceRecord {
    pub provider: StatsProvider,
    pub match_external_id: String,
    pub competition_external_id: String,
    pub season_external_id: String,
    pub date: Date,
    pub home_club_external_id: String,
    pub away_club_external_id: String,
    pub home_goals: Option<u8>,
    pub away_goals: Option<u8>,
    pub status: MatchStatus,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchContext {
    pub match_external_id: String,
    pub competition_external_id: String,
    pub season_external_id: String,
    pub date: Date,
    pub home_club_external_id: String,
    pub away_club_external_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerContext {
    pub match_context: MatchContext,
    pub player_external_id: String,
    pub club_external_id: String,
}

/// Parse a cached Sofascore event payload. Sofascore does not expose a documented public API,
/// so callers must treat this adapter as optional and own access/caching policy outside the crate.
pub fn parse_sofascore_event(
    input: &str,
    provenance_id: ProvenanceId,
) -> Result<MatchResultSourceRecord, String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let event = root.get("event").unwrap_or(&root);
    let timestamp = required_i64(event, &["startTimestamp"])?;
    let home = required_id(event, &["homeTeam", "id"])?;
    let away = required_id(event, &["awayTeam", "id"])?;
    Ok(MatchResultSourceRecord {
        provider: StatsProvider::Sofascore,
        match_external_id: required_id(event, &["id"] )?,
        competition_external_id: id_at(event, &["tournament", "id"]).unwrap_or_else(|| "unknown".into()),
        season_external_id: id_at(event, &["season", "id"]).unwrap_or_else(|| "unknown".into()),
        date: unix_date(timestamp)?,
        home_club_external_id: home,
        away_club_external_id: away,
        home_goals: u8_at(event, &["homeScore", "current"]),
        away_goals: u8_at(event, &["awayScore", "current"]),
        status: sofascore_status(string_at(event, &["status", "type"]).as_deref()),
        provenance_id,
    })
}

/// Parse cached Sofascore event statistics into one source record per team.
pub fn parse_sofascore_team_statistics(
    input: &str,
    context: &MatchContext,
    provenance_id: ProvenanceId,
) -> Result<Vec<TeamStatSourceRecord>, String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let periods = root
        .get("statistics")
        .and_then(Value::as_array)
        .ok_or_else(|| "Sofascore statistics array missing".to_owned())?;
    let mut home = BTreeMap::<String, f64>::new();
    let mut away = BTreeMap::<String, f64>::new();

    // Prefer the ALL period when present to avoid summing period and match totals together.
    let selected = periods
        .iter()
        .find(|period| string_at(period, &["period"]).as_deref() == Some("ALL"))
        .or_else(|| periods.first())
        .ok_or_else(|| "Sofascore statistics array empty".to_owned())?;
    if let Some(groups) = selected.get("groups").and_then(Value::as_array) {
        for group in groups {
            if let Some(items) = group.get("statisticsItems").and_then(Value::as_array) {
                for item in items {
                    let Some(name) = string_at(item, &["name"]) else { continue };
                    let key = normalized_metric_name(&name);
                    if let Some(value) = numeric_at(item, &["home"]) { home.insert(key.clone(), value); }
                    if let Some(value) = numeric_at(item, &["away"]) { away.insert(key, value); }
                }
            }
        }
    }

    Ok(vec![
        TeamStatSourceRecord {
            provider: StatsProvider::Sofascore,
            club_external_id: context.home_club_external_id.clone(),
            opponent_external_id: context.away_club_external_id.clone(),
            competition_external_id: context.competition_external_id.clone(),
            season_external_id: context.season_external_id.clone(),
            match_external_id: context.match_external_id.clone(),
            date: context.date,
            is_home: true,
            metrics: metrics_from_map(home),
            provenance_id,
        },
        TeamStatSourceRecord {
            provider: StatsProvider::Sofascore,
            club_external_id: context.away_club_external_id.clone(),
            opponent_external_id: context.home_club_external_id.clone(),
            competition_external_id: context.competition_external_id.clone(),
            season_external_id: context.season_external_id.clone(),
            match_external_id: context.match_external_id.clone(),
            date: context.date,
            is_home: false,
            metrics: metrics_from_map(away),
            provenance_id,
        },
    ])
}

/// Parse the cached Sofascore event/player statistics response for a known player/match context.
pub fn parse_sofascore_player_statistics(
    input: &str,
    context: &PlayerContext,
    provenance_id: ProvenanceId,
) -> Result<PlayerStatSourceRecord, String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let object = root
        .get("statistics")
        .unwrap_or(&root)
        .as_object()
        .ok_or_else(|| "Sofascore player statistics object missing".to_owned())?;
    let minutes = object.get("minutesPlayed").and_then(value_u16);
    let metrics = numeric_object_metrics(object, &["minutesPlayed"]);
    Ok(PlayerStatSourceRecord {
        provider: StatsProvider::Sofascore,
        player_external_id: context.player_external_id.clone(),
        club_external_id: context.club_external_id.clone(),
        competition_external_id: context.match_context.competition_external_id.clone(),
        season_external_id: context.match_context.season_external_id.clone(),
        match_external_id: context.match_context.match_external_id.clone(),
        date: context.match_context.date,
        minutes,
        metrics,
        provenance_id,
    })
}

/// Parse StatsBomb Open Data `matches/<competition>/<season>.json`.
pub fn parse_statsbomb_matches(
    input: &str,
    provenance_id: ProvenanceId,
) -> Result<Vec<MatchResultSourceRecord>, String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let matches = root.as_array().ok_or_else(|| "StatsBomb matches payload must be an array".to_owned())?;
    let mut out = Vec::with_capacity(matches.len());
    for value in matches {
        out.push(MatchResultSourceRecord {
            provider: StatsProvider::StatsBomb,
            match_external_id: required_id(value, &["match_id"] )?,
            competition_external_id: id_at(value, &["competition", "competition_id"]).unwrap_or_else(|| "unknown".into()),
            season_external_id: id_at(value, &["season", "season_id"]).unwrap_or_else(|| "unknown".into()),
            date: parse_iso_date(required_str(value, &["match_date"] )?)?,
            home_club_external_id: required_id(value, &["home_team", "home_team_id"] )?,
            away_club_external_id: required_id(value, &["away_team", "away_team_id"] )?,
            home_goals: u8_at(value, &["home_score"]),
            away_goals: u8_at(value, &["away_score"]),
            status: MatchStatus::Finished,
            provenance_id,
        });
    }
    Ok(out)
}

/// Aggregate a StatsBomb Open Data event file into player and team match statistics.
/// Event counts use `event_<type>` metric names and shots additionally accumulate `xg`.
pub fn parse_statsbomb_events(
    input: &str,
    context: &MatchContext,
    provenance_id: ProvenanceId,
) -> Result<(Vec<PlayerStatSourceRecord>, Vec<TeamStatSourceRecord>), String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let events = root.as_array().ok_or_else(|| "StatsBomb events payload must be an array".to_owned())?;
    let mut players = BTreeMap::<(String, String), BTreeMap<String, f64>>::new();
    let mut teams = BTreeMap::<String, BTreeMap<String, f64>>::new();

    for event in events {
        let Some(team_id) = id_at(event, &["team", "id"]) else { continue };
        let event_name = string_at(event, &["type", "name"]).unwrap_or_else(|| "unknown".into());
        let metric = format!("event_{}", normalized_metric_name(&event_name));
        *teams.entry(team_id.clone()).or_default().entry(metric.clone()).or_default() += 1.0;
        if let Some(player_id) = id_at(event, &["player", "id"]) {
            *players.entry((player_id, team_id.clone())).or_default().entry(metric).or_default() += 1.0;
        }
        if let Some(xg) = numeric_at(event, &["shot", "statsbomb_xg"]) {
            *teams.entry(team_id.clone()).or_default().entry("xg".into()).or_default() += xg;
            if let Some(player_id) = id_at(event, &["player", "id"]) {
                *players.entry((player_id, team_id)).or_default().entry("xg".into()).or_default() += xg;
            }
        }
    }

    let player_records = players
        .into_iter()
        .map(|((player_external_id, club_external_id), metrics)| PlayerStatSourceRecord {
            provider: StatsProvider::StatsBomb,
            player_external_id,
            club_external_id,
            competition_external_id: context.competition_external_id.clone(),
            season_external_id: context.season_external_id.clone(),
            match_external_id: context.match_external_id.clone(),
            date: context.date,
            minutes: None,
            metrics: metrics_from_map(metrics),
            provenance_id,
        })
        .collect();
    let team_records = teams
        .into_iter()
        .map(|(club_external_id, metrics)| {
            let is_home = club_external_id == context.home_club_external_id;
            let opponent_external_id = if is_home {
                context.away_club_external_id.clone()
            } else {
                context.home_club_external_id.clone()
            };
            TeamStatSourceRecord {
                provider: StatsProvider::StatsBomb,
                club_external_id,
                opponent_external_id,
                competition_external_id: context.competition_external_id.clone(),
                season_external_id: context.season_external_id.clone(),
                match_external_id: context.match_external_id.clone(),
                date: context.date,
                is_home,
                metrics: metrics_from_map(metrics),
                provenance_id,
            }
        })
        .collect();
    Ok((player_records, team_records))
}

/// Parse football-data.org v4 `/matches` responses from a cached API response.
pub fn parse_football_data_matches(
    input: &str,
    provenance_id: ProvenanceId,
) -> Result<Vec<MatchResultSourceRecord>, String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let matches = root
        .get("matches")
        .and_then(Value::as_array)
        .ok_or_else(|| "football-data.org matches array missing".to_owned())?;
    let mut out = Vec::with_capacity(matches.len());
    for value in matches {
        let date_text = required_str(value, &["utcDate"])?;
        let date_prefix = date_text.get(..10).ok_or_else(|| "invalid football-data.org utcDate".to_owned())?;
        out.push(MatchResultSourceRecord {
            provider: StatsProvider::FootballDataOrg,
            match_external_id: required_id(value, &["id"] )?,
            competition_external_id: id_at(value, &["competition", "id"]).unwrap_or_else(|| "unknown".into()),
            season_external_id: id_at(value, &["season", "id"]).unwrap_or_else(|| "unknown".into()),
            date: parse_iso_date(date_prefix)?,
            home_club_external_id: required_id(value, &["homeTeam", "id"] )?,
            away_club_external_id: required_id(value, &["awayTeam", "id"] )?,
            home_goals: u8_at(value, &["score", "fullTime", "home"]),
            away_goals: u8_at(value, &["score", "fullTime", "away"]),
            status: football_data_status(string_at(value, &["status"]).as_deref()),
            provenance_id,
        });
    }
    Ok(out)
}

/// Parse the common openfootball JSON shape (`rounds[].matches[]`). Team IDs fall back to names
/// because many CC0 datasets intentionally contain no numeric team identifiers.
pub fn parse_openfootball_matches(
    input: &str,
    competition_external_id: &str,
    season_external_id: &str,
    provenance_id: ProvenanceId,
) -> Result<Vec<MatchResultSourceRecord>, String> {
    let root: Value = serde_json::from_str(input).map_err(json_error)?;
    let rounds = root
        .get("rounds")
        .and_then(Value::as_array)
        .ok_or_else(|| "openfootball rounds array missing".to_owned())?;
    let mut out = Vec::new();
    for round in rounds {
        let Some(matches) = round.get("matches").and_then(Value::as_array) else { continue };
        for value in matches {
            let home = team_token(value.get("team1")).ok_or_else(|| "openfootball team1 missing".to_owned())?;
            let away = team_token(value.get("team2")).ok_or_else(|| "openfootball team2 missing".to_owned())?;
            let date = parse_iso_date(required_str(value, &["date"] )?)?;
            let (home_goals, away_goals) = openfootball_score(value);
            let match_external_id = id_at(value, &["id"]).unwrap_or_else(|| {
                format!("{}:{}:{}:{}", season_external_id, date.days_since_unix_epoch(), home, away)
            });
            out.push(MatchResultSourceRecord {
                provider: StatsProvider::OpenFootball,
                match_external_id,
                competition_external_id: competition_external_id.to_owned(),
                season_external_id: season_external_id.to_owned(),
                date,
                home_club_external_id: home,
                away_club_external_id: away,
                home_goals,
                away_goals,
                status: if home_goals.is_some() && away_goals.is_some() { MatchStatus::Finished } else { MatchStatus::Scheduled },
                provenance_id,
            });
        }
    }
    Ok(out)
}

fn numeric_object_metrics(object: &Map<String, Value>, excluded: &[&str]) -> Vec<SourceMetric> {
    let mut values = BTreeMap::new();
    for (name, value) in object {
        if excluded.contains(&name.as_str()) { continue; }
        if let Some(number) = numeric_value(value) {
            values.insert(normalized_metric_name(name), number);
        }
    }
    metrics_from_map(values)
}

fn metrics_from_map(values: BTreeMap<String, f64>) -> Vec<SourceMetric> {
    values
        .into_iter()
        .filter(|(_, value)| value.is_finite())
        .map(|(name, value)| SourceMetric { name, value })
        .collect()
}

fn normalized_metric_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            underscore = false;
        } else if !out.is_empty() && !underscore {
            out.push('_');
            underscore = true;
        }
    }
    while out.ends_with('_') { out.pop(); }
    out
}

fn json_error(error: serde_json::Error) -> String { format!("invalid JSON: {error}") }

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path { current = current.get(*key)?; }
    Some(current)
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    value_at(value, path).and_then(|value| match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

fn id_at(value: &Value, path: &[&str]) -> Option<String> { string_at(value, path) }

fn required_id(value: &Value, path: &[&str]) -> Result<String, String> {
    id_at(value, path).ok_or_else(|| format!("required id missing at {}", path.join(".")))
}

fn required_str<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    value_at(value, path)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("required string missing at {}", path.join(".")))
}

fn required_i64(value: &Value, path: &[&str]) -> Result<i64, String> {
    value_at(value, path)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("required integer missing at {}", path.join(".")))
}

fn numeric_at(value: &Value, path: &[&str]) -> Option<f64> { value_at(value, path).and_then(numeric_value) }

fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64().filter(|value| value.is_finite()),
        Value::String(text) => {
            let trimmed = text.trim().trim_end_matches('%').replace(',', "");
            let mut parsed = trimmed.parse::<f64>().ok()?;
            if text.trim().ends_with('%') { parsed /= 100.0; }
            parsed.is_finite().then_some(parsed)
        }
        _ => None,
    }
}

fn value_u16(value: &Value) -> Option<u16> {
    value.as_u64().and_then(|number| u16::try_from(number).ok()).or_else(|| {
        value.as_str().and_then(|text| text.parse::<u16>().ok())
    })
}

fn u8_at(value: &Value, path: &[&str]) -> Option<u8> {
    value_at(value, path)
        .and_then(|value| value.as_u64().or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok())))
        .and_then(|number| u8::try_from(number).ok())
}

fn parse_iso_date(value: &str) -> Result<Date, String> {
    let mut parts = value.split('-');
    let year = parts.next().ok_or_else(|| "date year missing".to_owned())?.parse::<i32>().map_err(|_| "invalid date year".to_owned())?;
    let month = parts.next().ok_or_else(|| "date month missing".to_owned())?.parse::<u8>().map_err(|_| "invalid date month".to_owned())?;
    let day = parts.next().ok_or_else(|| "date day missing".to_owned())?.parse::<u8>().map_err(|_| "invalid date day".to_owned())?;
    if parts.next().is_some() { return Err("invalid ISO date".into()); }
    Date::from_ymd(year, month, day).map_err(|_| "invalid Gregorian date".to_owned())
}

fn unix_date(timestamp: i64) -> Result<Date, String> {
    let days = timestamp.div_euclid(86_400);
    i32::try_from(days).map(Date).map_err(|_| "timestamp outside Date range".to_owned())
}

fn sofascore_status(value: Option<&str>) -> MatchStatus {
    match value.unwrap_or_default().to_ascii_lowercase().as_str() {
        "finished" => MatchStatus::Finished,
        "inprogress" | "in_progress" | "live" => MatchStatus::Live,
        "postponed" => MatchStatus::Postponed,
        "cancelled" | "canceled" => MatchStatus::Cancelled,
        "notstarted" | "not_started" => MatchStatus::Scheduled,
        _ => MatchStatus::Unknown,
    }
}

fn football_data_status(value: Option<&str>) -> MatchStatus {
    match value.unwrap_or_default() {
        "FINISHED" => MatchStatus::Finished,
        "LIVE" | "IN_PLAY" | "PAUSED" => MatchStatus::Live,
        "POSTPONED" | "SUSPENDED" => MatchStatus::Postponed,
        "CANCELLED" => MatchStatus::Cancelled,
        "SCHEDULED" | "TIMED" => MatchStatus::Scheduled,
        _ => MatchStatus::Unknown,
    }
}

fn team_token(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(name) => Some(name.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Object(object) => object.get("id").and_then(|value| string_at(value, &[])).or_else(|| object.get("name").and_then(Value::as_str).map(str::to_owned)),
        _ => None,
    }
}

fn openfootball_score(value: &Value) -> (Option<u8>, Option<u8>) {
    if let Some(score) = value.get("score") {
        if let Some(ft) = score.get("ft").and_then(Value::as_array) {
            if ft.len() >= 2 {
                return (ft.first().and_then(value_to_u8), ft.get(1).and_then(value_to_u8));
            }
        }
        if let Some(score_text) = score.as_str() {
            if let Some((left, right)) = score_text.split_once('-') {
                return (left.trim().parse().ok(), right.trim().parse().ok());
            }
        }
    }
    (u8_at(value, &["score1"]), u8_at(value, &["score2"]))
}

fn value_to_u8(value: &Value) -> Option<u8> {
    value.as_u64().and_then(|number| u8::try_from(number).ok()).or_else(|| value.as_str().and_then(|text| text.parse::<u8>().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sofascore_event_and_team_stats() {
        let event = r#"{"event":{"id":42,"startTimestamp":1704067200,"tournament":{"id":7},"season":{"id":8},"homeTeam":{"id":10},"awayTeam":{"id":20},"homeScore":{"current":2},"awayScore":{"current":1},"status":{"type":"finished"}}}"#;
        let parsed = parse_sofascore_event(event, ProvenanceId(1)).unwrap();
        assert_eq!(parsed.home_goals, Some(2));
        let context = MatchContext { match_external_id: "42".into(), competition_external_id: "7".into(), season_external_id: "8".into(), date: parsed.date, home_club_external_id: "10".into(), away_club_external_id: "20".into() };
        let stats = r#"{"statistics":[{"period":"ALL","groups":[{"statisticsItems":[{"name":"Ball possession","home":"55%","away":"45%"},{"name":"Total shots","home":12,"away":8}]}]}]}"#;
        let teams = parse_sofascore_team_statistics(stats, &context, ProvenanceId(2)).unwrap();
        assert_eq!(teams.len(), 2);
        assert_eq!(teams[0].metrics.iter().find(|m| m.name == "ball_possession").unwrap().value, 0.55);
    }

    #[test]
    fn parses_statsbomb_match_and_events() {
        let matches = r#"[{"match_id":1,"match_date":"2024-01-02","competition":{"competition_id":9},"season":{"season_id":10},"home_team":{"home_team_id":11},"away_team":{"away_team_id":12},"home_score":1,"away_score":0}]"#;
        let result = parse_statsbomb_matches(matches, ProvenanceId(1)).unwrap();
        assert_eq!(result[0].home_club_external_id, "11");
        let context = MatchContext { match_external_id: "1".into(), competition_external_id: "9".into(), season_external_id: "10".into(), date: result[0].date, home_club_external_id: "11".into(), away_club_external_id: "12".into() };
        let events = r#"[{"type":{"name":"Shot"},"team":{"id":11},"player":{"id":99},"shot":{"statsbomb_xg":0.25}},{"type":{"name":"Pass"},"team":{"id":11},"player":{"id":99}}]"#;
        let (players, teams) = parse_statsbomb_events(events, &context, ProvenanceId(2)).unwrap();
        assert_eq!(players[0].metrics.iter().find(|m| m.name == "xg").unwrap().value, 0.25);
        assert_eq!(teams[0].metrics.iter().find(|m| m.name == "event_pass").unwrap().value, 1.0);
    }

    #[test]
    fn parses_football_data_and_openfootball_matches() {
        let football_data = r#"{"matches":[{"id":1,"utcDate":"2024-02-03T15:00:00Z","status":"FINISHED","competition":{"id":2},"season":{"id":3},"homeTeam":{"id":4},"awayTeam":{"id":5},"score":{"fullTime":{"home":3,"away":2}}}]}"#;
        assert_eq!(parse_football_data_matches(football_data, ProvenanceId(1)).unwrap()[0].home_goals, Some(3));

        let open = r#"{"rounds":[{"matches":[{"date":"2024-02-03","team1":"A","team2":"B","score":{"ft":[2,2]}}]}]}"#;
        let matches = parse_openfootball_matches(open, "epl", "2023-24", ProvenanceId(1)).unwrap();
        assert_eq!(matches[0].away_goals, Some(2));
        assert_eq!(matches[0].home_club_external_id, "A");
    }

    #[test]
    fn parses_sofascore_player_metrics_without_inventing_missing_values() {
        let match_context = MatchContext { match_external_id: "1".into(), competition_external_id: "2".into(), season_external_id: "3".into(), date: Date(10), home_club_external_id: "4".into(), away_club_external_id: "5".into() };
        let context = PlayerContext { match_context, player_external_id: "7".into(), club_external_id: "4".into() };
        let record = parse_sofascore_player_statistics(r#"{"statistics":{"minutesPlayed":90,"rating":7.4,"goals":1,"expectedGoals":0.61,"optionalMissing":null}}"#, &context, ProvenanceId(1)).unwrap();
        assert_eq!(record.minutes, Some(90));
        assert!(record.metrics.iter().any(|metric| metric.name == "expectedgoals" && (metric.value - 0.61).abs() < 1e-9));
        assert!(!record.metrics.iter().any(|metric| metric.name == "optionalmissing"));
    }
}
