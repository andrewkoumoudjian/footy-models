#!/usr/bin/env python3
"""Pre-match timing forecast versus *later* verified match events.

The momentum figure is not a goal-hazard forecast. This audit separates the
relative shot/goal pressure peak, absolute scoring rate, red-card probability,
and conditional-on-goal timing proper scores. It NEVER trains on this result.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path


def _bin_index(minute, windows):
    if not isinstance(minute, int) or not 0 <= minute <= 120:
        raise ValueError("invalid football clock minute")
    for index, row in enumerate(windows):
        if row["minute_start"] <= minute < row["minute_end"]:
            return index
    raise ValueError("actual minute is outside modeled regulation time")


def pressure_index(row, weight=3.0):
    if not math.isfinite(weight) or weight < 0:
        raise ValueError("invalid pressure goal multiplier")
    values = [
        float(row[k])
        for k in ("expected_home_shots", "expected_away_shots",
                  "expected_home_goals", "expected_away_goals")
    ]
    if any(not math.isfinite(v) or v < 0 for v in values):
        raise ValueError("invalid expected event rate")
    home = values[0] + weight * values[2]
    away = values[1] + weight * values[3]
    return 100.0 * (home - away) / (home + away) if home + away > 0 else 0.0


def probability_at_least_two_of_three(p):
    return 3 * p * p * (1 - p) + p * p * p


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rerun_drift(original, rerun, home="Liverpool", away="Man City"):
    """Check identical seed/path fixtures, not arbitrary other model runs."""
    def find_one(payload):
        matches = [
            f for f in payload["fixtures"]
            if (f["home"], f["away"]) == (home, away)
        ]
        if len(matches) != 1:
            raise ValueError("ambiguous Liverpool/City reproduction fixture")
        return matches[0]

    a, b = find_one(original), find_one(rerun)
    if any(a[k] != b[k] for k in ("date", "seed", "paths")):
        raise ValueError("cannot compare different match dates, seeds or path counts")
    if rerun.get("as_of") != original.get("as_of"):
        raise ValueError("cannot compare mismatched feature timestamps")
    av = [pressure_index(w) for w in a["15_minute_windows"]]
    bv = [pressure_index(w) for w in b["15_minute_windows"]]
    return {
        "original_peak_window": min(range(6), key=lambda i: av[i]),
        "rerun_peak_window": min(range(6), key=lambda i: bv[i]),
        "pressure_index_original": av,
        "pressure_index_rerun": bv,
        "exact_equal_full_15_min_windows": a["15_minute_windows"] == b["15_minute_windows"],
        "maximum_absolute_pressure_index_difference": max(
            abs(x - y) for x, y in zip(av, bv)
        ),
        "warning": "Same-seed replay does not prove reproducibility unless full input hashes and binary revision are identical.",
    }


def check_run_manifest(manifest_path, repo_root):
    data = json.loads(manifest_path.read_text())
    files = {
        "matched_registration": "artifacts/premier-league-2026-10-11/fm_matched_squads.json",
        "dated_team_results": "data/pl/asof_team_stats.csv",
        "eligible_player_inputs": "data/pl/player_fm_join.csv",
        "baseline_team_forecast": "data/pl/forecasts.json",
        "fm26_raw_local": "/home/drewclaw/fm26-data/fm26_men_full_attrs.csv",
    }
    results = {}
    for key, location in files.items():
        p = Path(location)
        if not p.is_absolute():
            p = repo_root / p
        actual = sha256(p) if p.is_file() else None
        results[key] = {
            "file_present": actual is not None,
            "matches_snapshot": actual == data["input_sha256"].get(key),
            "expected_sha256": data["input_sha256"].get(key),
            "current_sha256": actual,
        }
    return results


def score_timing(forecast, actual):
    """Return information independent of any particular chart implementation."""
    date = actual["date"]
    if not forecast.get("as_of") or not forecast["as_of"] < date:
        raise ValueError("a pre-match forecast timestamp is required")
    matches = [
        f for f in forecast["fixtures"]
        if (f["date"], f["home"], f["away"]) == (
            date, actual["home"], actual["away"]
        )
    ]
    if len(matches) != 1:
        raise ValueError("fixture missing or ambiguous")
    f = matches[0]
    windows = f["15_minute_windows"]
    if len(windows) != 6:
        raise ValueError("six 15-minute windows required")
    for i, w in enumerate(windows):
        if (w["minute_start"], w["minute_end"]) != (15 * i, 15 * (i + 1)):
            raise ValueError("noncontiguous prediction windows")
    if int(f["paths"]) < 10:
        raise ValueError("too few Monte Carlo paths for timing audit")
    if f["expected_away_goals"] <= 0:
        raise ValueError("zero away expected goals")
    if actual["away_goals"] != len(actual["away_goal_minutes"]):
        raise ValueError("actual away goal count inconsistent with minutes")
    if actual["home_goals"] != 0:
        raise ValueError("case labels should not be silently repurposed")
    if not actual["provenance"]["actual_result_observed_after_match"]:
        raise ValueError("the result is not independently verified")

    goal_bins = [_bin_index(minute, windows) for minute in actual["away_goal_minutes"]]
    red_bins = [_bin_index(minute, windows) for minute in actual["home_red_card_minutes"]]
    momentum = [pressure_index(row) for row in windows]
    peak = min(range(6), key=lambda i: momentum[i])
    peak_city_goals = max(range(6), key=lambda i: windows[i]["expected_away_goals"])
    peak_city_shots = max(range(6), key=lambda i: windows[i]["expected_away_shots"])

    goals = [float(row["expected_away_goals"]) for row in windows]
    total = sum(goals)
    if any(rate <= 0 for rate in goals):
        raise ValueError("all bins must have positive predicted goal rate for log scoring")
    p = [v / total for v in goals]
    timing_log_loss = -sum(math.log(p[i]) for i in goal_bins)
    flat_log_loss = len(goal_bins) * math.log(len(windows))
    goals_in_peak = goal_bins.count(peak)

    sensitivity = {}
    for multiplier in (0, 1, 3, 5, 10):
        pressure = [pressure_index(w, multiplier) for w in windows]
        sensitivity[str(multiplier)] = int(min(range(6), key=lambda i: pressure[i]))

    return {
        "contract": "locked_2026_epl_prematch_goal_timing_audit_v1",
        "pre_match_as_of": forecast["as_of"],
        "result_date": date,
        "fixture": f"{f['home']} vs {f['away']}",
        "paths": int(f["paths"]),
        "seed": int(f["seed"]),
        "actual_away_goal_minutes": actual["away_goal_minutes"],
        "actual_home_red_card_minutes": actual["home_red_card_minutes"],
        "actual_goal_windows": goal_bins,
        "actual_red_card_windows": red_bins,
        "pressure_peak_window": peak,
        "pressure_peak_minutes": [windows[peak]["minute_start"], windows[peak]["minute_end"]],
        "peak_pressure_index": momentum[peak],
        "actual_goals_in_pressure_peak": goals_in_peak,
        "peak_absolute_away_goal_rate_window": peak_city_goals,
        "peak_absolute_away_shot_rate_window": peak_city_shots,
        "goal_multiplier_pressure_peak_sensitivity": sensitivity,
        "pressure_peak_decomposition_vs_previous_window": (
            {
                "home_shot_change": (
                    windows[peak]["expected_home_shots"] -
                    windows[peak - 1]["expected_home_shots"]
                ),
                "away_shot_change": (
                    windows[peak]["expected_away_shots"] -
                    windows[peak - 1]["expected_away_shots"]
                ),
                "home_goal_rate_change": (
                    windows[peak]["expected_home_goals"] -
                    windows[peak - 1]["expected_home_goals"]
                ),
                "away_goal_rate_change": (
                    windows[peak]["expected_away_goals"] -
                    windows[peak - 1]["expected_away_goals"]
                ),
            } if peak > 0 else None
        ),
        "conditional_away_goal_window_probabilities": p,
        "peak_window_goal_probability_model": p[peak],
        "goal_clock_independence_assumption": (
            "Multinomial goal-minute likelihood and 2-of-3 bin probabilities "
            "condition on realized goal count and assume independent goal-time "
            "draws; they are diagnostics, not a complete joint-path likelihood."
        ),
        "peak_window_goal_probability_flat": 1.0 / 6,
        "probability_at_least_two_of_three_goals_in_predeclared_peak": {
            "model": probability_at_least_two_of_three(p[peak]),
            "flat": probability_at_least_two_of_three(1.0 / 6),
        },
        "any_of_six_windows_has_two_or_more_goals_under_flat_iid_timing": (
            1.0 - 6.0 * 5.0 * 4.0 / 6.0**3
        ),
        "conditional_away_goal_time_negative_log_loss": {
            "model": timing_log_loss,
            "flat": flat_log_loss,
            "model_improvement_nats_three_goals": flat_log_loss - timing_log_loss,
            "model_improvement_nats_per_goal": (flat_log_loss - timing_log_loss) / len(goal_bins),
        },
        "simulated_red_card_probability_in_actual_card_bin_any_team": windows[red_bins[0]]["p_red_card"],
        "simulated_substitution_trigger_probability_in_peak_any_team": windows[peak]["p_substitution"],
        "windows": [
            {
                "start": w["minute_start"], "end": w["minute_end"],
                "relative_city_momentum_index": momentum[i],
                "expected_city_shots": w["expected_away_shots"],
                "expected_city_goals": w["expected_away_goals"],
                "city_goal_probability_given_an_eventual_city_goal": p[i],
                "any_team_red_card_probability": w["p_red_card"],
                "substitution_trigger_probability": w["p_substitution"],
            }
            for i, w in enumerate(windows)
        ],
        "interpretation": [
            "The pre-match relative-pressure peak aligned with the red card and two of three City goals.",
            "The third goal came in the following period. The peak of absolute City scoring RATE was different.",
            "The goal timing likelihood has very little edge over a uniform-in-time conditional baseline.",
            "The red-card probability is any-team incidence, not player-specific Gravenberch dismissal probability.",
            "The pressure index mixes goals with shots, and the simulation's substitutions are scheduled around minute 60.",
            "One fixture selected for apparent success is insufficient evidence of timing calibration.",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--forecast", type=Path,
                        default=Path("artifacts/premier-league-2026-10-11/player_paths.json"))
    parser.add_argument("--actual", type=Path,
                        default=Path("artifacts/validation/liverpool-city-actual-2026-10-11.json"))
    parser.add_argument("--reproduction", type=Path,
                        help="Optional same-seed rerun, compared without overwriting the frozen original.")
    parser.add_argument("--run-manifest", type=Path,
                        default=Path("artifacts/premier-league-2026-10-11/run_manifest.json"))
    parser.add_argument("--output", type=Path,
                        default=Path("artifacts/validation/liverpool-city-timing-audit.json"))
    args = parser.parse_args()
    forecast = json.loads(args.forecast.read_text())
    actual = json.loads(args.actual.read_text())
    report = score_timing(forecast, actual)
    report["source_pre_match_forecast_sha256"] = sha256(args.forecast)
    report["source_post_match_result_sha256"] = sha256(args.actual)
    report["source_pre_match_run_manifest_sha256"] = sha256(args.run_manifest)
    report["pre_match_input_fingerprint_check_today"] = check_run_manifest(
        args.run_manifest, Path(__file__).resolve().parents[1]
    )
    if args.reproduction is not None:
        report["same_seed_current_reproduction_check"] = rerun_drift(
            forecast, json.loads(args.reproduction.read_text())
        )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(json.dumps({
        k: report[k] for k in (
            "pressure_peak_minutes",
            "actual_goals_in_pressure_peak",
            "peak_absolute_away_goal_rate_window",
            "goal_multiplier_pressure_peak_sensitivity",
            "conditional_away_goal_time_negative_log_loss",
        )
    }, indent=2))


if __name__ == "__main__":
    main()
