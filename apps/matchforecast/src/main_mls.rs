#![forbid(unsafe_code)]
//! MLS fixture forecasts (CF Montréal + upcoming opponents) from cached SofaScore team stats,
//! per-player season aggregates, and joined FM26 attributes.

#[path = "form.rs"]
mod form;

use footy_core::{AttributeId, ClubId, Confidence, Date, MatchId, MetricId, PlayerId, RoleId};
use footy_forecast::{NextMatchParticipationModel, PlayerParticipationFeatures, StatFamily};
use footy_match::{
    simulate_second_resolved_dynamic_mental_match, AttributeEventLoading, AttributeMatchupRule,
    CausalChannelGraph, ChannelEdge, DynamicMentalTransitionParameters, FormationPrior,
    FormationTemplate, GoalChannelLoading, LineupCandidate, LineupProjectionInput,
    LineupSamplingParameters, MatchMentalContext, MentalDimension, MentalMatchModel, MentalMeasure,
    MentalMetricLoading, MentalModelParameters, MentalSelectionLoading, MetricChannelLoading,
    PairwiseSynergy, PlayerAttributeValue, PlayerAttributeState, PlayerMatchupState, PlayerMentalState,
    RoleFit, SecondResolvedSimulationParameters, SpecificGoalProcess, SpecificInMatchParameters,
    SpecificMatchEnvironment,
    SpecificPlayerEventModel, SpecificScoreStateMetricModifier, SpecificTeamContext,
    StablePsychologyState, TacticalSlot, TeamMentalState,
    TeamProcessChannel, PrimitiveMatchSimulationInput,
};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

const M_SHOT: u16 = 1;
const M_XG: u16 = 2;
const M_XA: u16 = 3;
const M_PASS: u16 = 4;
const M_PROGC: u16 = 5;
const A_FINISH: u16 = 1;
const A_TECH: u16 = 2;
const A_DEFEND: u16 = 3;

struct TeamData {
    name: String,
    club: ClubId,
    matches: usize,
    gf_pm: f64,
    ga_pm: f64,
    xga_pm: f64,
    shots_pm: f64,
    shots_face_pm: f64,
    sot_pm: f64,
    sot_face_pm: f64,
}

struct PlayerRow {
    id: u64,
    name: String,
    team: String,
    minutes: f64,
    selection_listings: f64,
    selection_appearances: f64,
    selection_starts: f64,
    selection_minutes: f64,
    selection_start_minutes: f64,
    team_fixtures: f64,
    last_listed_date: Option<Date>,
    days_since_last_appearance: f64,
    days_since_last_listed: f64,
    shots_p90: f64,
    xg_p90: f64,
    xa_p90: f64,
    passes_p90: f64,
    prog_carries_p90: f64,
    fm_ca: Option<f64>,
    fm_finishing: Option<f64>,
    fm_technique: Option<f64>,
    fm_defending: Option<f64>,
    fm_consistency: Option<f64>,
    fm_dirtiness: Option<f64>,
}

type Row = HashMap<String, String>;

fn require_delimited_columns(path: &str, delimiter: char, required: &[&str]) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("read {path}: {error}"))?;
    let header = text.lines().next().ok_or_else(|| format!("{path} is empty"))?;
    let columns: std::collections::BTreeSet<&str> = header.split(delimiter).collect();
    let missing: Vec<&str> = required.iter().copied().filter(|column| !columns.contains(column)).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("{path} is stale; missing columns: {}", missing.join(", ")))
    }
}

fn validate_live_artifacts(as_of_text: &str) -> Result<String, String> {
    require_delimited_columns(
        "data/sofascore_cfm/player_fm_join.csv",
        ',',
        &[
            "player_id", "name", "team", "minutes", "first_listed_date", "last_listed_date", "last_appearance_date",
            "selection_listings", "selection_appearances", "selection_starts", "selection_minutes",
            "selection_start_minutes", "team_fixtures", "shots_p90", "xg_p90", "xa_p90",
            "passes_p90", "prog_carries_p90", "fm_ca", "fm_finishing", "fm_technique",
            "fm_tackling", "fm_marking", "fm_consistency", "fm_dirtiness",
        ],
    )?;
    require_delimited_columns(
        "data/sofascore_cfm/selection_training.tsv",
        '\t',
        &[
            "date", "club_fixtures", "prior_listings", "prior_appearances", "prior_starts", "prior_minutes",
            "prior_start_minutes", "days_since_last_appearance", "days_since_last_listed",
            "appeared", "started", "minutes",
        ],
    )?;
    let form_path = format!("data/sofascore_cfm/asof_form/{as_of_text}_form.csv");
    require_delimited_columns(
        &form_path,
        ',',
        &["player_id", "date", "minutes", "shots", "xg", "xa", "passes", "prog_carries"],
    )?;
    Ok(form_path)
}

fn read_csv(path: &str) -> Vec<Row> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut lines = text.lines();
    let header: Vec<String> = lines.next().expect("header").split(',').map(String::from).collect();
    lines
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            header
                .iter()
                .zip(line.split(','))
                .map(|(key, value)| (key.clone(), value.to_string()))
                .collect()
        })
        .collect()
}

fn f(row: &Row, key: &str) -> f64 {
    row.get(key).and_then(|value| value.parse::<f64>().ok()).unwrap_or(0.0)
}

fn date_from(text: &str) -> Date {
    let parts: Vec<&str> = text.split('-').collect();
    Date::from_ymd(parts[0].parse().unwrap(), parts[1].parse().unwrap(), parts[2].parse().unwrap()).unwrap()
}

fn optional_date_from(text: &str) -> Option<Date> {
    let mut parts = text.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() { return None; }
    Date::from_ymd(year, month, day).ok()
}

fn stable(text: &str) -> u32 {
    let bytes = format!("club:{text}");
    let mut hash: u32 = 0x811c9dc5;
    for byte in bytes.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash % 900_000 + 1
}

fn metric_name(metric: MetricId) -> &'static str {
    match metric.0 {
        M_SHOT => "shots",
        M_XG => "xg",
        M_XA => "xa",
        M_PASS => "passes",
        M_PROGC => "progressive_carries",
        _ => "unknown",
    }
}

