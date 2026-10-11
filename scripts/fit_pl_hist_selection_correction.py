#!/usr/bin/env python3
"""Fit a monotone selection-marginal correction from the fit period of a same-engine run.

The lineup sampler converts candidate start probabilities into realized path start frequencies.
Because an XI is sampled without replacement, the realized marginals compress toward the pool
middle: very likely starters are pulled down and unlikely starters are pushed toward zero. The
correction table maps a *desired* pre-match start probability to the *input* probability that the
sampler should receive so its realized marginal lands on the desired value.

Only fit-period fixtures are used. The table is applied by `footy-matchforecast-pl-hist
--selection-correction <table.json>` and evaluated on the frozen OOS window.
"""
from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def chronological_split(fixtures, oos_matches):
    start = len(fixtures) - oos_matches
    boundary = fixtures[start]["date"]
    while start > 0 and fixtures[start - 1]["date"] == boundary:
        start -= 1
    return fixtures[:start], fixtures[start:]


def isotonic_increasing(pairs):
    """Pool-adjacent-violators: returns (x, y) with y non-decreasing in x."""
    pairs = sorted(pairs)
    blocks = []  # [sum_y, weight, x_center]
    for x, y in pairs:
        blocks.append([y, 1.0, x])
        while len(blocks) > 1 and blocks[-2][0] / blocks[-2][1] > blocks[-1][0] / blocks[-1][1]:
            right = blocks.pop()
            left = blocks.pop()
            blocks.append([left[0] + right[0], left[1] + right[1], (left[2] * left[1] + right[2] * right[1]) / (left[1] + right[1])])
    return [(block[2], block[0] / block[1]) for block in blocks]


def inverse_at(curve, desired):
    """Input x whose fitted realized output equals `desired`, by interpolation."""
    # curve is a non-decreasing list of (x, y). Invert y -> x.
    if desired <= curve[0][1]:
        return curve[0][0] * (desired / max(curve[0][1], 1e-9))
    if desired >= curve[-1][1]:
        # Extrapolate linearly in logit space toward certainty.
        x0, y0 = curve[-1]
        return min(0.995, x0 + (desired - y0) * 1.0)
    for index in range(1, len(curve)):
        x1, y1 = curve[index]
        x0, y0 = curve[index - 1]
        if y1 >= desired:
            if y1 - y0 < 1e-12:
                return x1
            fraction = (desired - y0) / (y1 - y0)
            return x0 + fraction * (x1 - x0)
    return curve[-1][0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predictions", default="artifacts/validation/pl-hist-same-engine-paths.json")
    parser.add_argument("--hist-dir", default="data/statsbomb/pl_hist")
    parser.add_argument("--oos-matches", type=int, default=24)
    parser.add_argument("--bins", type=int, default=24)
    parser.add_argument("--output", default="artifacts/validation/pl-hist-selection-correction.json")
    args = parser.parse_args()

    document = json.loads((ROOT / args.predictions).read_text())
    fixtures = sorted(document["fixtures"], key=lambda f: (f["date"], f["match_id"]))
    fit, oos = chronological_split(fixtures, args.oos_matches)
    assert fit[-1]["date"] < oos[0]["date"]

    pairs = []
    for fixture in fit:
        mid = fixture["match_id"]
        rows = {int(row["player_id"]): row for row in csv.DictReader(open(ROOT / args.hist_dir / f"players/{mid}.csv"))}
        for player in fixture["players"]:
            row = rows.get(int(player["uid"]))
            if row is None:
                continue
            team_fixtures = float(row["team_fixtures"])
            starts = float(row["selection_starts"])
            prior = (starts + 1.0) / (team_fixtures + 2.0) if team_fixtures >= 2 and float(row["selection_listings"]) >= 1 else None
            if prior is None:
                continue  # fallback usage priors have a different construction
            pairs.append((prior, float(player["start_probability"])))
    if len(pairs) < 100:
        raise SystemExit(f"only {len(pairs)} fit pairs; need at least 100")

    # Quantile bins reduce per-player sampling noise before enforcing monotonicity.
    pairs.sort()
    bin_size = max(1, len(pairs) // args.bins)
    binned = []
    for start in range(0, len(pairs), bin_size):
        chunk = pairs[start:start + bin_size]
        if len(chunk) < 5:
            continue
        binned.append((sum(x for x, _ in chunk) / len(chunk), sum(y for _, y in chunk) / len(chunk)))
    curve = isotonic_increasing(binned)

    grid = [index / 100.0 for index in range(1, 100)]
    table = [[desired, max(0.005, min(0.995, inverse_at(curve, desired)))] for desired in grid]
    record = {
        "schema_version": 1,
        "source_predictions": args.predictions,
        "fit_fixtures": len(fit),
        "fit_end_date": fit[-1]["date"],
        "oos_start_date": oos[0]["date"],
        "fit_pairs": len(pairs),
        "observed_curve": curve,
        "correction_table": table,
        "note": "desired start probability -> sampler input probability, fitted on fit-period fixtures only",
    }
    output = ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(record, indent=1) + "\n")
    print(f"fitted {len(pairs)} pairs over {len(curve)} isotonic blocks; saved {output}")
    for desired in (0.05, 0.25, 0.5, 0.75, 0.9, 0.95):
        corrected = next(value for point, value in table if abs(point - desired) < 1e-9)
        print(f"desired {desired:.2f} -> sampler input {corrected:.3f}")


if __name__ == "__main__":
    main()
