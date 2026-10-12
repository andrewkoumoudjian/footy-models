#!/usr/bin/env python3
"""Pooled MLS defensive-event heads: tackles + ball recoveries (no deep learning).

Same recipe as the shots head: pooled ridge-Poisson residual over a shared
as-of empirical-Bayes baseline, no player-ID coefficients, chronological
splits, proper Poisson log loss + occurrence Brier + mean-calibration ratio.
FM defensive attributes enter every head: tackles use Tackling/Marking/
Aggression/Strength/Pace/Positioning, recoveries use Positioning/
Anticipation/WorkRate/Stamina/Teamwork/Pace, plus age/CA/recency/fm_verified.
Unverified rows keep zero FM features with fm_verified=0 (masked, kept).

Labels exist only in MLS SofaScore (ESPN has no defensive events), so the
residual is MLS-fitted; EPL application is a disclosed cross-league transfer
(league flag included, coefficient data-driven). Output appends both heads to
a copy of the v1 heads artifact (v1 content preserved).
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from datetime import date
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from fit_pooled_epl_mls_shot_model import (  # noqa: E402
    fit_ridge,
    poisson_loss,
)
from build_fm_mls_defense_dataset import (  # noqa: E402
    RECOVERY_ROLE_PRIORS,
    TACKLE_ROLE_PRIORS,
)

PENALTIES = (0.1, 1.0, 10.0, 100.0)
HEADS = {
    "tackles": {
        "attrs": ("Tackling", "Marking", "Aggression", "Strength", "Pace",
                  "Positioning"),
        "priors": TACKLE_ROLE_PRIORS,
    },
    "recoveries": {
        "attrs": ("Positioning", "Anticipation", "WorkRate", "Stamina",
                  "Teamwork", "Pace"),
        "priors": RECOVERY_ROLE_PRIORS,
    },
}


def sha256(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def load_union(wanted: set[str]):
    from fm26_complete import load_union as _load
    union, _ = _load(wanted_uids=wanted)
    return union


def feature_block(union_row: dict | None, day: date, attr_names: tuple) -> dict:
    if union_row is None:
        return {"age": 0.0, "ca": 0.0, "attrs": [0.0] * len(attr_names),
                "verified": 0.0}
    dob = union_row.get("dob", "")
    try:
        born = date.fromisoformat(dob)
        age = max(15.0, min(50.0, (day - born).days / 365.25))
    except ValueError:
        return {"age": 0.0, "ca": 0.0, "attrs": [0.0] * len(attr_names),
                "verified": 0.0}
    try:
        ca = float(union_row.get("ca") or 0.0)
    except ValueError:
        ca = 0.0
    attrs = []
    attributes = union_row.get("attributes") or {}
    for name in attr_names:
        try:
            value = float(attributes.get(name) or 0.0)
        except ValueError:
            value = 0.0
        attrs.append((value - 10.0) / 10.0 if value > 0.0 else 0.0)
    return {"age": (age - 25.0) / 10.0,
            "ca": (ca - 100.0) / 100.0 if ca > 0 else 0.0,
            "attrs": attrs, "verified": 1.0}


def build_rows(samples_path: Path, union: dict, target: str,
               attr_names: tuple) -> list[dict]:
    rows = []
    for line in samples_path.read_text().splitlines():
        if not line.strip():
            continue
        s = json.loads(line)
        if target not in s["labels"]:
            continue
        minutes = float(s["minutes"])
        if not (0 < minutes <= 130):
            raise ValueError("invalid exposure")
        day = date.fromisoformat(s["date"])
        fm = union.get(str(s["fm_uid"])) if s["fm_uid"] else None
        block = feature_block(fm if s["fm_verified"] else None, day, attr_names)
        x = [1.0, block["age"], block["ca"], *block["attrs"],
             float(s["last_seen_log_days"]), block["verified"]]
        lab = s["labels"][target]
        rows.append({
            "league": "MLS", "split": s["split"], "date": s["date"],
            "match_id": str(s["match_id"]), "player_id": f"mls:{s['player_id']}",
            "shots": int(lab["events"]), "minutes": minutes,
            "baseline": float(lab["prior_rate"]) * minutes / 90.0,
            "x": x,
        })
    return rows


def occurrence_brier(rows: list[dict], beta) -> float:
    total, n = 0.0, 0
    for r in rows:
        if beta is not None:
            mu = r["baseline"] * math.exp(float(np.clip(np.dot(r["x"], beta),
                                                        -3.0, 3.0)))
        else:
            mu = r["baseline"]
        p = 1.0 - math.exp(-mu)
        total += (p - float(r["shots"] > 0)) ** 2
        n += 1
    return total / max(1, n)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--samples", type=Path, default=ROOT / "data/ml/fm26-mls-defense.jsonl")
    ap.add_argument("--heads-v1", type=Path,
                    default=ROOT / "data/ml/unified_forecast_heads_v1.json")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refuse overwrite")
    sample_uids: set[str] = set()
    n_samples = 0
    for line in a.samples.read_text().splitlines():
        if line.strip():
            s = json.loads(line)
            n_samples += 1
            if s["fm_uid"]:
                sample_uids.add(str(s["fm_uid"]))
    union = load_union(sample_uids)
    report = json.loads(a.heads_v1.read_text())
    if report.get("schema") != "unified_epl_mls_forecast_heads_v1":
        ap.error("unsupported v1 heads artifact")
    report["defense_note"] = (
        "MLS-fitted residuals (no EPL defensive labels exist); EPL "
        "application is a disclosed cross-league transfer via the shared "
        "player-agnostic coefficients, flagged transfer_unvalidated.")
    for target, spec in HEADS.items():
        attr_names = spec["attrs"]
        feature_names = ["league_mls", "age_years_norm", "fm_ca_norm",
                         *(f"fm_{x}_norm" for x in attr_names),
                         "last_seen_log_days", "fm_verified"]
        rows = build_rows(a.samples, union, target, attr_names)
        split = {k: [r for r in rows if r["split"] == k]
                 for k in ("train", "validation", "test")}
        if any(not v for v in split.values()):
            raise ValueError(f"{target}: splits incomplete")
        fitted = {p: fit_ridge(split["train"], p) for p in PENALTIES}
        val = {"identity": poisson_loss(split["validation"], None)}
        for p, b in fitted.items():
            val[str(p)] = poisson_loss(split["validation"], b)
        best = min(val, key=val.get)
        beta = None if best == "identity" else fitted[float(best)]
        test = split["test"]
        base = poisson_loss(test, None)
        cand = poisson_loss(test, beta)
        pred_total = sum(
            r["baseline"] * math.exp(float(np.clip(np.dot(r["x"], beta),
                                                   -3.0, 3.0)))
            if beta is not None else r["baseline"] for r in test)
        obs_total = sum(r["shots"] for r in test)
        import random
        days: dict = {}
        for r in test:
            days.setdefault((r["date"], r["match_id"]), []).append(r)
        groups = list(days.values())
        rng = random.Random(20261012)
        ests = []
        for _ in range(2500):
            sample = [rng.choice(groups) for _ in groups]
            flat = [r for g in sample for r in g]
            ests.append(poisson_loss(flat, beta) - poisson_loss(flat, None))
        ests.sort()
        report[target] = {
            "feature_names": feature_names,
            "penalties": list(PENALTIES),
            "best": best, "validation": val,
            "coefficients": beta.tolist() if beta is not None else None,
            "counts": {k: len(v) for k, v in split.items()},
            "test": {"baseline": base, "candidate": cand,
                     "delta": cand - base, "n": len(test),
                     "mean_predicted_events": pred_total / len(test),
                     "mean_observed_events": obs_total / len(test),
                     "calibration_ratio": pred_total / max(1e-9, obs_total),
                     "occurrence_brier_baseline": occurrence_brier(test, None),
                     "occurrence_brier_candidate": occurrence_brier(test, beta)},
            "test_delta_95pc_matchday_bootstrap": [ests[62], ests[2437]],
            "simulation_contract": (
                f"corrected per-90 = asof_baseline_per90 * exp(X.beta); "
                f"engine multiplies by simulated minutes (no team anchor: "
                f"defense has no score-level totals to preserve)"),
            "gate": "research_only; EPL transfer unvalidated for lack of labels",
        }
    report["defense_provenance"] = {
        "samples_sha256": sha256(a.samples),
        "v1_heads_sha256": sha256(a.heads_v1),
        "fm_union": "complete-everyone union, fm_row_usable parity gate",
    }
    a.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({t: {"best": report[t]["best"],
                          "test": {k: v for k, v in report[t]["test"].items()},
                          "ci": report[t]["test_delta_95pc_matchday_bootstrap"]}
                      for t in HEADS}, indent=2))


if __name__ == "__main__":
    main()