fn write_player_forecast(
    match_id: u64,
    forecast: &footy_match::SecondResolvedDynamicMentalForecast,
    players: &[PlayerRow],
) -> std::io::Result<String> {
    let directory = "data/sofascore_cfm/forecasts";
    std::fs::create_dir_all(directory)?;
    let path = format!("{directory}/{match_id}_players.tsv");
    let mut rows: Vec<_> = forecast.player_metrics.iter().collect();
    rows.sort_by_key(|row| (row.player.0, row.metric.0));
    let mut output = String::from(
        "player_id\tname\tteam\tmetric\tstart_probability\tappearance_probability\texpected_minutes\tmean_total\tp10_total\tp50_total\tp90_total\tconditional_mean_total\tconditional_mean_per90\n",
    );
    for row in rows {
        let player = players.iter().find(|value| value.id == row.player.0);
        let name = player.map(|value| value.name.as_str()).unwrap_or("synthetic_depth");
        let team = player.map(|value| value.team.as_str()).unwrap_or("");
        let conditional_total = row.conditional_on_appearance_total.map(|value| value.mean).unwrap_or(0.0);
        let conditional_per90 = row.conditional_on_appearance_per90.map(|value| value.mean).unwrap_or(0.0);
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{:.6}\t{:.6}\t{:.3}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}",
            row.player.0,
            name.replace('\t', " ").replace('\n', " ").replace('\r', " "),
            team.replace('\t', " ").replace('\n', " ").replace('\r', " "),
            metric_name(row.metric),
            row.start_probability.get(),
            row.appearance_probability.get(),
            row.expected_minutes,
            row.unconditional_total.mean,
            row.unconditional_total.p10,
            row.unconditional_total.p50,
            row.unconditional_total.p90,
            conditional_total,
            conditional_per90,
        ).expect("writing to String cannot fail");
    }
    std::fs::write(&path, output)?;
    Ok(path)
}

struct LeagueWindows {
    /// team -> rows of matches involving that team (strictly before cutoff)
    teams: BTreeMap<String, Vec<Row>>,
    cutoff: Date,
}

impl LeagueWindows {
    fn build(path: &str, cutoff: Date) -> Self {
        let mut teams: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for row in read_csv(path) {
            if date_from(&row["date"]) >= cutoff {
                continue; // strictly pre-match information only
            }
            teams.entry(row["home"].clone()).or_default().push(row.clone());
            teams.entry(row["away"].clone()).or_default().push(row.clone());
        }
        Self { teams, cutoff }
    }

    fn team(&self, name: &str) -> TeamData {
        let empty = Vec::new();
        let rows = self.teams.get(name).unwrap_or(&empty);
        if rows.is_empty() {
            return TeamData {
                name: name.to_string(),
                club: ClubId(stable(name)),
                matches: 0,
                gf_pm: 1.40,
                ga_pm: 1.40,
                xga_pm: 1.40,
                shots_pm: 12.0,
                shots_face_pm: 12.0,
                sot_pm: 4.5,
                sot_face_pm: 4.5,
            };
        }
        let ewma = |pick: &dyn Fn(&Row) -> f64, prior: f64| -> f64 {
            let mut numerator = 2.0 * prior;
            let mut denominator = 2.0;
            for row in rows {
                let age_days = self.cutoff.days_since(date_from(&row["date"])).max(1) as f64;
                let weight = (-std::f64::consts::LN_2 * age_days / 60.0).exp();
                let value = pick(row);
                if value.is_finite() && value >= 0.0 {
                    numerator += weight * value;
                    denominator += weight;
                }
            }
            numerator / denominator.max(f64::EPSILON)
        };
        let gf_pm = ewma(&|row| if row["home"] == name { f(row, "home_goals") } else { f(row, "away_goals") }, 1.40);
        let ga_pm = ewma(&|row| if row["home"] == name { f(row, "away_goals") } else { f(row, "home_goals") }, 1.40);
        // The team rows already contain both sides of each fixture; conceded xG is simply the
        // opponent's xG from that same row. The previous opponent-row reconstruction duplicated
        // work and obscured the sign convention used downstream.
        let xga_pm = ewma(&|row| if row["home"] == name { f(row, "away_xg") } else { f(row, "home_xg") }, 1.40);
        let shots_pm = ewma(&|row| {
            if row["home"] == name { f(row, "home_shots") } else { f(row, "away_shots") }
        }, 12.0);
        let shots_face_pm = ewma(&|row| {
            if row["home"] == name { f(row, "away_shots") } else { f(row, "home_shots") }
        }, 12.0);
        let sot_pm = ewma(&|row| {
            if row["home"] == name { f(row, "home_sot") } else { f(row, "away_sot") }
        }, 4.5);
        let sot_face_pm = ewma(&|row| {
            if row["home"] == name { f(row, "away_sot") } else { f(row, "home_sot") }
        }, 4.5);
        TeamData {
            name: name.to_string(),
            club: ClubId(stable(name)),
            matches: rows.len(),
            gf_pm,
            ga_pm,
            xga_pm,
            shots_pm,
            shots_face_pm,
            sot_pm,
            sot_face_pm,
        }
    }
}

fn load_players(path: &str, min_minutes: f64, as_of: Date) -> Vec<PlayerRow> {
    read_csv(path)
        .into_iter()
        .map(|row| {
            let first_listed_date = row.get("first_listed_date").and_then(|value| optional_date_from(value));
            let last_listed_date = row.get("last_listed_date").and_then(|value| optional_date_from(value));
            let last_appearance_date = row.get("last_appearance_date").and_then(|value| optional_date_from(value));
            let appearance_anchor = last_appearance_date.or(first_listed_date).or(last_listed_date);
            PlayerRow {
                id: f(&row, "player_id") as u64,
                name: row.get("name").cloned().unwrap_or_default(),
                team: row.get("team").cloned().unwrap_or_default(),
                minutes: f(&row, "minutes"),
                selection_listings: f(&row, "selection_listings"),
                selection_appearances: f(&row, "selection_appearances"),
                selection_starts: f(&row, "selection_starts"),
                selection_minutes: f(&row, "selection_minutes"),
                selection_start_minutes: f(&row, "selection_start_minutes"),
                team_fixtures: f(&row, "team_fixtures"),
                last_listed_date,
                days_since_last_appearance: appearance_anchor
                    .map(|date| as_of.days_since(date).max(0) as f64)
                    .unwrap_or(45.0),
                days_since_last_listed: last_listed_date
                    .map(|date| as_of.days_since(date).max(0) as f64)
                    .unwrap_or(45.0),
                shots_p90: f(&row, "shots_p90"),
                xg_p90: f(&row, "xg_p90"),
                xa_p90: f(&row, "xa_p90"),
                passes_p90: f(&row, "passes_p90"),
                prog_carries_p90: f(&row, "prog_carries_p90"),
                fm_ca: row.get("fm_ca").and_then(|v| v.parse().ok()),
                fm_finishing: row.get("fm_finishing").and_then(|v| v.parse().ok()),
                fm_technique: row.get("fm_technique").and_then(|v| v.parse().ok()),
                fm_defending: Some(
                    0.5 * row.get("fm_marking").and_then(|v| v.parse::<f64>().ok()).unwrap_or(10.0)
                        + 0.5 * row.get("fm_tackling").and_then(|v| v.parse::<f64>().ok()).unwrap_or(10.0),
                ),
                fm_consistency: row.get("fm_consistency").and_then(|v| v.parse().ok()),
                fm_dirtiness: row.get("fm_dirtiness").and_then(|v| v.parse().ok()),
            }
        })
        .filter(|row| {
            let fresh = row.last_listed_date.map_or(false, |date| {
                let age = as_of.days_since(date);
                age >= 0 && age <= 45
            });
            row.minutes >= min_minutes && !row.team.is_empty() && fresh
        })
        .collect()
}

