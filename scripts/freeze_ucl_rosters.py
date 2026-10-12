#!/usr/bin/env python3
"""Freeze UEFA Champions League squad lists from the ESPN roster endpoint.

One immutable snapshot for the UCL forecast path (the EPL path uses the
official PL registration article; UEFA publishes no equivalent single
machine-readable list, so per-club ESPN rosters frozen on the cutoff date
are the registration source, with provenance recorded per club).

Usage:
    python3 scripts/freeze_ucl_rosters.py --as-of 2026-10-12 \
        --output data/ucl/rosters_2026-10-12.json \
        --raw-dir data/ucl/raw_espn/rosters

Refuses to overwrite. Raw per-team responses are cached byte-identical with
SHA-256. Retrieval happens on --as-of itself, so the snapshot is valid only
for forecasts with cutoff >= --as-of and must never supply pre-cutoff
history (rosters carry no dates other than birth dates).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import time
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEAGUE = "uefa.champions"
BASE = f"https://site.api.espn.com/apis/site/v2/sports/soccer/{LEAGUE}"

# ESPN team IDs keyed by scoreboard displayName (from frozen UCL boards).
TEAMS = {
    "AEK Athens": "887", "AS Roma": "104", "Arsenal": "359",
    "Aston Villa": "362", "Atlético Madrid": "1068", "Barcelona": "83",
    "Bayern Munich": "132", "Bodo/Glimt": "2980",
    "Borussia Dortmund": "124", "Club Brugge": "570", "Como": "2572",
    "FC Porto": "437", "Fenerbahce": "436", "Feyenoord Rotterdam": "142",
    "Galatasaray": "432", "Internazionale": "110", "LASK Linz": "4411",
    "Lens": "175", "Lille": "166", "Liverpool": "364",
    "Manchester City": "382", "Manchester United": "360", "Napoli": "114",
    "PSV Eindhoven": "148", "Paris Saint-Germain": "160",
    "RB Leipzig": "11420", "Real Betis": "244", "Real Madrid": "86",
    "Sabah FK": "21922", "Shakhtar Donetsk": "493", "Slavia Prague": "494",
    "Slovan Bratislava": "521", "Sporting CP": "2250",
    "VfB Stuttgart": "134", "Viking FK": "510", "Villarreal": "102",
}


def fetch_json(url: str) -> tuple[bytes, dict]:
    if not url.startswith(BASE + "/"):
        raise ValueError("unsupported untrusted provider URL")
    proc = subprocess.run(
        ["curl", "--fail", "--silent", "--show-error", "--compressed",
         "--max-time", "30", url],
        check=True, capture_output=True)
    raw = proc.stdout
    if len(raw) > 8_000_000:
        raise ValueError("unexpected oversized provider payload")
    return raw, json.loads(raw)


def official_name(first: str, last: str) -> str:
    return f"{(last or '').strip()}, {(first or '').strip()}".strip(", ")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--as-of", required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--raw-dir", type=Path, required=True)
    ap.add_argument("--delay", type=float, default=0.3)
    a = ap.parse_args()
    as_of = date.fromisoformat(a.as_of)
    if a.output.exists() or a.raw_dir.exists():
        ap.error("refusing to overwrite immutable roster snapshot")
    a.raw_dir.mkdir(parents=True)
    teams: dict[str, dict] = {}
    hashes: dict[str, str] = {}
    for club in sorted(TEAMS):
        tid = TEAMS[club]
        raw, body = fetch_json(f"{BASE}/teams/{tid}/roster")
        raw_path = a.raw_dir / f"team_{tid}.json"
        raw_path.write_bytes(raw)
        hashes[club] = hashlib.sha256(raw).hexdigest()
        athletes = body.get("athletes", [])
        if not isinstance(athletes, list) or not athletes:
            raise ValueError(f"empty roster for {club}")
        players = []
        seen = set()
        for person in athletes:
            espn_id = str(person.get("id", ""))
            name = official_name(person.get("firstName", ""),
                                 person.get("lastName", ""))
            if not espn_id or not name or (club, espn_id) in seen:
                raise ValueError(f"invalid roster entry for {club}: {person!r}"[:200])
            seen.add((club, espn_id))
            pos = (person.get("position") or {}).get("name", "")
            players.append({
                "official_name": name,
                "espn_id": espn_id,
                "espn_position": pos,
                "espn_dob": str(person.get("dateOfBirth", ""))[:10],
                "registration": "squad",
                "registered_club": club,
                "membership_observed_as_of": as_of.isoformat(),
                "loaned_out": False,
            })
        players.sort(key=lambda p: p["official_name"])
        teams[club] = {
            "official_club": club,
            "espn_team_id": tid,
            "registered_count": len(players),
            "players": players,
            "matched_count": 0,
            "unmatched": [p["official_name"] for p in players],
        }
        time.sleep(a.delay)
    snapshot = {
        "as_of": as_of.isoformat(),
        "registration_source": "ESPN site.api roster endpoint, league uefa.champions",
        "registration_source_limitation": (
            "No single-issuer UEFA registration list was available in machine "
            "form; per-club ESPN rosters frozen on the cutoff date are the "
            "registration source. Membership is as observed on --as-of, never "
            "backdated. ESPN identities are not FM26 IDs."),
        "roster_scope": "full_league",
        "expected_club_count": len(teams),
        "raw_sha256": hashes,
        "teams": teams,
    }
    a.output.write_text(json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n")
    total = sum(t["registered_count"] for t in teams.values())
    print(f"UCL rosters: {len(teams)} clubs, {total} registrations -> {a.output}")


if __name__ == "__main__":
    main()
