#![forbid(unsafe_code)]
//! Specific-match forecast pipeline driver.
//!
//! Wires cached StatsBomb open-data payloads through the source-neutral parsers, builds strictly
//! as-of player/team performance aggregates, calibrates the second-resolved match input from the
//! underlying player statistics, and runs the production Monte Carlo fixture engine.

use footy_core::{AttributeId, ClubId, Confidence, Date, MatchId, MetricId, PlayerId, ProvenanceId, RoleId};
use footy_dataset::{
        build_sporting_dataset, CanonicalDataset, CanonicalIdentity, CanonicalIssue, CanonicalPlayer,
        SportingClubRecord, SportingPlayerLink, SportingProvenance,
    };
use footy_dataset::canonicalize_sporting_lineups;
use footy_forecast::StatFamily;
use footy_match::{
    simulate_second_resolved_dynamic_mental_match, AttributeEventLoading, AttributeMatchupRule,
    CausalChannelGraph, ChannelEdge, DynamicMentalTransitionParameters, FormationPrior,
    FormationTemplate, GoalChannelLoading, LineupCandidate, LineupProjectionInput,
    LineupSamplingParameters, MatchMentalContext, MentalMatchModel, MentalMeasure,
    MentalMetricLoading, MentalModelParameters, MentalSelectionLoading, MetricChannelLoading,
    PairwiseSynergy, PlayerAttributeValue, PlayerAttributeState, PlayerMatchupState, PlayerMentalState,
    RoleFit, SecondResolvedSimulationParameters, SpecificEnvironmentMetricModifier,
    SpecificGoalProcess, SpecificInMatchParameters, SpecificMatchEnvironment, SpecificPlayerEventModel,
    SpecificScoreStateMetricModifier, SpecificTeamContext, SpecificTeamMetricModifier,
    StablePsychologyState, TacticalSlot, TeamMentalState, TeamProcessChannel,
};
use footy_performance::{build_team_window, PlayerMatch};
use footy_source_stats::{
    parse_statsbomb_events, parse_statsbomb_lineups, MatchContext as SourceMatchContext,
    MatchResultSourceRecord, MatchStatus, PlayerStatSourceRecord, StatsProvider, TeamStatSourceRecord,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::env;

const PROVENANCE_ID: u64 = 1;
const METRIC_PASS: u16 = 1;
const METRIC_SHOT: u16 = 2;
const METRIC_XG: u16 = 3;
const METRIC_CARRY: u16 = 4;
const METRIC_RECOVERY: u16 = 5;
const METRIC_PRESSURE: u16 = 6;
const ATTR_TECHNICAL: u16 = 1;
const ATTR_FINISHING: u16 = 2;
const ATTR_DEFENDING: u16 = 3;

struct Args {
    data_dir: String,
    paths: u32,
    seed: u64,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut data_dir = String::from("data/statsbomb");
    let mut paths = 2000_u32;
    let mut seed = 42_u64;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--data" => data_dir = iter.next().expect("--data requires a value").clone(),
            "--paths" => paths = iter.next().expect("--paths requires a value").parse().expect("paths must be a number"),
            "--seed" => seed = iter.next().expect("--seed requires a value").parse().expect("seed must be a number"),
            other => panic!("unknown argument {other}"),
        }
    }
    run(Args { data_dir, paths, seed });
}

fn read_json(path: &str) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn date_from(text: &str) -> Date {
    let parts: Vec<&str> = text.split('-').collect();
    assert!(parts.len() == 3, "bad date {text}");
    Date::from_ymd(parts[0].parse().unwrap(), parts[1].parse().unwrap(), parts[2].parse().unwrap())
        .expect("valid date")
}