fn formation(id: u16) -> FormationTemplate {
    FormationTemplate {
        id,
        slots: (0_u8..11)
            .map(|slot| TacticalSlot {
                slot,
                role: RoleId(u16::from(slot) + 1),
                x: 0.06 + f64::from(slot % 11) * 0.075,
                y: f64::from(slot % 5) / 4.0,
            })
            .collect(),
    }
}

fn squad(
    players: &[PlayerRow],
    team_name: &str,
    club: ClubId,
    participation_model: Option<&NextMatchParticipationModel>,
) -> Vec<LineupCandidate> {
    let mut mine: Vec<&PlayerRow> = players.iter().filter(|p| p.team == team_name).collect();
    mine.sort_by(|a, b| b.minutes.total_cmp(&a.minutes));
    let max_minutes = mine.iter().map(|p| p.minutes).fold(1e-9, f64::max);
    let mut candidates: Vec<LineupCandidate> = mine
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let usage = p.minutes / max_minutes;
            let has_selection_history = p.team_fixtures >= 2.0 && p.selection_listings >= 1.0;
            let (mut starter_probability, mut appearance_probability, mut expected_minutes_if_start) = if has_selection_history {
                let prior_strength = 2.0;
                let start = (p.selection_starts + 0.50 * prior_strength)
                    / (p.team_fixtures + prior_strength);
                let appearance = (p.selection_appearances + 0.75 * prior_strength)
                    / (p.team_fixtures + prior_strength);
                let estimated_start_minutes = if p.selection_starts > 0.0 {
                    (p.selection_start_minutes + 76.0 * prior_strength)
                        / (p.selection_starts + prior_strength)
                } else {
                    76.0
                };
                (start.clamp(0.02, 0.98), appearance.max(start).clamp(0.02, 0.99), estimated_start_minutes.clamp(45.0, 90.0))
            } else {
                let start = if index < 11 {
                    (0.45 + 0.50 * usage).clamp(0.05, 0.95)
                } else {
                    (0.10 + 0.25 * usage).clamp(0.02, 0.45)
                };
                (start, (start + if index < 11 { 0.18 } else { 0.35 }).clamp(start, 0.98), if index < 11 { 76.0 } else { 55.0 })
            };
            let mut minutes_sigma = 12.0;
            let mut availability_probability = 0.96;
            if has_selection_history {
                if let Some(model) = participation_model {
                    let features = PlayerParticipationFeatures {
                        club_fixtures: p.team_fixtures,
                        prior_listings: p.selection_listings,
                        prior_appearances: p.selection_appearances,
                        prior_starts: p.selection_starts,
                        prior_minutes: p.selection_minutes,
                        prior_start_minutes: p.selection_start_minutes,
                        days_since_last_appearance: p.days_since_last_appearance,
                        days_since_last_listed: p.days_since_last_listed,
                    };
                    if let Ok(prediction) = model.predict(features) {
                        starter_probability = prediction.start_probability.clamp(0.01, 0.99);
                        appearance_probability = prediction.appearance_probability.clamp(starter_probability, 0.995);
                        expected_minutes_if_start = prediction.expected_minutes_if_start;
                        minutes_sigma = prediction.minutes_sigma_if_start;
                        availability_probability = 1.0;
                    }
                }
            }
            LineupCandidate {
                player: PlayerId(p.id),
                start_probability: starter_probability,
                availability_probability,
                ability_index: p.fm_ca.unwrap_or(105.0) / 10.0,
                fatigue: 0.15,
                expected_minutes_if_start,
                minutes_sigma,
                substitute_probability: (appearance_probability - starter_probability).max(0.0),
                role_fits: (1_u16..=11).map(|role| RoleFit { role: RoleId(role), fit: 1.0 }).collect(),
            }
        })
        .collect();
    candidates.sort_by(|left, right| {
        let left_appearance = left.start_probability + left.substitute_probability;
        let right_appearance = right.start_probability + right.substitute_probability;
        right_appearance
            .total_cmp(&left_appearance)
            .then_with(|| right.start_probability.total_cmp(&left.start_probability))
    });
    candidates.truncate(18);
    // Pad to at least 14 candidates so lineup sampling always has bench depth.
    while candidates.len() < 14 {
        let index = candidates.len();
        candidates.push(LineupCandidate {
            player: PlayerId(u64::from(club.0) * 1000 + index as u64 + 800000),
            start_probability: 0.02,
            availability_probability: 0.9,
            ability_index: 10.0,
            fatigue: 0.15,
            expected_minutes_if_start: 30.0,
            minutes_sigma: 12.0,
            substitute_probability: 0.5,
            role_fits: (1_u16..=11).map(|role| RoleFit { role: RoleId(role), fit: 0.8 }).collect(),
        });
    }
    candidates
}

