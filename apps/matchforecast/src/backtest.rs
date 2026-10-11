#![forbid(unsafe_code)]
//! Walk-forward backtest of the MLS forecast pipeline on past CF Montréal fixtures.
//!
//! For every fixture: rebuild team windows strictly before the match date, load the
//! pre-computed as-of player aggregates (scripts/prep_backtest.py), simulate, and score
//! against the real result. Outputs Brier score, log loss, accuracy, calibration table.

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
    OutcomeCalibrationSample, OutcomeCalibrator, PairwiseSynergy, PlayerAttributeValue,
    PlayerAttributeState, PlayerMatchupState,
    PlayerMentalState, PrimitiveMatchSimulationInput, RoleFit,
    SecondResolvedSimulationParameters, SpecificGoalProcess,
    SpecificInMatchParameters, SpecificMatchEnvironment, SpecificPlayerEventModel,
    SpecificScoreStateMetricModifier, SpecificTeamContext,
    StablePsychologyState, TacticalSlot, TeamMentalState, TeamProcessChannel,
};
use std::collections::HashMap;

const M_SHOT: u16 = 1;
const M_XG: u16 = 2;
const M_XA: u16 = 3;
const M_PASS: u16 = 4;
const M_PROGC: u16 = 5;
const A_FINISH: u16 = 1;
const A_TECH: u16 = 2;
const A_DEFEND: u16 = 3;

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

fn read_csv(path: &str) -> Vec<Row> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"));
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
    row.get(key).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

fn date_from(text: &str) -> Date {
    let parts: Vec<&str> = text.split('-').collect();
    Date::from_ymd(parts[0].parse().unwrap(), parts[1].parse().unwrap(), parts[2].parse().unwrap()).unwrap()
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

#[derive(Clone)]
struct TeamData {
    gf_pm: f64,
    ga_pm: f64,
    xg_pm: f64,
    xga_pm: f64,
    shots_pm: f64,
    shots_face_pm: f64,
    sot_pm: f64,
    sot_face_pm: f64,
}

struct PlayerRow {
    id: u64,
    team: String,
    minutes: f64,
    selection_listings: f64,
    selection_appearances: f64,
    selection_starts: f64,
    selection_minutes: f64,
    selection_start_minutes: f64,
    team_fixtures: f64,
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

fn load_players(path: &str, min_minutes: f64) -> Vec<PlayerRow> {
    read_csv(path)
        .into_iter()
        .map(|row| PlayerRow {
            id: f(&row, "player_id") as u64,
            team: row.get("team").cloned().unwrap_or_default(),
            minutes: f(&row, "minutes"),
            selection_listings: f(&row, "selection_listings"),
            selection_appearances: f(&row, "selection_appearances"),
            selection_starts: f(&row, "selection_starts"),
            selection_minutes: f(&row, "selection_minutes"),
            selection_start_minutes: f(&row, "selection_start_minutes"),
            team_fixtures: f(&row, "team_fixtures"),
            days_since_last_appearance: f(&row, "days_since_last_appearance"),
            days_since_last_listed: f(&row, "days_since_last_listed"),
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
        })
        .filter(|row| row.minutes >= min_minutes && !row.team.is_empty())
        .collect()
}

/// Team windows rebuilt for a specific as-of cutoff.
fn team_window(all_rows: &[Row], name: &str, cutoff: Date) -> TeamData {
    let rows: Vec<&Row> = all_rows
        .iter()
        .filter(|row| date_from(&row["date"]) < cutoff && (row["home"] == name || row["away"] == name))
        .collect();
    if rows.is_empty() {
        return TeamData {
            gf_pm: 1.40,
            ga_pm: 1.40,
            xg_pm: 1.40,
            xga_pm: 1.40,
            shots_pm: 12.0,
            shots_face_pm: 12.0,
            sot_pm: 4.5,
            sot_face_pm: 4.5,
        };
    }
    // Team state should move as teams actually change. A flat season-to-date mean lets stale early
    // fixtures dominate late-season forecasts, so use a causal 60-day EWMA and shrink sparse
    // effective samples toward league priors. Two prior-match equivalents keeps the first few
    // fixtures from producing extreme attack/defence estimates.
    let ewma = |pick: &dyn Fn(&Row) -> f64, prior: f64| -> f64 {
        let half_life_days = 60.0;
        let prior_strength = 2.0;
        let mut numerator = prior * prior_strength;
        let mut denominator = prior_strength;
        for row in &rows {
            let age_days = cutoff.days_since(date_from(&row["date"])).max(1) as f64;
            let weight = (-std::f64::consts::LN_2 * age_days / half_life_days).exp();
            let value = pick(row);
            if value.is_finite() && value >= 0.0 {
                numerator += weight * value;
                denominator += weight;
            }
        }
        numerator / denominator.max(f64::EPSILON)
    };
    TeamData {
        gf_pm: ewma(&|row| if row["home"] == name { f(row, "home_goals") } else { f(row, "away_goals") }, 1.40),
        ga_pm: ewma(&|row| if row["home"] == name { f(row, "away_goals") } else { f(row, "home_goals") }, 1.40),
        xg_pm: ewma(&|row| if row["home"] == name { f(row, "home_xg") } else { f(row, "away_xg") }, 1.40),
        // xGA: opponent's xG in the same fixture (same row).
        xga_pm: ewma(&|row| if row["home"] == name { f(row, "away_xg") } else { f(row, "home_xg") }, 1.40),
        shots_pm: ewma(&|row| {
            if row["home"] == name { f(row, "home_shots") } else { f(row, "away_shots") }
        }, 12.0),
        shots_face_pm: ewma(&|row| {
            if row["home"] == name { f(row, "away_shots") } else { f(row, "home_shots") }
        }, 12.0),
        sot_pm: ewma(&|row| {
            if row["home"] == name {
                f(row, "home_sot")
            } else {
                f(row, "away_sot")
            }
        }, 4.5),
        sot_face_pm: ewma(&|row| {
            if row["home"] == name {
                f(row, "away_sot")
            } else {
                f(row, "home_sot")
            }
        }, 4.5),
    }
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
                // Empirical-Bayes shrinkage prevents one or two early fixtures from creating 0/1
                // selection probabilities. These are current-club counts prepared strictly before
                // the target fixture, so transferred players do not inherit an old club's XI prior.
                let prior_strength = 2.0;
                let start = (p.selection_starts + 0.50 * prior_strength)
                    / (p.team_fixtures + prior_strength);
                let appearance = (p.selection_appearances + 0.75 * prior_strength)
                    / (p.team_fixtures + prior_strength);
                let starts = p.selection_starts;
                let minutes = if starts > 0.0 {
                    (p.selection_start_minutes + 76.0 * prior_strength) / (starts + prior_strength)
                } else {
                    76.0
                };
                (start.clamp(0.02, 0.98), appearance.max(start).clamp(0.02, 0.99), minutes.clamp(45.0, 90.0))
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
                        // The participation heads are unconditional on pre-match availability, so
                        // multiplying by a second 0.96 availability prior would double-count the
                        // same absence risk. Explicit confirmed absences can still intervene later.
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

fn event_models_for(
    players: &[PlayerRow],
    team_name: &str,
) -> Vec<SpecificPlayerEventModel> {
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
                    reference_per90: 1.25,
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
                    reference_per90: 0.12,
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
                    reference_per90: 1.5,
                    aggregate_matchup_sensitivity: 0.03,
                    attribute_matchup_sensitivity: 0.03,
                    teammate_sensitivity: 0.0,
                    role_fit_sensitivity: 0.05,
                    fatigue_sensitivity: 0.10,
                },
            ]
        })
        .collect::<Vec<_>>()
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
    // Half-shrink toward the observed team shot-volume anchor so noisy early windows do not fully
    // override the learned/player-level rate structure.
    let scale = ((target_pm / xi_sum) * 0.5 + 0.5).clamp(0.6, 2.5);
    for model in models.iter_mut().filter(|model| model.metric.0 == M_SHOT) {
        model.mean_per90 *= scale;
        model.variance_per90 *= scale;
    }
}

