#!/usr/bin/env python3
"""Score the historical same-engine walk-forward run against real labels and causal baselines.

Inputs:
- `artifacts/validation/pl-hist-same-engine-paths.json` from `footy-matchforecast-pl-hist`
- `data/statsbomb/pl_hist/{fixtures.json,labels/*.json,players/*.csv,team_rows.csv}` from
  `scripts/build_pl_hist_inputs.py`

Output: a machine-readable OOS evidence report (`--output`) plus a human summary on stdout.

Scoring rules follow `configs/forecast_targets_v1.json`:
- match W/D/L: multiclass log loss (Brier secondary)
- exact score: joint log loss (RPS secondary)
- team goals: discrete CRPS (MAE secondary)
- player goal/assist: occurrence Brier
- player appearance: appearance Brier + minutes MAE
- 15-minute and 1-minute goal occurrence: Brier
- evolving score state: multiclass Brier

Causal baselines (fit only on earlier matches, never on OOS fixtures):
- regularized independent Poisson with home advantage from earlier results
- league-average Poisson
- previous-only player per-match rates (empirical-Bayes shrunk)
- previous-only per-window team goal hazards

The evaluation split is chronological. Calibration is frozen at the OOS start: the temperature /
draw-bias parameters fitted by the Rust engine on all *fit-period* fixtures are applied unchanged to
every OOS fixture. Nothing in the OOS window informs the model or the calibration.
"""
from __future__ import annotations

import argparse
import json
import math
import random
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from run_pl_pipeline import model_code_fingerprint  # noqa: E402  (shared engine fingerprint)

ENGINE_MODEL = "simulate_second_resolved_dynamic_mental_match"
EPS = 1e-12


def read_json(path):
    return json.loads(Path(path).read_text())


def poisson_pmf(lam, k_max=15):
    if lam <= 0.0:
        return [1.0] + [0.0] * k_max
    values = []
    log_p = -lam
    for k in range(k_max + 1):
        if k > 0:
            log_p += math.log(lam) - math.log(k)
        values.append(math.exp(log_p))
    return values


def score_grid(lam_home, lam_away, k_max=12):
    home = poisson_pmf(lam_home, k_max)
    away = poisson_pmf(lam_away, k_max)
    grid = {}
    for h, ph in enumerate(home):
        for a, pa in enumerate(away):
            grid[(h, a)] = ph * pa
    return grid


def outcome_from_grid(grid):
    home = sum(p for (h, a), p in grid.items() if h > a)
    draw = sum(p for (h, a), p in grid.items() if h == a)
    away = sum(p for (h, a), p in grid.items() if h < a)
    return [home, draw, away]


def pmf_marginals(grid):
    home, away = defaultdict(float), defaultdict(float)
    for (h, a), p in grid.items():
        home[h] += p
        away[a] += p
    return dict(home), dict(away)


def log_loss(probabilities, actual):
    return -math.log(max(probabilities[actual], EPS))


def brier(probabilities, actual):
    return sum((p - (1.0 if index == actual else 0.0)) ** 2 for index, p in enumerate(probabilities))


def multiclass_brier(probabilities, actual):
    return brier(probabilities, actual)


def joint_log_loss(grid, home_goals, away_goals, floor):
    probability = max(grid.get((home_goals, away_goals), 0.0), floor)
    return -math.log(probability)


def ranked_probability_score(grid, home_goals, away_goals, k_max=12):
    """Ranked probability score for two ordered marginals (home goals, away goals) + total."""
    total = 0.0
    home_pmf, away_pmf = pmf_marginals(grid)
    for pmf, actual in ((home_pmf, home_goals), (away_pmf, away_goals)):
        cumulative = 0.0
        for k in range(k_max + 1):
            cumulative += pmf.get(k, 0.0)
            target = 1.0 if k >= actual else 0.0
            total += (cumulative - target) ** 2
    return total / 2.0


def discrete_crps(pmf, actual, k_max=12):
    cumulative = 0.0
    score = 0.0
    for k in range(k_max + 1):
        cumulative += pmf.get(k, 0.0)
        target = 1.0 if k >= actual else 0.0
        score += (cumulative - target) ** 2
    return score


def count_log_loss(pmf, actual, floor):
    return -math.log(max(pmf.get(actual, 0.0), floor))


def occurrence_from_mean(mean):
    return 1.0 - math.exp(-max(mean, 0.0))


def apply_calibrator(raw, temperature, draw_bias):
    logits = [math.log(max(p, EPS)) / temperature for p in raw]
    logits[1] += draw_bias
    maximum = max(logits)
    weights = [math.exp(value - maximum) for value in logits]
    total = sum(weights)
    return [value / total for value in weights]


def short(name):
    return name.split(" ")[0]


