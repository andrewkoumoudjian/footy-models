#![forbid(unsafe_code)]
//! Strictly causal historical walk-forward driver for the production second-resolved engine.
//!
//! This runs the *actual* `simulate_second_resolved_dynamic_mental_match` engine on historical
//! Premier League fixtures using an **observed-stat-only** input contract built by
//! `scripts/build_pl_hist_inputs.py`:
//!
//! - No FM26 attributes, no current squads, no present-day values of any kind.
//! - Player pools and per-90 rates come only from event-covered matches before kickoff.
//! - Team goal/shot windows come only from earlier results.
//!
//! It is a walk-forward evaluation, not a production forecast: each fixture is simulated with
//! exactly the pre-kickoff inputs available on that date, and the raw path output is exported for
//! out-of-sample scoring. An optional online `OutcomeCalibrator` records the correction that would
//! have been fitted on strictly earlier out-of-sample fixtures (never the current or future ones).
//!
//! The feature contract is deliberately different from the 2026 FM26 pipeline. Results here
//! validate the *engine mechanics and calibration on observed-stat inputs*; they are not evidence
//! for the FM26-enhanced model.

use crate::driver::{
    anchor_shot_volume, channel_graph, date_from, ensure_candidate_primitives, event_models_for,
    formation, load_players_with_freshness, mental_model, squad, LeagueWindows, PlayerRow, TeamData,
    M_PROGC, M_SHOT, M_XA, M_XG, M_PASS,
};
use footy_core::{MatchId, MetricId, PlayerId};
use footy_match::{
    apply_dixon_coles, simulate_second_resolved_dynamic_mental_match, DynamicMentalTransitionParameters, FormationPrior,
    LineupProjectionInput, LineupSamplingParameters, OutcomeCalibrationSample, OutcomeCalibrator,
    PlayerMatchupState, PrimitiveMatchSimulationInput, SecondResolvedDynamicMentalForecast,
    SecondResolvedSimulationParameters, SpecificGoalProcess, SpecificInMatchParameters,
    SpecificMatchEnvironment, SpecificScoreStateMetricModifier,
    SpecificTeamContext,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// Calibrated production input contract for the observed-stat-only historical run. These mirror
/// the shared MLS/StatsBomb `build_input` contract (same engine entry point) and are exposed as
/// explicit experiment parameters rather than hidden constants.
#[derive(Debug, Clone, Copy)]
pub struct HistoricalInputContract {
    pub home_advantage_multiplier: f64,
    pub home_attack_log_adjustment: f64,
    pub away_attack_log_adjustment: f64,
    pub goal_rate_scale: f64,
    pub path_log_rate_sigma: f64,
    pub max_substitutions: u8,
    pub red_card_probability_per90: f64,
    pub injury_probability_per90: f64,
    /// Shared-tempo correlation between per-path home/away log goal rates. Zero keeps the
    /// legacy independent sampling; nonzero values are research-only until a frozen OOS
    /// comparison shows a proper-score improvement with uncertainty.
    pub path_rate_correlation: f64,
    /// Research-only: preserve each team's causal attacking/defensive strength in its
    /// own goal intensity instead of giving both teams the same blended anchor.
    pub team_strength_anchor: bool,
}

impl Default for HistoricalInputContract {
    fn default() -> Self {
        Self {
            home_advantage_multiplier: 1.09,
            home_attack_log_adjustment: 0.10,
            away_attack_log_adjustment: 0.0,
            goal_rate_scale: 1.0,
            path_log_rate_sigma: 0.28,
            // 2015/16 Premier League allowed three substitutions per team.
            max_substitutions: 3,
            red_card_probability_per90: 0.02,
            injury_probability_per90: 0.02,
            path_rate_correlation: 0.0,
            team_strength_anchor: false,
        }
    }
}

fn neutral_ability() -> footy_state::AbilityVector {
    // No historical attribute source exists for 2015/16. Every player receives the same neutral
    // latent index and no attribute/matchup rules are loaded, so no present-day FM information can
    // leak into the simulation.
    footy_state::AbilityVector::from_components([10.5; 10])
}

/// Piecewise-linear application of a fitted desired→input selection correction. The lineup sampler
/// compresses realized start marginals toward the pool middle; the correction table (fit on
/// fit-period fixtures only) maps a calibrated pre-match start probability to the sampler input
/// that reproduces it.
fn corrected_start_probability(table: &[(f64, f64)], value: f64) -> f64 {
    if table.is_empty() {
        return value;
    }
    if value <= table[0].0 {
        return table[0].1;
    }
    if value >= table[table.len() - 1].0 {
        return table[table.len() - 1].1;
    }
    for pair in table.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if value <= x1 {
            if x1 - x0 < 1e-12 {
                return y1;
            }
            let fraction = (value - x0) / (x1 - x0);
            return y0 + fraction * (y1 - y0);
        }
    }
    table[table.len() - 1].1
}

