#!/usr/bin/env python3
"""Apply the fitted pooled EPL+MLS shot-rate head to math-engine inputs.

Reads a `math_shot_goal_v1` input bundle plus the
`unified_epl_mls_forecast_heads_v1` artifact and writes an enriched copy in
which each player with a strictly pre-cutoff ESPN shot history carries
`corrected_shots_per90 = asof_baseline_per90 * exp(clip(X.beta, -3, 3))`.
Players without a pre-cutoff history keep the engine's research prior with no
correction key (missing stays missing). FM features enter only through
verified crosswalk identities joined to usable complete-everyone FM26 union
rows -- the same gate as fitting. The Rust binary consumes the corrected rate
in place of `shots_per90` and records per-player provenance; xG/conversion,
lineups and team goals are untouched (conversion/participation heads are
gated challengers, not wired in).

Refuses to overwrite existing artifacts.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from fit_pooled_epl_mls_shot_model import (  # noqa: E402
    POOLED_FEATURE_NAMES,
    ROLE_PRIOR_EPL_NEUTRAL,
)
from fit_unified_forecast_heads import (  # noqa: E402
    crosswalk_key,
    fm_feature_block,
    load_crosswalk,
    load_fm_union,
)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def espn_history(espn_path: Path, as_of: date) -> dict[tuple[str, str], dict]:
    """Causal per-player ESPN shot/minute histories strictly before as_of."""
    archive = json.loads(espn_path.read_text())
    matches = sorted(archive["matches"], key=lambda m: (m["date"], m["espn_match_id"]))
    history: dict[tuple[str, str], dict] = {}
    for match in matches:
        day = date.fromisoformat(match["date"])
        if day >= as_of:
            continue
        for p in match["players"]:
            key = (p["club"], str(p["espn_id"]))
            st = history.setdefault(key, {"shots": 0, "minutes": 0.0,
                                          "last_date": None})
            st["shots"] += int(p["shots"]) if p["shots"] is not None else 0
            st["minutes"] += float(p["minutes"])
            st["last_date"] = day
    return history


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input", type=Path, required=True,
                    help="math_shot_goal_v1 bundle from build_inputs.py")
    ap.add_argument("--heads", type=Path,
                    default=ROOT / "data/ml/unified_forecast_heads_v1.json")
    ap.add_argument("--espn", type=Path,
                    default=ROOT / "data/ml/math_engine/espn_frozen_20261011.json")
    ap.add_argument("--crosswalk", type=Path,
                    default=ROOT / "data/ml/math_engine/espn_pipeline_final_oct12/crosswalk.json")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refuse overwrite")
    data = json.loads(a.input.read_text())
    if data.get("schema") != "math_shot_goal_v1":
        ap.error("unsupported input schema")
    heads = json.loads(a.heads.read_text())
    if heads.get("schema") != "unified_epl_mls_forecast_heads_v1":
        ap.error("unsupported heads artifact")
    if list(heads["feature_names_shots"]) != list(POOLED_FEATURE_NAMES):
        ap.error("feature contract drift between fit and application")
    beta = heads["shots"]["coefficients"]
    if beta is None:
        ap.error("shots head selected identity; nothing to apply")
    as_of = date.fromisoformat(data["as_of"])

    crosswalk = load_crosswalk(a.crosswalk)
    fm_usable = load_fm_union(sorted(set(crosswalk.values())))
    history = espn_history(a.espn, as_of)
    # Invert to FM uid -> ESPN history. Ambiguous FM identities (one FM uid
    # behind several ESPN athlete ids) are left uncorrected, never guessed.
    fm_to_espn: dict[str, tuple[str, str]] = {}
    ambiguous: set[str] = set()
    espn_to_fm: dict[tuple[str, str], str] = {}
    for (xw_club, espn_id), fm_uid in crosswalk.items():
        espn_to_fm[(xw_club, espn_id)] = fm_uid
    # Rebuild ESPN-side keys with full club names for the history join.
    for espn_key in history:
        xkey = crosswalk_key(espn_key[0], espn_key[1])
        fm_uid = crosswalk.get(xkey)
        if fm_uid is None:
            continue
        if fm_uid in fm_to_espn:
            ambiguous.add(fm_uid)
        else:
            fm_to_espn[fm_uid] = espn_key

    corrected = missing = ambiguated = 0
    for player in data["players"]:
        fm_uid = str(player["id"])
        if fm_uid in ambiguous:
            ambiguated += 1
            continue
        espn_key = fm_to_espn.get(fm_uid)
        if espn_key is None or espn_key not in history:
            missing += 1
            continue
        st = history[espn_key]
        baseline = ((st["shots"] + 5.0 * ROLE_PRIOR_EPL_NEUTRAL)
                    / (st["minutes"] / 90.0 + 5.0))
        gap = math.log1p(max(0, (as_of - st["last_date"]).days)) / 5.0
        fm = fm_usable.get(fm_uid)
        block = fm_feature_block(fm, fm.get("dob", "") if fm else "", as_of)
        x = [0.0, block["age"], block["ca"], *block["attrs"], gap, block["verified"]]
        dot = max(-3.0, min(3.0, sum(v * b for v, b in zip(x, beta))))
        rate = baseline * math.exp(dot)
        if not math.isfinite(rate) or rate <= 0.0:
            ap.error(f"non-finite corrected rate for FM uid {fm_uid}")
        player["corrected_shots_per90"] = rate
        corrected += 1
    data["pooled_correction"] = {
        "schema": "unified_epl_mls_forecast_heads_v1",
        "artifact_sha256": sha256(a.heads),
        "heads": {"shots_best": heads["shots"]["best"],
                  "shots_coefficients": beta},
        "as_of": data["as_of"],
        "fm_effective": data.get("fm_effective"),
        "players_total": len(data["players"]),
        "players_corrected": corrected,
        "players_missing_espn_history": missing,
        "players_ambiguous_fm_identity": ambiguated,
        "conversion_and_participation": "gated challengers, not applied",
    }
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(data, sort_keys=True, separators=(",", ":")) + "\n")
    print(json.dumps(data["pooled_correction"], indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