class TeamHistory:
    """Causal team goal/shots history from the prepared team rows."""

    def __init__(self, path):
        rows = []
        with open(path) as handle:
            header = handle.readline().strip().split(",")
            for line in handle:
                cells = line.rstrip("\n").split(",")
                if not cells or len(cells) != len(header):
                    continue
                row = dict(zip(header, cells))
                rows.append(row)
        self.rows = sorted(rows, key=lambda r: r["date"])
        self.by_team = defaultdict(list)
        for row in self.rows:
            self.by_team[row["home"]].append(row)
            self.by_team[row["away"]].append(row)

    def prior(self, team, date_text):
        return [row for row in self.by_team.get(team, []) if row["date"] < date_text]

    def league(self, date_text):
        home_goals = away_goals = 0.0
        matches = 0
        for row in self.rows:
            if row["date"] >= date_text:
                continue
            home_goals += float(row["home_goals"])
            away_goals += float(row["away_goals"])
            matches += 1
        if matches == 0:
            return 1.35, 1.10
        return home_goals / matches, away_goals / matches

    def poisson_rates(self, home, away, date_text, prior_strength=5.0):
        league_home, league_away = self.league(date_text)
        league = (league_home + league_away) / 2.0

        def team_rates(team):
            rows = self.prior(team, date_text)
            goals_for = goals_against = 0.0
            for row in rows:
                if row["home"] == team:
                    goals_for += float(row["home_goals"])
                    goals_against += float(row["away_goals"])
                else:
                    goals_for += float(row["away_goals"])
                    goals_against += float(row["home_goals"])
            n = len(rows)
            attack = (goals_for + prior_strength * league) / (n + prior_strength) / league
            defense = (goals_against + prior_strength * league) / (n + prior_strength) / league
            return attack, defense

        attack_home, defense_home = team_rates(home)
        attack_away, defense_away = team_rates(away)
        home_advantage = max(1.0, min(1.35, league_home / max(league_away, EPS)))
        lam_home = attack_home * defense_away * league * home_advantage
        lam_away = attack_away * defense_home * league
        return max(lam_home, 0.05), max(lam_away, 0.05)


