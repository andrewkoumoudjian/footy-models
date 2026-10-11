#!/usr/bin/env python3
"""Causal fit/evaluation for the simulator's *per-player* 90-minute red hazard.

HR/AR describe recorded match red cards; fit on earlier seasons and evaluate on
later matches with known labels. No future match may enter the prior. Treating
22 starter slots as exposed for 90 minutes is an explicit approximation.
This does NOT evaluate downstream score-dependent Rust match forecasts.
"""
import argparse
import csv
from datetime import date
import json
import math
import random
from pathlib import Path

DEFAULT_PLAYER_PROBABILITY = 0.02
SLOTS = 22


def load_labeled_matches(path):
    with Path(path).open(newline="", encoding="utf-8") as stream:
        rows = list(csv.DictReader(stream))
    matches = []
    for row in rows:
        if row["hr"] in ("", "NA", None) or row["ar"] in ("", "NA", None):
            continue
        cards = int(row["hr"]) + int(row["ar"])
        if cards < 0:
            raise ValueError("negative card total")
        matches.append((date.fromisoformat(row["date"]), int(cards > 0)))
    return sorted(matches)


def fit_prior(matches, start, end):
    train = [event for day, event in matches if start <= day <= end]
    if len(train) < 200:
        raise ValueError(f"insufficient pre-cutoff labeled games ({len(train)} < 200)")
    # Weak Jeffreys smoothing for a Bernoulli any-red-card match incidence.
    p_match = (sum(train) + 0.5) / (len(train) + 1.0)
    p_player = -math.expm1(math.log1p(-p_match) / SLOTS)
    return {"fixtures": len(train), "red_fixtures": sum(train),
            "match_red_probability": p_match, "per_player_per90_probability": p_player}


def evaluate(prior, observed, baseline_player_probability=DEFAULT_PLAYER_PROBABILITY):
    reference_p = 1.0 - (1.0 - baseline_player_probability) ** SLOTS
    fitted_p = prior["match_red_probability"]
    brier = lambda p: sum((p - y) ** 2 for y in observed) / len(observed)
    log_loss = lambda p: -sum(y * math.log(p) + (1-y) * math.log1p(-p) for y in observed) / len(observed)
    return {"n": len(observed), "observed_match_red_rate": sum(observed) / len(observed),
            "engine_default_match_red_probability": reference_p,
            "fitted_match_red_probability": fitted_p,
            "default_brier": brier(reference_p), "fitted_brier": brier(fitted_p),
            "default_log_loss": log_loss(reference_p), "fitted_log_loss": log_loss(fitted_p)}


def matchday_bootstrap_brier_difference(prior, dated_labels, repetitions=2000):
    """Paired bootstrap, all fixtures on a match date resampled as one unit."""
    grouped = {}
    default_p = 1 - (1 - DEFAULT_PLAYER_PROBABILITY) ** SLOTS
    fitted_p = prior["match_red_probability"]
    for day, label in dated_labels:
        improvement = (fitted_p-label)**2 - (default_p-label)**2
        grouped.setdefault(day, []).append(improvement)
    days = sorted(grouped)
    rng = random.Random(42)
    scores = []
    for _ in range(repetitions):
        drawn = [grouped[rng.choice(days)] for _ in days]
        scores.append(sum(sum(day) for day in drawn) / sum(len(day) for day in drawn))
    scores.sort()
    return [scores[int(0.025*(repetitions-1))], scores[int(0.975*(repetitions-1))]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", default="data/pl/historical_galsaar_results.csv")
    parser.add_argument("--manifest", default="data/pl/historical_galsaar_manifest.json")
    parser.add_argument("--fit-start", default="2012-01-01")
    parser.add_argument("--fit-end", default="2016-02-07")
    parser.add_argument("--eval-end", default="2019-05-31")
    parser.add_argument("--output", default="artifacts/validation/pl-red-card-prior-research.json")
    args = parser.parse_args()
    start, end, eval_end = map(date.fromisoformat, (args.fit_start, args.fit_end, args.eval_end))
    if not start <= end < eval_end:
        raise ValueError("fit window must end before validation window")
    source_manifest = json.loads(Path(args.manifest).read_text())
    source_sha256 = source_manifest["archive_sha256"]
    if len(source_sha256) != 64:
        raise ValueError("invalid upstream archive checksum")
    matches = load_labeled_matches(args.results)
    fitted = fit_prior(matches, start, end)
    dated_oos = [(day, observed) for day, observed in matches if end < day <= eval_end]
    if len(dated_oos) < 200:
        raise ValueError("too few out-of-sample matches")
    metrics = evaluate(fitted, [observed for _, observed in dated_oos])
    metrics["matched_day_bootstrap_95_brier_fitted_minus_default"] = matchday_bootstrap_brier_difference(fitted, dated_oos)
    report = {"schema_version": 1, "source": "galsaar/EnglishPremierLeagueEPLResults results.csv.zip",
              "archive_sha256": source_sha256,
              "fit_start_date": start.isoformat(), "fit_end_date": end.isoformat(),
              "oos_start_exclusive": end.isoformat(), "oos_end_date": eval_end.isoformat(),
              "fit": fitted, "oos": metrics,
              "acceptance": "Research-only approximation; do not change simulator default before full same-engine OOS outcome validation.",
              "assumption": "22 equally exposed field-player slots for 90 minutes; actual playing time, dismissals and match state are not accounted for in this benchmark."}
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Red prior fit: per-player p90={fitted['per_player_per90_probability']:.6f}, OOS {len(dated_oos)} matches; Brier default={metrics['default_brier']:.5f} fitted={metrics['fitted_brier']:.5f}, 95% paired matchday CI {metrics['matched_day_bootstrap_95_brier_fitted_minus_default']}")


if __name__ == "__main__":
    main()