fn channel_graph() -> CausalChannelGraph {
    CausalChannelGraph {
        metric_loadings: vec![
            MetricChannelLoading { metric: MetricId(M_PASS), channel: TeamProcessChannel::Possession, weight: 0.4 },
            MetricChannelLoading { metric: MetricId(M_PASS), channel: TeamProcessChannel::Progression, weight: 0.2 },
            MetricChannelLoading { metric: MetricId(M_PROGC), channel: TeamProcessChannel::Progression, weight: 0.6 },
            MetricChannelLoading { metric: MetricId(M_SHOT), channel: TeamProcessChannel::ShotVolume, weight: 1.0 },
            MetricChannelLoading { metric: MetricId(M_XG), channel: TeamProcessChannel::ShotQuality, weight: 1.0 },
            MetricChannelLoading { metric: MetricId(M_XA), channel: TeamProcessChannel::ChanceCreation, weight: 0.8 },
        ],
        edges: vec![
            ChannelEdge { from: TeamProcessChannel::Possession, to: TeamProcessChannel::Progression, weight: 0.5 },
            ChannelEdge { from: TeamProcessChannel::Progression, to: TeamProcessChannel::ChanceCreation, weight: 0.6 },
            ChannelEdge { from: TeamProcessChannel::ShotVolume, to: TeamProcessChannel::ChanceCreation, weight: 0.4 },
            ChannelEdge { from: TeamProcessChannel::ShotQuality, to: TeamProcessChannel::Finishing, weight: 0.8 },
            ChannelEdge { from: TeamProcessChannel::ChanceCreation, to: TeamProcessChannel::BoxEntries, weight: 0.6 },
        ],
        goal_loadings: vec![
            GoalChannelLoading { channel: TeamProcessChannel::Finishing, attack_log_weight: 0.30, defense_log_weight: 0.0 },
            GoalChannelLoading { channel: TeamProcessChannel::ChanceCreation, attack_log_weight: 0.18, defense_log_weight: 0.0 },
        ],
    }
}

fn event_models_for(players: &[PlayerRow], team_name: &str) -> Vec<SpecificPlayerEventModel> {
    players
        .iter()
        .filter(|p| p.team == team_name)
        .flat_map(|p| {
            let n_appearances = (p.minutes / 80.0).max(3.0);
            let variance_factor = 1.0 + 8.0 / n_appearances;
            let shots = form::shrunk_player_rate(p.shots_p90, 1.25, p.minutes);
            let xg = form::shrunk_player_rate(p.xg_p90, 0.15, p.minutes);
            let xa = form::shrunk_player_rate(p.xa_p90, 0.12, p.minutes);
            let passes = form::shrunk_player_rate(p.passes_p90, 32.0, p.minutes);
            let prog_carries = form::shrunk_player_rate(p.prog_carries_p90, 1.5, p.minutes);
            [
                SpecificPlayerEventModel {
                    player: PlayerId(p.id),
                    metric: MetricId(M_SHOT),
                    family: StatFamily::Count,
                    mean_per90: shots.max(1e-3),
                    variance_per90: shots.max(1e-3) * variance_factor,
                    epistemic_sigma: (shots.max(0.05) / n_appearances.sqrt()).min(0.20),
                    reference_per90: 1.25, // league avg player shots per90
                    aggregate_matchup_sensitivity: 0.04,
                    attribute_matchup_sensitivity: 0.03,
                    teammate_sensitivity: 0.0,
                    role_fit_sensitivity: 0.05,
                    fatigue_sensitivity: 0.10,
                },
                SpecificPlayerEventModel {
                    player: PlayerId(p.id),
                    metric: MetricId(M_XG),
                    family: StatFamily::ContinuousNonNegative,
                    mean_per90: xg.max(1e-3),
                    variance_per90: xg.max(1e-3) * variance_factor * 1.5,
                    epistemic_sigma: (xg.max(0.01) / n_appearances.sqrt()).min(0.06),
                    reference_per90: 0.15,
                    aggregate_matchup_sensitivity: 0.04,
                    attribute_matchup_sensitivity: 0.03,
                    teammate_sensitivity: 0.0,
                    role_fit_sensitivity: 0.05,
                    fatigue_sensitivity: 0.08,
                },
                SpecificPlayerEventModel {
                    player: PlayerId(p.id),
                    metric: MetricId(M_XA),
                    family: StatFamily::ContinuousNonNegative,
                    mean_per90: xa.max(1e-3),
                    variance_per90: xa.max(1e-3) * variance_factor * 1.5,
                    epistemic_sigma: (xa.max(0.01) / n_appearances.sqrt()).min(0.08),
                    reference_per90: 0.12, // league avg player xA per90
                    aggregate_matchup_sensitivity: 0.04,
                    attribute_matchup_sensitivity: 0.03,
                    teammate_sensitivity: 0.0,
                    role_fit_sensitivity: 0.05,
                    fatigue_sensitivity: 0.08,
                },
                SpecificPlayerEventModel {
                    player: PlayerId(p.id),
                    metric: MetricId(M_PASS),
                    family: StatFamily::Count,
                    mean_per90: passes.max(1.0),
                    variance_per90: passes.max(1.0) * variance_factor,
                    epistemic_sigma: (passes.max(1.0) / n_appearances.sqrt()).min(3.0),
                    reference_per90: 32.0,
                    aggregate_matchup_sensitivity: 0.02,
                    attribute_matchup_sensitivity: 0.02,
                    teammate_sensitivity: 0.0,
                    role_fit_sensitivity: 0.05,
                    fatigue_sensitivity: 0.08,
                },
                SpecificPlayerEventModel {
                    player: PlayerId(p.id),
                    metric: MetricId(M_PROGC),
                    family: StatFamily::Count,
                    mean_per90: prog_carries.max(1e-3),
                    variance_per90: prog_carries.max(1e-3) * variance_factor,
                    epistemic_sigma: (prog_carries.max(0.1) / n_appearances.sqrt()).min(1.0),
                    reference_per90: 1.5, // league avg player progressive carries per90
                    aggregate_matchup_sensitivity: 0.03,
                    attribute_matchup_sensitivity: 0.03,
                    teammate_sensitivity: 0.0,
                    role_fit_sensitivity: 0.05,
                    fatigue_sensitivity: 0.10,
                },
            ]
        })
        .collect()
}

fn anchor_shot_volume(models: &mut [SpecificPlayerEventModel], target_pm: f64) {
    if !target_pm.is_finite() || target_pm <= 0.0 {
        return;
    }
    let mut by_mean: Vec<&SpecificPlayerEventModel> =
        models.iter().filter(|model| model.metric.0 == M_SHOT).collect();
    if by_mean.len() < 8 {
        return;
    }
    by_mean.sort_by(|left, right| right.mean_per90.total_cmp(&left.mean_per90));
    let xi_sum: f64 = by_mean.iter().take(11).map(|model| model.mean_per90 * 0.78).sum();
    if xi_sum <= 1e-6 {
        return;
    }
    let scale = ((target_pm / xi_sum) * 0.5 + 0.5).clamp(0.6, 2.5);
    for model in models.iter_mut().filter(|model| model.metric.0 == M_SHOT) {
        model.mean_per90 *= scale;
        model.variance_per90 *= scale;
    }
}