class WindowHistory:
    """Previous-only per-window team goal hazards from covered matches."""

    def __init__(self, labels, fixtures):
        self.team_window_goals = defaultdict(lambda: [0] * 6)  # team -> goals per 15-min window
        self.team_matches = defaultdict(int)
        self.league_window_goals = [0] * 6
        self.league_matches = 0
        self.team_minute_goals = defaultdict(lambda: [0] * 90)
        self.league_minute_goals = [0] * 90
        self.state_after = [[0.0, 0.0, 0.0] for _ in range(6)]  # window -> [home_lead, level, away_lead]
        self.state_matches = 0
        self.league_red_windows = [0] * 6
        self.league_sub_windows = [0] * 6
        for fixture in fixtures:
            mid = fixture["match_id"]
            label = labels.get(mid)
            if label is None:
                continue
            goals = label["goals"]
            for club in (fixture["home"], fixture["away"]):
                self.team_matches[club] += 1
            self.league_matches += 1
            for goal in goals:
                if goal["own_goal"]:
                    continue
                window = min(5, goal["minute_index"] // 15)
                self.team_window_goals[goal["team"]][window] += 1
                self.league_window_goals[window] += 1
                minute = min(89, goal["minute_index"])
                self.team_minute_goals[goal["team"]][minute] += 1
                self.league_minute_goals[minute] += 1
            home = away = 0
            for window in range(6):
                for goal in goals:
                    if goal["own_goal"] or min(5, goal["minute_index"] // 15) != window:
                        continue
                    if goal["team"] == label["home"]:
                        home += 1
                    else:
                        away += 1
                state = 0 if home > away else (1 if home == away else 2)
                self.state_after[window][state] += 1
            self.state_matches += 1
            for red in label["reds"]:
                self.league_red_windows[min(5, red["minute_index"] // 15)] += 1
            for sub in label["subs"]:
                self.league_sub_windows[min(5, sub["minute_index"] // 15)] += 1

    def state_prior(self, window):
        total = max(self.state_matches, 1)
        counts = [value / total for value in self.state_after[window]]
        # Laplace smoothing toward the league-average state distribution.
        smoothed = [(value * total + 1.0) / (total + 3.0) for value in counts]
        return smoothed

    def red_prior(self, window):
        return (self.league_red_windows[window] + 0.5) / (self.league_matches + 1.0)

    def sub_prior(self, window):
        return (self.league_sub_windows[window] + 0.5) / (self.league_matches + 1.0)

    def window_rates(self, home, away, prior_strength=4.0):
        """Poisson rate for P(any goal in window) for the fixture, league-shrunk per team."""
        rates = []
        for window in range(6):
            league_rate = self.league_window_goals[window] / max(self.league_matches, 1)
            total = 0.0
            for club in (home, away):
                n = self.team_matches.get(club, 0)
                goals = self.team_window_goals[club][window]
                total += (goals + prior_strength * league_rate) / (n + prior_strength)
            rates.append(total)
        return rates

    def minute_rates(self, home, away, prior_strength=8.0):
        league_rate = [value / max(self.league_matches, 1) for value in self.league_minute_goals]
        rates = []
        for minute in range(90):
            total = 0.0
            for club in (home, away):
                n = self.team_matches.get(club, 0)
                goals = self.team_minute_goals[club][minute]
                total += (goals + prior_strength * league_rate[minute]) / (n + prior_strength)
            rates.append(total)
        return rates


def prior_player_rates(player_csv):
    """Previous-only per-match player rates from the causal input rows."""
    import csv as csv_module
    rates = {}
    with open(player_csv) as handle:
        for row in csv_module.DictReader(handle):
            uid = int(row["player_id"])
            listings = float(row["selection_listings"])
            appearances = float(row["selection_appearances"])
            starts = float(row["selection_starts"])
            minutes = float(row["minutes"])
            team_fixtures = float(row["team_fixtures"])
            shots = float(row.get("shots_p90", 0.0)) * minutes / 90.0 if minutes > 0 else 0.0
            goals = float(row.get("goals_p90", 0.0)) * minutes / 90.0 if minutes > 0 else 0.0
            assists = float(row.get("assists_p90", 0.0)) * minutes / 90.0 if minutes > 0 else 0.0
            rates[uid] = {
                "start_rate": (starts + 0.5 * 2.0) / (team_fixtures + 2.0),
                "appearance_rate": (appearances + 0.75 * 2.0) / (team_fixtures + 2.0),
                "minutes_prior": (minutes + 45.0) / (team_fixtures + 1.0),
                "shots_prior": (shots + 3.0 * 1.25) / (team_fixtures + 3.0),
                "goals_prior": (goals + 3.0 * 0.15) / (team_fixtures + 3.0),
                "assists_prior": (assists + 3.0 * 0.10) / (team_fixtures + 3.0),
            }
    return rates


def goal_windows(label):
    windows = [0] * 6
    for goal in label["goals"]:
        if goal["own_goal"]:
            continue
        windows[min(5, goal["minute_index"] // 15)] += 1
    return windows


def goal_minutes(label):
    minutes = [0] * 90
    for goal in label["goals"]:
        if goal["own_goal"]:
            continue
        minutes[min(89, goal["minute_index"])] += 1
    return minutes


def state_after_windows(label):
    """Observed score state (home lead=0, level=1, away lead=2) after each 15-minute window."""
    states = []
    home = away = 0
    for window in range(6):
        for goal in label["goals"]:
            if goal["own_goal"]:
                continue
            if goal["minute_index"] // 15 == window or (window == 5 and goal["minute_index"] >= 75):
                if goal["team"] == label["home"]:
                    home += 1
                else:
                    away += 1
        states.append(0 if home > away else (1 if home == away else 2))
    return states


def bootstrap_ci(per_date, statistic, iterations=2000, seed=42):
    """Matchday-block bootstrap over resampled calendar dates."""
    dates = sorted(per_date)
    if len(dates) < 2:
        return [float("nan"), float("nan")]
    rng = random.Random(seed)
    values = []
    for _ in range(iterations):
        sampled = [dates[rng.randrange(len(dates))] for _ in range(len(dates))]
        rows = [row for date in sampled for row in per_date[date]]
        values.append(statistic(rows))
    values.sort()
    lower = values[int(0.025 * len(values))]
    upper = values[int(0.975 * len(values)) - 1]
    return [lower, upper]


def comparison_metrics(fixture, label, paths):
    """Headline metric subset for a second (experiment) prediction document."""
    home_goals = int(fixture["home_goals_actual"])
    away_goals = int(fixture["away_goals_actual"])
    actual_outcome = 0 if home_goals > away_goals else (1 if home_goals == away_goals else 2)
    raw = fixture["raw_probabilities"]
    grid = {(int(h), int(a)): p for h, a, p in fixture["scoreline_distribution"]}
    floor = 0.5 / max(int(paths), 1)
    home_pmf, away_pmf = pmf_marginals(grid)
    row = {
        "wdl_log_loss_raw": log_loss(raw, actual_outcome),
        "exact_score_log_loss_engine": joint_log_loss(grid, home_goals, away_goals, floor),
        "team_goals_crps_engine": (discrete_crps(home_pmf, home_goals) + discrete_crps(away_pmf, away_goals)) / 2.0,
    }
    label_players = {int(p["player_id"]): p for p in label["players"]}
    predicted_players = {int(p["uid"]): p for p in fixture["players"]}
    goal_brier = assist_brier = 0.0
    start_brier = appearance_brier = minutes_mae = 0.0
    for uid, prediction in predicted_players.items():
        actual = label_players.get(uid) or {"goals": 0.0, "assists": 0.0, "minutes": 0.0, "started": False}
        goal_brier += (prediction["goal_occurrence_probability"] - float(actual["goals"] > 0)) ** 2
        assist_brier += (prediction["assist_occurrence_probability"] - float(actual["assists"] > 0)) ** 2
        start_brier += (prediction["start_probability"] - float(bool(actual["started"]))) ** 2
        appearance_brier += (prediction["appearance_probability"] - float(actual["minutes"] > 0)) ** 2
        minutes_mae += abs(prediction["expected_minutes"] - actual["minutes"])
    count = max(len(predicted_players), 1)
    row["player_goal_occurrence_brier_engine"] = goal_brier / count
    row["player_assist_occurrence_brier_engine"] = assist_brier / count
    row["player_start_brier_engine"] = start_brier / count
    row["player_appearance_brier_engine"] = appearance_brier / count
    row["player_minutes_mae_engine"] = minutes_mae / count
    actual_windows = goal_windows(label)
    row["window15_brier_engine"] = sum(
        (window["p_any_goal"] - float(actual_windows[index] > 0)) ** 2
        for index, window in enumerate(fixture["windows_15min"])
    ) / 6.0
    actual_minutes = goal_minutes(label)
    row["minute_brier_engine"] = sum(
        (minute["p_any_goal"] - float(actual_minutes[index] > 0)) ** 2
        for index, minute in enumerate(fixture["minutes_1min"])
    ) / 90.0
    return row



def chronological_split(fixtures, oos_matches):
    """Last N fixtures, extended backwards so no calendar date straddles the boundary."""
    if len(fixtures) <= oos_matches:
        raise ValueError("not enough fixtures for the requested OOS window")
    start = len(fixtures) - oos_matches
    boundary = fixtures[start]["date"]
    while start > 0 and fixtures[start - 1]["date"] == boundary:
        start -= 1
    if start == 0:
        raise ValueError("OOS extension consumed every fixture")
    return fixtures[:start], fixtures[start:]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--predictions", default="artifacts/validation/pl-hist-same-engine-paths.json")
    parser.add_argument("--hist-dir", default="data/statsbomb/pl_hist")
    parser.add_argument("--output", default="artifacts/validation/pl-hist-same-engine-oos.json")
    parser.add_argument("--oos-matches", type=int, default=24)
    parser.add_argument("--bootstrap", type=int, default=2000)
    parser.add_argument("--experiment", default=None, help="experiment label recorded in the report")
    parser.add_argument("--compare-predictions", default=None,
                        help="second same-engine path export (e.g. a calibrated-contract experiment) "
                             "to compare against this run on the identical OOS fixtures")
    args = parser.parse_args()

    hist = ROOT / args.hist_dir
    predictions = read_json(ROOT / args.predictions)
    fixtures_doc = read_json(hist / "fixtures.json")
    fixture_meta = {int(f["match_id"]): f for f in fixtures_doc["fixtures"]}
    labels = {}
    for mid in fixture_meta:
        path = hist / f"labels/{mid}.json"
        if path.is_file():
            labels[mid] = read_json(path)
    team_history = TeamHistory(hist / "team_rows.csv")

    simulated = sorted(predictions["fixtures"], key=lambda f: (f["date"], f["match_id"]))
    if len(simulated) < args.oos_matches + 6:
        raise SystemExit(f"only {len(simulated)} simulated fixtures; need more than {args.oos_matches + 6}")
    try:
        fit, oos = chronological_split(simulated, args.oos_matches)
    except ValueError as error:
        raise SystemExit(str(error))
    fit_end_date = fit[-1]["date"]
    oos_start_date = oos[0]["date"]
    oos_end_date = oos[-1]["date"]

    # Frozen calibration: parameters fitted on all fit-period fixtures only.
    last_fit = fit[-1]["calibration"]
    temperature = last_fit["temperature"]
    draw_bias = last_fit["draw_logit_bias"]

    # Previous-only window/minute history uses every covered fixture before each OOS date; the
    # WindowHistory object is rebuilt per date below to stay strictly causal.
    window_histories = {}
    for fixture in oos:
        prior_fixtures = [meta for meta in fixture_meta.values() if meta["date"] < fixture["date"]]
        window_histories[fixture["match_id"]] = WindowHistory(
            {meta["match_id"]: labels[meta["match_id"]] for meta in prior_fixtures if meta["match_id"] in labels},
            prior_fixtures,
        )

    # Per-fixture metric rows for the bootstrap.
    metric_rows = defaultdict(list)
    coverage = {
        "oos_fixtures": len(oos),
        "actual_non_own_goals": 0,
        "goals_by_pool_players": 0,
        "actual_assists": 0,
        "assists_by_pool_players": 0,
        "player_appearances": 0,
        "player_rows_scored": 0,
        "club_counts": defaultdict(int),
    }

    for fixture in oos:
        mid = fixture["match_id"]
        label = labels.get(mid)
        if label is None:
            continue
        date = fixture["date"]
        home, away = fixture["home"], fixture["away"]
        coverage["club_counts"][home] += 1
        coverage["club_counts"][away] += 1
        home_goals = int(fixture["home_goals_actual"])
        away_goals = int(fixture["away_goals_actual"])
        actual_outcome = 0 if home_goals > away_goals else (1 if home_goals == away_goals else 2)

        raw = fixture["raw_probabilities"]
        calibrated = apply_calibrator(raw, temperature, draw_bias)
        lam_home, lam_away = team_history.poisson_rates(home, away, date)
        poisson_grid = score_grid(lam_home, lam_away)
        poisson_probs = outcome_from_grid(poisson_grid)
        league_home, league_away = team_history.league(date)
        league_grid = score_grid(league_home, league_away)
        league_probs = outcome_from_grid(league_grid)

        engine_grid = {(int(h), int(a)): p for h, a, p in fixture["scoreline_distribution"]}
        floor = 0.5 / max(int(predictions["paths"]), 1)

        row = {}
        row["match_id"] = mid
        row["wdl_log_loss_raw"] = log_loss(raw, actual_outcome)
        row["wdl_log_loss_calibrated"] = log_loss(calibrated, actual_outcome)
        row["wdl_log_loss_poisson"] = log_loss(poisson_probs, actual_outcome)
        row["wdl_log_loss_league"] = log_loss(league_probs, actual_outcome)
        row["wdl_brier_raw"] = multiclass_brier(raw, actual_outcome)
        row["wdl_brier_poisson"] = multiclass_brier(poisson_probs, actual_outcome)
        row["exact_score_log_loss_engine"] = joint_log_loss(engine_grid, home_goals, away_goals, floor)
        row["exact_score_log_loss_poisson"] = joint_log_loss(poisson_grid, home_goals, away_goals, floor)
        row["exact_score_rps_engine"] = ranked_probability_score(engine_grid, home_goals, away_goals)
        row["exact_score_rps_poisson"] = ranked_probability_score(poisson_grid, home_goals, away_goals)

        home_pmf, away_pmf = pmf_marginals(engine_grid)
        poisson_home_pmf, poisson_away_pmf = pmf_marginals(poisson_grid)
        row["team_goals_crps_engine"] = (discrete_crps(home_pmf, home_goals) + discrete_crps(away_pmf, away_goals)) / 2.0
        row["team_goals_crps_poisson"] = (discrete_crps(poisson_home_pmf, home_goals) + discrete_crps(poisson_away_pmf, away_goals)) / 2.0
        row["team_goals_mae_engine"] = (abs(fixture["expected_home_goals"] - home_goals) + abs(fixture["expected_away_goals"] - away_goals)) / 2.0
        row["team_goals_mae_poisson"] = (abs(lam_home - home_goals) + abs(lam_away - away_goals)) / 2.0
        row["team_goals_count_log_loss_engine"] = (count_log_loss(home_pmf, home_goals, floor) + count_log_loss(away_pmf, away_goals, floor)) / 2.0
        row["team_goals_count_log_loss_poisson"] = (count_log_loss(poisson_home_pmf, home_goals, floor) + count_log_loss(poisson_away_pmf, away_goals, floor)) / 2.0

        # Player metrics on the actual matchday squad universe.
        label_players = {int(p["player_id"]): p for p in label["players"]}
        predicted_players = {int(p["uid"]): p for p in fixture["players"]}
        prior_rates = prior_player_rates(hist / f"players/{mid}.csv")
        for uid, prediction in predicted_players.items():
            actual = label_players.get(uid)
            coverage["player_rows_scored"] += 1
            if actual is None:
                actual = {"minutes": 0.0, "started": False, "shots": 0.0, "goals": 0.0, "assists": 0.0}
            else:
                coverage["player_appearances"] += int(actual["minutes"] > 0)
            row["player_start_brier_engine"] = row.get("player_start_brier_engine", 0.0) + (prediction["start_probability"] - float(bool(actual["started"]))) ** 2
            row["player_start_brier_prior"] = row.get("player_start_brier_prior", 0.0) + (prior_rates.get(uid, {}).get("start_rate", 0.5) - float(bool(actual["started"]))) ** 2
            row["player_appearance_brier_engine"] = row.get("player_appearance_brier_engine", 0.0) + (prediction["appearance_probability"] - float(actual["minutes"] > 0)) ** 2
            row["player_appearance_brier_prior"] = row.get("player_appearance_brier_prior", 0.0) + (prior_rates.get(uid, {}).get("appearance_rate", 0.5) - float(actual["minutes"] > 0)) ** 2
            row["player_minutes_mae_engine"] = row.get("player_minutes_mae_engine", 0.0) + abs(prediction["expected_minutes"] - actual["minutes"])
            row["player_minutes_mae_prior"] = row.get("player_minutes_mae_prior", 0.0) + abs(prior_rates.get(uid, {}).get("minutes_prior", 45.0) - actual["minutes"])
            shots_mean = prediction["metrics"].get("shots", {}).get("mean_unconditional", 0.0)
            row["player_shots_mae_engine"] = row.get("player_shots_mae_engine", 0.0) + abs(shots_mean - actual["shots"])
            row["player_shots_mae_prior"] = row.get("player_shots_mae_prior", 0.0) + abs(prior_rates.get(uid, {}).get("shots_prior", 1.0) - actual["shots"])
            row["player_shot_occurrence_brier_engine"] = row.get("player_shot_occurrence_brier_engine", 0.0) + (occurrence_from_mean(shots_mean) - float(actual["shots"] > 0)) ** 2
            row["player_shot_occurrence_brier_prior"] = row.get("player_shot_occurrence_brier_prior", 0.0) + (occurrence_from_mean(prior_rates.get(uid, {}).get("shots_prior", 1.0)) - float(actual["shots"] > 0)) ** 2
            row["player_goal_occurrence_brier_engine"] = row.get("player_goal_occurrence_brier_engine", 0.0) + (prediction["goal_occurrence_probability"] - float(actual["goals"] > 0)) ** 2
            row["player_goal_occurrence_brier_prior"] = row.get("player_goal_occurrence_brier_prior", 0.0) + (occurrence_from_mean(prior_rates.get(uid, {}).get("goals_prior", 0.15)) - float(actual["goals"] > 0)) ** 2
            row["player_assist_occurrence_brier_engine"] = row.get("player_assist_occurrence_brier_engine", 0.0) + (prediction["assist_occurrence_probability"] - float(actual["assists"] > 0)) ** 2
            row["player_assist_occurrence_brier_prior"] = row.get("player_assist_occurrence_brier_prior", 0.0) + (occurrence_from_mean(prior_rates.get(uid, {}).get("assists_prior", 0.10)) - float(actual["assists"] > 0)) ** 2
        row["player_count"] = len(predicted_players)
        # Convert per-fixture sums into per-player means so fixtures with different pool sizes
        # contribute equally to the OOS average and to the matchday bootstrap.
        for key in list(row):
            if key.startswith("player_") and key != "player_count":
                row[key] = row[key] / max(len(predicted_players), 1)

        # Coverage: actual goals/assists credited to pool players.
        for goal in label["goals"]:
            if goal["own_goal"]:
                continue
            coverage["actual_non_own_goals"] += 1
            if goal["player_id"] in predicted_players:
                coverage["goals_by_pool_players"] += 1
        for assist in label["assists"]:
            coverage["actual_assists"] += 1
            if assist["player_id"] in predicted_players:
                coverage["assists_by_pool_players"] += 1

        # Timeline metrics.
        actual_windows = goal_windows(label)
        for index, window in enumerate(fixture["windows_15min"]):
            row[f"window15_brier_engine_{index}"] = (window["p_any_goal"] - float(actual_windows[index] > 0)) ** 2
        window_rates = window_histories[mid].window_rates(home, away)
        for index, rate in enumerate(window_rates):
            row[f"window15_brier_prior_{index}"] = (occurrence_from_mean(rate) - float(actual_windows[index] > 0)) ** 2
        row["window15_brier_engine"] = sum(row[f"window15_brier_engine_{index}"] for index in range(6)) / 6.0
        actual_minutes = goal_minutes(label)
        for index, minute in enumerate(fixture["minutes_1min"]):
            row[f"minute_brier_engine_{index}"] = (minute["p_any_goal"] - float(actual_minutes[index] > 0)) ** 2
        minute_rates = window_histories[mid].minute_rates(home, away)
        for index, rate in enumerate(minute_rates):
            row[f"minute_brier_prior_{index}"] = (occurrence_from_mean(rate) - float(actual_minutes[index] > 0)) ** 2
        row["minute_brier_engine"] = sum(row[f"minute_brier_engine_{index}"] for index in range(90)) / 90.0
        actual_states = state_after_windows(label)
        for index, window in enumerate(fixture["windows_15min"]):
            row[f"state_brier_engine_{index}"] = multiclass_brier(window["end_of_window_outcomes"], actual_states[index])
            row[f"state_brier_prior_{index}"] = multiclass_brier(window_histories[mid].state_prior(index), actual_states[index])
        reds = [0] * 6
        subs = [0] * 6
        for red in label["reds"]:
            reds[min(5, red["minute_index"] // 15)] += 1
        for sub in label["subs"]:
            subs[min(5, sub["minute_index"] // 15)] += 1
        for index, window in enumerate(fixture["windows_15min"]):
            row[f"red_brier_engine_{index}"] = (window["p_red_card"] - float(reds[index] > 0)) ** 2
            row[f"red_brier_prior_{index}"] = (window_histories[mid].red_prior(index) - float(reds[index] > 0)) ** 2
            row[f"sub_brier_engine_{index}"] = (window["p_substitution"] - float(subs[index] > 0)) ** 2
            row[f"sub_brier_prior_{index}"] = (window_histories[mid].sub_prior(index) - float(subs[index] > 0)) ** 2

        metric_rows[date].append(row)

    # Aggregate metrics over the OOS window.
    def flatten(key):
        return [row[key] for rows in metric_rows.values() for row in rows if key in row]

    def mean(values):
        return sum(values) / len(values) if values else float("nan")

    def mean_difference(model_key, reference_key, rows):
        return mean([row[model_key] - row[reference_key] for row in rows if model_key in row and reference_key in row])

    summary = {}
    for key in (
        "wdl_log_loss_raw", "wdl_log_loss_calibrated", "wdl_log_loss_poisson", "wdl_log_loss_league",
        "wdl_brier_raw", "wdl_brier_poisson", "exact_score_log_loss_engine", "exact_score_log_loss_poisson",
        "exact_score_rps_engine", "exact_score_rps_poisson", "team_goals_crps_engine", "team_goals_crps_poisson",
        "team_goals_mae_engine", "team_goals_mae_poisson", "team_goals_count_log_loss_engine",
        "team_goals_count_log_loss_poisson",
    ):
        summary[key] = mean(flatten(key))

    # Player metrics are per-player sums inside each fixture row; average over fixtures.
    player_keys = sorted({key for rows in metric_rows.values() for row in rows for key in row
                          if key.startswith("player_") and key != "player_count"})
    for key in player_keys:
        summary[key] = mean(flatten(key))

    for prefix in ("window15_brier_engine_", "window15_brier_prior_", "minute_brier_engine_", "minute_brier_prior_",
                   "state_brier_engine_", "state_brier_prior_", "red_brier_engine_", "red_brier_prior_",
                   "sub_brier_engine_", "sub_brier_prior_"):
        summary[prefix.rstrip("_")] = mean([value for rows in metric_rows.values() for row in rows for key, value in row.items() if key.startswith(prefix)])

    # Bootstrap CIs of model-minus-reference differences on matchday blocks.
    def difference_statistic(model_key, reference_key):
        return lambda rows: mean_difference(model_key, reference_key, rows)

    bootstrap = {}
    for name, model_key, reference_key in (
        ("wdl_log_loss", "wdl_log_loss_raw", "wdl_log_loss_poisson"),
        ("wdl_log_loss_vs_league", "wdl_log_loss_raw", "wdl_log_loss_league"),
        ("exact_score_log_loss", "exact_score_log_loss_engine", "exact_score_log_loss_poisson"),
        ("team_goals_crps", "team_goals_crps_engine", "team_goals_crps_poisson"),
        ("player_goal_occurrence_brier", "player_goal_occurrence_brier_engine", "player_goal_occurrence_brier_prior"),
        ("player_assist_occurrence_brier", "player_assist_occurrence_brier_engine", "player_assist_occurrence_brier_prior"),
        ("player_start_brier", "player_start_brier_engine", "player_start_brier_prior"),
        ("player_appearance_brier", "player_appearance_brier_engine", "player_appearance_brier_prior"),
        ("player_shots_mae", "player_shots_mae_engine", "player_shots_mae_prior"),
    ):
        per_date = {
            date: [row for row in rows if model_key in row and reference_key in row]
            for date, rows in metric_rows.items()
        }
        bootstrap[name] = bootstrap_ci(per_date, difference_statistic(model_key, reference_key), args.bootstrap)

    def timeline_bootstrap(prefix):
        per_date = {}
        for date, rows in metric_rows.items():
            pairs = []
            for row in rows:
                keys = [key for key in row if key.startswith(prefix + "engine_")]
                for key in keys:
                    reference = key.replace("engine", "prior")
                    if reference in row:
                        pairs.append((row[key], row[reference]))
            per_date[date] = pairs
        statistic = lambda pairs: mean([model - reference for model, reference in pairs]) if pairs else float("nan")
        return bootstrap_ci(per_date, statistic, args.bootstrap)

    bootstrap["window15_goal_brier"] = timeline_bootstrap("window15_brier_")
    bootstrap["minute_goal_brier"] = timeline_bootstrap("minute_brier_")
    bootstrap["state_brier"] = timeline_bootstrap("state_brier_")
    bootstrap["red_card_brier"] = timeline_bootstrap("red_brier_")
    bootstrap["substitution_brier"] = timeline_bootstrap("sub_brier_")

    # Optional paired comparison against a second same-engine run (calibration experiment).
    comparison = None
    if args.compare_predictions:
        second = read_json(ROOT / args.compare_predictions)
        second_by_id = {int(f["match_id"]): f for f in second["fixtures"]}
        date_by_id = {int(f["match_id"]): f["date"] for f in oos}
        baseline_by_id = {row["match_id"]: row for rows in metric_rows.values() for row in rows}
        keys = [
            "wdl_log_loss_raw", "exact_score_log_loss_engine", "team_goals_crps_engine",
            "player_goal_occurrence_brier_engine", "player_assist_occurrence_brier_engine",
            "player_start_brier_engine", "player_appearance_brier_engine", "player_minutes_mae_engine",
            "window15_brier_engine", "minute_brier_engine",
        ]
        paired_by_date = defaultdict(list)
        for mid, other_fixture in second_by_id.items():
            label = labels.get(mid)
            base_row = baseline_by_id.get(mid)
            if label is None or base_row is None:
                continue
            other_row = comparison_metrics(other_fixture, label, second["paths"])
            for key in keys:
                if key in base_row:
                    paired_by_date[date_by_id[mid]].append((key, base_row[key] - other_row[key]))
        comparison = {
            "source": args.compare_predictions,
            "paths": second["paths"],
            "input_contract": second["input_contract"],
            "note": "mean difference is metrics(--predictions) minus metrics(--compare-predictions); "
                    "with the documented experiment invocation (--predictions = experiment, "
                    "--compare-predictions = baseline) negative means the experiment run has lower "
                    "loss / better score",
            "metrics": {},
        }
        for key in keys:
            per_date = {date: [value for name, value in entries if name == key] for date, entries in paired_by_date.items()}
            per_date = {date: values for date, values in per_date.items() if values}
            statistic = lambda values: mean(values) if values else float("nan")
            mean_difference = mean([value for values in per_date.values() for value in values])
            comparison["metrics"][key] = {
                "mean_difference_predictions_minus_compare": mean_difference,
                "matched_day_bootstrap_95": bootstrap_ci(per_date, statistic, args.bootstrap),
            }

    coverage["club_counts"] = dict(sorted(coverage["club_counts"].items(), key=lambda item: -item[1]))
    coverage["goal_coverage"] = coverage["goals_by_pool_players"] / max(coverage["actual_non_own_goals"], 1)
    coverage["assist_coverage"] = coverage["assists_by_pool_players"] / max(coverage["actual_assists"], 1)

    def target(distinct, primary, model_value, reference_value, bootstrap_interval, label_coverage):
        return {
            "distinct_oos_fixtures": distinct,
            "primary_score": primary,
            "model_value": model_value,
            "causal_reference_value": reference_value,
            "matched_day_bootstrap_95": bootstrap_interval,
            "label_coverage": label_coverage,
        }

    targets = {
        "match_wdl": target(len(oos), "multiclass_log_loss", summary["wdl_log_loss_raw"],
                            summary["wdl_log_loss_poisson"], bootstrap["wdl_log_loss"], 1.0),
        "match_exact_score": target(len(oos), "joint_log_loss", summary["exact_score_log_loss_engine"],
                                    summary["exact_score_log_loss_poisson"], bootstrap["exact_score_log_loss"], 1.0),
        "team_goals": target(len(oos), "discrete_crps", summary["team_goals_crps_engine"],
                             summary["team_goals_crps_poisson"], bootstrap["team_goals_crps"], 1.0),
        "player_goal": target(len(oos), "player_goal_occurrence_brier", summary["player_goal_occurrence_brier_engine"],
                              summary["player_goal_occurrence_brier_prior"], bootstrap["player_goal_occurrence_brier"],
                              coverage["goal_coverage"]),
        "player_primary_assist": target(len(oos), "player_assist_occurrence_brier", summary["player_assist_occurrence_brier_engine"],
                                        summary["player_assist_occurrence_brier_prior"], bootstrap["player_assist_occurrence_brier"],
                                        coverage["assist_coverage"]),
        "player_appearance_minutes": target(len(oos), "appearance_brier_plus_minutes_mae", summary["player_appearance_brier_engine"],
                                            summary["player_appearance_brier_prior"], bootstrap["player_appearance_brier"], 1.0),
        "time_goal_and_shot_15min": target(len(oos), "window_goal_occurrence_brier", summary["window15_brier_engine"],
                                           summary["window15_brier_prior"], bootstrap["window15_goal_brier"], 1.0),
        "time_goal_1min": target(len(oos), "one_minute_goal_occurrence_brier", summary["minute_brier_engine"],
                                 summary["minute_brier_prior"], bootstrap["minute_goal_brier"], 1.0),
        "time_score_state": target(len(oos), "multiclass_brier", summary["state_brier_engine"],
                                   summary["state_brier_prior"], bootstrap["state_brier"], 1.0),
        "red_cards_substitutions": target(len(oos), "incident_occurrence_brier", summary["red_brier_engine"],
                                          summary["red_brier_prior"], bootstrap["red_card_brier"], 1.0),
    }

    report = {
        "schema_version": 1,
        "model": ENGINE_MODEL,
        "model_code_sha256": model_code_fingerprint(),
        "protocol": "same_engine_walk_forward_frozen_pre_kickoff",
        "feature_contract": "observed_stats_only_no_fm26_no_current_squads",
        "fit_end_date": fit_end_date,
        "oos_start_date": oos_start_date,
        "oos_end_date": oos_end_date,
        "tested_without_selection_or_tuning": args.experiment is None,
        "same_fixture_engine_and_output_contract": True,
        "paths_per_fixture": int(predictions["paths"]),
        "fit_fixtures": len(fit),
        "oos_fixtures": len(oos),
        "frozen_calibration": {"temperature": temperature, "draw_logit_bias": draw_bias,
                               "fit_on_fixtures": int(last_fit["fit_on_prior_fixtures"])},
        "metrics": summary,
        "matched_day_bootstrap_95": bootstrap,
        "targets": targets,
        "coverage": coverage,
        "input_contract": predictions["input_contract"],
        "experiment": args.experiment,
        "comparison": comparison,
        "interpretation": (
            "Same-engine walk-forward evaluation of the second-resolved Rust match engine with an "
            "observed-stat-only historical feature contract (no FM26, no current squads). This "
            "validates or fails to validate the engine mechanics and calibration on historical data; "
            "it does not validate the 2026 FM26-enhanced pipeline. Sample size and club coverage are "
            "limited by the 74 event-covered 2015/16 matches, so release-policy thresholds are not met."
        ),
    }
    output = ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, default=str) + "\n")

    print(f"OOS {len(oos)} fixtures {oos_start_date}..{oos_end_date} (fit {len(fit)}, end {fit_end_date})")
    print(f"paths={predictions['paths']} frozen calibration T={temperature:.3f} draw_bias={draw_bias:+.3f}")
    print(f"W/D/L log loss raw={summary['wdl_log_loss_raw']:.4f} calibrated={summary['wdl_log_loss_calibrated']:.4f} "
          f"poisson={summary['wdl_log_loss_poisson']:.4f} league={summary['wdl_log_loss_league']:.4f}")
    print(f"exact score log loss engine={summary['exact_score_log_loss_engine']:.4f} poisson={summary['exact_score_log_loss_poisson']:.4f}")
    print(f"team goals CRPS engine={summary['team_goals_crps_engine']:.4f} poisson={summary['team_goals_crps_poisson']:.4f} "
          f"MAE engine={summary['team_goals_mae_engine']:.4f} poisson={summary['team_goals_mae_poisson']:.4f}")
    print(f"player goal Brier engine={summary['player_goal_occurrence_brier_engine']:.4f} prior={summary['player_goal_occurrence_brier_prior']:.4f} "
          f"assist engine={summary['player_assist_occurrence_brier_engine']:.4f} prior={summary['player_assist_occurrence_brier_prior']:.4f}")
    print(f"window15 goal Brier engine={summary['window15_brier_engine']:.4f} prior={summary['window15_brier_prior']:.4f}")
    print(f"coverage: goals in pool {coverage['goal_coverage']:.3f}, assists {coverage['assist_coverage']:.3f}")
    print(f"saved {output}")


if __name__ == "__main__":
    main()
