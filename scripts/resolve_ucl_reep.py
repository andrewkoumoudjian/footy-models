#!/usr/bin/env python3
"""UCL registrations -> Reep-first typed FM identity discovery (canonical v3).

Same matching function as the EPL path (typed Reep bridges, native FM26
union, independent Transfermarkt DOB corroboration, staff-db rescue policy
downstream) applied to the 36-club UCL league-phase roster snapshot. No new
identity evidence, no looser gates: the only difference from
match_premier_league_fm26.py is the registration source (frozen ESPN rosters,
see scripts/freeze_ucl_rosters.py) and the 36-club full-league invariant.

Usage:
    python3 scripts/resolve_ucl_reep.py --rosters data/ucl/rosters_2026-10-12.json \
        --output data/ucl/squads_2026-10-12_canonical.json

Refuses to overwrite a differing snapshot (same identical-rerun rule as EPL).
"""
from __future__ import annotations

import argparse
import json
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from recover_epl_remaining_identities import resolve_official_roster  # noqa: E402

EXPECTED_CLUBS = 36
FM_MEN = Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv")
FM_COMPLETE = Path("/home/drewclaw/fm26-data/fm26_complete_v6.csv")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--rosters", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--reep-active", type=Path,
                    default=ROOT / "data/identity/reep/active.json")
    ap.add_argument("--tm-players", type=Path, default=ROOT / "data/tm_players.csv")
    ap.add_argument("--sortitoutsi-roster", type=Path,
                    default=Path("/home/drewclaw/fm26-data/sortitoutsi_fm26_roster.csv"))
    ap.add_argument("--sortitoutsi-map", type=Path,
                    default=Path("/home/drewclaw/fm26-data/sortitoutsi_uid_map.csv"))
    a = ap.parse_args()
    frozen = json.loads(a.rosters.read_text())
    cutoff = date.fromisoformat(frozen["as_of"])
    official = {
        "as_of": str(cutoff),
        "registration_source": frozen["registration_source"],
        "registration_source_published": frozen["as_of"],
        "registration_source_limitation": frozen.get("registration_source_limitation", ""),
        "roster_scope": "full_league",
        "expected_club_count": EXPECTED_CLUBS,
        "fm_source": "FM26 complete-everyone union (complete v6 primary + men provenance)",
        "teams": {},
    }
    for club in sorted(frozen["teams"]):
        block = frozen["teams"][club]
        entries = [{
            "official_name": p["official_name"],
            "registration": p["registration"],
            "registered_club": club,
            "membership_observed_as_of": str(cutoff),
            "loaned_out": bool(p.get("loaned_out")),
        } for p in block["players"]]
        official["teams"][club] = {
            "official_club": club,
            "registered_count": len(entries),
            "players": entries,
            "matched_count": 0,
            "unmatched": [p["official_name"] for p in entries],
        }
    output, ledger = resolve_official_roster(
        official, a.reep_active, FM_MEN, a.tm_players,
        a.sortitoutsi_roster, a.sortitoutsi_map, FM_COMPLETE,
        expected_clubs=EXPECTED_CLUBS)
    review_path = a.output.with_name(a.output.stem + "_identity_evidence.json")
    serial = json.dumps(output, indent=2, ensure_ascii=False) + "\n"
    review_serial = json.dumps({
        "as_of": str(cutoff),
        "resolver": "canonical_reep_first_v3",
        "competition": "UEFA Champions League 2026-27 league phase",
        "source_policy": "complete-everyone FM union primary, Reep and typed IDs "
                         "first; no CA/kind/gender pre-filter, no name-only "
                         "auto-ranking (same gates as the EPL path)",
        "players": ledger,
    }, indent=2, ensure_ascii=False) + "\n"
    if a.output.exists() or review_path.exists():
        if (a.output.is_file() and review_path.is_file()
                and a.output.read_text(encoding="utf-8") == serial
                and review_path.read_text(encoding="utf-8") == review_serial):
            print("UNCHANGED identical canonical UCL roster and identity audit",
                  a.output, review_path)
            return
        raise ValueError("prior canonical UCL snapshot differs; choose a new versioned output name")
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(serial, encoding="utf-8")
    review_path.write_text(review_serial, encoding="utf-8")
    attached = sum(1 for p in ledger if (p.get("accepted_candidate") or {}).get("fm_uid"))
    print(f"UCL identity: {attached}/{len(ledger)} registrations carry an FM candidate UID")
    for team, rows in output["teams"].items():
        print(f"{team:22s} {rows['matched_count']}/{rows['registered_count']} FM candidate IDs; "
              f"unresolved {len(rows['unmatched'])}")
    print("WROTE", a.output, review_path)


if __name__ == "__main__":
    main()