fn attributes_and_states_for(players: &[PlayerRow], team_name: &str) -> (Vec<PlayerAttributeState>, Vec<PlayerMatchupState>) {
    let mut attribute_states = Vec::new();
    let mut matchup_states = Vec::new();
    for p in players.iter().filter(|p| p.team == team_name) {
        attribute_states.push(PlayerAttributeState {
            player: PlayerId(p.id),
            attributes: vec![
                PlayerAttributeValue { attribute: AttributeId(A_FINISH), value: p.fm_finishing.unwrap_or(10.0).clamp(1.0, 20.0) },
                PlayerAttributeValue { attribute: AttributeId(A_TECH), value: p.fm_technique.unwrap_or(10.0).clamp(1.0, 20.0) },
                PlayerAttributeValue { attribute: AttributeId(A_DEFEND), value: p.fm_defending.unwrap_or(10.0).clamp(1.0, 20.0) },
            ],
        });
        let base = (p.fm_ca.unwrap_or(105.0) / 10.0) as f32;
        matchup_states.push(PlayerMatchupState {
            player: PlayerId(p.id),
            ability: footy_state::AbilityVector::from_components([
                base, base * 0.95, base, base * 0.92, base * 0.9, base * 0.85, base * 0.9, base, base, 6.0,
            ]),
        });
    }
    (attribute_states, matchup_states)
}

fn ensure_candidate_primitives(
    lineup: &LineupProjectionInput,
    attribute_states: &mut Vec<PlayerAttributeState>,
    player_states: &mut Vec<PlayerMatchupState>,
    event_models: &mut Vec<SpecificPlayerEventModel>,
) {
    for candidate in &lineup.candidates {
        if player_states.iter().any(|state| state.player == candidate.player) {
            continue;
        }
        attribute_states.push(PlayerAttributeState {
            player: candidate.player,
            attributes: vec![
                PlayerAttributeValue { attribute: AttributeId(A_FINISH), value: 10.0 },
                PlayerAttributeValue { attribute: AttributeId(A_TECH), value: 10.0 },
                PlayerAttributeValue { attribute: AttributeId(A_DEFEND), value: 10.0 },
            ],
        });
        let base = candidate.ability_index as f32;
        player_states.push(PlayerMatchupState {
            player: candidate.player,
            ability: footy_state::AbilityVector::from_components([
                base, base * 0.95, base, base * 0.92, base * 0.9, base * 0.85,
                base * 0.9, base, base, 6.0,
            ]),
        });
        for (metric, family, reference, sigma) in [
            (MetricId(M_SHOT), StatFamily::Count, 1.25_f64, 0.30_f64),
            (MetricId(M_XG), StatFamily::ContinuousNonNegative, 0.15, 0.10),
            (MetricId(M_XA), StatFamily::ContinuousNonNegative, 0.12, 0.10),
            (MetricId(M_PASS), StatFamily::Count, 32.0, 5.0),
            (MetricId(M_PROGC), StatFamily::Count, 1.5, 1.25),
        ] {
            event_models.push(SpecificPlayerEventModel {
                player: candidate.player,
                metric,
                family,
                mean_per90: reference,
                variance_per90: reference * 2.5,
                epistemic_sigma: sigma,
                reference_per90: reference,
                aggregate_matchup_sensitivity: 0.02,
                attribute_matchup_sensitivity: 0.02,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.04,
                fatigue_sensitivity: 0.08,
            });
        }
    }
}

fn mental_model(home: &TeamData, away: &TeamData, home_ids: &[PlayerId], away_ids: &[PlayerId],
                dirtiness_of: &dyn Fn(PlayerId) -> Option<f64>, consistency_of: &dyn Fn(PlayerId) -> Option<f64>) -> MentalMatchModel {
    let measure = |value: f64| MentalMeasure::new(value, Confidence::new(0.7).unwrap_or(Confidence::ONE)).expect("measure");
    let form_bias = |team: &TeamData| ((team.gf_pm - team.ga_pm) * 0.06).clamp(-0.25, 0.25);
    let squad_mental = |ids: &[PlayerId]| -> Vec<PlayerMentalState> {
        ids.iter()
            .map(|player| {
                let discipline = dirtiness_of(*player);
                let consistency = consistency_of(*player);
                PlayerMentalState {
                    player: *player,
                    stable: StablePsychologyState {
                        professionalism: measure(0.30),
                        ambition: measure(0.30),
                        consistency: measure(consistency.map(|value| (value - 12.0) / 24.0).unwrap_or(0.0)),
                        pressure_resilience: measure(0.30),
                        adaptability: measure(0.25),
                        leadership: measure(0.25),
                        teamwork: measure(0.35),
                        discipline: measure(discipline.map(|value| -(value - 12.0) / 24.0).unwrap_or(0.0)),
                    },
                    morale: measure(0.10),
                    self_belief: measure(0.10),
                    form: measure(0.05),
                    form_trend: measure(0.0),
                    motivation: measure(0.15),
                    role_satisfaction: measure(0.10),
                }
            })
            .collect()
    };
    let team_state = |team: &TeamData, bias: f64| TeamMentalState {
        club: team.club,
        collective_morale: measure(bias),
        collective_belief: measure(bias),
        cohesion: measure(0.25),
        leadership: measure(0.25),
    };
    MentalMatchModel {
        players: [squad_mental(home_ids), squad_mental(away_ids)].concat(),
        home_team: team_state(home, form_bias(home)),
        away_team: team_state(away, form_bias(away)),
        metric_loadings: vec![
            MentalMetricLoading { metric: MetricId(M_SHOT), dimension: MentalDimension::SelfBelief, mean_log_weight: 0.06, log_variance_weight: -0.04 },
            MentalMetricLoading { metric: MetricId(M_XA), dimension: MentalDimension::Morale, mean_log_weight: 0.05, log_variance_weight: -0.02 },
        ],
        team_metric_loadings: Vec::new(),
        selection_loadings: vec![MentalSelectionLoading {
            dimension: MentalDimension::Motivation,
            start_logit_weight: 0.08,
            substitute_logit_weight: 0.08,
        }],
        context: MatchMentalContext { pressure_intensity: 0.6, environmental_novelty: 0.15, hostility: 0.25 },
        parameters: MentalModelParameters {
            discipline_red_card_log_weight: -0.35,
            low_morale_red_card_log_weight: 0.25,
            ..MentalModelParameters::default()
        },
    }
}

