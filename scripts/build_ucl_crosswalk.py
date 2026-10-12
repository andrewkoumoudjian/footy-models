#!/usr/bin/env python3
"""UCL ESPN<->FM crosswalk with the EPL path's verified reconciliation rules.

Resolves every UCL registration against ESPN athlete IDs observed in frozen
pre-cutoff UCL summaries using attach_espn.resolve_roster_identity (unique
same-club exact full name, or unique same-club legal given+family token
pair; never edit distance, FM CA, or fame). Only the two verified statuses
enter the crosswalk the bundle consumes; everything else is recorded as
unresolved with its reason, never guessed.

Usage:
    python3 scripts/build_ucl_crosswalk.py \
        --espn data/ucl/espn_ucl_md1_20261012.json \
               data/ucl/espn_ucl_202526_20261012.json \
               data/ucl/espn_ucl_202425_20261012.json \
        --squads data/ucl/squads_2026-10-12_full.json \
        --output data/ucl/crosswalk_ucl_20261012.json

Refuses to overwrite. All summary matches must strictly precede the squad
as-of date.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MATH_ENGINE = ROOT / "apps/matchforecast/math_engine"
sys.path.insert(0, str(MATH_ENGINE))
sys.path.insert(0, str(ROOT / "scripts"))

from attach_espn import (  # noqa: E402
    club as provider_club,
    get_roster_aliases,
    name_key,
    resolve_roster_identity,
)

VERIFIED = frozenset({
    "exact_full_name_and_club",
    "unique_legal_given_family_token_pair_and_club",
})


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--espn", type=Path, nargs="+", required=True)
    ap.add_argument("--squads", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refusing to overwrite immutable crosswalk")
    squads = json.loads(a.squads.read_text())
    cutoff = squads["as_of"]
    date.fromisoformat(cutoff)
    by_team_name: dict = defaultdict(set)
    by_team_people: dict = defaultdict(dict)
    apps: dict = defaultdict(lambda: {"appearances": 0, "starts": 0})
    for espn_path in a.espn:
        espn = json.loads(espn_path.read_text())
        if espn.get("schema") != "espn_player_appearances_v1":
            raise ValueError(f"incompatible ESPN source schema: {espn_path}")
        for match in espn["matches"]:
            if match["date"] >= cutoff:
                raise ValueError(f"summary is not before cutoff: {match}")
            for player in match["players"]:
                ident = str(player["espn_id"])
                team = provider_club(player["club"])
                by_team_name[(team, name_key(player["name"]))].add(ident)
                prior = by_team_people[team].get(ident)
                if prior is not None and prior != player["name"]:
                    raise ValueError("same provider ID has conflicting names")
                by_team_people[team][ident] = player["name"]
                if player.get("appeared"):
                    apps[(team, ident)]["appearances"] += 1
                    apps[(team, ident)]["starts"] += 1 if player.get("started") else 0
    known = get_roster_aliases(squads)
    del known
    people: list[dict] = []
    reasons: Counter = Counter()
    occupied: set = set()
    for team in sorted(squads["teams"]):
        for person in squads["teams"][team]["players"]:
            official = person["official_name"]
            if person.get("loaned_out"):
                reasons["loaned_out_not_observed"] += 1
                continue
            espn_id, method = resolve_roster_identity(
                official, person.get("fm_name"), team, by_team_name, by_team_people)
            if espn_id is None:
                reasons[method] += 1
                people.append({"club": team, "fm_uid": str(person["fm_uid"])
                               if person.get("fm_uid") else None,
                               "status": "unresolved", "reason": method})
                continue
            if (team, espn_id) in occupied:
                raise ValueError(f"ESPN athlete {espn_id} mapped twice for {team}")
            occupied.add((team, espn_id))
            entry = {"club": team, "espn_id": espn_id,
                     "fm_uid": str(person["fm_uid"]) if person.get("fm_uid") else None,
                     "status": method if method in VERIFIED else "unresolved",
                     "n_appearances": apps[(team, espn_id)]["appearances"],
                     "n_starts": apps[(team, espn_id)]["starts"]}
            if method not in VERIFIED:
                entry["reason"] = method
                reasons[method] += 1
            else:
                reasons[method] += 1
            people.append(entry)
    verified = [p for p in people if p.get("status") in VERIFIED and p.get("fm_uid")]
    keys = [(p["club"], p["espn_id"]) for p in verified]
    if len(set(keys)) != len(keys):
        raise ValueError("duplicate verified crosswalk identity")
    a.output.write_text(json.dumps({
        "schema": "ucl_espn_fm_crosswalk_v1",
        "competition": "UEFA Champions League 2026-27 league phase",
        "as_of": cutoff,
        "reconciliation_rule": "attach_espn.resolve_roster_identity; verified = "
                               "exact_full_name_and_club | "
                               "unique_legal_given_family_token_pair_and_club",
        "espn_sources": [str(p) for p in a.espn],
        "identity_reconciliation": dict(reasons),
        "verified_identities": len(verified),
        "people": people,
    }, indent=1, ensure_ascii=False) + "\n")
    print(f"UCL crosswalk: {len(verified)} verified ESPN<->FM identities "
          f"({dict(reasons)}) -> {a.output}")


if __name__ == "__main__":
    main()
