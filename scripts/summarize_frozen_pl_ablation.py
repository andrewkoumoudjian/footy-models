#!/usr/bin/env python3
"""Read-only comparison of provenance-matched, *diagnostic* EPL variants.

Not an event-forecast accuracy backtest. Different research flags change the
RNG consumption/order so identical seed is not paired counterfactual paths.
"""
import argparse
import json
import math
from pathlib import Path

from run_frozen_pl_research import verify_archive


def momentum(window):
    home = float(window["expected_home_shots"]) + 3 * float(window["expected_home_goals"])
    away = float(window["expected_away_shots"]) + 3 * float(window["expected_away_goals"])
    if min(home, away) < 0 or not math.isfinite(home + away):
        raise ValueError("invalid simulated team event rates")
    return 100 * (home - away) / (home + away) if home + away else 0.0


def measure(directory):
    manifest = verify_archive(directory)
    forecast = json.loads(
        (directory / manifest["archived_files"]["forecast_1"]).read_text()
    )
    fixture = forecast["fixtures"]
    if len(fixture) != 1:
        raise ValueError("mechanism comparison requires one fixture per run")
    windows = fixture[0]["15_minute_windows"]
    if len(windows) != 6:
        raise ValueError("six simulated windows required")
    values = [momentum(w) for w in windows]
    return manifest, {
        "variant": manifest["variant"],
        "research_flags": manifest["research_flags"],
        "fixture": [fixture[0]["home"], fixture[0]["away"]],
        "paths": manifest["paths"],
        "home_win": fixture[0]["home_win"],
        "draw": fixture[0]["draw"],
        "away_win": fixture[0]["away_win"],
        "home_expected_goals": fixture[0]["expected_home_goals"],
        "away_expected_goals": fixture[0]["expected_away_goals"],
        "peak_relative_city_pressure_bin": int(min(range(6), key=lambda i: values[i])),
        "peak_absolute_city_goal_bin": int(max(
            range(6), key=lambda i: windows[i]["expected_away_goals"]
        )),
        "pressure_index": values,
        "window_expected_home_shots": [w["expected_home_shots"] for w in windows],
        "window_expected_away_shots": [w["expected_away_shots"] for w in windows],
        "window_expected_home_goals": [w["expected_home_goals"] for w in windows],
        "window_expected_away_goals": [w["expected_away_goals"] for w in windows],
        "probability_any_red": [w["p_red_card"] for w in windows],
        "probability_substitution_trigger": [w["p_substitution"] for w in windows],
    }


def compare(directories):
    if len(directories) < 2:
        raise ValueError("at least two independently archived variants required")
    raw = [measure(directory) for directory in directories]
    fingerprints = ("binary", "results", "players", "baseline")
    anchor = raw[0][0]
    for manifest, _ in raw[1:]:
        for key in fingerprints:
            if manifest["sha256"][key] != anchor["sha256"][key]:
                raise ValueError(f"experiment inputs/executable do not match: {key}")
        for key in ("cutoff", "forecast_date", "fixtures", "paths", "seed"):
            if manifest[key] != anchor[key]:
                raise ValueError(f"run metadata mismatch: {key}")
    variants = [row for _, row in raw]
    if len({json.dumps(x["research_flags"], sort_keys=True) for x in variants}) != len(variants):
        raise ValueError("repeated variant, not an intervention comparison")
    return {
        "contract": "matched_frozen_2026_liverpool_mechanism_ablation_v1",
        "sources": [str(path) for path in directories],
        "shared_input_and_exact_executable_sha256": {
            key: anchor["sha256"][key] for key in fingerprints
        },
        "cutoff": anchor["cutoff"],
        "paths_per_variant": anchor["paths"],
        "seed": anchor["seed"],
        "variants": variants,
        "limitations": [
            "These are post-result mechanism diagnostics, not previously unseen match predictions.",
            "Changing flags alters stochastic event sequencing; same seed is not a coupled causal path comparison.",
            "A few paths cannot establish stability, predictive power or goal-time calibration.",
            "The 60–75 pressure ratio is descriptive and substitutions are scheduled from expected player minutes.",
            "Disabling scheduled tactical substitutions leaves injury-forced substitutions enabled.",
            "Setting red probability zero disables red events, not an observed targeted 62-minute dismissal.",
            "Never select any final model or parameters from this already-known Liverpool result.",
        ],
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("archives", type=Path, nargs="+")
    p.add_argument("--output", type=Path,
                   default=Path("artifacts/validation/liverpool-mechanism-ablation.json"))
    args = p.parse_args()
    report = compare(args.archives)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    for row in report["variants"]:
        print(row["variant"], "peak", row["peak_relative_city_pressure_bin"] * 15,
              "to", (row["peak_relative_city_pressure_bin"] + 1) * 15,
              "min; City goals", round(row["away_expected_goals"], 3))


if __name__ == "__main__":
    main()
