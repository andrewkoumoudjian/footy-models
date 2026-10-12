#!/usr/bin/env python3
"""Dated, provider-ID-based ESPN observed shot-rate priors for the EPL roster.

Resolves every official registration against uniquely verified ESPN athlete
IDs with the same strict reconciliation as the research crosswalk (unique
same-club exact full name, or unique same-club legal given+family token pair;
never edit distance, FM CA, or fame). Observed exposure is aggregated only
from completed matches strictly before the roster's as-of cutoff. Shot rates
are the Gamma-Poisson posterior means validated by the exposure-offset
evaluation (five full-match role-prior exposures), never raw per-appearance
averages. Unresolved identities stay unresolved; nothing is fabricated.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MATH_ENGINE = ROOT / "apps/matchforecast/math_engine"
sys.path.insert(0, str(MATH_ENGINE))

from attach_espn import (  # noqa: E402  (isolated research path import)
    club as provider_club,
    get_roster_aliases,
    name_key,
    resolve_roster_identity,
    verify_raw_archive,
)

SCHEMA = "espn_observed_shot_priors_v1"
# Same validated role intensities as crates/forecast player_shot_count ShotRole.
ROLE_PRIOR_PER90 = {"GK": 0.02, "DEF": 0.40, "MID": 0.90, "FWD": 2.20, "UNKNOWN": 0.95}
PRIOR_EXPOSURES = 5.0


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def position_group(raw: str) -> str:
    pos = (raw or "").strip().casefold()
    if any(p in pos for p in ("goalkeeper", "keeper", "goalie")):
        return "GK"
    if any(p in pos for p in ("back", "defender", "defence", "defense")):
        return "DEF"
    if any(p in pos for p in ("midfield", "wing-back")):
        return "MID"
    if any(p in pos for p in ("striker", "forward", "winger", "attacker", "attack")):
        return "FWD"
    return "UNKNOWN"


def role_of(person) -> str:
    """Production-consistent role evidence: registered position, else the
    verified-FM-attribute goalkeeper rule used by prepare_pl_player_inputs.
    Unverified identity-only players stay UNKNOWN and inherit the validated
    Unknown role prior."""
    group = position_group(person.get("historical_position", ""))
    if group == "GK":
        return group
    attrs = person.get("attributes") or {}
    def fl(value, default=0.0):
        try:
            return float(value) if value not in (None, "") else default
        except (TypeError, ValueError):
            return default
    if person.get("fm_attributes_verified") and fl(attrs.get("Reflexes")) >= 12 \
            and fl(attrs.get("Handling")) >= 11:
        return "GK"
    return group


def collect_observed(espn: dict, cutoff: str) -> tuple[dict, dict, int]:
    """Aggregate per-(club, athlete) exposure from matches strictly before cutoff."""
    by_team_name: dict = defaultdict(set)
    by_team_people: dict = defaultdict(dict)
    stats: dict = defaultdict(lambda: {"starts": 0, "appearances": 0,
                                       "minutes": 0.0, "shots": 0.0, "goals": 0.0})
    matches_by_club: Counter = Counter()
    for match in espn["matches"]:
        if match["date"] >= cutoff:
            continue
        clubs = {provider_club(match["home"]), provider_club(match["away"])}
        if len(clubs) != 2:
            raise ValueError("duplicate provider fixture clubs")
        used: set = set()
        for player in match["players"]:
            ident = str(player["espn_id"])
            team = provider_club(player["club"])
            if team not in clubs:
                raise ValueError("provider player club outside fixture")
            if (team, ident) in used:
                raise ValueError("duplicate provider person within a match")
            used.add((team, ident))
            by_team_name[(team, name_key(player["name"]))].add(ident)
            prior_name = by_team_people[team].get(ident)
            if prior_name is not None and prior_name != player["name"]:
                raise ValueError("same provider ID has conflicting names")
            by_team_people[team][ident] = player["name"]
            entry = stats[(team, ident)]
            if player["started"]:
                entry["starts"] += 1
            if player["appeared"]:
                if player["shots"] is None:
                    raise ValueError("played player has no measured shots")
                entry["appearances"] += 1
                entry["minutes"] += float(player["minutes"])
                entry["shots"] += float(player["shots"])
                entry["goals"] += float(player["goals"])
        matches_by_club.update(clubs)
    return (by_team_name, by_team_people), stats, matches_by_club


def build(espn_path: Path, squads_path: Path, output: Path) -> dict:
    espn = json.loads(espn_path.read_text())
    if espn.get("schema") != "espn_player_appearances_v1":
        raise ValueError("incompatible ESPN source schema")
    verify_raw_archive(espn)
    squads = json.loads(squads_path.read_text())
    cutoff = squads["as_of"]
    if date.fromisoformat(espn["as_of_exclusive_calendar_date"]) > date.today():
        raise ValueError("ESPN source partition is future-dated")
    roster_day = date.fromisoformat(cutoff)
    indices, stats, matches_by_club = collect_observed(espn, cutoff)
    by_team_name, by_team_people = indices
    total_registrations = sum(len(team["players"]) for team in squads["teams"].values())
    if total_registrations == 0:
        raise ValueError("empty registered squad universe")
    known = get_roster_aliases(squads)
    occupied: set = set()
    people: list[dict] = []
    reasons: Counter = Counter()
    for team, squad in squads["teams"].items():
        for person in squad["players"]:
            official = person["official_name"]
            if person.get("loaned_out") or "(Loan)" in official:
                reasons["loaned_out_not_observed"] += 1
                continue
            espn_id, method = resolve_roster_identity(
                official, person.get("fm_name"), team, by_team_name, by_team_people)
            if espn_id is None:
                reasons[method] += 1
                continue
            if (team, espn_id) in occupied:
                raise ValueError(f"ESPN athlete {espn_id} mapped to multiple roster identities for {team}")
            occupied.add((team, espn_id))
            entry = stats[(team, espn_id)]
            fixtures = matches_by_club[team]
            if fixtures < 2 or entry["starts"] > entry["appearances"] or entry["appearances"] > fixtures:
                raise ValueError(f"invalid/insufficient ESPN match exposure for {team}:{official}")
            role_prior = ROLE_PRIOR_PER90[role_of(person)]
            exposure = entry["minutes"] / 90.0
            posterior = (entry["shots"] + PRIOR_EXPOSURES * role_prior) / (exposure + PRIOR_EXPOSURES)
            group = role_of(person)
            people.append({
                "club": team,
                "official_name": official,
                "fm_uid": str(person["fm_uid"]) if person.get("fm_uid") else None,
                "espn_id": espn_id,
                "espn_name": by_team_people[team][espn_id],
                "reconciliation_method": method,
                "team_completed_matches_in_window": fixtures,
                "observed_starts": entry["starts"],
                "observed_appearances": entry["appearances"],
                "observed_minutes": round(entry["minutes"], 3),
                "observed_shots": entry["shots"],
                "observed_goals": entry["goals"],
                "role": group,
                "role_prior_per90": role_prior,
                "gamma_poisson_shots_per90": round(posterior, 5),
                "prior_exposures": PRIOR_EXPOSURES,
                "observed_window_exclusive_end": cutoff,
            })
    result = {
        "schema": SCHEMA,
        "league": "EPL",
        "roster_as_of": cutoff,
        "observed_window_exclusive_end": cutoff,
        "espn_archive": {
            "path": str(espn_path),
            "sha256": sha256(espn_path),
            "as_of_exclusive_calendar_date": espn["as_of_exclusive_calendar_date"],
            "retrieved_completed_at_utc": espn["retrieved_completed_at_utc"],
        },
        "squads": {"path": str(squads_path), "sha256": sha256(squads_path)},
        "identity_reconciliation_counts": dict(reasons),
        "people_count": len(people),
        "rate_method": (
            "Gamma-Poisson posterior (observed shots + 5 role-prior exposures) / "
            "(observed minutes/90 + 5); validated by footy-shot-rate-eval "
            "chronological source-separated evaluation"
        ),
        "people": people,
    }
    if output.exists():
        raise ValueError(f"immutable observed-prior snapshot already exists: {output}")
    output.write_text(json.dumps(result, indent=1, ensure_ascii=False) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--espn", type=Path, required=True)
    parser.add_argument("--squads", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = build(args.espn, args.squads, args.output)
    print(json.dumps({
        "output": str(args.output),
        "people": result["people_count"],
        "reconciliation": result["identity_reconciliation_counts"],
        "window_exclusive_end": result["observed_window_exclusive_end"],
    }, indent=1))


if __name__ == "__main__":
    main()