/// Relative home/away goal intensity from pre-match team rates. Normalize the
/// two values to an average of one: the experiment changes *allocation* of
/// goals, not the (previously rejected) overall goal-level calibration.
/// The shot-on-target proxy follows the same 0.30 coefficient as the original
/// shared goal anchor. All six inputs are causal EWMA values from LeagueWindows.
fn team_goal_strength_ratios(home: &TeamData, away: &TeamData) -> (f64, f64) {
    let goal_home = 0.5 * (home.gf_pm + away.ga_pm);
    let goal_away = 0.5 * (away.gf_pm + home.ga_pm);
    let shot_home = 0.15 * (home.sot_pm + away.sot_face_pm);
    let shot_away = 0.15 * (away.sot_pm + home.sot_face_pm);
    let home_strength = 0.6 * goal_home + 0.4 * shot_home;
    let away_strength = 0.6 * goal_away + 0.4 * shot_away;
    let average = 0.5 * (home_strength + away_strength);
    if !average.is_finite() || average <= 0.0 {
        return (1.0, 1.0);
    }
    ((home_strength / average).max(1e-6), (away_strength / average).max(1e-6))
}

/// Observed-stat-only variant of the shared match-input builder. Identical engine entry point,
/// no FM26/results-model layer.
pub(crate) fn build_historical_input(
    match_id: MatchId,
    home: &TeamData,
    away: &TeamData,
    players: &[PlayerRow],
    contract: HistoricalInputContract,
    selection_correction: &[(f64, f64)],
) -> PrimitiveMatchSimulationInput {
    let mut home_lineup = LineupProjectionInput {
        club: home.club,
        formations: vec![
            FormationPrior { formation: formation(442), probability: 0.55 },
            FormationPrior { formation: formation(433), probability: 0.45 },
        ],
        candidates: squad(players, &home.name, home.club, None),
        locked_starters: Vec::new(),
        parameters: LineupSamplingParameters::default(),
    };
    let mut away_lineup = LineupProjectionInput {
        club: away.club,
        formations: vec![
            FormationPrior { formation: formation(442), probability: 0.55 },
            FormationPrior { formation: formation(433), probability: 0.45 },
        ],
        candidates: squad(players, &away.name, away.club, None),
        locked_starters: Vec::new(),
        parameters: LineupSamplingParameters::default(),
    };
    if !selection_correction.is_empty() {
        for candidate in home_lineup.candidates.iter_mut().chain(away_lineup.candidates.iter_mut()) {
            let appearance = candidate.start_probability + candidate.substitute_probability;
            let corrected = corrected_start_probability(selection_correction, candidate.start_probability);
            candidate.start_probability = corrected;
            candidate.substitute_probability = (appearance - corrected).max(0.0);
        }
    }

    let mut home_models = event_models_for(players, &home.name);
    let mut away_models = event_models_for(players, &away.name);
    anchor_shot_volume(&mut home_models, home.shots_pm.max(4.0));
    anchor_shot_volume(&mut away_models, away.shots_pm.max(4.0));
    let mut event_models = home_models;
    event_models.extend(away_models);

    let mut attribute_states = Vec::new();
    let mut player_states: Vec<PlayerMatchupState> = home_lineup
        .candidates
        .iter()
        .chain(away_lineup.candidates.iter())
        .map(|candidate| PlayerMatchupState { player: candidate.player, ability: neutral_ability() })
        .collect();
    ensure_candidate_primitives(&home_lineup, &mut attribute_states, &mut player_states, &mut event_models);
    ensure_candidate_primitives(&away_lineup, &mut attribute_states, &mut player_states, &mut event_models);

    let env_rate = ((home.gf_pm + home.ga_pm + away.gf_pm + away.ga_pm) / 4.0).clamp(0.9, 2.2);
    let sot_rate = ((home.sot_pm * 0.30 + away.sot_face_pm * 0.30)
        + (away.sot_pm * 0.30 + home.sot_face_pm * 0.30))
        / 4.0;
    let blended_rate = (env_rate * 0.6 + sot_rate.clamp(0.9, 2.4) * 0.4).clamp(1.05, 2.1)
        * contract.goal_rate_scale.max(1e-6);
    let (home_strength, away_strength) = if contract.team_strength_anchor {
        team_goal_strength_ratios(home, away)
    } else {
        (1.0, 1.0)
    };

    let team_context = |team: &TeamData| SpecificTeamContext {
        club: team.club,
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
        // Observed-stat-only contract: no attribute loadings or matchup rules at all.
        own_attribute_loadings: Vec::new(),
        aggregate_matchup_rules: Vec::new(),
        attribute_matchup_rules: Vec::new(),
        synergies: Vec::new(),
        channel_graph: channel_graph(),
        score_state_modifiers: vec![SpecificScoreStateMetricModifier {
            metric: MetricId(M_SHOT),
            trailing_multiplier: 1.08,
            leading_multiplier: 0.94,
        }],
        incident_profiles: Vec::new(),
        in_match: SpecificInMatchParameters {
            injury_probability_per90: contract.injury_probability_per90,
            red_card_probability_per90: contract.red_card_probability_per90,
            max_substitutions: contract.max_substitutions,
            minimum_tactical_substitution_minute: 58.0,
        },
        home_context: team_context(home),
        away_context: team_context(away),
        environment: SpecificMatchEnvironment {
            home_residual_attack_log_adjustment: contract.home_attack_log_adjustment + home_strength.ln(),
            away_residual_attack_log_adjustment: contract.away_attack_log_adjustment + away_strength.ln(),
            metric_modifiers: Vec::new(),
        },
        goal_process: SpecificGoalProcess {
            goals_per_team_match: blended_rate,
            home_advantage_multiplier: contract.home_advantage_multiplier,
            path_log_rate_sigma: contract.path_log_rate_sigma,
            path_rate_correlation: contract.path_rate_correlation,
            max_abs_segment_log_adjustment: 1.2,
            segments: 6,
        },
        matchup_spatial_sigma: 0.25,
        max_player_rate_log_adjustment: 1.0,
    };
    assert!(
        input.validate(),
        "historical observed-stat input failed validation for {} vs {}",
        home.name, away.name
    );
    input
}