fn attributes_and_states_for(
    players: &[PlayerRow],
    team_name: &str,
) -> (Vec<PlayerAttributeState>, Vec<PlayerMatchupState>) {
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

fn fallback_state(player: PlayerId) -> (PlayerAttributeState, PlayerMatchupState) {
    (
        PlayerAttributeState {
            player,
            attributes: vec![
                PlayerAttributeValue { attribute: AttributeId(A_FINISH), value: 10.0 },
                PlayerAttributeValue { attribute: AttributeId(A_TECH), value: 10.0 },
                PlayerAttributeValue { attribute: AttributeId(A_DEFEND), value: 10.0 },
            ],
        },
        PlayerMatchupState {
            player,
            ability: footy_state::AbilityVector::from_components([
                10.5, 10.0, 10.5, 9.7, 9.5, 8.9, 9.5, 10.5, 10.5, 6.0,
            ]),
        },
    )
}

fn add_fallback_player(
    candidate: PlayerId,
    attribute_states: &mut Vec<PlayerAttributeState>,
    player_states: &mut Vec<PlayerMatchupState>,
    event_models: &mut Vec<SpecificPlayerEventModel>,
) {
    if player_states.iter().any(|state| state.player == candidate) {
        return;
    }
    let (attribute_state, matchup_state) = fallback_state(candidate);
    attribute_states.push(attribute_state);
    player_states.push(matchup_state);
    for (metric, family, reference) in [
        (MetricId(M_SHOT), StatFamily::Count, 1.25_f64),
        (MetricId(M_XG), StatFamily::ContinuousNonNegative, 0.15),
        (MetricId(M_XA), StatFamily::ContinuousNonNegative, 0.12),
        (MetricId(M_PASS), StatFamily::Count, 32.0),
        (MetricId(M_PROGC), StatFamily::Count, 1.5),
    ] {
        event_models.push(SpecificPlayerEventModel {
            player: candidate,
            metric,
            family,
            mean_per90: reference,
            variance_per90: reference * 2.0,
            epistemic_sigma: 0.15,
            reference_per90: reference,
            aggregate_matchup_sensitivity: 0.03,
            attribute_matchup_sensitivity: 0.02,
            teammate_sensitivity: 0.0,
            role_fit_sensitivity: 0.05,
            fatigue_sensitivity: 0.08,
        });
    }
}

fn input_home_fallback(lineup: &LineupProjectionInput) -> Vec<PlayerId> {
    lineup
        .candidates
        .iter()
        .filter(|candidate| candidate.player.0 >= 800000 || (candidate.player.0 % 1000000) >= 800000 && candidate.player.0 > 100000000)
        .map(|candidate| candidate.player)
        .collect()
}

fn build_input(
    fixture_mid: u64,
    match_id: MatchId,
    home_name: &str,
    away_name: &str,
    home: &TeamData,
    away: &TeamData,
    players: &[PlayerRow],
    participation_model: Option<&NextMatchParticipationModel>,
    use_player_learning: bool,
    use_legacy_team_attack_residual: bool,
) -> PrimitiveMatchSimulationInput {
    let home_club = ClubId(stable(home_name));
    let away_club = ClubId(stable(away_name));
    let home_lineup = LineupProjectionInput {
        club: home_club,
        formations: vec![
            FormationPrior { formation: formation(442), probability: 0.55 },
            FormationPrior { formation: formation(433), probability: 0.45 },
        ],
        candidates: squad(players, home_name, home_club, participation_model),
        locked_starters: Vec::new(),
        parameters: LineupSamplingParameters::default(),
    };
    let away_lineup = LineupProjectionInput {
        club: away_club,
        formations: vec![
            FormationPrior { formation: formation(442), probability: 0.55 },
            FormationPrior { formation: formation(433), probability: 0.45 },
        ],
        candidates: squad(players, away_name, away_club, participation_model),
        locked_starters: Vec::new(),
        parameters: LineupSamplingParameters::default(),
    };

    let (home_attributes, home_states) = attributes_and_states_for(players, home_name);
    let (away_attributes, away_states) = attributes_and_states_for(players, away_name);
    let mut attribute_states = home_attributes;
    attribute_states.extend(away_attributes);
    let mut player_states = home_states;
    player_states.extend(away_states);

    // Player `shots` and the team anchor are now the same statistic: total shots. This used to use
    // a mislabelled SofaScore field plus an ad-hoc +2 offset, which distorted both player forecasts
    // and the downstream shot-volume channel.
    let home_anchor = home.shots_pm.max(4.0);
    let away_anchor = away.shots_pm.max(4.0);
    let mut home_models = event_models_for(players, home_name);
    let mut away_models = event_models_for(players, away_name);

    // Learned next-match rates are primary when enough causal training examples exist. The HMM is
    // retained as a fallback rather than multiplied on top of the learned rate, which would count
    // recent form twice. A conservative blend protects sparse/noisy histories from large jumps.
    if let Ok(form_text) = std::fs::read_to_string(format!("data/sofascore_cfm/bk_form/{fixture_mid}_form.csv")) {
        let regimes = form::regimes_from_csv(&form_text);
        let learned = if use_player_learning {
            form::learned_rate_forecasts_from_csv(&form_text)
        } else {
            HashMap::new()
        };
        let mut learned_applied = 0usize;
        let mut regime_applied = 0usize;
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
                learned_applied += 1;
            } else if let Some(regime) = regimes.get(&model.player.0) {
                let multiplier = regime.mean_multiplier();
                if (multiplier - 1.0).abs() > 1e-6 {
                    model.mean_per90 *= multiplier;
                    regime_applied += 1;
                }
            }
        }
        eprintln!(
            "player rates: {} learned player-metrics, {} HMM fallbacks ({} regimes fitted)",
            learned_applied,
            regime_applied,
            regimes.len()
        );
    }

    // Re-anchor shot volume only after player-rate learning so the fixture-level team aggregate and
    // the player distribution remain coherent. xA/carries are deliberately not tied to shot volume.
    anchor_shot_volume(&mut home_models, home_anchor);
    anchor_shot_volume(&mut away_models, away_anchor);
    let mut event_models = home_models;
    event_models.extend(away_models);

    for candidate in input_home_fallback(&home_lineup) {
        add_fallback_player(candidate, &mut attribute_states, &mut player_states, &mut event_models);
    }
    for candidate in input_home_fallback(&away_lineup) {
        add_fallback_player(candidate, &mut attribute_states, &mut player_states, &mut event_models);
    }

    // FIX 1: anchor the base goal rate to the two teams' actual scoring environment instead of
    // a league constant. Average attack/defense rates of the four relevant windows.
    let env_rate =
        ((home.gf_pm + home.ga_pm + away.gf_pm + away.ga_pm) / 4.0).clamp(0.9, 2.2);
    // Blend with observed SOT-derived expectation: goals ~ SOT * conversion (~0.30).
    let sot_rate = ((home.sot_pm * 0.30 + away.sot_face_pm * 0.30)
        + (away.sot_pm * 0.30 + home.sot_face_pm * 0.30))
        / 4.0;
    let blended_rate = (env_rate * 0.6 + sot_rate.clamp(0.9, 2.4) * 0.4).clamp(1.05, 2.1);

    let team_context = |club: ClubId, team: &TeamData| SpecificTeamContext {
        club,
        // Player xG is now an explicit ShotQuality -> Finishing cause. Reusing raw team xG as a
        // residual attack term counts the same attacking strength twice. Keep the old path only as
        // an ablation so the backtest can quantify the change directly.
        residual_attack_log_adjustment: if use_legacy_team_attack_residual {
            ((team.xg_pm - 1.40) / 1.40 * 0.35).clamp(-0.35, 0.35)
        } else {
            0.0
        },
        // The simulator subtracts this term from the opponent's attack log-rate, so positive means
        // defensive suppression. Lower xGA must therefore map to a positive adjustment.
        residual_defense_log_adjustment: ((1.40 - team.xga_pm) / 1.40 * 0.35).clamp(-0.35, 0.35),
        metric_modifiers: Vec::new(),
    };

    let input = PrimitiveMatchSimulationInput {
        match_id,
        home: home_club,
        away: away_club,
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
            footy_match::MatchupRule {
                metric: Some(MetricId(M_SHOT)),
                attacker_dimension: footy_match::AbilityDimension::Finishing,
                defender_dimension: footy_match::AbilityDimension::Defending,
                weight: 1.0,
                scale: 6.0,
            },
        ],
        attribute_matchup_rules: vec![AttributeMatchupRule {
            metric: Some(MetricId(M_SHOT)),
            attacker_attribute: AttributeId(A_FINISH),
            defender_attribute: AttributeId(A_DEFEND),
            weight: 1.0,
            scale: 6.0,
        }],
        synergies: Vec::<PairwiseSynergy>::new(),
        channel_graph: channel_graph(),
        score_state_modifiers: vec![SpecificScoreStateMetricModifier {
            metric: MetricId(M_SHOT),
            trailing_multiplier: 1.08,
            leading_multiplier: 0.94,
        }],
        incident_profiles: Vec::new(),
        in_match: SpecificInMatchParameters {
            injury_probability_per90: 0.02,
            red_card_probability_per90: 0.02,
            max_substitutions: 5,
            minimum_tactical_substitution_minute: 58.0,
        },
        home_context: team_context(home_club, home),
        away_context: team_context(away_club, away),
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
    if !input.validate() {
        let hl_ok = input.home_lineup.validate();
        let al_ok = input.away_lineup.validate();
        eprintln!("VALIDATION FAIL {home_name} vs {away_name}: home_lineup={hl_ok} away_lineup={al_ok}");
        eprintln!("  home candidates: {}", input.home_lineup.candidates.len());
        eprintln!("  away candidates: {}", input.away_lineup.candidates.len());
        eprintln!("  event models: {} attribute states: {} player states: {}",
            input.event_models.len(), input.attribute_states.len(), input.player_states.len());
        let state_ids: std::collections::BTreeSet<u64> = input.player_states.iter().map(|value| value.player.0).collect();
        for candidate in input.home_lineup.candidates.iter().chain(input.away_lineup.candidates.iter()) {
            if !state_ids.contains(&candidate.player.0) {
                eprintln!("  missing state for player {}", candidate.player.0);
            }
        }
        let mut bad_models = 0;
        for model in &input.event_models {
            let ok = model.mean_per90.is_finite()
                && model.mean_per90 > 0.0
                && model.variance_per90.is_finite()
                && model.variance_per90 >= 0.0
                && model.epistemic_sigma.is_finite()
                && model.epistemic_sigma >= 0.0
                && model.reference_per90.is_finite()
                && model.reference_per90 > 0.0;
            if !ok {
                if bad_models < 6 {
                    eprintln!("  invalid event model: player={} metric={} mean={} var={}",
                        model.player.0, model.metric.0, model.mean_per90, model.variance_per90);
                }
                bad_models += 1;
            }
        }
        // duplicate (player, metric) pairs also fail validation
        let mut keys = std::collections::BTreeSet::new();
        for model in &input.event_models {
            if !keys.insert((model.player, model.metric)) {
                eprintln!("  duplicate event model: player={} metric={}", model.player.0, model.metric.0);
            }
        }
        panic!("input validation failed for {home_name} vs {away_name}");
    }
    input
}

