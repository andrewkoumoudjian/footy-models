#!/usr/bin/env python3
"""FM26 squad attack/defense prior indices per EPL club (verified traits only).

attack = standardized mean(Finishing, OffTheBall, Composure) over verified
attackers/midfielders; defense = standardized mean(Tackling, Marking,
Positioning) over verified defenders (plus verified goalkeepers'
Handling/Reflexes average as a separate gk column, currently informational).
Only squad-verified + union-usable rows enter (same gate as the bundle);
rescued staff rows included with provenance counted apart. Vintage recorded;
valid only for forecasts after the FM snapshot (never for historical fits
predating it -- enforced by the OOS binary's --score-from-day).
"""
from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from build_universal_bundle import role_of  # noqa: E402
from fm_staff_rescue import load_rescue_map, rescue_candidate_uids  # noqa: E402
from fit_unified_forecast_heads import load_fm_union  # noqa: E402

ATTACK_ATTRS = ("Finishing", "OffTheBall", "Composure")
DEFENSE_ATTRS = ("Tackling", "Marking", "Positioning")
GK_ATTRS = ("Handling", "Reflexes")


def mean_attrs(attributes: dict, names: tuple) -> float | None:
    values = []
    for name in names:
        try:
            value = float(attributes.get(name) or 0.0)
        except ValueError:
            value = 0.0
        if value > 0.0:
            values.append(value)
    if not values:
        return None
    return sum(values) / len(values)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--squads", type=Path, default=ROOT / "data/pl/squads_2026-10-11_full.json")
    ap.add_argument("--ledger", type=Path, default=
        ROOT / "data/identity/reep/epl_canonical_clean_v11_evidence.json")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refuse overwrite")
    squads = json.loads(a.squads.read_text())["teams"]
    wanted: set[str] = set()
    for players in squads.values():
        for p in players["players"]:
            if p.get("fm_uid") and not p.get("loaned_out"):
                wanted.add(str(p["fm_uid"]))
    candidates = rescue_candidate_uids(a.ledger)
    fm_usable = load_fm_union(sorted(wanted | candidates))
    rescue = load_rescue_map(a.ledger, fm_usable)
    raw: dict[str, dict] = {}
    counts: dict[str, int] = {}
    for club in sorted(squads):
        attack, defense, gk = [], [], []
        for entry in squads[club]["players"]:
            if entry.get("loaned_out"):
                continue
            fm_uid = entry.get("fm_uid")
            key = (club, entry["official_name"])
            if fm_uid is None and key in rescue:
                fm_uid = rescue[key]["fm_uid"]
            if not entry.get("fm_attributes_verified") and key not in rescue:
                continue
            row = fm_usable.get(str(fm_uid))
            if row is None:
                continue
            flat = {name: row.get(f"attr_{name}") for name in
                    set(ATTACK_ATTRS + DEFENSE_ATTRS + GK_ATTRS)}
            role = role_of(entry.get("historical_position"))
            if role in ("attack", "midfield"):
                value = mean_attrs(flat, ATTACK_ATTRS)
                if value is not None:
                    attack.append(value)
            if role == "defender":
                value = mean_attrs(flat, DEFENSE_ATTRS)
                if value is not None:
                    defense.append(value)
            if role == "goalkeeper":
                value = mean_attrs(flat, GK_ATTRS)
                if value is not None:
                    gk.append(value)
        raw[club] = {"attack": attack, "defense": defense, "gk": gk}
        counts[club] = {k: len(v) for k, v in raw[club].items()}
    attack_means = {c: (sum(v["attack"]) / len(v["attack"]) if v["attack"] else None)
                    for c, v in raw.items()}
    defense_means = {c: (sum(v["defense"]) / len(v["defense"]) if v["defense"] else None)
                     for c, v in raw.items()}
    valid_a = [v for v in attack_means.values() if v is not None]
    valid_d = [v for v in defense_means.values() if v is not None]
    priors = {}
    for club in sorted(squads):
        priors[club] = {
            "attack": ((attack_means[club] - statistics.mean(valid_a))
                       / statistics.pstdev(valid_a)) if attack_means[club] is not None else 0.0,
            "defense": ((defense_means[club] - statistics.mean(valid_d))
                        / statistics.pstdev(valid_d)) if defense_means[club] is not None else 0.0,
        }
    a.output.write_text(json.dumps({
        "schema": "fm26_team_prior_indices_v1",
        "vintage": "FM26 v26.2 March 2026; valid only for forecasts after 2026-03-01",
        "definition": {
            "attack": "z-scored mean(Finishing, OffTheBall, Composure) over verified attackers/mids",
            "defense": "z-scored mean(Tackling, Marking, Positioning) over verified defenders",
        },
        "verified_counts": counts,
        "priors": priors,
    }, indent=2, sort_keys=True) + "\n")
    print(json.dumps({c: {k: round(v, 2) for k, v in p.items()}
                      for c, p in priors.items()}, indent=2))


if __name__ == "__main__":
    main()
