#!/usr/bin/env python3
"""Slice OOS shot-loss by prior exposure: does the FM residual help debutants?

Rebuilds the production narrow rows (identical code path), loads the fitted
v1 coefficients, and scores baseline vs candidate within low-exposure
(<=180 prior minutes, ~debutants) and high-exposure test slices. Diagnostic
only: no refitting, no selection. If FM differentiates debutants but not
veterans, the single-regime head is misspecified and a two-regime head is
justified; if it helps nowhere, FM adds nothing to shot rates, full stop.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from fit_pooled_epl_mls_shot_model import poisson_loss  # noqa: E402
import fit_unified_forecast_heads as heads  # noqa: E402
from fit_unified_forecast_heads import (  # noqa: E402
    epl_rows_v2,
    load_crosswalk,
    load_fm_union,
    matchday_bootstrap_ci,
)

LOW_MINUTES = 180.0


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--mls-sets", type=Path,
                    default=ROOT / "data/ml/fm26-mls-candidate-sets-union-v2.jsonl")
    ap.add_argument("--mls-manifest", type=Path,
                    default=ROOT / "data/ml/fm26-mls-candidate-sets-union-v2.manifest.json")
    ap.add_argument("--espn", type=Path,
                    default=ROOT / "data/ml/math_engine/espn_frozen_20261011.json")
    ap.add_argument("--crosswalk", type=Path,
                    default=ROOT / "data/ml/math_engine/espn_pipeline_final_oct12/crosswalk.json")
    ap.add_argument("--heads", type=Path,
                    default=ROOT / "data/ml/unified_forecast_heads_v1.json")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refuse overwrite")
    artifact = json.loads(a.heads.read_text())
    import numpy as np
    beta = np.asarray(artifact["shots"]["coefficients"], dtype=float)

    manifest = json.loads(a.mls_manifest.read_text())
    exposure_ix = manifest["feature_names"].index("log_prior_exposure")
    mls = heads.mls_rows_v2(a.mls_sets, a.mls_manifest)
    # Recover prior minutes from the logged exposure feature.
    raw_lines = [json.loads(line) for line in a.mls_sets.read_text().splitlines()
                 if line.strip()]
    expo = {}
    for fx in raw_lines:
        for c in fx["candidates"]:
            if c["label"] is None:
                continue
            key = (str(fx["fixture_id"]), f"mls:{c['player_id']}")
            expo[key] = 90.0 * (math.exp(c["features"][exposure_ix]) - 1.0)
    for r in mls:
        r["past_minutes"] = expo[(r["match_id"], r["player_id"])]

    crosswalk = load_crosswalk(a.crosswalk)
    fm_usable = load_fm_union(sorted(set(crosswalk.values())))
    epl, _ = epl_rows_v2(a.espn, crosswalk, fm_usable)

    test = [r for r in mls + epl if r["split"] == "test" and r["minutes"] > 0]
    low = [r for r in test if r["past_minutes"] <= LOW_MINUTES]
    high = [r for r in test if r["past_minutes"] > LOW_MINUTES]
    ver = [r for r in test if r["x"][-1] > 0.5]
    low_ver = [r for r in low if r["x"][-1] > 0.5]

    def pair(rows):
        base = poisson_loss(rows, None)
        cand = poisson_loss(rows, beta)
        return {"n": len(rows), "baseline": base, "candidate": cand,
                "delta": cand - base}

    out = {
        "low_exposure": pair(low), "high_exposure": pair(high),
        "verified_only": pair(ver), "low_and_verified": pair(low_ver),
        "low_n_matchdays": len({(r["league"], r["date"], r["match_id"])
                                for r in low}),
    }
    for name, rows in (("low", low), ("high", high)):
        if len(rows) > 20:
            ci, days = matchday_bootstrap_ci(
                rows, lambda rs: poisson_loss(rs, beta),
                lambda rs: poisson_loss(rs, None))
            out[f"{name}_delta_ci"] = ci
            out[f"{name}_matchdays"] = days
    a.output.write_text(json.dumps(out, indent=2, sort_keys=True) + "\n")
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
