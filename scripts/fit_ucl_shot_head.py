#!/usr/bin/env python3
"""Fit a UCL pooled shot-rate residual head with chronological OOS.

Rows mirror epl_rows_v2 (shared as-of EB baselines, FM blocks through the
verified UCL crosswalk, DOB-quarantined rows masked) over the frozen UCL
summary archives with ONE shared causal history in date order. Splits are
strictly chronological:

    train      match dates < 2025-08-01  (2024-25 UCL season)
    validation 2025-08-01 .. 2026-07-01 (2025-26 UCL season)
    test       match dates >= 2026-07-01 (2026-27 MD1; disclosed small-n)

Selection is smallest Poisson loss on validation (baseline included);
test reports the delta with a matchday-clustered bootstrap CI. FM attributes
enter every fitted head (fm_verified-masked where unverified), per repo
policy. The emitted artifact reuses the unified heads schema so the same
Rust engine consumes it; tackles are copied verbatim from the EPL+MLS v2
artifact as a DISCLOSED cross-league transfer (no UCL defensive labels
exist anywhere), exactly like the EPL v3 run.

Promotion rule: coefficients are written only when validation selects a
fitted ridge over the identity baseline. Otherwise the script reports the
null result and exits nonzero without writing (caller keeps the v2
challenger, disclosed as ungated transfer).
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
    PENALTIES,
    POOLED_FEATURE_NAMES,
    ROLE_PRIOR_EPL_NEUTRAL,
    fit_ridge,
    poisson_loss,
)
from fit_unified_forecast_heads import (  # noqa: E402
    crosswalk_key,
    fm_feature_block,
    load_crosswalk,
    load_fm_union,
    matchday_bootstrap_ci,
)

TRAIN_END = "2025-08-01"
VAL_END = "2026-07-01"


def ucl_rows(archives: list[dict], crosswalk: dict, fm_usable: dict) -> list[dict]:
    matches = sorted(
        (m for archive in archives for m in archive["matches"]),
        key=lambda m: (m["date"], m["espn_match_id"]))
    history: dict[tuple[str, str], dict] = {}
    rows: list[dict] = []
    for match in matches:
        day = date.fromisoformat(match["date"])
        for p in match["players"]:
            key = (p["club"], str(p["espn_id"]))
            st = history.get(key)
            past_shots = st["shots"] if st else 0
            past_minutes = st["minutes"] if st else 0.0
            gap_feat = math.log1p(120) / 5.0 if st is None else math.log1p(
                max(0, (day - st["last_date"]).days)) / 5.0
            prior = (past_shots + 5.0 * ROLE_PRIOR_EPL_NEUTRAL) / (past_minutes / 90.0 + 5.0)
            fm_uid = crosswalk.get(crosswalk_key(p["club"], p["espn_id"]))
            fm = fm_usable.get(fm_uid) if fm_uid is not None else None
            block = fm_feature_block(fm, fm.get("dob", "") if fm else "", day)
            x = [0.0, block["age"], block["ca"], *block["attrs"],
                 gap_feat, block["verified"]]
            if len(x) != len(POOLED_FEATURE_NAMES):
                raise ValueError("pooled feature contract drift")
            if not all(map(math.isfinite, x)):
                raise ValueError("bad UCL feature")
            minutes = float(p["minutes"])
            shots_raw = p["shots"]
            shots = int(shots_raw) if shots_raw is not None else 0
            d = match["date"]
            split = ("train" if d < TRAIN_END else "validation"
                     if d < VAL_END else "test")
            rows.append({
                "league": "UCL", "split": split, "date": d,
                "match_id": str(match["espn_match_id"]),
                "minutes": minutes, "shots": shots,
                "baseline": prior * minutes / 90.0,
                "x": x, "fm_uid": fm_uid,
                "fm_verified": bool(block["verified"]),
            })
        for p in match["players"]:
            key = (p["club"], str(p["espn_id"]))
            st = history.get(key)
            ps = int(p["shots"]) if p["shots"] is not None else 0
            pm = float(p["minutes"])
            if st is None:
                history[key] = {"shots": ps, "minutes": pm, "last_date": day}
            else:
                st["shots"] += ps
                st["minutes"] += pm
                st["last_date"] = day
    return rows


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--espn", type=Path, nargs="+", required=True)
    ap.add_argument("--crosswalk", type=Path, required=True)
    ap.add_argument("--v2-heads", type=Path,
                    default=ROOT / "data/ml/unified_forecast_heads_v2.json")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refusing to overwrite immutable heads artifact")
    archives = [json.loads(p.read_text()) for p in a.espn]
    for archive in archives:
        if archive.get("schema") != "espn_player_appearances_v1":
            raise ValueError("incompatible ESPN source schema")
    crosswalk = load_crosswalk(a.crosswalk)
    fm_usable = load_fm_union(sorted(set(crosswalk.values())))
    rows = ucl_rows(archives, crosswalk, fm_usable)
    by_split = {"train": [r for r in rows if r["split"] == "train"],
                "validation": [r for r in rows if r["split"] == "validation"],
                "test": [r for r in rows if r["split"] == "test"]}
    counts = {k: len(v) for k, v in by_split.items()}
    if min(counts.values()) < len(POOLED_FEATURE_NAMES):
        raise ValueError(f"insufficient UCL rows for pooled fit: {counts}")
    base_val = poisson_loss(by_split["validation"], None)
    fits = {}
    for penalty in PENALTIES:
        beta = fit_ridge(by_split["train"], penalty)
        fits[penalty] = (beta, poisson_loss(by_split["validation"], beta))
    best_penalty = min(fits, key=lambda p: fits[p][1])
    best_beta, best_val = fits[best_penalty]
    base_test = poisson_loss(by_split["test"], None)
    if best_val >= base_val:
        print(json.dumps({
            "result": "null_validation_selects_baseline",
            "counts": counts,
            "validation": {"baseline": base_val,
                           "best_ridge": best_val, "penalty": best_penalty},
            "test": {"baseline": base_test},
        }, indent=1))
        raise SystemExit(3)
    test_fitted = poisson_loss(by_split["test"], best_beta)
    ci, clusters = matchday_bootstrap_ci(
        by_split["test"],
        lambda rs: poisson_loss(rs, best_beta),
        lambda rs: poisson_loss(rs, None))
    v2 = json.loads(a.v2_heads.read_text())
    artifact = {
        "schema": "unified_epl_mls_forecast_heads_v1",
        "model_family": "pooled_player_agnostic_residuals_over_asof_empirical_bayes",
        "competition": "UEFA Champions League (UCL-fitted shots; transferred tackles)",
        "feature_names_shots": list(POOLED_FEATURE_NAMES),
        "shots": {
            "best": str(best_penalty),
            "coefficients": [float(v) for v in best_beta],
            "counts": counts,
            "splits": {"train_end": TRAIN_END, "validation_end": VAL_END},
            "validation": {"baseline": base_val, "fitted": best_val},
            "test": {"baseline": base_test, "fitted": test_fitted,
                     "delta": test_fitted - base_test},
            "test_delta_95pc_matchday_bootstrap": list(ci),
            "test_matchdays": clusters,
            "fm_verified_test_rows": sum(1 for r in by_split["test"] if r["fm_verified"]),
        },
        "tackles": v2["tackles"],
        "defense_note": ("UCL tackles copied from EPL+MLS v2 (no UCL defensive "
                         "labels exist); cross-league transfer, ungated challenger."),
        "selection": "smallest Poisson loss on validation; identity/baseline included",
        "gate": "research_only; UCL test is MD1-only (small-n), re-check as 2026-27 accumulates",
        "provenance": {
            "espn": [str(p) for p in a.espn],
            "crosswalk": str(a.crosswalk),
            "tackles_source": str(a.v2_heads),
        },
    }
    a.output.write_text(json.dumps(artifact, indent=1) + "\n")
    print(json.dumps({
        "result": "ucl_shot_head_fitted",
        "counts": counts,
        "fm_verified": {k: sum(1 for r in v if r["fm_verified"]) for k, v in by_split.items()},
        "validation": {"baseline": round(base_val, 5), "fitted": round(best_val, 5),
                       "penalty": best_penalty},
        "test": {"baseline": round(base_test, 5), "fitted": round(test_fitted, 5),
                 "delta": round(test_fitted - base_test, 5), "ci": [round(v, 5) for v in ci],
                 "matchdays": clusters},
        "output": str(a.output),
    }, indent=1))


if __name__ == "__main__":
    main()