fn stable_hash64(bytes: &[u8]) -> u64 {
    // FNV-1a; deterministic provenance checksum for the manifest record.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn run(args: Args) {
    let manifest_path = format!("{}/manifest.json", args.data_dir);
    let manifest = read_json(&manifest_path);
    let fixture = &manifest["fixture"];
    let history: Vec<Value> = manifest["history"]
        .as_array()
        .expect("manifest history array")
        .clone();

    let fx_id = fixture["match_id"].as_u64().unwrap().to_string();
    let fx_date = date_from(fixture["date"].as_str().unwrap());
    let home_ext = fixture["home_id"].as_u64().unwrap().to_string();
    let away_ext = fixture["away_id"].as_u64().unwrap().to_string();
    let home_name = fixture["home"].as_str().unwrap();
    let away_name = fixture["away"].as_str().unwrap();
    let actual_home_goals = fixture["actual_home_goals"].as_u64().unwrap();
    let actual_away_goals = fixture["actual_away_goals"].as_u64().unwrap();

    println!("=== Fixture forecast pipeline (second-resolved MC) ===");
    println!("fixture: {home_name} vs {away_name}, {} (match {fx_id})", fixture["date"].as_str().unwrap());
    println!("history: {} prior matches (strictly as-of)", history.len());
    println!("engine params: paths={} seed={}", args.paths, args.seed);

    // ---- 1. Parse every historical payload through the source-neutral adapters -----------------
    let mut contexts: BTreeMap<String, SourceMatchContext> = BTreeMap::new();
    let mut all_player_records: Vec<PlayerStatSourceRecord> = Vec::new();
    let mut all_team_records: Vec<TeamStatSourceRecord> = Vec::new();
    let mut all_match_results: Vec<MatchResultSourceRecord> = Vec::new();
    let mut all_lineup_records = Vec::new();
    let mut external_players: BTreeMap<(String, String), (String, Option<Date>)> = BTreeMap::new();

    let mut entries: Vec<&Value> = history.iter().collect();
    entries.push(fixture);
    for entry in &entries {
        let mid = entry["match_id"].as_u64().unwrap().to_string();
        let date_text = entry["date"].as_str().unwrap().to_string();
        let h_ext = entry["home_id"].as_u64().unwrap().to_string();
        let a_ext = entry["away_id"].as_u64().unwrap().to_string();
        let context = SourceMatchContext {
            match_external_id: mid.clone(),
            competition_external_id: "PL".into(),
            season_external_id: "2015-16".into(),
            date: date_from(&date_text),
            home_club_external_id: h_ext.clone(),
            away_club_external_id: a_ext.clone(),
        };
        contexts.insert(mid.clone(), context);

        if entry.get("events_file").is_none() {
            continue; // The fixture itself is not parsed (never observed pre-match).
        }
        let events_text = std::fs::read_to_string(format!("{}/events/{mid}.json", args.data_dir)).expect("events file");
        let lineups_text = std::fs::read_to_string(format!("{}/lineups/{mid}.json", args.data_dir)).expect("lineups file");
        let ctx = &contexts[&mid];
        let (players, teams) = parse_statsbomb_events(&events_text, ctx, ProvenanceId(PROVENANCE_ID))
            .unwrap_or_else(|error| panic!("parse events {mid}: {error}"));
        all_player_records.extend(players);
        all_team_records.extend(teams);
        all_match_results.push(MatchResultSourceRecord {
            provider: StatsProvider::StatsBomb,
            match_external_id: mid.clone(),
            competition_external_id: "PL".into(),
            season_external_id: "2015-16".into(),
            date: ctx.date,
            home_club_external_id: h_ext.clone(),
            away_club_external_id: a_ext.clone(),
            home_goals: Some(entry["home_goals"].as_u64().unwrap() as u8),
            away_goals: Some(entry["away_goals"].as_u64().unwrap() as u8),
            status: MatchStatus::Finished,
            provenance_id: ProvenanceId(PROVENANCE_ID),
        });
        all_lineup_records.extend(
            parse_statsbomb_lineups(&lineups_text, ctx, ProvenanceId(PROVENANCE_ID))
                .unwrap_or_else(|error| panic!("parse lineups {mid}: {error}")),
        );
    }

    // Player names/birthdates come from lineup payloads (needed for canonical identity).
    for entry in &entries {
        if entry.get("events_file").is_none() { continue; }
        let mid = entry["match_id"].as_u64().unwrap().to_string();
        let lineups: Value = read_json(&format!("{}/lineups/{mid}.json", args.data_dir));
        for team in lineups.as_array().expect("lineup array") {
            let club_ext = team["team_id"].as_u64().unwrap().to_string();
            for player in team["lineup"].as_array().expect("lineup players") {
                let pid = player["player_id"].as_u64().unwrap().to_string();
                external_players
                    .entry((pid, club_ext.clone()))
                    .or_insert_with(|| (player["player_name"].as_str().unwrap().to_string(), None))
                    .0 = player["player_name"].as_str().unwrap().to_string();
            }
        }
    }
    println!(
        "parsed: {} player-stat records, {} team-stat records, {} results, {} lineups, {} distinct players",
        all_player_records.len(), all_team_records.len(), all_match_results.len(),
        all_lineup_records.len(), external_players.len()
    );

    // ---- 2. Build a minimal canonical base + sporting dataset ---------------------------------
    let provenances = vec![CanonicalProvenanceShim];
    let _ = provenances;
    build_and_forecast(args, fx_id, fx_date, home_ext, away_ext, home_name.to_string(), away_name.to_string(),
        actual_home_goals, actual_away_goals,
        all_player_records, all_team_records, all_match_results, all_lineup_records, external_players,
        contexts);
}

struct CanonicalProvenanceShim;

struct PlayerAggregate {
    club: ClubId,
    appearances: u32,
    minutes: f64,
    pass: f64,
    shot: f64,
    xg: f64,
    carry: f64,
    recovery: f64,
    pressure: f64,
}