fn metric_key(metric: MetricId) -> &'static str {
    match metric.0 {
        M_SHOT => "shots",
        M_XG => "xg",
        M_XA => "xa",
        M_PASS => "passes",
        M_PROGC => "progressive_carries",
        _ => "unknown",
    }
}

fn summarise_fixture(
    fixture: &Value,
    forecast: &SecondResolvedDynamicMentalForecast,
    players: &[PlayerRow],
) -> Value {
    let outcome = &forecast.outcome;
    let lookup = |id: PlayerId| players.iter().find(|value| value.id == id.0);
    let mut players_out: BTreeMap<u64, Value> = BTreeMap::new();
    for row in &forecast.player_metrics {
        let entry = players_out.entry(row.player.0).or_insert_with(|| {
            json!({
                "uid": row.player.0,
                "name": lookup(row.player).map_or("unseen", |value| value.name.as_str()),
                "team": lookup(row.player).map_or("", |value| value.team.as_str()),
                "start_probability": row.start_probability.get(),
                "appearance_probability": row.appearance_probability.get(),
                "expected_minutes": row.expected_minutes,
                "metrics": {},
            })
        });
        let conditional = row.conditional_on_appearance_total.map(|value| value.mean).unwrap_or(0.0);
        if let Some(object) = entry.get_mut("metrics").and_then(Value::as_object_mut) {
            object.insert(metric_key(row.metric).to_string(), json!({
                "mean_unconditional": row.unconditional_total.mean,
                "p10": row.unconditional_total.p10,
                "p50": row.unconditional_total.p50,
                "p90": row.unconditional_total.p90,
                "mean_conditional_on_appearance": conditional,
            }));
        }
    }
    let goal_occurrence: BTreeMap<u64, (f64, f64)> = forecast
        .player_goal_occurrence
        .iter()
        .map(|row| (row.player.0, (row.occurrence_probability, row.expected_per_match)))
        .collect();
    let assist_occurrence: BTreeMap<u64, (f64, f64)> = forecast
        .player_assist_occurrence
        .iter()
        .map(|row| (row.player.0, (row.occurrence_probability, row.expected_per_match)))
        .collect();
    for (uid, entry) in players_out.iter_mut() {
        let (goal_p, goal_mean) = goal_occurrence.get(uid).copied().unwrap_or((0.0, 0.0));
        let (assist_p, assist_mean) = assist_occurrence.get(uid).copied().unwrap_or((0.0, 0.0));
        if let Some(object) = entry.as_object_mut() {
            object.insert("goal_occurrence_probability".to_string(), json!(goal_p));
            object.insert("expected_attributed_goals".to_string(), json!(goal_mean));
            object.insert("assist_occurrence_probability".to_string(), json!(assist_p));
            object.insert("expected_attributed_assists".to_string(), json!(assist_mean));
        }
    }
    // Team goal marginal pmfs derived from the same joint scoreline table.
    let mut home_pmf: BTreeMap<u16, f64> = BTreeMap::new();
    let mut away_pmf: BTreeMap<u16, f64> = BTreeMap::new();
    for cell in &forecast.scoreline_distribution {
        *home_pmf.entry(cell.home_goals).or_default() += cell.probability;
        *away_pmf.entry(cell.away_goals).or_default() += cell.probability;
    }
    let pmf_json = |pmf: &BTreeMap<u16, f64>| -> Vec<Value> {
        pmf.iter().map(|(goals, probability)| json!([goals, probability])).collect()
    };
    json!({
        "match_id": fixture["match_id"],
        "date": fixture["date"],
        "home": fixture["home"],
        "away": fixture["away"],
        "home_goals_actual": fixture["home_goals"],
        "away_goals_actual": fixture["away_goals"],
        "home_win": outcome.home_win.get(),
        "draw": outcome.draw.get(),
        "away_win": outcome.away_win.get(),
        "expected_home_goals": outcome.home_goals.mean,
        "expected_away_goals": outcome.away_goals.mean,
        "home_goal_distribution": pmf_json(&home_pmf),
        "away_goal_distribution": pmf_json(&away_pmf),
        "scoreline_distribution": forecast.scoreline_distribution.iter()
            .map(|cell| json!([cell.home_goals, cell.away_goals, cell.probability])).collect::<Vec<_>>(),
        "unattributed_goals_per_match": forecast.unattributed_goals_per_match,
        "mean_state_events_per_path": forecast.mean_state_events_per_path,
        "same_second_collision_probability": forecast.same_second_collision_probability.get(),
        "windows_15min": forecast.windows_15min.iter().map(|window| json!({
            "minute_start": window.start_minute,
            "minute_end": window.end_minute,
            "p_any_goal": window.probability_any_goal,
            "p_home_goal": window.probability_home_goal,
            "p_away_goal": window.probability_away_goal,
            "p_red_card": window.probability_red_card,
            "p_substitution": window.probability_substitution,
            "p_completed_substitution": window.probability_completed_substitution,
            "expected_home_goals": window.expected_home_goals,
            "expected_away_goals": window.expected_away_goals,
            "expected_home_shots": window.expected_home_shots,
            "expected_away_shots": window.expected_away_shots,
            "end_of_window_outcomes": [window.end_home_lead, window.end_draw, window.end_away_lead],
        })).collect::<Vec<_>>(),
        "minutes_1min": forecast.minutes_1min.iter().map(|window| json!({
            "minute": window.start_minute,
            "p_any_goal": window.probability_any_goal,
            "p_home_goal": window.probability_home_goal,
            "p_away_goal": window.probability_away_goal,
            "expected_home_goals": window.expected_home_goals,
            "expected_away_goals": window.expected_away_goals,
        })).collect::<Vec<_>>(),
        "players": players_out.into_values().collect::<Vec<_>>(),
    })
}

