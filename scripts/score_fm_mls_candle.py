#!/usr/bin/env python3
"""Paired, match-clustered score uncertainty for the research Candle shot-rate pilot."""
import argparse
import json
import math
import random
from collections import defaultdict
from pathlib import Path


def per_player_delta(item):
    y = int(item["shots"])
    baseline = max(float(item["baseline_expected_shots"]), 1e-10)
    learned = max(float(item["candle_expected_shots"]), 1e-10)
    if y < 0 or not all(math.isfinite(x) and x > 0 for x in (baseline, learned)):
        raise ValueError("non-finite or invalid evaluation row")
    # Constant factorial term cancels in the paired Poisson negative-log-likelihood.
    return (learned - baseline) - y * math.log(learned / baseline)


def summarize_split(rows, seed=42, draws=3000):
    grouped = defaultdict(list)
    for row in rows:
        grouped[row["match_id"]].append(per_player_delta(row))
    if len(grouped) < 2:
        raise ValueError("fewer than two matches in split")
    chunks = list(grouped.values())
    n = sum(len(group) for group in chunks)
    delta = sum(sum(group) for group in chunks) / n
    rng = random.Random(seed)
    samples = []
    for _ in range(draws):
        draws_for_matches = [chunks[rng.randrange(len(chunks))] for _ in chunks]
        samples.append(
            sum(sum(group) for group in draws_for_matches)
            / sum(len(group) for group in draws_for_matches)
        )
    samples.sort()
    return {
        "n_matches": len(chunks),
        "n_player_matches": n,
        "mean_poisson_log_loss_delta_candidate_minus_baseline": delta,
        "paired_fixture_bootstrap_95pct": [
            samples[int(0.025 * draws)],
            samples[int(0.975 * draws)],
        ],
        "bootstrap_draws": draws,
        "bootstrap_seed": seed,
        "uncertainty_caveat": "match-cluster bootstrap ignores between-fixture team/player serial correlation",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("predictions", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    rows = json.loads(args.predictions.read_text())
    if len({(r["match_id"], r["player_id"]) for r in rows}) != len(rows):
        raise ValueError("duplicate player-match evaluation row")
    split_rows = {
        s: [row for row in rows if row["split"] == s]
        for s in ("validation", "test")
    }
    if any(not group for group in split_rows.values()):
        raise ValueError("missing chronological validation/test")
    result = {
        "contract": "fm26_mls_candle_shots_asof_fixture_bootstrap_v1",
        "promotion": "NEVER_FROM_THIS_RESULT_ALONE",
        "splits": {s: summarize_split(group) for s, group in split_rows.items()},
    }
    output = args.output or args.predictions.with_name("uncertainty.json")
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
