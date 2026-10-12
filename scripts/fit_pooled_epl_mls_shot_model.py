#!/usr/bin/env python3
"""One pooled, player-agnostic EPL+MLS shot-rate model (no deep learning).

Combines 2026 MLS SofaScore candidate labels (with FM26 db linkage where
available) and 2026 EPL ESPN observed appearances (role-neutral priors, no FM
linkage claimed) into a single ridge-Poisson residual model over a shared
as-of baseline. No player-ID coefficients. Chronological splits. Proper
Poisson log loss on held-out matches. Research-only; feeds Rust simulator
shot priors as corrected per-90 rates, not as published forecasts.
"""
from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from datetime import date
from pathlib import Path

import numpy as np
from scipy.optimize import minimize

ROOT = Path(__file__).resolve().parents[1]

# Fixed BEFORE labels: residual features only; baseline carries the as-of rate.
POOLED_FEATURE_NAMES = (
    "league_mls",
    "age_years_norm",
    "fm_ca_norm",
    "fm_Finishing_norm",
    "fm_OffTheBall_norm",
    "fm_Dribbling_norm",
    "fm_Composure_norm",
    "fm_Anticipation_norm",
    "fm_Passing_norm",
    "last_seen_log_days",
    "fm_verified",
)
PENALTIES = (0.1, 1.0, 10.0, 100.0)
ROLE_PRIOR_EPL_NEUTRAL = 0.9


def mls_rows(sets_path: Path, manifest_path: Path):
    manifest = json.loads(manifest_path.read_text())
    names = manifest["feature_names"]
    # Map pooled names to MLS candidate-set columns. MLS sets have 43 cols:
    # 40 shot-dataset features + is_home, last_seen_log_days, fm_verified.
    # The 10-feature GLM subset uses age/ca/FM attrs/last_seen/fm_verified.
    # Here we add league_mls=1 and drop is_home (venue is not a player trait).
    need = {
        "age_years_norm": "age_years_norm",
        "fm_ca_norm": "fm_ca_norm",
        "fm_Finishing_norm": "fm_Finishing_norm",
        "fm_OffTheBall_norm": "fm_OffTheBall_norm",
        "fm_Dribbling_norm": "fm_Dribbling_norm",
        "fm_Composure_norm": "fm_Composure_norm",
        "fm_Anticipation_norm": "fm_Anticipation_norm",
        "fm_Passing_norm": "fm_Passing_norm",
        "last_seen_log_days": "last_seen_log_days",
        "fm_verified": "fm_verified",
    }
    ix = {k: names.index(v) for k, v in need.items()}
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
            if not (0 < minutes <= 130) or not (prior > 0) or not math.isfinite(prior) or shots < 0:
                raise ValueError("invalid MLS exposure")
            feats = [float(c["features"][i]) for i in ix.values()]
            if not all(map(math.isfinite, feats)):
                raise ValueError("bad MLS feature")
            # Order to POOLED_FEATURE_NAMES (league_mls=1 first)
            x = [1.0, *feats]
            rows.append({
                "league": "MLS", "split": fx["split"], "date": fx["date"],
                "match_id": str(fx["fixture_id"]), "player_id": f"mls:{c['player_id']}",
                "shots": shots, "minutes": minutes,
                "baseline": prior * minutes / 90.0, "x": x,
            })
    return rows


