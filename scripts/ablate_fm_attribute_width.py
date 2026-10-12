#!/usr/bin/env python3
"""Ablation: do more FM attributes improve the pooled shot head?

Compares the production 6-attr residual against a 16-attr residual on
IDENTICAL rows, splits, penalties and seeds. Both widths share the as-of EB
baseline; only the residual feature list changes. Winner is selected on
validation; test is reported for both widths plus the width-vs-width
matchday-cluster bootstrap CI. A null result means the extra ten attributes
carry no marginal OOS signal at current label volume.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from fit_pooled_epl_mls_shot_model import (  # noqa: E402
    fit_ridge,
    poisson_loss,
)
from fit_unified_forecast_heads import (  # noqa: E402
    WIDE_ATTRS,
    epl_rows_v2,
    fm_feature_block,
    load_crosswalk,
    load_fm_union,
    matchday_bootstrap_ci,
)

PENALTIES = (0.1, 1.0, 10.0, 100.0)


def wide_names() -> tuple:
    return ("league_mls", "age_years_norm", "fm_ca_norm",
            *(f"fm_{a}_norm" for a in WIDE_ATTRS),
            "last_seen_log_days", "fm_verified")


def wide_mls_rows(sets_path: Path, manifest_path: Path) -> list[dict]:
    manifest = json.loads(manifest_path.read_text())
    names = manifest["feature_names"]
    need = (["age_years_norm", "fm_ca_norm"]
            + [f"fm_{a}_norm" for a in WIDE_ATTRS]
            + ["last_seen_log_days", "fm_verified"])
    ix = [names.index(v) for v in need]
    rows = []
    for line in sets_path.read_text().splitlines():
        if not line.strip():
            continue
        fx = json.loads(line)
        for c in fx["candidates"]:
            if c["label"] is None:
                continue
            shots = int(c["label"]["shots"])
            minutes = float(c["label"]["minutes"])
            prior = float(c["asof_shots_per90"])
            if not (0 < minutes <= 130) or not (prior > 0) \
                    or not math.isfinite(prior) or shots < 0:
                raise ValueError("invalid MLS exposure")
            feats = [float(c["features"][i]) for i in ix]
            if not all(map(math.isfinite, feats)):
                raise ValueError("bad MLS feature")
            rows.append({
                "league": "MLS", "split": fx["split"], "date": fx["date"],
                "match_id": str(fx["fixture_id"]),
                "player_id": f"mls:{c['player_id']}",
                "shots": shots, "minutes": minutes,
                "baseline": prior * minutes / 90.0,
                "x": [1.0, *feats],
            })
    return rows


def wide_epl_block(fm: dict | None, dob: str, day, gap: float) -> list[float]:
    from datetime import date as date_cls
    block = fm_feature_block(fm, dob, day, WIDE_ATTRS)
    return [0.0, block["age"], block["ca"], *block["attrs"], gap,
            block["verified"]]


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
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refuse overwrite")

    # Narrow (production) rows: reuse the exact fitted pipeline.
    import fit_unified_forecast_heads as heads
    crosswalk = load_crosswalk(a.crosswalk)
    fm_usable = load_fm_union(sorted(set(crosswalk.values())))
    epl_narrow, _ = epl_rows_v2(a.espn, crosswalk, fm_usable)
    mls_narrow = heads.mls_rows_v2(a.mls_sets, a.mls_manifest)

    # Wide rows: same labels/splits, 16-attr residual features.
    mls_wide = wide_mls_rows(a.mls_sets, a.mls_manifest)
    epl_wide = []
    for r in epl_narrow:
        fm_uid = r.get("fm_uid")
        fm = fm_usable.get(fm_uid) if fm_uid else None
        from datetime import date as date_cls
        day = date_cls.fromisoformat(r["date"])
        gap = r["x"][-2]
        xw = wide_epl_block(fm if r["fm_verified"] else None,
                            fm.get("dob", "") if (fm and r["fm_verified"]) else "",
                            day, gap)
        epl_wide.append({**r, "x": xw})

    def splits(rows):
        return {k: [r for r in rows if r["split"] == k]
                for k in ("train", "validation", "test")}

    narrow = splits([r for r in mls_narrow + epl_narrow if r["minutes"] > 0])
    wide = splits([r for r in mls_wide
                   + [r for r in epl_wide if r["minutes"] > 0]])
    for name, part in (("narrow", narrow), ("wide", wide)):
        assert all(part.values()), f"{name} splits incomplete"

    out: dict = {"narrow_features": 11, "wide_features": len(wide_names()),
                 "wide_feature_names": list(wide_names())}
    betas = {}
    for name, part in (("narrow", narrow), ("wide", wide)):
        fitted = {p: fit_ridge(part["train"], p) for p in PENALTIES}
        val = {"identity": poisson_loss(part["validation"], None)}
        for p, b in fitted.items():
            val[str(p)] = poisson_loss(part["validation"], b)
        best = min(val, key=val.get)
        beta = None if best == "identity" else fitted[float(best)]
        betas[name] = beta
        test = part["test"]
        test_mls = [r for r in test if r["league"] == "MLS"]
        test_epl = [r for r in test if r["league"] == "EPL"]
        out[name] = {
            "best": best, "validation": val,
            "test_pooled": _pair(test, beta),
            "test_mls": _pair(test_mls, beta),
            "test_epl": _pair(test_epl, beta),
        }
    # Width-vs-width CI pairs identical labels: rebuild wide test in
    # narrow-test order via (match_id, player_id) keys.
    wide_test_by_key = {(r["match_id"], r["player_id"]): r for r in wide["test"]}
    wci, wdays = matchday_bootstrap_ci(
        narrow["test"],
        lambda rows: poisson_loss(
            [wide_test_by_key[(r["match_id"], r["player_id"])] for r in rows],
            betas["wide"]),
        lambda rows: poisson_loss(rows, betas["narrow"]))
    out["width_delta_95pc_matchday_bootstrap_wide_minus_narrow"] = wci
    out["width_test_matchdays"] = wdays
    out["verdict"] = ("adopt_wide" if wci[1] < 0.0 else
                      "keep_narrow" if wci[0] > 0.0 else "null_result_keep_narrow")
    a.output.write_text(json.dumps(out, indent=2, sort_keys=True) + "\n")
    print(json.dumps(out, indent=2))


def _pair(rows: list[dict], beta) -> dict:
    base = poisson_loss(rows, None)
    cand = poisson_loss(rows, beta)
    return {"baseline": base, "candidate": cand, "delta": cand - base,
            "n": len(rows)}


if __name__ == "__main__":
    main()