#[allow(clippy::too_many_arguments)]
fn run_fixture(
    fixture: &Value,
    windows: &LeagueWindows,
    players: &[PlayerRow],
    contract: HistoricalInputContract,
    selection_correction: &[(f64, f64)],
    dixon_coles_rho: f64,
    paths: u32,
    seed: u64,
) -> Option<(Value, [f64; 3], usize)> {
    let match_id = fixture["match_id"].as_u64().expect("match id");
    let home_name = fixture["home"].as_str().expect("home");
    let away_name = fixture["away"].as_str().expect("away");
    let home = windows.team(home_name);
    let away = windows.team(away_name);
    let input = build_historical_input(MatchId(match_id), &home, &away, players, contract, selection_correction);
    let home_ids: Vec<PlayerId> = input.home_lineup.candidates.iter().map(|value| value.player).collect();
    let away_ids: Vec<PlayerId> = input.away_lineup.candidates.iter().map(|value| value.player).collect();
    // Historical inputs have no FM dirtiness/consistency fields; neutral psychology only.
    let mental = mental_model(&home, &away, &home_ids, &away_ids, &|_| None, &|_| None);
    let forecast = simulate_second_resolved_dynamic_mental_match(
        &input,
        &mental,
        DynamicMentalTransitionParameters::default(),
        SecondResolvedSimulationParameters::default(),
        paths,
        seed.wrapping_add(match_id),
    )?;
    let actual = {
        let home_goals = fixture["home_goals"].as_u64().expect("home goals");
        let away_goals = fixture["away_goals"].as_u64().expect("away goals");
        match home_goals.cmp(&away_goals) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less => 2,
        }
    };
    let raw = [
        f64::from(forecast.outcome.home_win.get()),
        f64::from(forecast.outcome.draw.get()),
        f64::from(forecast.outcome.away_win.get()),
    ];
    let mut summary = summarise_fixture(fixture, &forecast, players);
    // Research-only Dixon–Coles layer: the raw path-observed table is immutable; the
    // corrected table is exported alongside with its rho and applicability flag.
    // Default rho = 0.0 skips the layer entirely (identity, no new keys beyond rho).
    if let Some(object) = summary.as_object_mut() {
        object.insert("dixon_coles_rho".to_string(), json!(dixon_coles_rho));
        if dixon_coles_rho != 0.0 {
            let home_mean = forecast.outcome.home_goals.mean;
            let away_mean = forecast.outcome.away_goals.mean;
            let cells: Vec<(u32, u32, f64)> = forecast
                .scoreline_distribution
                .iter()
                .map(|cell| (u32::from(cell.home_goals), u32::from(cell.away_goals), cell.probability))
                .collect();
            match apply_dixon_coles(&cells, home_mean, away_mean, dixon_coles_rho) {
                Some(adjusted) => {
                    // Dixon-Coles preserves goal marginals for an independent Poisson
                    // base, NOT an arbitrary empirical scoreline table. Explicitly
                    // record the incoherence introduced by this optional diagnostic.
                    let mut marginal_changes: BTreeMap<(u8, u32), f64> = BTreeMap::new();
                    for (raw_cell, adjusted_cell) in cells.iter().zip(&adjusted) {
                        let delta = adjusted_cell.2 - raw_cell.2;
                        *marginal_changes.entry((0, raw_cell.0)).or_default() += delta;
                        *marginal_changes.entry((1, raw_cell.1)).or_default() += delta;
                    }
                    let max_goal_marginal_shift = marginal_changes.values()
                        .copied().map(f64::abs).fold(0.0, f64::max);
                    object.insert("dixon_coles_max_goal_marginal_shift".to_string(), json!(max_goal_marginal_shift));
                    object.insert("dixon_coles_is_alternative_scoreline_forecast_only".to_string(), json!(true));
                    object.insert(
                        "scoreline_distribution_dc".to_string(),
                        adjusted
                            .iter()
                            .map(|(home, away, probability)| json!([home, away, probability]))
                            .collect(),
                    );
                    object.insert("dixon_coles_applied".to_string(), json!(true));
                }
                None => {
                    object.insert("dixon_coles_applied".to_string(), json!(false));
                }
            }
        }
    }
    Some((summary, raw, actual))
}