#[allow(clippy::too_many_arguments)]
fn build_and_forecast(
    args: Args,
    fx_id: String,
    fx_date: Date,
    home_ext: String,
    away_ext: String,
    home_name: String,
    away_name: String,
    actual_home_goals: u64,
    actual_away_goals: u64,
    player_records: Vec<PlayerStatSourceRecord>,
    team_records: Vec<TeamStatSourceRecord>,
    match_results: Vec<MatchResultSourceRecord>,
    lineup_records: Vec<footy_source_stats::LineupSourceRecord>,
    external_players: BTreeMap<(String, String), (String, Option<Date>)>,
    _contexts: BTreeMap<String, SourceMatchContext>,
) {
    // Canonical base requires FM players by invariant; we construct a minimal identity-only base
    // through the public builder using empty FM records is impossible, so we assemble the
    // canonical dataset struct directly with StatsBomb players as canonical identities.
    use footy_core::{DatasetVersion as CoreDatasetVersion};

    let _ = CoreDatasetVersion;
    let mut players = Vec::new();
    let mut identities = Vec::new();
    // Deterministic canonical ids from the external id.
    let mut id_of: HashMap<String, PlayerId> = HashMap::new();
    let mut sorted_external: Vec<(&(String, String), &(String, Option<Date>))> = external_players.iter().map(|(k, v)| (k, v)).collect();
    sorted_external.sort_by_key(|((pid, _), _)| (*pid).clone());
    for ((_pid, _club), (name, _dob)) in &sorted_external {
        let _ = name;
    }
    let mut seen_player = std::collections::BTreeSet::new();
    for ((player_ext, _club_ext), (name, dob)) in &sorted_external {
        if !seen_player.insert(player_ext.clone()) {
            continue;
        }
        let player = PlayerId(stable_hash64(format!("statsbomb-player:{player_ext}").as_bytes()));
        id_of.insert(player_ext.clone(), player);
        players.push(CanonicalPlayer {
            player,
            fm_external_id: format!("sb-{player_ext}"),
            name: name.clone(),
            birth_date: *dob,
        });
        identities.push(CanonicalIdentity {
            player,
            source: footy_core::Source::Manual,
            external_id: format!("sb-{player_ext}"),
            score: Confidence::ONE,
            method: footy_entities::LinkMethod::Manual,
            verified: true,
            provenance_id: ProvenanceId(PROVENANCE_ID),
        });
    }
    players.sort_by_key(|value| value.player);
    identities.sort_by_key(|value| value.player);

    let canonical_base = CanonicalDataset {
        dataset_version: footy_core::DatasetVersion(1),
        source_manifest_checksum: 0,
        provenances: Vec::new(),
        players,
        identities,
        unresolved_identities: Vec::new(),
        issues: Vec::<CanonicalIssue>::new(),
        fm_snapshots: Vec::new(),
        clubs: Vec::new(),
        competitions: Vec::new(),
        metric_registry: Vec::new(),
        market_values: Vec::new(),
        transfers: Vec::new(),
        contracts: Vec::new(),
        performance: Vec::new(),
        salaries: Vec::new(),
    };

    let sporting_provenances = vec![SportingProvenance {
        id: ProvenanceId(PROVENANCE_ID),
        provider: StatsProvider::StatsBomb,
        input_path: args.data_dir.clone(),
        source_checksum: 0,
    }];
    let mut links = Vec::new();
    let mut seen_external = std::collections::BTreeSet::new();
    for (player_ext, _) in external_players.keys() {
        if !seen_external.insert(player_ext.clone()) {
            continue; // same external player seen at multiple clubs/lineups
        }
        links.push(SportingPlayerLink {
            provider: StatsProvider::StatsBomb,
            external_id: player_ext.clone(),
            player: id_of[player_ext],
            score: Confidence::ONE,
            method: footy_entities::LinkMethod::Manual,
            verified: true,
        });
    }
    links.sort_by_key(|value| value.external_id.clone());

    let sporting = build_sporting_dataset(
        &canonical_base,
        sporting_provenances,
        links,
        player_records,
        team_records,
        match_results,
    )
    .expect("sporting dataset");

    let lineup_dataset = canonicalize_sporting_lineups(&sporting, lineup_records).expect("lineup dataset");

    println!(
        "canonical: {} players, {} clubs, {} metric defs, {} lineup obs, {} issues",
        canonical_base.players.len(), sporting.clubs.len(), sporting.metric_registry.len(),
        lineup_dataset.lineups.len(), sporting.issues.len() + lineup_dataset.issues.len()
    );

    // ---- 3. As-of player per-90 aggregates ------------------------------------------------------
    let club_map: HashMap<(u8, String), ClubId> = sporting
        .clubs
        .iter()
        .map(|record: &SportingClubRecord| ((provider_tag(record.provider), record.external_id.clone()), record.id))
        .collect();
    let home_club = club_map[&(2_u8, home_ext.clone())];
    let away_club = club_map[&(2_u8, away_ext.clone())];

    // StatsBomb event payloads carry no minutes; join minutes from the canonical lineup dataset.
    let mut minutes_of: HashMap<(PlayerId, footy_core::MatchId), u16> = HashMap::new();
    for lineup in &lineup_dataset.lineups {
        for row in &lineup.players {
            if let Some(minutes) = row.minutes {
                minutes_of.insert((row.player, lineup.match_id), minutes);
            }
        }
    }
    let mut enriched: Vec<PlayerMatch> = Vec::new();
    {
        // Recreate PlayerMatch rows for stat observations that have a minutes join.
        use footy_core::{CompetitionId as _Cid, ObservationState};
        let _ = _Cid;
        let stat_by_match: HashMap<footy_core::MatchId, Vec<&footy_dataset::CanonicalPlayerStatObservation>> = {
            let mut map: HashMap<footy_core::MatchId, Vec<&footy_dataset::CanonicalPlayerStatObservation>> = HashMap::new();
            for value in &sporting.player_stats {
                map.entry(value.match_id).or_default().push(value);
            }
            map
        };
        for (match_id, rows) in &stat_by_match {
            for value in rows {
                let Some(minutes) = minutes_of.get(&(value.player, *match_id)) else { continue };
                enriched.push(PlayerMatch {
                    player: value.player,
                    club: value.club,
                    competition: value.competition,
                    date: value.date,
                    minutes: *minutes,
                    metrics: value.metrics.iter().map(|metric| footy_performance::MetricValue { metric: metric.metric, value: metric.value }).collect(),
                    observation_state: ObservationState::Measured,
                });
            }
        }
    }
    enriched.sort_by_key(|value| (value.player, value.date, value.club, value.competition));
    let player_matches = enriched;
    println!("player-match observations (with minutes join): {}", player_matches.len());

    let mut agg: HashMap<PlayerId, PlayerAggregate> = HashMap::new();
    for pm in &player_matches {
        let age_days = fx_date.days_since(pm.date);
        if !(0..=270).contains(&age_days) { continue; } // season window only, strictly before fixture
        let entry = agg.entry(pm.player).or_insert(PlayerAggregate {
            club: pm.club, appearances: 0, minutes: 0.0,
            pass: 0.0, shot: 0.0, xg: 0.0, carry: 0.0, recovery: 0.0, pressure: 0.0,
        });
        entry.appearances += 1;
        entry.minutes += f64::from(pm.minutes);
        for metric in &pm.metrics {
            match metric.metric.0 {
                METRIC_PASS => entry.pass += metric.value,
                METRIC_SHOT => entry.shot += metric.value,
                METRIC_XG => entry.xg += metric.value,
                METRIC_CARRY => entry.carry += metric.value,
                METRIC_RECOVERY => entry.recovery += metric.value,
                METRIC_PRESSURE => entry.pressure += metric.value,
                _ => {}
            }
        }
    }
    let qualified: Vec<(&PlayerId, &PlayerAggregate)> = agg
        .iter()
        .filter(|(_, value)| value.appearances >= 5 && value.minutes >= 300.0)
        .collect();
    println!("qualified players (>=5 apps, >=300 min): {}", qualified.len());

    // ---- 4. As-of team windows ------------------------------------------------------------------
    let team_matches = sporting.team_matches();
    let home_window = build_team_window(&team_matches, home_club, fx_date, 180, 60.0);
    let away_window = build_team_window(&team_matches, away_club, fx_date, 180, 60.0);
    let goals_for_metric = MetricId(METRIC_PASS); // placeholder lookup below via form instead
    let _ = goals_for_metric;
    println!("home form: {}/{} pts, GF {} GA {}",
        home_window.form.points, home_window.form.matches, home_window.form.goals_for, home_window.form.goals_against);
    println!("away form: {}/{} pts, GF {} GA {}",
        away_window.form.points, away_window.form.matches, away_window.form.goals_for, away_window.form.goals_against);

    // ---- 5. Calibrate the second-resolved match input from the underlying stats -----------------
    let input = calibrate_input(&args, &fx_id, fx_date, home_club, away_club, &agg, &home_window, &away_window);
    let mental = calibrate_mental(home_club, away_club, &input);

    // ---- 6. Run the production engine -----------------------------------------------------------
    let Some(forecast) = simulate_second_resolved_dynamic_mental_match(
        &input,
        &mental,
        DynamicMentalTransitionParameters::default(),
        SecondResolvedSimulationParameters::default(),
        args.paths,
        args.seed,
    ) else {
        panic!("simulation rejected its inputs");
    };

    let outcome = &forecast.outcome;
    println!("\n=== Forecast: {home_name} vs {away_name} ===");
    println!("P(home win)={:.3}  P(draw)={:.3}  P(away win)={:.3}",
        outcome.home_win.get(), outcome.draw.get(), outcome.away_win.get());
    println!("home goals: mean {:.2} [p10 {:.0}, p50 {:.0}, p90 {:.0}]",
        outcome.home_goals.mean, outcome.home_goals.p10, outcome.home_goals.p50, outcome.home_goals.p90);
    println!("away goals: mean {:.2} [p10 {:.0}, p50 {:.0}, p90 {:.0}]",
        outcome.away_goals.mean, outcome.away_goals.p10, outcome.away_goals.p50, outcome.away_goals.p90);
    println!("xPoints: home {:.2} away {:.2}", outcome.home_expected_points, outcome.away_expected_points);
    println!("state events/path {:.1}; same-second collision prob {:.3}",
        forecast.mean_state_events_per_path, forecast.same_second_collision_probability.get());

    // Top player stat forecasts (expected shots conditional on appearance).
    let mut rows: Vec<_> = forecast.player_metrics.iter()
        .filter(|value| value.metric == MetricId(METRIC_SHOT) && value.start_probability.get() > 0.5)
        .collect();
    rows.sort_by(|left, right| {
        let left_mean = left.conditional_on_appearance_total.as_ref().map(|summary| summary.mean).unwrap_or(0.0);
        let right_mean = right.conditional_on_appearance_total.as_ref().map(|summary| summary.mean).unwrap_or(0.0);
        right_mean.total_cmp(&left_mean)
    });
    println!("\ntop projected shooters (cond. on start/appearance):");
    println!("player_id | P(start) | E(shots | appear) p10/p50/p90 | E(min)");
    for row in rows.iter().take(10) {
        if let Some(summary) = &row.conditional_on_appearance_total {
            println!("{} | {:.2} | {:.1}/{:.1}/{:.1} mean {:.2} | {:.0}",
                row.player.0, row.start_probability.get(),
                summary.p10, summary.p50, summary.p90, summary.mean, row.expected_minutes);
        }
    }

    println!("\nactual result: {home_name} {actual_home_goals}-{actual_away_goals} {away_name}");
    println!("determinism check: rerun with identical --paths/--seed must reproduce these numbers exactly.");
}