fn build_input(
    match_id: MatchId,
    home: &TeamData,
    away: &TeamData,
    players: &[PlayerRow],
    participation_model: Option<&NextMatchParticipationModel>,
    form_path: Option<&str>,
) -> PrimitiveMatchSimulationInput {
    let home_lineup = LineupProjectionInput {
        club: home.club,
        formations: vec![
            FormationPrior { formation: formation(442), probability: 0.55 },
            FormationPrior { formation: formation(433), probability: 0.45 },
        ],
        candidates: squad(players, &home.name, home.club, participation_model),
        locked_starters: Vec::new(),
        parameters: LineupSamplingParameters::default(),
    };
    let away_lineup = LineupProjectionInput {
        club: away.club,
        formations: vec![
            FormationPrior { formation: formation(442), probability: 0.55 },
            FormationPrior { formation: formation(433), probability: 0.45 },
        ],
        candidates: squad(players, &away.name, away.club, participation_model),
        locked_starters: Vec::new(),
        parameters: LineupSamplingParameters::default(),
    };

    let (home_attributes, home_states) = attributes_and_states_for(players, &home.name);
    let (away_attributes, away_states) = attributes_and_states_for(players, &away.name);

    let mut attribute_states = home_attributes;
    attribute_states.extend(away_attributes);
    let mut player_states = home_states;
    player_states.extend(away_states);

    let mut home_models = event_models_for(players, &home.name);
    let mut away_models = event_models_for(players, &away.name);
    if let Some(form_text) = form_path.and_then(|path| std::fs::read_to_string(path).ok()) {
        let learned = form::learned_rate_forecasts_from_csv(&form_text);
        let regimes = form::regimes_from_csv(&form_text);
        for model in home_models.iter_mut().chain(away_models.iter_mut()) {
            if let Some(prediction) = learned.get(&(model.player.0, model.metric.0)) {
                let baseline = model.mean_per90;
                let learned_mean = prediction
                    .mean_per90
                    .clamp((baseline * 0.55).max(0.0), (baseline * 1.65 + 0.05).max(0.05));
                model.mean_per90 = 0.35 * baseline + 0.65 * learned_mean;
                model.variance_per90 =
                    (0.35 * model.variance_per90 + 0.65 * prediction.variance_per90).max(1e-6);
                model.epistemic_sigma =
                    (0.35 * model.epistemic_sigma.powi(2) + 0.65 * prediction.epistemic_sigma.powi(2)).sqrt();
            } else if let Some(regime) = regimes.get(&model.player.0) {
                model.mean_per90 *= regime.mean_multiplier();
            }
        }
    }
    anchor_shot_volume(&mut home_models, home.shots_pm.max(4.0));
    anchor_shot_volume(&mut away_models, away.shots_pm.max(4.0));
    let mut event_models = home_models;
    event_models.extend(away_models);
    ensure_candidate_primitives(&home_lineup, &mut attribute_states, &mut player_states, &mut event_models);
    ensure_candidate_primitives(&away_lineup, &mut attribute_states, &mut player_states, &mut event_models);

    let env_rate =
        ((home.gf_pm + home.ga_pm + away.gf_pm + away.ga_pm) / 4.0).clamp(0.9, 2.2);
    let sot_rate = ((home.sot_pm * 0.30 + away.sot_face_pm * 0.30)
        + (away.sot_pm * 0.30 + home.sot_face_pm * 0.30))
        / 4.0;
    let blended_rate = (env_rate * 0.6 + sot_rate.clamp(0.9, 2.4) * 0.4).clamp(1.05, 2.1);

    let team_context = |team: &TeamData| SpecificTeamContext {
        club: team.club,
        // Raw team xG is not residual anymore: player xG already enters the explicit
        // ShotQuality -> Finishing path. Keeping both would double-count attacking strength.
        residual_attack_log_adjustment: 0.0,
        residual_defense_log_adjustment: ((1.40 - team.xga_pm) / 1.40 * 0.35).clamp(-0.35, 0.35),
        metric_modifiers: Vec::new(),
    };

    let input = PrimitiveMatchSimulationInput {
        match_id,
        home: home.club,
        away: away.club,
        home_lineup,
        away_lineup,
        player_states,
        attribute_states,
        event_models,
        own_attribute_loadings: vec![
            AttributeEventLoading { metric: MetricId(M_SHOT), attribute: AttributeId(A_FINISH), reference: 11.0, scale: 6.0, log_rate_weight: 0.10 },
            AttributeEventLoading { metric: MetricId(M_XG), attribute: AttributeId(A_FINISH), reference: 11.0, scale: 6.0, log_rate_weight: 0.08 },
            AttributeEventLoading { metric: MetricId(M_XA), attribute: AttributeId(A_TECH), reference: 11.0, scale: 6.0, log_rate_weight: 0.08 },
            AttributeEventLoading { metric: MetricId(M_PASS), attribute: AttributeId(A_TECH), reference: 11.0, scale: 6.0, log_rate_weight: 0.05 },
            AttributeEventLoading { metric: MetricId(M_PROGC), attribute: AttributeId(A_TECH), reference: 11.0, scale: 6.0, log_rate_weight: 0.06 },
        ],
        aggregate_matchup_rules: vec![
            footy_match::MatchupRule { metric: Some(MetricId(M_SHOT)), attacker_dimension: footy_match::AbilityDimension::Finishing, defender_dimension: footy_match::AbilityDimension::Defending, weight: 1.0, scale: 6.0 },
        ],
        attribute_matchup_rules: vec![
            AttributeMatchupRule { metric: Some(MetricId(M_SHOT)), attacker_attribute: AttributeId(A_FINISH), defender_attribute: AttributeId(A_DEFEND), weight: 1.0, scale: 6.0 },
        ],
        synergies: Vec::<PairwiseSynergy>::new(),
        channel_graph: channel_graph(),
        score_state_modifiers: vec![
            SpecificScoreStateMetricModifier { metric: MetricId(M_SHOT), trailing_multiplier: 1.08, leading_multiplier: 0.94 },
        ],
        incident_profiles: Vec::new(),
        in_match: SpecificInMatchParameters {
            injury_probability_per90: 0.02,
            red_card_probability_per90: 0.02,
            max_substitutions: 5,
            minimum_tactical_substitution_minute: 58.0,
        },
        home_context: team_context(home),
        away_context: team_context(away),
        environment: SpecificMatchEnvironment {
            home_residual_attack_log_adjustment: 0.10,
            away_residual_attack_log_adjustment: 0.0,
            metric_modifiers: Vec::new(),
        },
        goal_process: SpecificGoalProcess {
            goals_per_team_match: blended_rate,
            home_advantage_multiplier: 1.09,
            path_log_rate_sigma: 0.28,
            max_abs_segment_log_adjustment: 1.2,
            segments: 6,
        },
        matchup_spatial_sigma: 0.25,
        max_player_rate_log_adjustment: 1.0,
    };
    assert!(input.validate(), "calibrated MLS input failed validation for {} vs {}", home.name, away.name);
    input
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut paths = 300_u32;
    let mut seed = 42_u64;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--paths" => paths = iter.next().unwrap().parse().unwrap(),
            "--seed" => seed = iter.next().unwrap().parse().unwrap(),
            other => panic!("unknown arg {other}"),
        }
    }

    let as_of_text = "2026-08-23";
    let as_of = date_from(as_of_text);
    let form_path = validate_live_artifacts(as_of_text).unwrap_or_else(|error| {
        panic!(
            "{error}\nregenerate causal forecast artifacts with:\n  python3 scripts/build_player_aggregates.py\n  python3 scripts/join_fm_attributes.py\n  python3 scripts/build_selection_training.py\n  python3 scripts/build_form_series.py --live-cutoff {as_of_text}"
        )
    });
    let windows = LeagueWindows::build("data/sofascore_cfm/asof_team_stats.csv", as_of);
    let players = load_players("data/sofascore_cfm/player_fm_join.csv", 0.0, as_of);
    let form_path = Some(form_path.as_str());
    let participation_model = std::fs::read_to_string("data/sofascore_cfm/selection_training.tsv")
        .ok()
        .and_then(|text| form::learned_participation_model_from_tsv(&text, as_of));
    println!(
        "MLS pipeline: {} qualified players, {} team windows, participation={} (as-of 2026-08-23)",
        players.len(),
        windows.teams.len(),
        participation_model.as_ref().map_or("fallback".to_string(), |model| format!("learned:{}", model.observations()))
    );

    let fixtures: [(u64, &str, &str, &str, &str); 4] = [
        (15168339, "Inter Miami CF", "Inter Miami CF", "CF Montréal", "CF Montréal"),
        (15168358, "Philadelphia Union", "Philadelphia Union", "CF Montréal", "CF Montréal"),
        (16851667, "Vancouver Whitecaps (CanChamp)", "Vancouver Whitecaps", "CF Montréal", "CF Montréal"),
        (15168368, "CF Montréal", "CF Montréal", "Charlotte FC", "Charlotte FC"),
    ];

    for (index, (mid, home_label, home_stats, away_label, away_stats)) in fixtures.iter().enumerate() {
        let home = windows.team(home_stats);
        let away = windows.team(away_stats);
        println!("\n=== Fixture {home_label} vs {away_label} (history: {} and {} matches) ===",
            home.matches, away.matches);

        let input = build_input(
            MatchId(*mid),
            &home,
            &away,
            &players,
            participation_model.as_ref(),
            form_path,
        );
        let home_ids: Vec<PlayerId> = input.home_lineup.candidates.iter().map(|c| c.player).collect();
        let away_ids: Vec<PlayerId> = input.away_lineup.candidates.iter().map(|c| c.player).collect();
        let lookup = |id: PlayerId| players.iter().find(|p| PlayerId(p.id) == id);
        let dirtiness_of = |id: PlayerId| lookup(id).and_then(|p| p.fm_dirtiness);
        let consistency_of = |id: PlayerId| lookup(id).and_then(|p| p.fm_consistency);
        let mental = mental_model(&home, &away, &home_ids, &away_ids, &dirtiness_of, &consistency_of);

        let Some(forecast) = simulate_second_resolved_dynamic_mental_match(
            &input,
            &mental,
            DynamicMentalTransitionParameters::default(),
            SecondResolvedSimulationParameters::default(),
            paths,
            seed + index as u64,
        ) else {
            println!("simulation rejected inputs");
            continue;
        };

        let outcome = &forecast.outcome;
        println!(
            "P(home)={:.3} P(draw)={:.3} P(away)={:.3} | goals {:.2}-{:.2} | xPts {:.2}-{:.2}",
            outcome.home_win.get(), outcome.draw.get(), outcome.away_win.get(),
            outcome.home_goals.mean, outcome.away_goals.mean,
            outcome.home_expected_points, outcome.away_expected_points
        );
        println!(
            "engine: {} paths, {:.1} state events/path, collision prob {:.3}",
            forecast.paths, forecast.mean_state_events_per_path, forecast.same_second_collision_probability.get()
        );
        match write_player_forecast(*mid, &forecast, &players) {
            Ok(path) => println!("player forecast: {path}"),
            Err(error) => eprintln!("player forecast write failed for {mid}: {error}"),
        }
    }
}