def epl_rows(espn_path: Path):
    archive = json.loads(espn_path.read_text())
    matches = sorted(archive["matches"], key=lambda m: (m["date"], m["espn_match_id"]))
    history: dict[tuple[str, str], dict] = {}
    rows = []
    for m in matches:
        day = date.fromisoformat(m["date"])
        for p in m["players"]:
            key = (p["club"], p["espn_id"])
            st = history.get(key)
            past_shots = st["shots"] if st else 0
            past_minutes = st["minutes"] if st else 0.0
            if st is None:
                last_seen_feat = math.log1p(120) / 5.0
            else:
                gap = (day - st["last_date"]).days
                last_seen_feat = math.log1p(max(0, gap)) / 5.0
            prior = (past_shots + 5.0 * ROLE_PRIOR_EPL_NEUTRAL) / (past_minutes / 90.0 + 5.0)
            minutes = float(p["minutes"])
            if not (0 <= minutes <= 90):
                raise ValueError("invalid ESPN exposure")
            if minutes == 0:
                # Did-not-play rows carry no shot exposure; skip (not zero shots).
                continue
            shots = int(p["shots"]) if p["shots"] is not None else 0
            if shots < 0:
                raise ValueError("invalid ESPN shots")
            x = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, last_seen_feat, 0.0]
            rows.append({
                "league": "EPL", "split": None, "date": m["date"],
                "match_id": m["espn_match_id"], "player_id": f"espn:{p['espn_id']}",
                "shots": shots, "minutes": minutes,
                "baseline": prior * minutes / 90.0, "x": x,
                "_club": p["club"], "_name": p["name"],
            })
            # Update history AFTER building all rows for this match (causal).
            # Defer update to after match loop to avoid within-match leakage.
        # Causal update after labels emitted
        for p in m["players"]:
            key = (p["club"], p["espn_id"])
            st = history.get(key)
            ps = int(p["shots"]) if p["shots"] is not None else 0
            pm = float(p["minutes"])
            if st is None:
                history[key] = {"shots": ps, "minutes": pm, "last_date": day}
            else:
                st["shots"] += ps
                st["minutes"] += pm
                st["last_date"] = day
    # Assign EPL splits by same calendar cutoffs as MLS (train<06-01, val<08-01, else test).
    # All ESPN dates are >=2026-08-21, so all fall in test. This preserves chronology:
    # pooled train max (MLS <06-01) < pooled val min (MLS 06-01..08-01) < pooled test min (>=08-01).
    for r in rows:
        d = r["date"]
        if d < "2026-06-01":
            r["split"] = "train"
        elif d < "2026-08-01":
            r["split"] = "validation"
        else:
            r["split"] = "test"
    return rows


def fit_ridge(rows, penalty):
    x = np.asarray([r["x"] for r in rows], dtype=np.float64)
    y = np.asarray([r["shots"] for r in rows], dtype=np.float64)
    prior = np.asarray([r["baseline"] for r in rows], dtype=np.float64)
    if x.ndim != 2 or len(rows) < len(POOLED_FEATURE_NAMES):
        raise ValueError("insufficient pooled training")

    def objective(beta):
        logits = np.clip(x @ beta, -3.0, 3.0)
        mu = prior * np.exp(logits)
        return float((mu - y * logits).sum() / len(y) + penalty * (beta @ beta))

    def gradient(beta):
        raw = x @ beta
        clip = np.clip(raw, -3.0, 3.0)
        mu = prior * np.exp(clip)
        active = ((raw > -3.0) & (raw < 3.0)).astype(float)
        return x.T @ ((mu - y) * active) / len(y) + 2.0 * penalty * beta

    res = minimize(objective, np.zeros(x.shape[1]), jac=gradient,
                   method="L-BFGS-B", options={"maxiter": 1000, "gtol": 1e-9})
    if not res.success and np.linalg.norm(res.jac) > 1e-6:
        raise RuntimeError(f"pooled ridge-Poisson failed: {res.message}")
    return res.x


def predicted(row, beta):
    corr = 0.0 if beta is None else float(np.clip(np.dot(row["x"], beta), -3.0, 3.0))
    return row["baseline"] * math.exp(corr)