fn provider_tag(provider: StatsProvider) -> u8 {
    match provider {
        StatsProvider::Sofascore => 1,
        StatsProvider::StatsBomb => 2,
        StatsProvider::OpenFootball => 3,
        StatsProvider::FootballDataOrg => 4,
    }
}

fn calibrate_mental(home: ClubId, away: ClubId, input: &footy_match::PrimitiveMatchSimulationInput) -> MentalMatchModel {
    let measure = |value: f64| MentalMeasure::new(value, Confidence::new(0.7).unwrap_or(Confidence::ONE)).expect("measure");
    let squad_mental = |ids: &[PlayerId]| -> Vec<PlayerMentalState> {
        ids.iter()
            .map(|player| PlayerMentalState {
                player: *player,
                stable: StablePsychologyState {
                    professionalism: measure(0.3),
                    ambition: measure(0.3),
                    consistency: measure(0.35),
                    pressure_resilience: measure(0.3),
                    adaptability: measure(0.3),
                    leadership: measure(0.3),
                    teamwork: measure(0.35),
                    discipline: measure(0.35),
                },
                morale: measure(0.15),
                self_belief: measure(0.1),
                form: measure(0.1),
                form_trend: measure(0.05),
                motivation: measure(0.2),
                role_satisfaction: measure(0.15),
            })
            .collect()
    };
    let home_ids: Vec<PlayerId> = input.home_lineup.candidates.iter().map(|value| value.player).collect();
    let away_ids: Vec<PlayerId> = input.away_lineup.candidates.iter().map(|value| value.player).collect();
    let team_state = |club: ClubId| TeamMentalState {
        club,
        collective_morale: measure(0.15), collective_belief: measure(0.15),
        cohesion: measure(0.25), leadership: measure(0.25),
    };
    let metric = MetricId(METRIC_SHOT);
    MentalMatchModel {
        players: [squad_mental(&home_ids), squad_mental(&away_ids)].concat(),
        home_team: team_state(home),
        away_team: team_state(away),
        metric_loadings: vec![
            MentalMetricLoading { metric, dimension: footy_match::MentalDimension::SelfBelief, mean_log_weight: 0.06, log_variance_weight: -0.04 },
            MentalMetricLoading { metric: MetricId(METRIC_PASS), dimension: footy_match::MentalDimension::Morale, mean_log_weight: 0.04, log_variance_weight: -0.02 },
        ],
        team_metric_loadings: Vec::new(),
        selection_loadings: vec![MentalSelectionLoading {
            dimension: footy_match::MentalDimension::Motivation,
            start_logit_weight: 0.08, substitute_logit_weight: 0.08,
        }],
        context: MatchMentalContext { pressure_intensity: 0.7, environmental_novelty: 0.15, hostility: 0.3 },
        parameters: MentalModelParameters {
            discipline_red_card_log_weight: -0.30,
            low_morale_red_card_log_weight: 0.25,
            ..MentalModelParameters::default()
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn calibrate_input(
    _args: &Args,
    fx_id: &str,
    fx_date: Date,
    home_club: ClubId,
    away_club: ClubId,
    agg: &HashMap<PlayerId, PlayerAggregate>,
    home_window: &footy_performance::TeamPerformanceWindow,
    away_window: &footy_performance::TeamPerformanceWindow,
) -> footy_match::PrimitiveMatchSimulationInput {
    let league_pass_per90 = 28.0_f64; // reference rates from the pooled sample
    let league_shot_per90 = 1.4_f64;
    let league_xg_per90 = 0.16_f64;

    let mut candidates_of: HashMap<ClubId, Vec<LineupCandidate>> = HashMap::new();
    let mut event_models: Vec<SpecificPlayerEventModel> = Vec::new();
    let mut attribute_states: Vec<PlayerAttributeState> = Vec::new();
    let mut player_states: Vec<PlayerMatchupState> = Vec::new();

    let mut roster: Vec<(PlayerId, &PlayerAggregate)> = agg
        .iter()
        .filter(|(_, value)| value.appearances >= 5 && value.minutes >= 300.0 && (value.club == home_club || value.club == away_club))
        .map(|(player, value)| (*player, value))
        .collect();
    roster.sort_by(|(left_player, left), (right_player, right)| {
        left.club
            .cmp(&right.club)
            .then(right.minutes.total_cmp(&left.minutes))
            .then(left_player.cmp(right_player))
    });

    for (club, window_form_share) in [(home_club, 18usize), (away_club, 18usize)] {
        let squad: Vec<&(PlayerId, &PlayerAggregate)> =
            roster.iter().filter(|(_, value)| value.club == club).take(20).collect();
        let max_minutes = squad.iter().map(|(_, value)| value.minutes).fold(1e-9, f64::max);
        let _ = window_form_share;
        let mut candidates = Vec::new();
        for (index, (player, value)) in squad.iter().enumerate() {
            let usage = value.minutes / max_minutes;
            let starter_probability = if index < 11 { 0.55 + 0.40 * usage } else { 0.12 + 0.20 * usage };
            let pass_per90 = value.pass / value.minutes * 90.0;
            let shot_per90 = value.shot / value.minutes * 90.0;
            let xg_per90 = value.xg / value.minutes * 90.0;
            // Latent quality index from usage + production, standardized later by the sampler.
            let ability_index = 8.0 + 4.0 * usage + 0.01 * (pass_per90 - league_pass_per90) + 2.0 * (shot_per90 - league_shot_per90);
            let fatigue = 0.1;
            candidates.push(LineupCandidate {
                player: *player,
                start_probability: starter_probability.clamp(0.02, 0.95),
                availability_probability: 0.96,
                ability_index,
                fatigue,
                expected_minutes_if_start: if index < 11 { 78.0 } else { 26.0 },
                minutes_sigma: 12.0,
                substitute_probability: if index < 11 { 0.22 } else { 0.65 },
                role_fits: (1_u16..=11).map(|role| RoleFit { role: RoleId(role), fit: 1.0 }).collect(),
            });
            let n = (value.appearances.max(2)) as f64;
            let variance_factor = 1.0 + 8.0 / n;
            event_models.push(SpecificPlayerEventModel {
                player: *player,
                metric: MetricId(METRIC_SHOT),
                family: StatFamily::Count,
                mean_per90: shot_per90.max(1e-3),
                variance_per90: shot_per90.max(1e-3) * variance_factor,
                epistemic_sigma: (shot_per90.max(0.05) / n.sqrt()).min(0.15),
                reference_per90: league_shot_per90,
                aggregate_matchup_sensitivity: 0.04,
                attribute_matchup_sensitivity: 0.03,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.10,
            });
            event_models.push(SpecificPlayerEventModel {
                player: *player,
                metric: MetricId(METRIC_PASS),
                family: StatFamily::Count,
                mean_per90: pass_per90.max(1.0),
                variance_per90: pass_per90.max(1.0) * variance_factor,
                epistemic_sigma: (pass_per90.max(1.0) / n.sqrt()).min(2.5),
                reference_per90: league_pass_per90,
                aggregate_matchup_sensitivity: 0.02,
                attribute_matchup_sensitivity: 0.015,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.08,
            });
            event_models.push(SpecificPlayerEventModel {
                player: *player,
                metric: MetricId(METRIC_XG),
                family: StatFamily::ContinuousNonNegative,
                mean_per90: xg_per90.max(1e-3),
                variance_per90: xg_per90.max(1e-3) * variance_factor * 1.5,
                epistemic_sigma: (xg_per90.max(0.01) / n.sqrt()).min(0.05),
                reference_per90: league_xg_per90,
                aggregate_matchup_sensitivity: 0.04,
                attribute_matchup_sensitivity: 0.03,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.08,
            });
            // Attribute proxies derived from the same underlying stats (technical ~ passing above
            // league rate; finishing ~ xG per shot; defending ~ recoveries+pressures share).
            let technical: f64 = 12.0 + 2.0 * ((pass_per90 - league_pass_per90) / 10.0);
            let finishing = 12.0 + 3.0 * ((xg_per90 / shot_per90.max(0.05)) - 0.115) * 20.0;
            let defensive_rate = (value.recovery + value.pressure) / value.minutes * 90.0;
            let defending = 12.0 + 2.0 * ((defensive_rate - 8.0) / 10.0);
            attribute_states.push(PlayerAttributeState {
                player: *player,
                attributes: vec![
                    PlayerAttributeValue { attribute: AttributeId(ATTR_TECHNICAL), value: technical.clamp(4.0, 19.0) },
                    PlayerAttributeValue { attribute: AttributeId(ATTR_FINISHING), value: finishing.clamp(4.0, 19.0) },
                    PlayerAttributeValue { attribute: AttributeId(ATTR_DEFENDING), value: defending.clamp(4.0, 19.0) },
                ],
            });
            player_states.push(PlayerMatchupState {
                player: *player,
                ability: footy_forecast_stub_ability(ability_index),
            });
        }
        candidates.sort_by(|left, right| right.start_probability.total_cmp(&left.start_probability));
        candidates_of.insert(club, candidates);
    }

    let formation = |id: u16| FormationTemplate {
        id,
        slots: (0_u8..11)
            .map(|slot| TacticalSlot {
                slot,
                role: RoleId(u16::from(slot) + 1),
                x: 0.06 + f64::from(slot % 11) * 0.075,
                y: f64::from(slot % 5) / 4.0,
            })
            .collect(),
    };
    let mut lineup_input = |club: ClubId| {
        let mut candidates = candidates_of.remove(&club).expect("squad candidates");
        while candidates.len() < 11 {
            let filler = PlayerId(u64::from(club.0) * 10000 + candidates.len() as u64 + 990000);
            candidates.push(LineupCandidate {
                player: filler,
                start_probability: 0.02,
                availability_probability: 0.9,
                ability_index: 8.0,
                fatigue: 0.1,
                expected_minutes_if_start: 45.0,
                minutes_sigma: 12.0,
                substitute_probability: 0.5,
                role_fits: (1_u16..=11).map(|role| RoleFit { role: RoleId(role), fit: 0.8 }).collect(),
            });
            // Fillers must satisfy every per-player validation requirement.
            event_models.push(SpecificPlayerEventModel {
                player: filler,
                metric: MetricId(METRIC_SHOT),
                family: StatFamily::Count,
                mean_per90: league_shot_per90,
                variance_per90: league_shot_per90 * 2.0,
                epistemic_sigma: 0.15,
                reference_per90: league_shot_per90,
                aggregate_matchup_sensitivity: 0.04,
                attribute_matchup_sensitivity: 0.03,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.10,
            });
            event_models.push(SpecificPlayerEventModel {
                player: filler,
                metric: MetricId(METRIC_PASS),
                family: StatFamily::Count,
                mean_per90: league_pass_per90,
                variance_per90: league_pass_per90 * 2.0,
                epistemic_sigma: 2.5,
                reference_per90: league_pass_per90,
                aggregate_matchup_sensitivity: 0.02,
                attribute_matchup_sensitivity: 0.015,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.08,
            });
            event_models.push(SpecificPlayerEventModel {
                player: filler,
                metric: MetricId(METRIC_XG),
                family: StatFamily::ContinuousNonNegative,
                mean_per90: league_xg_per90,
                variance_per90: league_xg_per90 * 3.0,
                epistemic_sigma: 0.05,
                reference_per90: league_xg_per90,
                aggregate_matchup_sensitivity: 0.04,
                attribute_matchup_sensitivity: 0.03,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.08,
            });
            attribute_states.push(PlayerAttributeState {
                player: filler,
                attributes: vec![
                    PlayerAttributeValue { attribute: AttributeId(ATTR_TECHNICAL), value: 12.0 },
                    PlayerAttributeValue { attribute: AttributeId(ATTR_FINISHING), value: 12.0 },
                    PlayerAttributeValue { attribute: AttributeId(ATTR_DEFENDING), value: 12.0 },
                ],
            });
            player_states.push(PlayerMatchupState {
                player: filler,
                ability: footy_forecast_stub_ability(8.0),
            });
        }
        LineupProjectionInput {
            club,
            formations: vec![FormationPrior { formation: formation(442), probability: 0.6 }, FormationPrior { formation: formation(433), probability: 0.4 }],
            candidates,
            locked_starters: Vec::new(),
            parameters: LineupSamplingParameters::default(),
        }
    };
    let home_lineup = lineup_input(home_club);
    let away_lineup = lineup_input(away_club);

    let channel_graph = CausalChannelGraph {
        metric_loadings: vec![
            MetricChannelLoading { metric: MetricId(METRIC_PASS), channel: TeamProcessChannel::Possession, weight: 0.5 },
            MetricChannelLoading { metric: MetricId(METRIC_PASS), channel: TeamProcessChannel::Progression, weight: 0.25 },
            MetricChannelLoading { metric: MetricId(METRIC_CARRY), channel: TeamProcessChannel::Progression, weight: 0.3 },
            MetricChannelLoading { metric: MetricId(METRIC_XG), channel: TeamProcessChannel::ShotQuality, weight: 1.0 },
            MetricChannelLoading { metric: MetricId(METRIC_SHOT), channel: TeamProcessChannel::ShotVolume, weight: 1.0 },
            MetricChannelLoading { metric: MetricId(METRIC_RECOVERY), channel: TeamProcessChannel::CounterPress, weight: 0.5 },
            MetricChannelLoading { metric: MetricId(METRIC_PRESSURE), channel: TeamProcessChannel::Pressing, weight: 0.5 },
        ],
        edges: vec![
            ChannelEdge { from: TeamProcessChannel::Possession, to: TeamProcessChannel::Progression, weight: 0.5 },
            ChannelEdge { from: TeamProcessChannel::Progression, to: TeamProcessChannel::ChanceCreation, weight: 0.6 },
            ChannelEdge { from: TeamProcessChannel::ShotVolume, to: TeamProcessChannel::ChanceCreation, weight: 0.5 },
            ChannelEdge { from: TeamProcessChannel::ShotQuality, to: TeamProcessChannel::Finishing, weight: 0.8 },
            ChannelEdge { from: TeamProcessChannel::ChanceCreation, to: TeamProcessChannel::BoxEntries, weight: 0.6 },
            ChannelEdge { from: TeamProcessChannel::Pressing, to: TeamProcessChannel::TransitionDefense, weight: 0.5 },
            ChannelEdge { from: TeamProcessChannel::TransitionDefense, to: TeamProcessChannel::DefensiveSuppression, weight: 0.7 },
        ],
        goal_loadings: vec![
            GoalChannelLoading { channel: TeamProcessChannel::Finishing, attack_log_weight: 0.30, defense_log_weight: 0.0 },
            GoalChannelLoading { channel: TeamProcessChannel::ChanceCreation, attack_log_weight: 0.18, defense_log_weight: 0.0 },
            GoalChannelLoading { channel: TeamProcessChannel::DefensiveSuppression, attack_log_weight: 0.0, defense_log_weight: 0.25 },
        ],
    };

    let team_context = |club: ClubId, window: &footy_performance::TeamPerformanceWindow| {
        let gf_per_match = f64::from(window.form.goals_for) / f64::from(window.form.matches.max(1));
        let ga_per_match = f64::from(window.form.goals_against) / f64::from(window.form.matches.max(1));
        SpecificTeamContext {
            club,
            residual_attack_log_adjustment: ((gf_per_match - 1.35) / 1.35 * 0.35).clamp(-0.35, 0.35),
            residual_defense_log_adjustment: ((1.35 - ga_per_match) / 1.35 * 0.35).clamp(-0.35, 0.35),
            metric_modifiers: vec![
                SpecificTeamMetricModifier { metric: MetricId(METRIC_SHOT), production_multiplier: 1.0, opponent_suppression_multiplier: 1.0 },
            ],
        }
    };

    println!(
        "squads: home candidates {} / away candidates {}, event models {}, attribute states {}",
        home_lineup.candidates.len(),
        away_lineup.candidates.len(),
        event_models.len(),
        attribute_states.len()
    );
    footy_match::PrimitiveMatchSimulationInput {
        match_id: MatchId(fx_id.parse().unwrap()),
        home: home_club,
        away: away_club,
        home_lineup,
        away_lineup,
        player_states,
        attribute_states,
        event_models,
        own_attribute_loadings: vec![
            AttributeEventLoading { metric: MetricId(METRIC_SHOT), attribute: AttributeId(ATTR_FINISHING), reference: 12.0, scale: 6.0, log_rate_weight: 0.08 },
            AttributeEventLoading { metric: MetricId(METRIC_PASS), attribute: AttributeId(ATTR_TECHNICAL), reference: 12.0, scale: 6.0, log_rate_weight: 0.06 },
        ],
        aggregate_matchup_rules: vec![
            footy_match::MatchupRule { metric: Some(MetricId(METRIC_SHOT)), attacker_dimension: footy_match::AbilityDimension::Finishing, defender_dimension: footy_match::AbilityDimension::Defending, weight: 1.0, scale: 6.0 },
            footy_match::MatchupRule { metric: Some(MetricId(METRIC_PASS)), attacker_dimension: footy_match::AbilityDimension::BallProgression, defender_dimension: footy_match::AbilityDimension::Defending, weight: 1.0, scale: 6.0 },
        ],
        attribute_matchup_rules: vec![
            AttributeMatchupRule { metric: Some(MetricId(METRIC_SHOT)), attacker_attribute: AttributeId(ATTR_FINISHING), defender_attribute: AttributeId(ATTR_DEFENDING), weight: 1.0, scale: 6.0 },
        ],
        synergies: Vec::<PairwiseSynergy>::new(),
        channel_graph,
        score_state_modifiers: vec![
            SpecificScoreStateMetricModifier { metric: MetricId(METRIC_SHOT), trailing_multiplier: 1.08, leading_multiplier: 0.94 },
            SpecificScoreStateMetricModifier { metric: MetricId(METRIC_PASS), trailing_multiplier: 1.05, leading_multiplier: 1.02 },
        ],
        incident_profiles: Vec::new(),
        in_match: SpecificInMatchParameters {
            injury_probability_per90: 0.02,
            red_card_probability_per90: 0.025,
            max_substitutions: 3,
            minimum_tactical_substitution_minute: 58.0,
        },
        home_context: team_context(home_club, home_window),
        away_context: team_context(away_club, away_window),
        environment: SpecificMatchEnvironment {
            home_residual_attack_log_adjustment: 0.10,
            away_residual_attack_log_adjustment: 0.0,
            metric_modifiers: vec![SpecificEnvironmentMetricModifier { metric: MetricId(METRIC_SHOT), home_multiplier: 1.03, away_multiplier: 0.98 }],
        },
        goal_process: SpecificGoalProcess {
            goals_per_team_match: 1.38,
            home_advantage_multiplier: 1.10,
            path_log_rate_sigma: 0.12,
            path_rate_correlation: 0.0,
            max_abs_segment_log_adjustment: 1.2,
            segments: 6,
        },
        matchup_spatial_sigma: 0.25,
        max_player_rate_log_adjustment: 1.0,
    }
    .tap_validate(fx_date)
}

trait ValidateTap {
    fn tap_validate(self, fx_date: Date) -> footy_match::PrimitiveMatchSimulationInput;
}

impl ValidateTap for footy_match::PrimitiveMatchSimulationInput {
    fn tap_validate(self, _fx_date: Date) -> footy_match::PrimitiveMatchSimulationInput {
        if !self.validate() {
            eprintln!("validation diagnostics:");
            eprintln!("  home==away: {}", self.home == self.away);
            eprintln!("  home_lineup valid: {}", self.home_lineup.validate());
            eprintln!("  away_lineup valid: {}", self.away_lineup.validate());
            // `valid()` is private; replicate the checks inline.
            let bad_models = self
                .event_models
                .iter()
                .filter(|value| {
                    !(value.mean_per90.is_finite()
                        && value.variance_per90.is_finite()
                        && value.epistemic_sigma.is_finite()
                        && value.reference_per90.is_finite()
                        && value.reference_per90 > 0.0)
                })
                .count();
            eprintln!("  suspicious event models (non-finite): {bad_models}");
            let dup_models = {
                let mut seen = std::collections::BTreeSet::new();
                self.event_models.iter().any(|value| !seen.insert((value.player, value.metric)))
            };
            eprintln!("  duplicate event models: {dup_models}");
            let candidates: Vec<PlayerId> = self.home_lineup.candidates.iter().chain(self.away_lineup.candidates.iter()).map(|value| value.player).collect();
            let states: std::collections::BTreeSet<PlayerId> = self.player_states.iter().map(|value| value.player).collect();
            let models: std::collections::BTreeSet<PlayerId> = self.event_models.iter().map(|value| value.player).collect();
            let attributes: std::collections::BTreeSet<PlayerId> = self.attribute_states.iter().map(|value| value.player).collect();
            for player in &candidates {
                if !states.contains(player) { eprintln!("  candidate missing matchup state: {player:?}"); }
                if !models.contains(player) { eprintln!("  candidate missing event model: {player:?}"); }
                if !attributes.contains(player) { eprintln!("  candidate missing attribute state: {player:?}"); }
            }
            panic!("calibrated match input failed validation");
        }
        self
    }
}

fn footy_forecast_stub_ability(index: f64) -> footy_state::AbilityVector {
    // Latent aggregate ability derived from usage/production; matchup rules consume the
    // finishing/defending/ball-progression dimensions most.
    let base = index as f32;
    footy_state::AbilityVector::from_components([
        base, base * 0.95, base, base * 0.9, base * 0.9, base * 0.85, base * 0.9,
        base, base, 6.5,
    ])
}
