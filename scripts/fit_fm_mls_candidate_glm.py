#!/usr/bin/env python3
"""Small, interpretable, ridge-Poisson challenger on as-of candidate player sets.

Selection uses July validation only, including the identity-rate baseline.
August is a *previously inspected* exploratory test; no promotion decision.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np
from scipy.optimize import minimize

# Fixed BEFORE any labels are inspected: role one-hot is represented by other
# existing priors, so use only small interpretable residual features.
FEATURE_NAMES = (
    "age_years_norm", "fm_ca_norm", "fm_Finishing_norm", "fm_OffTheBall_norm",
    "fm_Dribbling_norm", "fm_Composure_norm", "fm_Anticipation_norm",
    "fm_Passing_norm", "last_seen_log_days", "fm_verified",
)
PENALTIES = (0.1, 1.0, 10.0, 100.0)


def rows_from_sets(fixtures, all_names):
    feature_ix = [all_names.index(name) for name in FEATURE_NAMES]
    samples = []
    for fixture in fixtures:
        for candidate in fixture["candidates"]:
            if not candidate["fm_verified"] or candidate["label"] is None:
                continue
            shots = int(candidate["label"]["shots"])
            minutes = float(candidate["label"]["minutes"])
            prior = float(candidate["asof_shots_per90"])
            if (not 0 < minutes <= 130 or not prior > 0 or not math.isfinite(prior)
                    or shots < 0):
                raise ValueError("invalid pre-match observed-label exposure")
            selected = [float(candidate["features"][i]) for i in feature_ix]
            if len(selected) != len(FEATURE_NAMES) or not all(map(math.isfinite, selected)):
                raise ValueError("bad feature")
            samples.append({
                "split": fixture["split"], "date": fixture["date"],
                "match_id": fixture["fixture_id"],
                "player_id": candidate["player_id"],
                "shots": shots, "minutes": minutes,
                "baseline": prior * minutes / 90.0, "x": selected,
            })
    return samples


def fit_ridge(rows, penalty):
    x = np.asarray([r["x"] for r in rows], dtype=np.float64)
    y = np.asarray([r["shots"] for r in rows], dtype=np.float64)
    prior = np.asarray([r["baseline"] for r in rows], dtype=np.float64)
    if x.ndim != 2 or len(rows) < len(FEATURE_NAMES):
        raise ValueError("insufficient chronological training")

    def objective(beta):
        logits = np.clip(x @ beta, -3.0, 3.0)
        mu = prior * np.exp(logits)
        loss = (mu - y * logits).sum() / len(y)
        # Avoid biased unshrunk intercept and keep rates close to as-of priors.
        return loss + penalty * (beta @ beta)

    def gradient(beta):
        raw = x @ beta
        clip = np.clip(raw, -3.0, 3.0)
        mu = prior * np.exp(clip)
        active = ((raw > -3.0) & (raw < 3.0)).astype(float)
        return x.T @ ((mu - y) * active) / len(y) + 2.0 * penalty * beta

    result = minimize(
        objective, np.zeros(x.shape[1]), jac=gradient,
        method="L-BFGS-B", options={"maxiter": 1000, "gtol": 1e-9},
    )
    if not result.success and np.linalg.norm(result.jac) > 1e-6:
        raise RuntimeError(f"ridge-Poisson failed convergence: {result.message}")
    return result.x


def predicted(row, beta):
    correction = 0.0 if beta is None else float(np.clip(np.dot(row["x"], beta), -3.0, 3.0))
    return row["baseline"] * math.exp(correction)


def log_loss(rows, beta):
    return sum(
        (mu := predicted(row, beta))
        - row["shots"] * math.log(max(mu, 1e-12))
        + math.lgamma(row["shots"] + 1)
        for row in rows
    ) / len(rows)


def fit_validate(samples):
    split = {
        kind: [r for r in samples if r["split"] == kind]
        for kind in ("train", "validation", "test")
    }
    if any(not rows for rows in split.values()):
        raise ValueError("all three splits are mandatory")
    if not (max(r["date"] for r in split["train"])
            < min(r["date"] for r in split["validation"])
            and max(r["date"] for r in split["validation"])
            < min(r["date"] for r in split["test"])):
        raise ValueError("overlapping periods")
    fitted = {penalty: fit_ridge(split["train"], penalty) for penalty in PENALTIES}
    results = {"identity": log_loss(split["validation"], None)}
    for penalty, weights in fitted.items():
        results[str(penalty)] = log_loss(split["validation"], weights)
    best = min(results, key=results.get)
    beta = None if best == "identity" else fitted[float(best)]
    prediction_rows = [
        {
            "split": r["split"], "date": r["date"], "match_id": r["match_id"],
            "player_id": r["player_id"], "shots": r["shots"],
            "actual_minutes": r["minutes"],
            "baseline_expected_shots": r["baseline"],
            "candle_expected_shots": predicted(r, beta),
        }
        for name in ("validation", "test") for r in split[name]
    ]
    report = {
        "experiment": "fm26_mls_asof_candidate_ridge_poisson_v1",
        "feature_names": FEATURE_NAMES,
        "penalties_challenged": PENALTIES,
        "selection": "smallest proper Poisson log loss on July validation; identity included",
        "best": best, "coefficients": beta.tolist() if beta is not None else None,
        "counts": {key: len(value) for key, value in split.items()},
        "validation": {
            "baseline_poisson_ll": results["identity"],
            "candidate_poisson_ll": results[best],
            "delta": results[best] - results["identity"],
            "all_candidate_validation_losses": results,
        },
        "test": {
            "baseline_poisson_ll": log_loss(split["test"], None),
            "candidate_poisson_ll": log_loss(split["test"], beta),
            "delta": log_loss(split["test"], beta) - log_loss(split["test"], None),
        },
        "test_disclosure": "August already inspected in MLP/attention research. Exploratory only.",
        "gate": "no_production_promotion_without_larger_future_chronological_validation",
    }
    return report, prediction_rows


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("input", type=Path)
    p.add_argument("--output", type=Path, default=Path("data/ml/fm26-mls-glm-results"))
    args = p.parse_args()
    manifest = json.loads(args.input.with_suffix(".manifest.json").read_text())
    fixtures = [json.loads(line) for line in args.input.read_text().splitlines() if line.strip()]
    rows = rows_from_sets(fixtures, manifest["feature_names"])
    report, predictions = fit_validate(rows)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    (args.output / "predictions.json").write_text(json.dumps(predictions, indent=2) + "\n")
    print(json.dumps({
        "selected": report["best"], "validation": report["validation"],
        "test": report["test"], "n": report["counts"],
    }, indent=2))


if __name__ == "__main__":
    main()