/// Premier League driver: reuses the MLS production *player-level* primitives
/// and the actual second-resolved simulation engine, with dated official
/// registered squads and their FM26 attributes. No current lineup confirmed.
pub(crate) fn run_premier_league() {
    use serde_json::{json, Value};
    use std::path::Path;
    let mut paths = 120_u32;
    let mut seed = 42_u64;
    let mut only: Option<String> = None;
    let mut output_path = "artifacts/premier-league-2026-10-11/player_paths.json".to_string();
    let mut as_of_date = "2026-10-11".to_string();
    let mut results_path = "data/pl/asof_team_stats.csv".to_string();
    let mut players_path = "data/pl/player_fm_join.csv".to_string();
    let mut baseline_path = "data/pl/forecasts.json".to_string();
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--paths" => paths = iter.next().expect("--paths value").parse().expect("valid paths"),
            "--seed" => seed = iter.next().expect("--seed value").parse().expect("valid seed"),
            "--only" => only = Some(iter.next().expect("--only team substring")),
            "--output" => output_path = iter.next().expect("--output destination"),
            "--as-of" => as_of_date = iter.next().expect("--as-of value"),
            "--results" => results_path = iter.next().expect("--results input"),
            "--players" => players_path = iter.next().expect("--players input"),
            "--baseline" => baseline_path = iter.next().expect("--baseline JSON"),
            _ => panic!("unknown argument: {flag}"),
        }
    }
    assert!(paths > 0 && paths <= 10000);
    // --as-of is the match forecast date: only earlier results are used.
    let as_of = date_from(&as_of_date);
    let windows = LeagueWindows::build(&results_path, as_of);
    let players = load_players(&players_path, 0.0, as_of);
    let baseline: Value = serde_json::from_str(&std::fs::read_to_string(&baseline_path).expect("read baseline"))
        .expect("parse baseline");
    let mut out = Vec::new();
    for (idx, fixture) in baseline["fixtures"].as_array().expect("fixtures").iter().enumerate() {
        let home_name = fixture["home"].as_str().expect("home");
        let away_name = fixture["away"].as_str().expect("away");
        if only.as_ref().is_some_and(|filter| !home_name.contains(filter) && !away_name.contains(filter)) {
            continue;
        }
        let home = windows.team(home_name);
        let away = windows.team(away_name);
        let home_count = players.iter().filter(|p| p.team == home_name).count();
        let away_count = players.iter().filter(|p| p.team == away_name).count();
        assert!(home_count >= 14 && away_count >= 14,
            "insufficient matched FM26 players {home_name}={home_count} {away_name}={away_count}");
        let mut input = build_input(MatchId(1_000_000 + idx as u64), &home, &away, &players, None, None);
        // The existing result model contributes a pre-match intensity anchor,
        // not the W/D/L result; the engine still simulates goals through the
        // player/team event graph and game-state feedback for every path.
        let hg = fixture["home_expected_goals"].as_f64().expect("home target");
        let ag = fixture["away_expected_goals"].as_f64().expect("away target");
        let reference = ((hg + ag) / 2.0).clamp(0.6, 2.4);
        input.goal_process.goals_per_team_match = reference;
        input.goal_process.home_advantage_multiplier = 1.0;
        input.goal_process.path_log_rate_sigma = 0.20;
        input.environment.home_residual_attack_log_adjustment = (hg / reference).ln().clamp(-0.35, 0.35);
        input.environment.away_residual_attack_log_adjustment = (ag / reference).ln().clamp(-0.35, 0.35);
        let home_ids: Vec<_> = input.home_lineup.candidates.iter().map(|p| p.player).collect();
        let away_ids: Vec<_> = input.away_lineup.candidates.iter().map(|p| p.player).collect();
        let lookup = |id: PlayerId| players.iter().find(|p| p.id == id.0);
        let mental = mental_model(&home, &away, &home_ids, &away_ids,
            &|id| lookup(id).and_then(|p| p.fm_dirtiness),
            &|id| lookup(id).and_then(|p| p.fm_consistency));
        assert!(input.validate(),"invalid player input {home_name} vs {away_name}");
        let prediction = simulate_second_resolved_dynamic_mental_match(&input, &mental,
            DynamicMentalTransitionParameters::default(),
            SecondResolvedSimulationParameters::default(), paths, seed + idx as u64)
            .unwrap_or_else(||panic!("simulation rejected {home_name} vs {away_name}"));
        let event_windows: Vec<_> = prediction.windows_15min.iter().map(|w| json!({
            "minute_start":w.start_minute,"minute_end":w.end_minute,
            "p_any_goal":w.probability_any_goal,"p_home_goal":w.probability_home_goal,
            "p_away_goal":w.probability_away_goal,"p_red_card":w.probability_red_card,
            "p_substitution":w.probability_substitution,
            "expected_home_goals":w.expected_home_goals,"expected_away_goals":w.expected_away_goals,
            "expected_home_shots":w.expected_home_shots,"expected_away_shots":w.expected_away_shots,
            "end_of_window_outcomes":[w.end_home_lead,w.end_draw,w.end_away_lead],
        })).collect();
        let player_info: Vec<_> = prediction.player_metrics.iter().filter(|p|p.metric.0 == M_SHOT)
            .map(|p|json!({"uid":p.player.0,"name":lookup(p.player).map_or("unknown",|x|x.name.as_str()),
                "start_probability":p.start_probability.get(),"appearance_probability":p.appearance_probability.get(),
                "expected_minutes":p.expected_minutes,"expected_shots":p.unconditional_total.mean})).collect();
        let o=&prediction.outcome;
        println!("{home_name} vs {away_name}: {} paths | H {:.3} D {:.3} A {:.3} | goals {:.2}-{:.2}",
            prediction.paths,o.home_win.get(),o.draw.get(),o.away_win.get(),o.home_goals.mean,o.away_goals.mean);
        out.push(json!({"date":fixture["date"],"home":home_name,"away":away_name,
            "paths":prediction.paths,"seed":seed+idx as u64,
            "registered_fm_players":{"home":home_count,"away":away_count},
            "home_win":o.home_win.get(),"draw":o.draw.get(),"away_win":o.away_win.get(),
            "expected_home_goals":o.home_goals.mean,"expected_away_goals":o.away_goals.mean,
            "match_state_events_per_path":prediction.mean_state_events_per_path,
            "same_second_collision_probability":prediction.same_second_collision_probability.get(),
            "15_minute_windows":event_windows,"players":player_info,
            "comparison_results_fm_model":{"home_win":fixture["home_win"],
                "draw":fixture["draw"],"away_win":fixture["away_win"]}}));
    }
    let artifact=json!({"as_of":"2026-10-10","model":"simulate_second_resolved_dynamic_mental_match",
        "status":"exploratory player-level pre-match scenario; not a statistically validated EPL event forecast",
        "roster":"2026/27 registered squad list + matched FM26 attributes",
        "caveats":["No confirmed lineups or 2026/27 player minutes; starts are random with CA/exposure priors",
            "FM-derived event processes have not been validated on EPL player events",
            "Player goal process anchored to independently refitted results+FM benchmark",
            "P(any shot) per window is simulated player-event process; goals/red cards/subs are path-observed"],
        "fixtures":out});
    let path=Path::new(&output_path);
    std::fs::create_dir_all(path.parent().unwrap()).expect("create artifact directory");
    std::fs::write(path, serde_json::to_string_pretty(&artifact).unwrap()+"\n").expect("write paths");
}