def poisson_loss(rows, beta):
    return sum(
        predicted(r, beta) - r["shots"] * math.log(max(predicted(r, beta), 1e-12))
        + math.lgamma(r["shots"] + 1)
        for r in rows
    ) / len(rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--mls-sets", type=Path, default=ROOT / "data/ml/fm26-mls-candidate-sets.jsonl")
    ap.add_argument("--mls-manifest", type=Path, default=ROOT / "data/ml/fm26-mls-candidate-sets.manifest.json")
    ap.add_argument("--espn", type=Path, default=ROOT / "data/ml/math_engine/espn_frozen_20261011.json")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refuse overwrite")
    mls = mls_rows(a.mls_sets, a.mls_manifest)
    epl = epl_rows(a.espn)
    pooled = mls + epl
    split = {k: [r for r in pooled if r["split"] == k] for k in ("train", "validation", "test")}
    if any(not v for v in split.values()):
        raise ValueError(f"all three pooled splits required, got { {k: len(v) for k, v in split.items()} }")
    if not (max(r["date"] for r in split["train"]) < min(r["date"] for r in split["validation"])
            and max(r["date"] for r in split["validation"]) < min(r["date"] for r in split["test"])):
        raise ValueError("pooled chronological overlap")
    fitted = {p: fit_ridge(split["train"], p) for p in PENALTIES}
    val_losses = {"identity": poisson_loss(split["validation"], None)}
    for p, b in fitted.items():
        val_losses[str(p)] = poisson_loss(split["validation"], b)
    best = min(val_losses, key=val_losses.get)
    beta = None if best == "identity" else fitted[float(best)]
    # Pooled + per-league test
    test = split["test"]
    test_mls = [r for r in test if r["league"] == "MLS"]
    test_epl = [r for r in test if r["league"] == "EPL"]
    report = {
        "schema": "pooled_epl_mls_shot_rate_v1",
        "model_family": "pooled_ridge_poisson_residual_over_asof_baseline",
        "feature_names": list(POOLED_FEATURE_NAMES),
        "penalties": list(PENALTIES),
        "selection": "smallest Poisson log loss on pooled validation (MLS Jun-Jul); identity included",
        "best": best,
        "coefficients": beta.tolist() if beta is not None else None,
        "counts": {
            "train": len(split["train"]), "validation": len(split["validation"]), "test": len(test),
            "train_mls": sum(1 for r in split["train"] if r["league"] == "MLS"),
            "train_epl": sum(1 for r in split["train"] if r["league"] == "EPL"),
            "validation_mls": sum(1 for r in split["validation"] if r["league"] == "MLS"),
            "test_mls": len(test_mls), "test_epl": len(test_epl),
        },
        "validation": {
            "baseline": val_losses["identity"], "selected": val_losses[best],
            "delta": val_losses[best] - val_losses["identity"], "all": val_losses,
        },
        "test_pooled": {
            "baseline": poisson_loss(test, None), "candidate": poisson_loss(test, beta),
            "delta": poisson_loss(test, beta) - poisson_loss(test, None),
        },
        "test_mls": {
            "baseline": poisson_loss(test_mls, None), "candidate": poisson_loss(test_mls, beta),
            "delta": poisson_loss(test_mls, beta) - poisson_loss(test_mls, None),
        } if test_mls else None,
        "test_epl": {
            "baseline": poisson_loss(test_epl, None), "candidate": poisson_loss(test_epl, beta),
            "delta": poisson_loss(test_epl, beta) - poisson_loss(test_epl, None),
        } if test_epl else None,
        "identity_features": False,
        "parameter_sharing": "same residual coefficients for EPL and MLS; league_mls flag allowed, no player lookup",
        "simulation_contract": "corrected per-90 = asof_baseline_per90 * exp(X·beta); Rust engine consumes via shots_per90 priors",
        "gate": "research_only; no promotion without larger future chronological validation",
    }
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({
        "best": best, "counts": report["counts"],
        "validation": report["validation"], "test_pooled": report["test_pooled"],
        "test_mls": report["test_mls"], "test_epl": report["test_epl"],
    }, indent=2))


if __name__ == "__main__":
    main()