#[allow(clippy::too_many_arguments)]
pub fn run_premier_league_historical() {
    let mut paths = 500_u32;
    let mut seed = 42_u64;
    let mut jobs = 0_usize; // 0 = auto
    let mut fixtures_path = "data/statsbomb/pl_hist/fixtures.json".to_string();
    let mut team_rows_path = "data/statsbomb/pl_hist/team_rows.csv".to_string();
    let mut players_dir = "data/statsbomb/pl_hist/players".to_string();
    let mut output_path = "artifacts/validation/pl-hist-same-engine-paths.json".to_string();
    let mut use_calibration = true;
    let mut selection_correction_path: Option<String> = None;
    let mut contract = HistoricalInputContract::default();
    // Research-only Dixon–Coles rho applied as a post-hoc scoreline-table layer.
    // 0.0 (default) disables it; nonzero values must earn production via frozen OOS.
    let mut dixon_coles_rho = 0.0_f64;
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--paths" => paths = iter.next().expect("--paths value").parse().expect("valid paths"),
            "--seed" => seed = iter.next().expect("--seed value").parse().expect("valid seed"),
            "--jobs" => jobs = iter.next().expect("--jobs value").parse().expect("valid jobs"),
            "--fixtures" => fixtures_path = iter.next().expect("--fixtures value"),
            "--team-rows" => team_rows_path = iter.next().expect("--team-rows value"),
            "--players-dir" => players_dir = iter.next().expect("--players-dir value"),
            "--output" => output_path = iter.next().expect("--output value"),
            "--no-calibration" => use_calibration = false,
            "--selection-correction" => selection_correction_path = Some(iter.next().expect("path")),
            "--home-advantage-multiplier" => {
                contract.home_advantage_multiplier = iter.next().expect("value").parse().expect("number")
            }
            "--home-attack-log-adjustment" => {
                contract.home_attack_log_adjustment = iter.next().expect("value").parse().expect("number")
            }
            "--away-attack-log-adjustment" => {
                contract.away_attack_log_adjustment = iter.next().expect("value").parse().expect("number")
            }
            "--goal-rate-scale" => {
                contract.goal_rate_scale = iter.next().expect("value").parse().expect("number")
            }
            "--path-log-rate-sigma" => {
                contract.path_log_rate_sigma = iter.next().expect("value").parse().expect("number")
            }
            "--max-substitutions" => {
                contract.max_substitutions = iter.next().expect("value").parse().expect("number")
            }
            "--red-card-probability-per90" => {
                contract.red_card_probability_per90 = iter.next().expect("value").parse().expect("number")
            }
            "--path-rate-correlation" => {
                contract.path_rate_correlation = iter.next().expect("value").parse().expect("number")
            }
            "--dixon-coles-rho" => {
                dixon_coles_rho = iter.next().expect("value").parse().expect("number")
            }
            "--team-strength-anchor" => contract.team_strength_anchor = true,
            _ => panic!("unknown argument: {flag}"),
        }
    }
    assert!(paths > 0 && paths <= 20000, "paths out of range");
    let selection_correction: Vec<(f64, f64)> = selection_correction_path.as_deref().map_or(Vec::new(), |path| {
        let document: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("read selection correction"))
            .expect("parse selection correction");
        document["correction_table"].as_array().expect("correction_table array").iter()
            .map(|pair| {
                let values = pair.as_array().expect("pair");
                (values[0].as_f64().expect("desired"), values[1].as_f64().expect("input"))
            })
            .collect()
    });
    let fixtures_document: Value =
        serde_json::from_str(&std::fs::read_to_string(&fixtures_path).expect("read fixtures"))
            .expect("parse fixtures");
    let fixtures: Vec<Value> = fixtures_document["fixtures"]
        .as_array()
        .expect("fixtures array")
        .clone();

    // Preload per-fixture player pools sequentially (cheap); simulation is the expensive part.
    let mut prepared = Vec::new();
    for fixture in &fixtures {
        let evaluable = fixture["evaluable"].as_bool().unwrap_or(false);
        if !evaluable {
            prepared.push(None);
            continue;
        }
        let match_id = fixture["match_id"].as_u64().expect("match id");
        let date = date_from(fixture["date"].as_str().expect("date"));
        let players_path = format!("{players_dir}/{match_id}.csv");
        if !Path::new(&players_path).is_file() {
            prepared.push(None);
            continue;
        }
        // Historical pools keep every causally listed player; recency is handled by selection history.
        let players = load_players_with_freshness(&players_path, 0.0, date, None);
        let home_count = players.iter().filter(|p| p.team == fixture["home"].as_str().unwrap()).count();
        let away_count = players.iter().filter(|p| p.team == fixture["away"].as_str().unwrap()).count();
        if home_count < 11 || away_count < 11 {
            prepared.push(None);
            continue;
        }
        let windows = LeagueWindows::build(&team_rows_path, date);
        prepared.push(Some((players, windows)));
    }

    let worker_count = if jobs == 0 {
        std::thread::available_parallelism().map(|value| value.get()).unwrap_or(1)
    } else {
        jobs
    };
    let mut raw_results: Vec<Option<(Value, [f64; 3], usize)>> = vec![None; fixtures.len()];
    let mut skipped: Vec<Value> = Vec::new();
    let work: Vec<usize> = (0..fixtures.len()).filter(|index| prepared[*index].is_some()).collect();
    if worker_count <= 1 || work.len() <= 1 {
        for index in work {
            let (players, windows) = prepared[index].as_ref().expect("prepared");
            raw_results[index] = run_fixture(&fixtures[index], windows, players, contract, &selection_correction, dixon_coles_rho, paths, seed);
        }
    } else {
        let chunk_size = work.len().div_ceil(worker_count);
        let shared = std::sync::Arc::new(std::sync::Mutex::new(
            Vec::<(usize, Option<(Value, [f64; 3], usize)>)>::new(),
        ));
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in work.chunks(chunk_size) {
                let fixtures_ref = &fixtures;
                let prepared_ref = &prepared;
                let contract_ref = contract;
                let correction_ref = &selection_correction;
                let dixon_coles_ref = &dixon_coles_rho;
                let shared_ref = std::sync::Arc::clone(&shared);
                handles.push(scope.spawn(move || {
                    let mut local = Vec::new();
                    for index in chunk {
                        let (players, windows) = prepared_ref[*index].as_ref().expect("prepared");
                        let result = run_fixture(&fixtures_ref[*index], windows, players, contract_ref, correction_ref, *dixon_coles_ref, paths, seed);
                        local.push((*index, result));
                    }
                    shared_ref.lock().expect("lock").extend(local);
                }));
            }
            for handle in handles {
                handle.join().expect("worker join");
            }
        });
        for (index, result) in shared.lock().expect("lock").drain(..) {
            raw_results[index] = result;
        }
    }

    // Sequential walk-forward calibration: each fixture only ever sees earlier out-of-sample
    // forecasts. This is recorded, never applied silently to the raw path output.
    let mut calibration_history: Vec<OutcomeCalibrationSample> = Vec::new();
    let mut fixtures_out = Vec::new();
    for (index, fixture) in fixtures.iter().enumerate() {
        let Some((mut summary, raw, actual)) = raw_results[index].take() else {
            skipped.push(json!({
                "match_id": fixture["match_id"], "date": fixture["date"],
                "home": fixture["home"], "away": fixture["away"],
                "reason": if fixture["evaluable"].as_bool().unwrap_or(false) {
                    "insufficient causal player coverage"
                } else {
                    "not evaluable: fewer than 11 causally listed players for a club"
                },
            }));
            continue;
        };
        let calibrator = if use_calibration && calibration_history.len() >= 15 {
            OutcomeCalibrator::fit(&calibration_history).unwrap_or_default()
        } else {
            OutcomeCalibrator::default()
        };
        let calibrated = calibrator.apply(raw).unwrap_or(raw);
        if let Some(object) = summary.as_object_mut() {
            object.insert("raw_probabilities".to_string(), json!(raw));
            object.insert("calibration".to_string(), json!({
                "fit_on_prior_fixtures": calibration_history.len(),
                "temperature": calibrator.temperature,
                "draw_logit_bias": calibrator.draw_logit_bias,
                "calibrated_probabilities": calibrated,
            }));
        }
        fixtures_out.push(summary);
        calibration_history.push(OutcomeCalibrationSample { probabilities: raw, actual });
    }

    let document = json!({
        "schema_version": 1,
        "generated_by": "footy-matchforecast-pl-hist",
        "model": "simulate_second_resolved_dynamic_mental_match",
        "feature_contract": "observed_stats_only_no_fm26_no_current_squads",
        "paths": paths,
        "seed": seed,
        "input_contract": {
            "home_advantage_multiplier": contract.home_advantage_multiplier,
            "home_attack_log_adjustment": contract.home_attack_log_adjustment,
            "away_attack_log_adjustment": contract.away_attack_log_adjustment,
            "goal_rate_scale": contract.goal_rate_scale,
            "path_log_rate_sigma": contract.path_log_rate_sigma,
            "path_rate_correlation": contract.path_rate_correlation,
            "max_substitutions": contract.max_substitutions,
            "red_card_probability_per90": contract.red_card_probability_per90,
            "injury_probability_per90": contract.injury_probability_per90,
            "team_strength_anchor": contract.team_strength_anchor,
        },
        "calibration_enabled": use_calibration,
        "dixon_coles_rho": dixon_coles_rho,
        "selection_correction_points": selection_correction.len(),
        "fixtures": fixtures_out,
        "skipped": skipped,
    });
    let path = Path::new(&output_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create artifact directory");
    }
    std::fs::write(path, serde_json::to_string(&document).expect("serialize") + "\n").expect("write output");
    println!(
        "historical same-engine run: {} simulated fixtures, {} skipped, paths={}, output={}",
        document["fixtures"].as_array().map(|value| value.len()).unwrap_or(0),
        skipped.len(),
        paths,
        output_path
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::ClubId;

    fn team(name: &str, gf: f64, ga: f64, sot: f64, faced: f64) -> TeamData {
        TeamData {
            name: name.to_owned(), club: ClubId(1), matches: 20,
            gf_pm: gf, ga_pm: ga, xga_pm: ga,
            shots_pm: 12.0, sot_pm: sot, sot_face_pm: faced,
        }
    }

    #[test]
    fn identical_team_history_keeps_the_existing_anchor() {
        let a = team("A", 1.5, 1.2, 5.0, 4.0);
        let (home, away) = team_goal_strength_ratios(&a, &a);
        assert!((home - 1.0).abs() < 1e-12);
        assert!((away - 1.0).abs() < 1e-12);
    }

    #[test]
    fn differential_anchor_conserves_total_goal_rate_and_reverses_with_sides() {
        let strong = team("Strong", 2.1, 0.8, 6.2, 2.7);
        let weak = team("Weak", 0.8, 2.0, 2.8, 6.0);
        let (home, away) = team_goal_strength_ratios(&strong, &weak);
        assert!(home > 1.0 && away < 1.0);
        assert!((home + away - 2.0).abs() < 1e-12);
        let (reversed_home, reversed_away) = team_goal_strength_ratios(&weak, &strong);
        assert!((home - reversed_away).abs() < 1e-12);
        assert!((away - reversed_home).abs() < 1e-12);
    }

    #[test]
    fn invalid_strength_input_cannot_generate_non_finite_goal_hazards() {
        let empty = team("Empty", 0.0, 0.0, 0.0, 0.0);
        assert_eq!(team_goal_strength_ratios(&empty, &empty), (1.0, 1.0));
    }
}
