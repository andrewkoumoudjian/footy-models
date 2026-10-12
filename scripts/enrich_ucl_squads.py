#!/usr/bin/env python3
"""Enrich canonical UCL squads with bundle-level position/minutes fields.

The canonical resolver output carries identity + FM traits but no playing
role or minutes history. The EPL full squad file adds historical_position /
historical_minutes_2526 / historical_apps_2526 from the registration
pipeline; for UCL the only pre-cutoff role source is the frozen ESPN roster
position (Goalkeeper/Defender/Midfielder/Forward), mapped to the bundle
vocabulary. Minutes history is NOT available per player (missing stays
missing: None, which the bundle treats as a 0.0 experience weight with
cold-start provenance, never as fabricated form).

Refuses to overwrite. Input squads file is never mutated.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ESPN_TO_ROLE = {
    "Goalkeeper": "goalkeeper",
    "Defender": "defender",
    "Midfielder": "midfield",
    "Forward": "attack",
}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--squads", type=Path, required=True)
    ap.add_argument("--rosters", type=Path, required=True,
                    help="frozen ESPN rosters (position source)")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refusing to overwrite immutable squad snapshot")
    squads = json.loads(a.squads.read_text())
    rosters = json.loads(a.rosters.read_text())
    pos_of: dict[tuple[str, str], str] = {}
    for club, block in rosters["teams"].items():
        for p in block["players"]:
            pos_of[(club, p["official_name"])] = p.get("espn_position", "")
    counts = {"mapped": 0, "unknown": 0}
    out = json.loads(json.dumps(squads))
    for club, block in out["teams"].items():
        for p in block["players"]:
            role = ESPN_TO_ROLE.get(pos_of.get((club, p["official_name"]), ""), "unknown")
            p["historical_position"] = role
            p["historical_position_source"] = (
                "espn_roster_position_frozen_as_of" if role != "unknown"
                else "unknown_role_neutral")
            p["historical_minutes_2526"] = None
            p["historical_apps_2526"] = None
            counts["mapped" if role != "unknown" else "unknown"] += 1
    out["position_enrichment"] = {
        "source": "frozen ESPN roster positions only; no performance-derived roles",
        "counts": counts,
    }
    a.output.write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
    print(f"UCL enriched squads: {counts} -> {a.output}")


if __name__ == "__main__":
    main()
