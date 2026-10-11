#!/usr/bin/env python3
"""Fit the historical engine's goal contract from the fit period only, for a frozen OOS test.

Reads a same-engine path export (`pl-hist-same-engine-paths.json`), splits it chronologically with
the same rule as the scorer (`--oos-matches`), and estimates on the **fit period only**:

- overall goal level scale: observed goals / mean predicted goals
- home advantage correction: observed home/away goal ratio / predicted home/away ratio

The recommended `footy-matchforecast-pl-hist` flags are printed and recorded as JSON so a
calibration experiment is reproducible and auditable. Nothing here may be fitted on OOS fixtures;
the split is asserted to match the scorer's chronological rule.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]



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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predictions", default="artifacts/validation/pl-hist-same-engine-paths.json")
    parser.add_argument("--oos-matches", type=int, default=24)
    parser.add_argument("--output", default=None, help="optional JSON record path")
    args = parser.parse_args()

    document = json.loads((ROOT / args.predictions).read_text())
    fixtures = sorted(document["fixtures"], key=lambda f: (f["date"], f["match_id"]))
    if len(fixtures) < args.oos_matches + 6:
        raise SystemExit("not enough simulated fixtures for the requested split")
    fit, oos = chronological_split(fixtures, args.oos_matches)
    assert fit[-1]["date"] < oos[0]["date"], "chronological split violated"

    observed_goals = sum(int(f["home_goals_actual"]) + int(f["away_goals_actual"]) for f in fit)
    predicted_goals = sum(f["expected_home_goals"] + f["expected_away_goals"] for f in fit)
    observed_home = sum(int(f["home_goals_actual"]) for f in fit)
    observed_away = sum(int(f["away_goals_actual"]) for f in fit)
    predicted_home = sum(f["expected_home_goals"] for f in fit)
    predicted_away = sum(f["expected_away_goals"] for f in fit)
    assert predicted_goals > 0.0 and predicted_away > 0.0 and observed_away > 0

    scale = observed_goals / predicted_goals
    observed_ratio = observed_home / observed_away
    predicted_ratio = predicted_home / predicted_away
    home_correction = observed_ratio / predicted_ratio
    contract = document["input_contract"]
    # Split the total scale between the base goal rate and the home advantage multiplier so the
    # multiplicative home term absorbs only the home/away imbalance correction.
    recommended_scale = scale
    recommended_home_multiplier = contract["home_advantage_multiplier"] * home_correction

    record = {
        "schema_version": 1,
        "source_predictions": args.predictions,
        "paths": document["paths"],
        "fit_fixtures": len(fit),
        "fit_end_date": fit[-1]["date"],
        "oos_start_date": oos[0]["date"],
        "fit_observed_goals": observed_goals,
        "fit_predicted_goals": predicted_goals,
        "fit_goal_scale": scale,
        "fit_observed_home_away_ratio": observed_ratio,
        "fit_predicted_home_away_ratio": predicted_ratio,
        "home_ratio_correction": home_correction,
        "recommended_flags": {
            "--goal-rate-scale": round(recommended_scale, 4),
            "--home-advantage-multiplier": round(recommended_home_multiplier, 4),
        },
        "note": "fitted on fit-period fixtures only; evaluate on the frozen chronological OOS window",
    }
    print(json.dumps(record, indent=2))
    if args.output:
        path = ROOT / args.output
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(record, indent=2) + "\n")
        print(f"saved {path}")


if __name__ == "__main__":
    main()