fn mental_model(input: &PrimitiveMatchSimulationInput, players: &[PlayerRow]) -> MentalMatchModel {
    let measure =
        |value: f64| MentalMeasure::new(value, Confidence::new(0.7).unwrap_or(Confidence::ONE)).expect("measure");
    let lookup = |id: PlayerId| players.iter().find(|p| PlayerId(p.id) == id);
    let squad_mental = |candidates: &Vec<footy_match::LineupCandidate>| -> Vec<PlayerMentalState> {
        candidates
            .iter()
            .map(|candidate| {
                let consistency = lookup(candidate.player).and_then(|p| p.fm_consistency);
                let discipline = lookup(candidate.player).and_then(|p| p.fm_dirtiness);
                PlayerMentalState {
                    player: candidate.player,
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
    let form_bias = |team: &TeamData| ((team.gf_pm - team.ga_pm) * 0.06).clamp(-0.25, 0.25);
    // NOTE: form bias uses the same windows passed via input contexts; approximate here by 0
    // because team-level psychology enters through team_context residuals already.
    let _ = form_bias;
    MentalMatchModel {
        players: [
            squad_mental(&input.home_lineup.candidates),
            squad_mental(&input.away_lineup.candidates),
        ]
        .concat(),
        home_team: TeamMentalState {
            club: input.home,
            collective_morale: measure(0.0),
            collective_belief: measure(0.0),
            cohesion: measure(0.25),
            leadership: measure(0.25),
        },
        away_team: TeamMentalState {
            club: input.away,
            collective_morale: measure(0.0),
            collective_belief: measure(0.0),
            cohesion: measure(0.25),
            leadership: measure(0.25),
        },
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
        context: MatchMentalContext {
            pressure_intensity: 0.6,
            environmental_novelty: 0.15,
            hostility: 0.25,
        },
        parameters: MentalModelParameters {
            discipline_red_card_log_weight: -0.35,
            low_morale_red_card_log_weight: 0.25,
            ..MentalModelParameters::default()
        },
    }
}

struct FixtureMeta {
    mid: u64,
    date: Date,
    home: String,
    away: String,
    home_goals: f64,
    away_goals: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct ActualPlayerPerformance {
    minutes: f64,
    started: bool,
    shots: f64,
    xg: f64,
    xa: f64,
    passes: f64,
    progressive_carries: f64,
}

fn load_actual_player_performance(path: &str) -> Option<HashMap<u64, ActualPlayerPerformance>> {
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let mut players = HashMap::new();
    for side in ["home", "away"] {
        let rows = value.get(side)?.get("players")?.as_array()?;
        for row in rows {
            let Some(player_id) = row.get("player").and_then(|player| player.get("id")).and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let substitute = row.get("substitute").and_then(serde_json::Value::as_bool).unwrap_or(true);
            let statistics = row.get("statistics");
            let number = |key: &str| -> f64 {
                statistics
                    .and_then(|stats| stats.get(key))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0)
            };
            let minutes = number("minutesPlayed");
            players.insert(
                player_id,
                ActualPlayerPerformance {
                    minutes,
                    started: !substitute && minutes > 0.0,
                    shots: number("totalShots"),
                    xg: number("expectedGoals"),
                    xa: number("expectedAssists"),
                    passes: number("totalPass"),
                    progressive_carries: number("progressiveBallCarriesCount"),
                },
            );
        }
    }
    Some(players)
}

#[derive(Debug, Clone, Copy, Default)]
struct ErrorSummary {
    n: usize,
    signed_error: f64,
    absolute_error: f64,
    squared_error: f64,
}

impl ErrorSummary {
    fn add(&mut self, prediction: f64, actual: f64) {
        if !prediction.is_finite() || !actual.is_finite() {
            return;
        }
        let error = prediction - actual;
        self.n += 1;
        self.signed_error += error;
        self.absolute_error += error.abs();
        self.squared_error += error * error;
    }

    fn bias(self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.signed_error / self.n as f64 }
    }

    fn mae(self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.absolute_error / self.n as f64 }
    }

    fn rmse(self) -> f64 {
        if self.n == 0 { f64::NAN } else { (self.squared_error / self.n as f64).sqrt() }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct IntervalCoverageSummary {
    n: usize,
    covered: usize,
    total_width: f64,
}

impl IntervalCoverageSummary {
    fn add(&mut self, lower: f64, upper: f64, actual: f64) {
        if !lower.is_finite() || !upper.is_finite() || !actual.is_finite() || lower > upper {
            return;
        }
        self.n += 1;
        self.covered += usize::from(actual >= lower && actual <= upper);
        self.total_width += upper - lower;
    }

    fn coverage(self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.covered as f64 / self.n as f64 }
    }

    fn mean_width(self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.total_width / self.n as f64 }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ProbabilityErrorSummary {
    n: usize,
    brier: f64,
}

impl ProbabilityErrorSummary {
    fn add(&mut self, probability: f64, actual: bool) {
        if !probability.is_finite() {
            return;
        }
        let target = if actual { 1.0 } else { 0.0 };
        self.n += 1;
        self.brier += (probability - target).powi(2);
    }

    fn mean_brier(self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.brier / self.n as f64 }
    }
}

fn actual_metric(actual: ActualPlayerPerformance, metric: u16) -> Option<f64> {
    match metric {
        M_SHOT => Some(actual.shots),
        M_XG => Some(actual.xg),
        M_XA => Some(actual.xa),
        M_PASS => Some(actual.passes),
        M_PROGC => Some(actual.progressive_carries),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut paths = 300_u32;
    let mut seed = 42_u64;
    let mut use_player_learning = true;
    let mut use_participation_learning = true;
    let mut use_legacy_team_attack_residual = false;
    let mut use_result_calibration = true;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--paths" => paths = iter.next().unwrap().parse().unwrap(),
            "--seed" => seed = iter.next().unwrap().parse().unwrap(),
            "--no-player-learning" => use_player_learning = false,
            "--no-participation-learning" => use_participation_learning = false,
            "--legacy-team-attack-residual" => use_legacy_team_attack_residual = true,
            "--no-result-calibration" => use_result_calibration = false,
            other => panic!("unknown arg {other}"),
        }
    }

    println!(
        "backtest config: paths={paths} seed={seed} player_learning={use_player_learning} participation_learning={use_participation_learning} legacy_team_attack_residual={use_legacy_team_attack_residual} result_calibration={use_result_calibration}"
    );

    let meta_json: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string("data/sofascore_cfm/bk/fixtures.json").unwrap())
            .unwrap();
    let fixtures: Vec<FixtureMeta> = meta_json
        .iter()
        .map(|value| FixtureMeta {
            mid: value["match_id"].as_str().unwrap().parse().unwrap(),
            date: date_from(value["date"].as_str().unwrap()),
            home: value["home"].as_str().unwrap().to_string(),
            away: value["away"].as_str().unwrap().to_string(),
            home_goals: value["home_goals"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
            away_goals: value["away_goals"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        })
        .collect();

    let all_rows = read_csv("data/sofascore_cfm/asof_team_stats.csv");
    let selection_training = std::fs::read_to_string("data/sofascore_cfm/selection_training.tsv").ok();
    if use_participation_learning {
        require_delimited_columns(
            "data/sofascore_cfm/selection_training.tsv",
            '\t',
            &[
                "date", "club_fixtures", "prior_listings", "prior_appearances", "prior_starts", "prior_minutes",
                "prior_start_minutes", "days_since_last_appearance", "days_since_last_listed",
                "appeared", "started", "minutes",
            ],
        ).unwrap_or_else(|error| panic!("{error}; run python3 scripts/build_selection_training.py"));
    }

    println!("{:<26} {:>7} {:>8} {:>8} | {:>6} {:>6} | result", "fixture", "P(H)", "P(D)", "P(A)", "xG", "xGA");
    let mut outcomes = Vec::new(); // calibrated (ph, pd, pa, actual)
    let mut brier_sum = 0.0;
    let mut log_loss_sum = 0.0;
    let mut raw_brier_sum = 0.0;
    let mut raw_log_loss_sum = 0.0;
    let mut calibration_history = Vec::<OutcomeCalibrationSample>::new();
    let mut player_errors = HashMap::<u16, ErrorSummary>::new();
    let mut player_intervals = HashMap::<u16, IntervalCoverageSummary>::new();
    let mut minutes_errors = ErrorSummary::default();
    let mut goal_errors = ErrorSummary::default();
    let mut goal_intervals = IntervalCoverageSummary::default();
    let mut appearance_errors = ProbabilityErrorSummary::default();
    let mut start_errors = ProbabilityErrorSummary::default();
    let mut skipped_roster_coverage = 0usize;

    for fixture in &fixtures {
        let players_path = format!("data/sofascore_cfm/bk/{}_players.csv", fixture.mid);
        if !std::path::Path::new(&players_path).exists() {
            eprintln!("missing {players_path}, skipping");
            continue;
        }
        require_delimited_columns(
            &players_path,
            ',',
            &[
                "player_id", "team", "minutes", "selection_listings", "selection_appearances",
                "selection_starts", "selection_minutes", "selection_start_minutes", "team_fixtures",
                "days_since_last_appearance", "days_since_last_listed", "shots_p90", "xg_p90", "xa_p90", "passes_p90",
                "prog_carries_p90", "fm_ca", "fm_finishing", "fm_technique", "fm_tackling",
                "fm_marking", "fm_consistency", "fm_dirtiness",
            ],
        ).unwrap_or_else(|error| panic!("{error}; run python3 scripts/prep_backtest.py"));
        if use_player_learning {
            let form_path = format!("data/sofascore_cfm/bk_form/{}_form.csv", fixture.mid);
            require_delimited_columns(
                &form_path,
                ',',
                &["player_id", "date", "minutes", "shots", "xg", "xa", "passes", "prog_carries"],
            ).unwrap_or_else(|error| panic!("{error}; run python3 scripts/build_form_series.py"));
        }
        // Matchday listing is sufficient squad-membership evidence. A player can therefore enter
        // with zero prior minutes; event-rate shrinkage maps that cold start to league priors while
        // the participation head learns whether he is likely to be used.
        let players = load_players(&players_path, 0.0);
        let home_player_count = players.iter().filter(|player| player.team == fixture.home).count();
        let away_player_count = players.iter().filter(|player| player.team == fixture.away).count();
        if home_player_count < 11 || away_player_count < 11 {
            skipped_roster_coverage += 1;
            eprintln!(
                "insufficient causal roster coverage for {}: {}={} players, {}={} players; skipping",
                fixture.mid,
                fixture.home,
                home_player_count,
                fixture.away,
                away_player_count,
            );
            continue;
        }
        let home = team_window(&all_rows, &fixture.home, fixture.date);
        let away = team_window(&all_rows, &fixture.away, fixture.date);
        let participation_model = if use_participation_learning {
            selection_training
                .as_deref()
                .and_then(|text| form::learned_participation_model_from_tsv(text, fixture.date))
        } else {
            None
        };
        if let Some(model) = &participation_model {
            eprintln!("participation model: {} causal examples", model.observations());
        }

        let input = build_input(
            fixture.mid,
            MatchId(fixture.mid),
            &fixture.home,
            &fixture.away,
            &home,
            &away,
            &players,
            participation_model.as_ref(),
            use_player_learning,
            use_legacy_team_attack_residual,
        );
        let mental = mental_model(&input, &players);

        let Some(forecast) = simulate_second_resolved_dynamic_mental_match(
            &input,
            &mental,
            DynamicMentalTransitionParameters::default(),
            SecondResolvedSimulationParameters::default(),
            paths,
            seed.wrapping_add(fixture.mid),
        ) else {
            eprintln!("simulation failed for {}", fixture.mid);
            continue;
        };

        let outcome = &forecast.outcome;
        let raw_probs = [
            f64::from(outcome.home_win.get()),
            f64::from(outcome.draw.get()),
            f64::from(outcome.away_win.get()),
        ];
        let actual = if fixture.home_goals > fixture.away_goals {
            0
        } else if fixture.home_goals == fixture.away_goals {
            1
        } else {
            2
        };
        // Fit on prior out-of-sample forecasts only. Six observations is intentionally the minimum:
        // before that, two calibration parameters are too weakly identified to trust.
        let calibrator = if use_result_calibration && calibration_history.len() >= 6 {
            OutcomeCalibrator::fit(&calibration_history).unwrap_or_default()
        } else {
            OutcomeCalibrator::default()
        };
        let probs = calibrator.apply(raw_probs).unwrap_or(raw_probs);
        let (ph, pd, pa) = (probs[0], probs[1], probs[2]);
        let one_hot = [actual == 0, actual == 1, actual == 2];
        let brier: f64 = probs.iter().zip(one_hot).map(|(prob, truth)| (prob - u8::from(truth) as f64).powi(2)).sum();
        let ll = -probs
            .iter()
            .zip(one_hot)
            .filter(|(_, truth)| *truth)
            .map(|(prob, _)| prob.max(1e-9).ln())
            .sum::<f64>();
        let raw_brier: f64 = raw_probs.iter().zip(one_hot).map(|(prob, truth)| (prob - u8::from(truth) as f64).powi(2)).sum();
        let raw_ll = -raw_probs
            .iter()
            .zip(one_hot)
            .filter(|(_, truth)| *truth)
            .map(|(prob, _)| prob.max(1e-9).ln())
            .sum::<f64>();
        brier_sum += brier;
        log_loss_sum += ll;
        raw_brier_sum += raw_brier;
        raw_log_loss_sum += raw_ll;
        goal_errors.add(outcome.home_goals.mean, fixture.home_goals);
        goal_errors.add(outcome.away_goals.mean, fixture.away_goals);
        goal_intervals.add(outcome.home_goals.p10, outcome.home_goals.p90, fixture.home_goals);
        goal_intervals.add(outcome.away_goals.p10, outcome.away_goals.p90, fixture.away_goals);

        let label = ["H", "D", "A"][actual];
        println!(
            "{:<26} {:>7.3} {:>8.3} {:>8.3} | {:>6.2} vs {:<6.2} | actual {}-{} ({})",
            format!("{} vs {}", short(&fixture.home), short(&fixture.away)),
            ph,
            pd,
            pa,
            outcome.home_goals.mean,
            outcome.away_goals.mean,
            fixture.home_goals,
            fixture.away_goals,
            label,
        );
        outcomes.push((ph, pd, pa, actual));

        // Evaluate player forecasts on the exact same held-out fixture. Only players present in
        // the provider's matchday payload are scored; absent IDs are unknown rather than zeros.
        let actual_path = format!("data/sofascore_cfm/player_matches/{}.json", fixture.mid);
        if let Some(actual_players) = load_actual_player_performance(&actual_path) {
            let mut participation = HashMap::<u64, (f64, f64, f64)>::new();
            for metric in &forecast.player_metrics {
                let Some(actual_player) = actual_players.get(&metric.player.0).copied() else { continue; };
                if let Some(target) = actual_metric(actual_player, metric.metric.0) {
                    player_errors.entry(metric.metric.0).or_default().add(metric.unconditional_total.mean, target);
                    player_intervals.entry(metric.metric.0).or_default().add(
                        metric.unconditional_total.p10,
                        metric.unconditional_total.p90,
                        target,
                    );
                }
                participation.entry(metric.player.0).or_insert((
                    f64::from(metric.start_probability.get()),
                    f64::from(metric.appearance_probability.get()),
                    metric.expected_minutes,
                ));
            }
            for (player_id, (start_probability, appearance_probability, expected_minutes)) in participation {
                if let Some(actual_player) = actual_players.get(&player_id).copied() {
                    minutes_errors.add(expected_minutes, actual_player.minutes);
                    start_errors.add(start_probability, actual_player.started);
                    appearance_errors.add(appearance_probability, actual_player.minutes > 0.0);
                }
            }
        }

        // Current result becomes calibration evidence only after it has been scored.
        calibration_history.push(OutcomeCalibrationSample { probabilities: raw_probs, actual });
    }

    let n = outcomes.len() as f64;
    if n > 0.0 {
        let correct = outcomes.iter().filter(|(ph, pd, pa, actual)| {
            let pick = [(*ph, 0.0_f64), (*pd, 1.0), (*pa, 2.0)]
                .iter()
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, class)| *class as usize)
                .unwrap();
            pick == *actual
        }).count();
        // baseline: always pick most frequent class in this sample
        let mut counts = [0_usize; 3];
        for (_, _, _, actual) in &outcomes {
            counts[*actual] += 1;
        }
        let baseline = counts.iter().max().copied().unwrap_or(0);
        println!("\n=== Accuracy summary over {n} fixtures ===");
        println!("roster-coverage skips: {skipped_roster_coverage}");
        println!("top-pick accuracy : {}/{} = {:.1}%", correct, outcomes.len(), 100.0 * correct as f64 / n);
        println!("majority baseline : {:.1}% (pick the modal outcome: {} games)",
            100.0 * baseline as f64 / n, baseline);
        println!("mean Brier score  : {:.4}   (causal walk-forward calibrated; uniform = 0.667)",
            brier_sum / n);
        println!("mean log loss     : {:.4}   (causal walk-forward calibrated; uniform = 1.099)", log_loss_sum / n);
        println!("raw Brier score   : {:.4}", raw_brier_sum / n);
        println!("raw log loss      : {:.4}", raw_log_loss_sum / n);
        println!(
            "goal forecast      : n={:<4} MAE={:.3} RMSE={:.3} bias={:+.3} P10-P90 coverage={:.1}% width={:.3}",
            goal_errors.n,
            goal_errors.mae(),
            goal_errors.rmse(),
            goal_errors.bias(),
            100.0 * goal_intervals.coverage(),
            goal_intervals.mean_width(),
        );

        // calibration buckets by predicted home-win probability
        println!("\ncalibration (predicted P(H) bucket vs observed H rate):");
        let mut buckets = [[0_usize; 3]; 4]; // [bucket] = [n, predicted_sum*1000, home_actual]
        for &(ph, _pd, _pa, actual) in &outcomes {
            let bucket = ((ph * 4.0).min(3.999)) as usize;
            buckets[bucket][0] += 1;
            buckets[bucket][1] += (ph * 1000.0) as usize;
            buckets[bucket][2] += usize::from(actual == 0);
        }
        for (index, bucket) in buckets.iter().enumerate() {
            if bucket[0] == 0 {
                continue;
            }
            println!(
                "  {:.0}-{:.0}%: n={:<2} avg P(H)={:.3} observed H rate={:.3}",
                index * 25,
                (index + 1) * 25,
                bucket[0],
                bucket[1] as f64 / 1000.0 / bucket[0] as f64,
                bucket[2] as f64 / bucket[0] as f64
            );
        }

        println!("\n=== Player performance forecast errors ===");
        for (metric, label) in [
            (M_SHOT, "shots"),
            (M_XG, "xG"),
            (M_XA, "xA"),
            (M_PASS, "passes"),
            (M_PROGC, "progressive carries"),
        ] {
            if let Some(summary) = player_errors.get(&metric).copied() {
                let interval = player_intervals.get(&metric).copied().unwrap_or_default();
                println!(
                    "{label:<20}: n={:<4} MAE={:.4} RMSE={:.4} bias={:+.4} P10-P90={:.1}% width={:.4}",
                    summary.n,
                    summary.mae(),
                    summary.rmse(),
                    summary.bias(),
                    100.0 * interval.coverage(),
                    interval.mean_width(),
                );
            }
        }
        println!("expected minutes    : n={:<4} MAE={:.3} RMSE={:.3} bias={:+.3}", minutes_errors.n, minutes_errors.mae(), minutes_errors.rmse(), minutes_errors.bias());
        println!("start probability   : n={:<4} Brier={:.4}", start_errors.n, start_errors.mean_brier());
        println!("appearance probability: n={:<4} Brier={:.4}", appearance_errors.n, appearance_errors.mean_brier());
    }
}

fn short(name: &str) -> String {
    name.split(' ').next().unwrap_or(name).to_string()
}
