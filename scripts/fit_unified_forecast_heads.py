#!/usr/bin/env python3
"""Unified EPL+MLS forecast heads: shots, conversion, participation (no deep learning).

Player-agnostic, shared parameterization across leagues (league flag allowed,
no player-ID coefficients). Extends scripts/fit_pooled_epl_mls_shot_model.py:

- SHOTS: pooled ridge-Poisson residual over the shared as-of empirical-Bayes
  baseline, but EPL rows now carry REAL verified FM26 traits (ESPN<->FM
  crosswalk FM UIDs joined to the complete-everyone FM26 union with the same
  (name, DOB) usability gate as the MLS labels) instead of all-zero features.
  Unverifiable EPL players keep zero features with fm_verified=0 (masked,
  never another person's attributes).
- CONVERSION: pooled ridge-logistic goals|shots head over an as-of
  empirical-Bayes conversion baseline. Labels exist only in ESPN EPL, so MLS
  contributes no conversion rows (documented gap, not imputed zeros).
- PARTICIPATION: ridge-logistic starts head over an as-of empirical-Bayes
  start-rate baseline on full 40-man ESPN squad listings (DNP rows are
  informative non-starts, not dropped). MLS candidate sets have no start
  labels (documented gap).

Chronological splits, proper losses, validation-selected penalties and
temperature calibration, match-clustered bootstrap CIs, ablations against the
as-of empirical-Bayes baselines. Research-only artifact; the Rust match engine
consumes corrected rates through scripts/apply_pooled_correction.py, never by
reimplementing this fit.

Refuses to overwrite existing artifacts.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import sys
from collections import defaultdict
from datetime import date
from pathlib import Path

import numpy as np
from scipy.optimize import minimize

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from fit_pooled_epl_mls_shot_model import (  # noqa: E402
    POOLED_FEATURE_NAMES,
    ROLE_PRIOR_EPL_NEUTRAL,
    fit_ridge,
    poisson_loss,
)

CONVERSION_FEATURE_NAMES = tuple(n for n in POOLED_FEATURE_NAMES if n != "league_mls")
PARTICIPATION_FEATURE_NAMES = CONVERSION_FEATURE_NAMES
PENALTIES = (0.1, 1.0, 10.0, 100.0)
# Fixed a priori (typical football rates), not estimated on the evaluation window.
CONVERSION_PRIOR_MEAN = 0.105
CONVERSION_PRIOR_STRENGTH_SHOTS = 20.0
START_PRIOR_STRENGTH = 5.0
TEMPERATURE_GRID = tuple(round(v, 2) for v in
                         (0.5, 0.7, 0.85, 1.0, 1.2, 1.5, 2.0))
VERIFIED_CROSSWALK_STATUS = frozenset({
    "exact_full_name_and_club",
    "unique_legal_given_family_token_pair_and_club",
})
# ESPN archive uses full club names; the verified crosswalk uses short roster
# names. Aliases below are exact, closed, and verified against both sources;
# unknown clubs never alias (their rows keep fm_verified=0).
ESPN_CLUB_TO_CROSSWALK = {
    "Coventry City": "Coventry",
    "Hull City": "Hull",
    "Newcastle United": "Newcastle",
    "Manchester City": "Man City",
    "Nottingham Forest": "Nott'm Forest",
}


def crosswalk_key(club: str, espn_id) -> tuple[str, str]:
    return (ESPN_CLUB_TO_CROSSWALK.get(club, club), str(espn_id))


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def load_fm_union(wanted_uids: set[str]):
    from fm26_complete import MEN, load_union

    from build_fm_mls_shot_dataset import fm_row_usable

    union, _ = load_union(wanted_uids=wanted_uids)
    usable: dict[str, dict] = {}
    for uid, record in union.items():
        row = {
            **{f"attr_{f}": ("" if v is None else str(v))
               for f, v in (record.get("attributes") or {}).items()},
            "uid": uid, "name": record.get("name", ""), "dob": record.get("dob", ""),
            "kind": record.get("kind", ""), "attr_source": record.get("attr_source", ""),
            "ca": ("" if record.get("ca") is None else str(record["ca"])),
        }
        # A single union row is trivially unique on (name, DOB), so the MLS
        # usability gate reduces to its kind/provenance check: player/db is
        # observed, staff/db with a valid block is rescued, everything else is
        # never observed traits.
        good, _ = fm_row_usable([row])
        if good is not None:
            usable[uid] = row
    return usable


def load_crosswalk(path: Path) -> dict[tuple[str, str], str]:
    raw = json.loads(path.read_text())
    out: dict[tuple[str, str], str] = {}
    for person in raw.get("people", []):
        if person.get("status") not in VERIFIED_CROSSWALK_STATUS:
            continue
        fm_uid = person.get("fm_uid")
        espn_id = person.get("espn_id")
        club = person.get("club")
        if fm_uid is None or espn_id is None or club is None:
            continue
        key = (str(club), str(espn_id))
        if key in out:
            raise ValueError(f"duplicate verified crosswalk identity {key}")
        out[key] = str(fm_uid)
    return out


def fm_feature_block(fm: dict | None, dob: str, day: date) -> dict:
    """Normalized FM block mirroring the MLS label construction.

    (age-25)/10, (ca-100)/100, (attr-10)/10; unverified -> 0.0 with
    fm_verified=0. Missing/zero attrs are 0.0 (same convention as MLS rows,
    where missing flags live in the wider candidate-set columns, not here).
    """
    if fm is None:
        return {"age": 0.0, "ca": 0.0, "attrs": [0.0] * 6, "verified": 0.0}
    try:
        born = date.fromisoformat(dob)
        age = max(15.0, min(50.0, (day - born).days / 365.25))
    except ValueError:
        return {"age": 0.0, "ca": 0.0, "attrs": [0.0] * 6, "verified": 0.0}
    ca_raw = fm.get("ca") or ""
    try:
        ca = float(ca_raw)
    except ValueError:
        ca = 0.0
    attrs = []
    for name in ("Finishing", "OffTheBall", "Dribbling", "Composure",
                 "Anticipation", "Passing"):
        raw = fm.get(f"attr_{name}", "")
        try:
            value = float(raw)
        except ValueError:
            value = 0.0
        attrs.append((value - 10.0) / 10.0 if value > 0.0 else 0.0)
    return {"age": (age - 25.0) / 10.0, "ca": (ca - 100.0) / 100.0 if ca > 0 else 0.0,
            "attrs": attrs, "verified": 1.0}


def epl_rows_v2(espn_path: Path, crosswalk: dict, fm_usable: dict) -> list[dict]:
    """Causal EPL rows: EB baselines from strictly prior ESPN matches, real FM
    features where the crosswalk + FM usability gate verify them."""
    archive = json.loads(espn_path.read_text())
    matches = sorted(archive["matches"], key=lambda m: (m["date"], m["espn_match_id"]))
    history: dict[tuple[str, str], dict] = {}
    rows: list[dict] = []
    capped_own_goal_rows = 0
    for match in matches:
        day = date.fromisoformat(match["date"])
        for p in match["players"]:
            key = (p["club"], str(p["espn_id"]))
            st = history.get(key)
            past_shots = st["shots"] if st else 0
            past_minutes = st["minutes"] if st else 0.0
            past_goals = st["goals"] if st else 0
            past_shot_shots = st["s_shots"] if st else 0  # shots faced as exposure
            past_starts = st["starts"] if st else 0
            past_listings = st["listings"] if st else 0
            gap_feat = math.log1p(120) / 5.0 if st is None else math.log1p(
                max(0, (day - st["last_date"]).days)) / 5.0
            prior = (past_shots + 5.0 * ROLE_PRIOR_EPL_NEUTRAL) / (past_minutes / 90.0 + 5.0)
            conv_prior = ((past_goals + CONVERSION_PRIOR_MEAN * CONVERSION_PRIOR_STRENGTH_SHOTS)
                          / (past_shot_shots + CONVERSION_PRIOR_STRENGTH_SHOTS))
            # Half-strength start prior: (past_starts + 2.5) / (past_listings + 5).
            start_prior = (past_starts + 2.5) / (past_listings + 5.0)
            fm_uid = crosswalk.get(crosswalk_key(p["club"], p["espn_id"]))
            fm = fm_usable.get(fm_uid) if fm_uid is not None else None
            # DOB corroboration: the FM row must carry a usable DOB; the block
            # builder quarantines rows without one (verified=0).
            block = fm_feature_block(fm, fm.get("dob", "") if fm else "", day)
            x = [0.0, block["age"], block["ca"], *block["attrs"],
                 gap_feat, block["verified"]]
            if len(x) != len(POOLED_FEATURE_NAMES):
                raise ValueError("pooled feature contract drift")
            if not all(map(math.isfinite, x)):
                raise ValueError("bad EPL feature")
            minutes = float(p["minutes"])
            if not (0 <= minutes <= 130):
                raise ValueError("invalid ESPN exposure")
            shots_raw = p["shots"]
            shots = int(shots_raw) if shots_raw is not None else 0
            goals_raw = p.get("goals")
            goals = int(goals_raw) if goals_raw is not None else 0
            if shots < 0 or goals < 0:
                raise ValueError("invalid ESPN event counts")
            if goals > shots:
                # Own-goal accounting can credit goals without shots; the
                # binomial conversion head models goals-from-shots, so cap the
                # conversion target at shots and count the remainder apart.
                capped_own_goal_rows += 1
                goals = shots
            rows.append({
                "league": "EPL", "split": None, "date": match["date"],
                "match_id": str(match["espn_match_id"]),
                "player_id": f"espn:{p['espn_id']}",
                "minutes": minutes, "shots": shots, "goals": goals,
                "started": bool(p["started"]),
                "baseline": prior * minutes / 90.0,
                "asof_shots_per90": prior,
                "conv_baseline": min(0.7, max(1e-4, conv_prior)),
                "start_baseline": min(0.99, max(0.01, start_prior)),
                "x": x, "x_noleague": x[1:],
                "fm_uid": fm_uid, "fm_verified": bool(block["verified"]),
            })
        for p in match["players"]:
            key = (p["club"], str(p["espn_id"]))
            st = history.get(key)
            ps = int(p["shots"]) if p["shots"] is not None else 0
            pm = float(p["minutes"])
            pg = int(p.get("goals")) if p.get("goals") is not None else 0
            if st is None:
                history[key] = {"shots": ps, "minutes": pm, "goals": pg,
                                "s_shots": ps, "starts": int(bool(p["started"])),
                                "listings": 1, "last_date": day}
            else:
                st["shots"] += ps
                st["minutes"] += pm
                st["goals"] += pg
                st["s_shots"] += ps
                st["starts"] += int(bool(p["started"]))
                st["listings"] += 1
                st["last_date"] = day
    for r in rows:
        d = r["date"]
        r["split"] = ("train" if d < "2026-06-01" else "validation"
                      if d < "2026-08-01" else "test")
    return rows, {"capped_goals_exceeding_shots": capped_own_goal_rows}


def mls_rows_v2(sets_path: Path, manifest_path: Path) -> list[dict]:
    from fit_pooled_epl_mls_shot_model import mls_rows

    rows = mls_rows(sets_path, manifest_path)
    for r in rows:
        r["x_noleague"] = r["x"][1:]
        r["goals"] = None
        r["started"] = None
        r["minutes_all"] = r["minutes"]
    return rows


def sigmoid(z: float) -> float:
    if z >= 0:
        return 1.0 / (1.0 + math.exp(-z))
    e = math.exp(z)
    return e / (1.0 + e)


def fit_binomial_residual(rows: list[dict], trials_key: str, events_key: str,
                          base_key: str, penalty: float) -> np.ndarray:
    """Ridge-logistic residual over an as-of baseline rate: p = base * sigmoid
    correction is wrong multiplicatively; instead logit(p) = logit(base) + x.b.

    Implemented as logistic regression with offset logit(base).
    """
    xs, ys, ns, offs = [], [], [], []
    for r in rows:
        n = r[trials_key]
        if n is None or n <= 0:
            continue
        base = min(0.999, max(1e-6, r[base_key]))
        xs.append(r["x_noleague"])
        ys.append(r[events_key])
        ns.append(n)
        offs.append(math.log(base / (1.0 - base)))
    x = np.asarray(xs, dtype=np.float64)
    y = np.asarray(ys, dtype=np.float64)
    n = np.asarray(ns, dtype=np.float64)
    off = np.asarray(offs, dtype=np.float64)
    if len(x) < x.shape[1] + 1:
        raise ValueError("insufficient binomial training rows")

    def objective(beta):
        logits = np.clip(off + x @ beta, -35.0, 35.0)
        p = 1.0 / (1.0 + np.exp(-logits))
        nll = float((-(y * np.log(p + 1e-15) + (n - y) * np.log(1 - p + 1e-15))).sum()
                     / n.sum())
        return nll + penalty * float(beta @ beta)

    def gradient(beta):
        logits = np.clip(off + x @ beta, -35.0, 35.0)
        p = 1.0 / (1.0 + np.exp(-logits))
        return x.T @ ((p - y / n) * n) / n.sum() + 2.0 * penalty * beta

    res = minimize(objective, np.zeros(x.shape[1]), jac=gradient,
                   method="L-BFGS-B", options={"maxiter": 1000, "gtol": 1e-9})
    if not res.success and np.linalg.norm(res.jac) > 1e-6:
        raise RuntimeError(f"binomial residual fit failed: {res.message}")
    return res.x


def per_shot_prob(row: dict, base_key: str, beta: np.ndarray | None,
                   temperature: float = 1.0) -> float:
    """Residual-corrected per-shot probability with temperature scaling."""
    base = min(0.999, max(1e-6, row[base_key]))
    z = math.log(base / (1.0 - base))
    if beta is not None:
        z += float(np.dot(row["x_noleague"], beta))
    return sigmoid(max(-35.0, min(35.0, z)) / temperature)


def binomial_loss(rows: list[dict], trials_key: str, events_key: str,
                  base_key: str, beta: np.ndarray | None,
                  temperature: float = 1.0) -> float:
    total_n = 0.0
    nll = 0.0
    for r in rows:
        n = r[trials_key]
        if n is None or n <= 0:
            continue
        p = min(1 - 1e-12, max(1e-12, per_shot_prob(r, base_key, beta, temperature)))
        y = r[events_key]
        if not (0 <= y <= n):
            raise ValueError("conversion target outside binomial support")
        nll += -(y * math.log(p) + (n - y) * math.log(1 - p))
        total_n += n
    if total_n <= 0:
        raise ValueError("no binomial exposure")
    return nll / total_n


def occurrence_probs(rows: list[dict], trials_key: str, base_key: str,
                     beta: np.ndarray | None, temperature: float):
    """Player-match P(>=1 event) under the binomial head: 1-(1-p)^n.

    Shots>0 rows only for conversion (goals without shots are own-goal
    accounting, a separate process reported apart). Participation uses
    trials of 1, where this reduces to the start probability.
    """
    probs = []
    for r in rows:
        n = r[trials_key]
        if n is None or n <= 0:
            continue
        p = per_shot_prob(r, base_key, beta, temperature)
        probs.append(1.0 - (1.0 - p) ** n)
    return probs


def brier_and_ece(probs: list[float], outcomes: list[float], bins: int = 10):
    if len(probs) != len(outcomes) or not probs:
        raise ValueError("empty calibration inputs")
    brier = sum((p - y) ** 2 for p, y in zip(probs, outcomes)) / len(probs)
    edges = [i / bins for i in range(bins + 1)]
    ece = 0.0
    table = []
    for b in range(bins):
        idx = [i for i, p in enumerate(probs)
               if (edges[b] < p <= edges[b + 1]) or (b == 0 and p == 0.0)]
        if not idx:
            table.append({"bin": b, "n": 0, "mean_pred": None, "mean_obs": None})
            continue
        mp = sum(probs[i] for i in idx) / len(idx)
        mo = sum(outcomes[i] for i in idx) / len(idx)
        ece += len(idx) / len(probs) * abs(mp - mo)
        table.append({"bin": b, "n": len(idx), "mean_pred": mp, "mean_obs": mo})
    return brier, ece, table


def occurrence_outcomes(rows: list[dict], trials_key: str, events_key: str):
    """Player-match 0/1 indicators aligned with occurrence_probs."""
    outs = []
    for r in rows:
        if r[trials_key] is None or r[trials_key] <= 0:
            continue
        outs.append(float(r[events_key] > 0))
    return outs


def matchday_bootstrap_ci(rows: list[dict], loss_a, loss_b, draws: int = 2500,
                          seed: int = 42):
    """Paired bootstrap over matchdays (same-day matches are not independent)."""
    days: dict[str, list] = defaultdict(list)
    for r in rows:
        days[(r["league"], r["date"], r["match_id"])].append(r)
    groups = list(days.values())
    rng = random.Random(seed)
    ests = []
    for _ in range(draws):
        sample = [rng.choice(groups) for _ in groups]
        flat = [r for g in sample for r in g]
        ests.append(loss_a(flat) - loss_b(flat))
    ests.sort()
    return [ests[int(0.025 * draws)], ests[int(0.975 * draws)]], len(groups)


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
    for p in (a.mls_sets, a.mls_manifest, a.espn, a.crosswalk):
        if not p.exists():
            ap.error(f"missing input {p}")

    crosswalk = load_crosswalk(a.crosswalk)
    fm_usable = load_fm_union(sorted(set(crosswalk.values())))
    epl, epl_audit = epl_rows_v2(a.espn, crosswalk, fm_usable)
    mls = mls_rows_v2(a.mls_sets, a.mls_manifest)

    # ---- SHOTS head (pooled EPL+MLS) ----
    pooled = mls + epl_shots_only(epl)
    split = {k: [r for r in pooled if r["split"] == k]
             for k in ("train", "validation", "test")}
    if any(not v for v in split.values()):
        raise ValueError(f"splits incomplete: { {k: len(v) for k, v in split.items()} }")
    if not (max(r["date"] for r in split["train"]) < min(r["date"] for r in split["validation"])
            and max(r["date"] for r in split["validation"]) < min(r["date"] for r in split["test"])):
        raise ValueError("pooled chronological overlap")
    fitted = {p: fit_ridge([r for r in split["train"] if r["minutes"] > 0], p)
              for p in PENALTIES}
    val_loss = {"identity": poisson_loss([r for r in split["validation"] if r["minutes"] > 0],
                                         None)}
    for p, b in fitted.items():
        val_loss[str(p)] = poisson_loss([r for r in split["validation"] if r["minutes"] > 0],
                                        b)
    best = min(val_loss, key=val_loss.get)
    beta_shots = None if best == "identity" else fitted[float(best)]
    test = split["test"]
    test_mls = [r for r in test if r["league"] == "MLS"]
    test_epl = [r for r in test if r["league"] == "EPL"]
    shots_ci, shots_days = matchday_bootstrap_ci(
        [r for r in test if r["minutes"] > 0],
        lambda rows: poisson_loss(rows, beta_shots),
        lambda rows: poisson_loss(rows, None))

    # ---- CONVERSION head (ESPN EPL only: MLS has no goal labels) ----
    conv_rows = [r for r in epl if r["minutes"] > 0 and r["shots"] > 0]
    conv_dates = sorted(set(r["date"] for r in conv_rows))
    if len(conv_dates) < 6:
        raise ValueError("insufficient ESPN dates for conversion splits")
    cut1, cut2 = conv_dates[len(conv_dates) * 3 // 5], conv_dates[len(conv_dates) * 4 // 5]
    conv = {"train": [r for r in conv_rows if r["date"] < cut1],
            "validation": [r for r in conv_rows if cut1 <= r["date"] < cut2],
            "test": [r for r in conv_rows if r["date"] >= cut2]}
    conv_fits = {p: fit_binomial_residual(conv["train"], "shots", "goals",
                                          "conv_baseline", p) for p in PENALTIES}
    conv_val = {"baseline": binomial_loss(conv["validation"], "shots", "goals",
                                          "conv_baseline", None)}
    for p, b in conv_fits.items():
        conv_val[str(p)] = binomial_loss(conv["validation"], "shots", "goals",
                                         "conv_baseline", b)
    conv_best = min(conv_val, key=conv_val.get)
    beta_conv = None if conv_best == "baseline" else conv_fits[float(conv_best)]
    conv_temp = select_temperature(conv["validation"], "shots", "goals",
                                   "conv_baseline", beta_conv)
    conv_test_rows = conv["test"]
    # Goals are recorded per player-match, not per shot, so calibrate the
    # player-match occurrence probability P(>=1 goal | shots) = 1-(1-p)^n.
    # Rows with shots == 0 carry no conversion exposure; goals credited
    # without shots are own-goal accounting, reported apart below.
    conv_probs_raw = occurrence_probs(conv_test_rows, "shots", "conv_baseline",
                                      beta_conv, 1.0)
    conv_probs_cal = occurrence_probs(conv_test_rows, "shots", "conv_baseline",
                                      beta_conv, conv_temp)
    conv_outs = occurrence_outcomes(conv_test_rows, "shots", "goals")
    conv_zero_shot = [r for r in epl if r["date"] >= cut2 and r["minutes"] > 0
                      and r["shots"] == 0]
    conv_zero_shot_goal_rate = (sum(1 for r in conv_zero_shot if r["goals"] > 0)
                                / max(1, len(conv_zero_shot)))
    conv_brier_raw, conv_ece_raw, conv_tab_raw = brier_and_ece(conv_probs_raw, conv_outs)
    conv_brier_cal, conv_ece_cal, conv_tab_cal = brier_and_ece(conv_probs_cal, conv_outs)
    conv_probs_base = occurrence_probs(conv_test_rows, "shots", "conv_baseline", None, 1.0)
    conv_brier_base, conv_ece_base, _ = brier_and_ece(conv_probs_base, conv_outs)
    conv_ci, conv_days = matchday_bootstrap_ci(
        conv_test_rows,
        lambda rows: binomial_loss(rows, "shots", "goals", "conv_baseline", beta_conv,
                                   conv_temp),
        lambda rows: binomial_loss(rows, "shots", "goals", "conv_baseline", None))

    # ---- PARTICIPATION head (ESPN EPL full squad listings) ----
    part_rows = list(epl)  # DNP rows included: informative non-starts
    part_dates = sorted(set(r["date"] for r in part_rows))
    pcut1, pcut2 = part_dates[len(part_dates) * 3 // 5], part_dates[len(part_dates) * 4 // 5]
    part = {"train": [r for r in part_rows if r["date"] < pcut1],
            "validation": [r for r in part_rows if pcut1 <= r["date"] < pcut2],
            "test": [r for r in part_rows if r["date"] >= pcut2]}
    for grp in part.values():
        for r in grp:
            r["trials_one"] = 1
            r["started_int"] = int(r["started"])
    part_fits = {p: fit_binomial_residual(part["train"], "trials_one", "started_int",
                                          "start_baseline", p) for p in PENALTIES}
    part_val = {"baseline": binomial_loss(part["validation"], "trials_one", "started_int",
                                          "start_baseline", None)}
    for p, b in part_fits.items():
        part_val[str(p)] = binomial_loss(part["validation"], "trials_one", "started_int",
                                         "start_baseline", b)
    part_best = min(part_val, key=part_val.get)
    beta_part = None if part_best == "baseline" else part_fits[float(part_best)]
    part_temp = select_temperature(part["validation"], "trials_one", "started_int",
                                   "start_baseline", beta_part)
    part_probs_raw = occurrence_probs(part["test"], "trials_one", "start_baseline",
                                      beta_part, 1.0)
    part_probs_cal = occurrence_probs(part["test"], "trials_one", "start_baseline",
                                      beta_part, part_temp)
    part_outs = occurrence_outcomes(part["test"], "trials_one", "started_int")
    part_brier_raw, part_ece_raw, part_tab_raw = brier_and_ece(part_probs_raw, part_outs)
    part_brier_cal, part_ece_cal, part_tab_cal = brier_and_ece(part_probs_cal, part_outs)
    part_probs_base = occurrence_probs(part["test"], "trials_one", "start_baseline",
                                       None, 1.0)
    part_brier_base, part_ece_base, _ = brier_and_ece(
        part_probs_base, occurrence_outcomes(part["test"], "trials_one", "started_int"))
    part_ci, part_days = matchday_bootstrap_ci(
        part["test"],
        lambda rows: binomial_loss(rows, "trials_one", "started_int", "start_baseline",
                                   beta_part, part_temp),
        lambda rows: binomial_loss(rows, "trials_one", "started_int", "start_baseline",
                                   None))

    report = {
        "schema": "unified_epl_mls_forecast_heads_v1",
        "model_family": "pooled_player_agnostic_residuals_over_asof_empirical_bayes",
        "feature_names_shots": list(POOLED_FEATURE_NAMES),
        "feature_names_conversion": list(CONVERSION_FEATURE_NAMES),
        "feature_names_participation": list(PARTICIPATION_FEATURE_NAMES),
        "penalties": list(PENALTIES),
        "selection": "smallest proper loss on validation; identity/baseline included",
        "provenance": {
            "mls_sets_sha256": sha256(a.mls_sets),
            "mls_manifest_sha256": sha256(a.mls_manifest),
            "espn_sha256": sha256(a.espn),
            "crosswalk_sha256": sha256(a.crosswalk),
            "fm_union": "complete-everyone union via scripts/fm26_complete.load_union, "
                        "usability gate scripts/build_fm_mls_shot_dataset.fm_row_usable",
        },
        "epl_fm_linkage": {
            "verified_crosswalk_identities": len(crosswalk),
            "fm_union_usable": len(fm_usable),
            "epl_rows": len(epl),
            "epl_rows_fm_verified": sum(1 for r in epl if r["fm_verified"]),
            "capped_goals_exceeding_shots": epl_audit["capped_goals_exceeding_shots"],
        },
        "shots": {
            "best": best,
            "coefficients": beta_shots.tolist() if beta_shots is not None else None,
            "counts": {
                "train": len(split["train"]), "validation": len(split["validation"]),
                "test": len(test), "test_mls": len(test_mls), "test_epl": len(test_epl),
            },
            "validation": {"baseline": val_loss["identity"], "selected": val_loss[best],
                           "delta": val_loss[best] - val_loss["identity"], "all": val_loss},
            "test_pooled": loss_pair(test, beta_shots),
            "test_mls": loss_pair(test_mls, beta_shots),
            "test_epl": loss_pair(test_epl, beta_shots),
            "test_delta_95pc_matchday_bootstrap": shots_ci,
            "test_matchdays": shots_days,
        },
        "conversion": {
            "note": "ESPN EPL only: MLS candidate sets carry no goal labels; "
                    "no zeros imputed for MLS",
            "prior": {"mean": CONVERSION_PRIOR_MEAN,
                      "strength_shots": CONVERSION_PRIOR_STRENGTH_SHOTS},
            "splits": {k: len(v) for k, v in conv.items()},
            "split_dates": {"train_end": cut1, "validation_end": cut2},
            "best": conv_best, "validation": conv_val,
            "coefficients": beta_conv.tolist() if beta_conv is not None else None,
            "temperature_selected_on_validation": conv_temp,
            "test": {
                "baseline": binomial_loss(conv_test_rows, "shots", "goals",
                                          "conv_baseline", None),
                "candidate": binomial_loss(conv_test_rows, "shots", "goals",
                                           "conv_baseline", beta_conv, conv_temp),
                "brier_raw": conv_brier_raw, "ece_raw": conv_ece_raw,
                "brier_calibrated": conv_brier_cal, "ece_calibrated": conv_ece_cal,
                "brier_baseline": conv_brier_base, "ece_baseline": conv_ece_base,
                "reliability_raw": conv_tab_raw, "reliability_calibrated": conv_tab_cal,
                "zero_shot_test_rows": len(conv_zero_shot),
                "zero_shot_scored_rate": conv_zero_shot_goal_rate,
            },
            "test_delta_95pc_matchday_bootstrap": conv_ci,
            "test_matchdays": conv_days,
        },
        "participation": {
            "note": "ESPN EPL full 40-man squad listings; MLS candidate sets "
                    "carry no start labels; no zeros imputed for MLS",
            "prior_strength_listings": START_PRIOR_STRENGTH,
            "splits": {k: len(v) for k, v in part.items()},
            "split_dates": {"train_end": pcut1, "validation_end": pcut2},
            "best": part_best, "validation": part_val,
            "coefficients": beta_part.tolist() if beta_part is not None else None,
            "temperature_selected_on_validation": part_temp,
            "test": {
                "baseline": binomial_loss(part["test"], "trials_one", "started_int",
                                          "start_baseline", None),
                "candidate": binomial_loss(part["test"], "trials_one", "started_int",
                                           "start_baseline", beta_part, part_temp),
                "brier_raw": part_brier_raw, "ece_raw": part_ece_raw,
                "brier_calibrated": part_brier_cal, "ece_calibrated": part_ece_cal,
                "brier_baseline": part_brier_base, "ece_baseline": part_ece_base,
                "reliability_raw": part_tab_raw, "reliability_calibrated": part_tab_cal,
            },
            "test_delta_95pc_matchday_bootstrap": part_ci,
            "test_matchdays": part_days,
        },
        "identity_features": False,
        "simulation_contract": {
            "shots": "corrected per-90 = asof_baseline_per90 * exp(X.beta); "
                     "Rust engine consumes via shots_per90 priors",
            "conversion": "challenger only: engine conversion stays xG-weighted "
                          "until conversion-head OOS gain is demonstrated",
            "participation": "as-of start probabilities feed exact-cardinality "
                             "lineup priors; engine keeps minutes-weighted default "
                             "until participation-head OOS gain is demonstrated",
        },
        "gate": "research_only; no promotion without larger future chronological validation",
    }
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({
        "epl_fm_linkage": report["epl_fm_linkage"],
        "shots": {"best": best, "validation": report["shots"]["validation"],
                  "test_pooled": report["shots"]["test_pooled"],
                  "test_mls": report["shots"]["test_mls"],
                  "test_epl": report["shots"]["test_epl"],
                  "ci": shots_ci},
        "conversion_test": report["conversion"]["test"],
        "participation_test": report["participation"]["test"],
    }, indent=2))


def epl_shots_only(epl: list[dict]) -> list[dict]:
    """Shots head uses only appeared rows (DNP rows carry no shot exposure)."""
    return [r for r in epl if r["minutes"] > 0]


def loss_pair(rows: list[dict], beta) -> dict:
    use = [r for r in rows if r["minutes"] > 0]
    base = poisson_loss(use, None)
    cand = poisson_loss(use, beta)
    return {"baseline": base, "candidate": cand, "delta": cand - base, "n": len(use)}


def select_temperature(rows: list[dict], trials_key: str, events_key: str,
                       base_key: str, beta: np.ndarray | None) -> float:
    scored = [(binomial_loss(rows, trials_key, events_key, base_key, beta, t), t)
              for t in TEMPERATURE_GRID]
    return min(scored)[1]


if __name__ == "__main__":
    main()
