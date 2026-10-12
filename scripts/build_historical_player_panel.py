#!/usr/bin/env python3
"""Build a source-neutral, date-causal lineup and event observation panel.

Currently supported: locally cached StatsBomb EPL 2015/16 lineup/event labels
and complete dated SofaScore player-match sheets. These sources do NOT share
player identity or a calibrated competition distribution. FBref name-only
exports are deliberately not joined without persistent provider identities.

The fixture's observed roster/events are labels, never candidate inputs.
Candidate features are computed from *strictly older* fully covered fixtures;
first sightings and missing lineup sheets cannot create negative labels.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = ROOT / "data"


@dataclass(frozen=True)
class Observation:
    player_id: str
    name: str
    started: bool
    minutes: float
    shots: int | None
    goals: int | None
    assists: int | None
    position: str


@dataclass(frozen=True)
class Fixture:
    source: str
    league: str
    match_id: str
    day: str
    home: str
    away: str
    teams: dict[str, dict[str, Observation]]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_count(value, label: str) -> int | None:
    if value is None or value == "":
        return None
    number = float(value)
    if not number.is_integer() or number < 0 or not number < float("inf"):
        raise ValueError(f"invalid {label}: {value!r}")
    return int(number)


def validate_fixture(fixture: Fixture) -> None:
    date.fromisoformat(fixture.day)
    if fixture.home == fixture.away or set(fixture.teams) != {fixture.home, fixture.away}:
        raise ValueError(f"bad fixture teams: {fixture.match_id}")
    for team, players in fixture.teams.items():
        if sum(p.started for p in players.values()) != 11:
            raise ValueError(f"{fixture.source}/{fixture.match_id}/{team}: no confirmed XI")
        for player in players.values():
            if not (0 <= player.minutes <= 130) or player.started and player.minutes <= 0:
                raise ValueError(f"{fixture.match_id}: invalid minutes for {player.player_id}")
            if player.shots is not None and player.shots > 0 and player.minutes <= 0:
                raise ValueError(f"{fixture.match_id}: positive shots with zero minutes")
    if set(fixture.teams[fixture.home]) & set(fixture.teams[fixture.away]):
        raise ValueError(f"{fixture.match_id}: player appears on both teams")


def load_statsbomb(directory: Path) -> tuple[list[Fixture], dict[str, str]]:
    fixtures_path = directory / "fixtures.json"
    fixtures = json.loads(fixtures_path.read_text())["fixtures"]
    hashes = {str(fixtures_path): sha256(fixtures_path)}
    output = []
    for info in fixtures:
        file = directory / "labels" / f"{info['match_id']}.json"
        data = json.loads(file.read_text())
        hashes[str(file)] = sha256(file)
        lineup_file = directory.parent / "lineups" / f"{info['match_id']}.json"
        raw_lineups = json.loads(lineup_file.read_text())
        hashes[str(lineup_file)] = sha256(lineup_file)
        positions = {}
        for team_data in raw_lineups:
            club = team_data["team_name"]
            for player in team_data.get("lineup", []):
                prior_positions = player.get("positions") or []
                starts = [row for row in prior_positions if row.get("start_reason") == "Starting XI"]
                candidates = starts or prior_positions
                position = str(candidates[0].get("position", "")) if candidates else ""
                key = (club, int(player["player_id"]))
                if key in positions:
                    raise ValueError(f"duplicate StatsBomb position record: {key}")
                positions[key] = position
        if data.get("date") != info["date"]:
            raise ValueError(f"StatsBomb match-date mismatch: {info['match_id']}")
        teams = {info["home"]: {}, info["away"]: {}}
        for player in data["players"]:
            team = player["team"]
            if team not in teams:
                raise ValueError(f"unknown lineup club: {team}")
            pid = f"statsbomb:{int(player['player_id'])}"
            if pid in teams[team]:
                raise ValueError(f"duplicate lineup player: {pid}")
            teams[team][pid] = Observation(
                pid, player.get("name", ""), bool(player["started"]),
                float(player["minutes"]), require_count(player.get("shots"), "shots"),
                require_count(player.get("goals"), "goals"),
                require_count(player.get("assists"), "assists"),
                positions.get((team, int(player["player_id"])), ""),
            )
        fixture = Fixture("statsbomb", "EPL-2015-16", str(info["match_id"]),
                          info["date"], info["home"], info["away"], teams)
        validate_fixture(fixture)
        output.append(fixture)
    return output, hashes


def load_sofascore(directory: Path) -> tuple[list[Fixture], dict[str, str], Counter]:
    matches_path = directory / "asof_team_stats.csv"
    with matches_path.open(newline="", encoding="utf-8-sig") as handle:
        lookup = {row["match_id"]: row for row in csv.DictReader(handle)}
    hashes = {str(matches_path): sha256(matches_path)}
    exclusions = Counter()
    output = []
    for file in sorted((directory / "player_matches").glob("*.json")):
        mid = file.stem
        if mid not in lookup:
            exclusions["missing_fixture_metadata"] += 1
            continue
        data = json.loads(file.read_text())
        if data.get("confirmed") is not True:
            exclusions["unconfirmed_matchsheet"] += 1
            continue
        match = lookup[mid]
        teams = {}
        try:
            for side in ("home", "away"):
                name = match[side]
                players = (data.get(side) or {}).get("players")
                if not isinstance(players, list) or not players:
                    raise ValueError("incomplete matchsheet")
                entries = {}
                for item in players:
                    person = item.get("player") or {}
                    identifier = person.get("id")
                    if identifier is None:
                        raise ValueError("missing provider player ID")
                    pid = f"sofascore:{int(identifier)}"
                    stats = item.get("statistics") or {}
                    minutes = float(stats.get("minutesPlayed") or 0)
                    if pid in entries:
                        raise ValueError("duplicate player ID")
                    entries[pid] = Observation(
                        pid, person.get("name", ""), not bool(item.get("substitute", True)),
                        minutes, require_count(stats.get("totalShots"), "shots"),
                        require_count(stats.get("goals"), "goals"),
                        require_count(stats.get("goalAssist"), "assists"),
                        person.get("position", "") or "",
                    )
                teams[name] = entries
            fixture = Fixture("sofascore", "MLS-mixed-2026", mid, match["date"],
                              match["home"], match["away"], teams)
            validate_fixture(fixture)
        except (KeyError, ValueError, TypeError) as exc:
            exclusions[f"incomplete_or_invalid_matchsheet:{type(exc).__name__}"] += 1
            continue
        hashes[str(file)] = sha256(file)
        output.append(fixture)
    return output, hashes, exclusions


TRAINING_FIELDS = [
    "source", "league", "date", "match_id", "team", "player_id", "position",
    "first_seen_date", "latest_history_date", "club_fixtures", "prior_listings",
    "prior_appearances", "prior_starts", "prior_minutes", "prior_start_minutes",
    "days_since_last_appearance", "days_since_last_listed", "appeared", "started",
    "minutes", "shots", "goals", "assists", "shot_observed", "event_exposure_minutes",
]
LABEL_FIELDS = ["source", "league", "date", "match_id", "team", "player_id",
                "name", "position", "appeared", "started", "minutes", "shots",
                "goals", "assists", "shot_observed"]


def build_panel(fixtures: list[Fixture], *, max_age_days: dict[str, int]) -> tuple[list[dict], list[dict], dict]:
    # Source namespaces are disjoint. No same-name player join and no source pooling.
    fixtures = sorted(fixtures, key=lambda f: (f.day, f.source, f.match_id))
    seen_matches = set()
    for fixture in fixtures:
        key = (fixture.source, fixture.match_id)
        if key in seen_matches:
            raise ValueError(f"duplicate fixture {key}")
        seen_matches.add(key)
        validate_fixture(fixture)
    history: dict[tuple[str, str, str], dict] = {}
    fixtures_seen = defaultdict(int)
    current_club: dict[str, str] = {}
    rows, labels = [], []
    max_feature_day = ""
    # Compute every matchday from the exact same pre-match state. A match
    # played earlier on that date must not become a later fixture's feature.
    for day in sorted({f.day for f in fixtures}):
        today = [f for f in fixtures if f.day == day]
        for fixture in today:
            for team in (fixture.home, fixture.away):
                key = (fixture.source, fixture.league, team)
                actual = fixture.teams[team]
                for person in actual.values():
                    labels.append({
                        "source": fixture.source, "league": fixture.league,
                        "date": day, "match_id": fixture.match_id, "team": team,
                        "player_id": person.player_id, "name": person.name,
                        "position": person.position, "appeared": int(person.minutes > 0),
                        "started": int(person.started), "minutes": person.minutes,
                        "shots": person.shots if person.shots is not None else "",
                        "goals": person.goals if person.goals is not None else "",
                        "assists": person.assists if person.assists is not None else "",
                        "shot_observed": int(person.shots is not None),
                    })
                for (source, league, club_pid), state in history.items():
                    if (source, league) != (fixture.source, fixture.league):
                        continue
                    club, pid = club_pid
                    if club != team or current_club.get(pid) != team:
                        continue
                    age = (date.fromisoformat(day) - date.fromisoformat(state["last_listed"])).days
                    if age < 0 or age > max_age_days[fixture.source]:
                        continue
                    history_date = state["last_listed"]
                    if history_date >= day:
                        raise ValueError("future/same-day player observation entered a feature")
                    outcome = actual.get(pid)
                    appeared = int(bool(outcome and outcome.minutes > 0))
                    started = int(bool(outcome and outcome.started))
                    first_index = state["first_fixture_index"]
                    previous_fixtures = fixtures_seen[key] - first_index + 1
                    if previous_fixtures < 1:
                        raise ValueError("invalid club fixture exposure")
                    appearance_anchor = state["last_appearance"] or state["first_listed"]
                    rows.append({
                        "source": source, "league": league, "date": day,
                        "match_id": fixture.match_id, "team": team, "player_id": pid,
                        "position": state["position"],
                        "first_seen_date": state["first_listed"],
                        "latest_history_date": history_date,
                        "club_fixtures": previous_fixtures,
                        "prior_listings": state["listings"],
                        "prior_appearances": state["appearances"],
                        "prior_starts": state["starts"],
                        "prior_minutes": round(state["minutes"], 3),
                        "prior_start_minutes": round(state["start_minutes"], 3),
                        "days_since_last_appearance": (date.fromisoformat(day) - date.fromisoformat(appearance_anchor)).days,
                        "days_since_last_listed": age,
                        "appeared": appeared, "started": started,
                        "minutes": round(outcome.minutes if outcome else 0.0, 3),
                        "shots": outcome.shots if outcome and outcome.shots is not None else "",
                        "goals": outcome.goals if outcome and outcome.goals is not None else "",
                        "assists": outcome.assists if outcome and outcome.assists is not None else "",
                        "shot_observed": int(bool(outcome and outcome.shots is not None)),
                        "event_exposure_minutes": round(outcome.minutes, 3) if outcome and outcome.shots is not None else "",
                    })
                    max_feature_day = max(max_feature_day, history_date)
        # Only after all labels for the same calendar date are emitted can
        # today's player listings and appearances update the state.
        for fixture in today:
            for team in (fixture.home, fixture.away):
                key = (fixture.source, fixture.league, team)
                fixtures_seen[key] += 1
                for person in fixture.teams[team].values():
                    current_club[person.player_id] = team
                    history_key = (*key[:2], (team, person.player_id))
                    state = history.get(history_key)
                    if state is None:
                        state = dict(first_fixture_index=fixtures_seen[key], first_listed=day,
                                     last_listed=day, last_appearance=None, listings=0,
                                     appearances=0, starts=0, minutes=0.0, start_minutes=0.0,
                                     position=person.position)
                        history[history_key] = state
                    state["listings"] += 1
                    state["last_listed"] = day
                    if person.position:
                        # The target-day role updates state only after every
                        # fixture on that calendar date has been scored.
                        state["position"] = person.position
                    if person.minutes > 0:
                        state["appearances"] += 1
                        state["minutes"] += person.minutes
                        state["last_appearance"] = day
                    if person.started:
                        state["starts"] += 1
                        state["start_minutes"] += person.minutes
    report = {
        "schema_version": 1,
        "fixtures_by_source": dict(sorted(Counter(f.source for f in fixtures).items())),
        "training_candidates_by_source": dict(sorted(Counter(row["source"] for row in rows).items())),
        "observed_player_matches_by_source": dict(sorted(Counter(row["source"] for row in labels).items())),
        "first_training_date_by_source": {s: min(r["date"] for r in rows if r["source"] == s)
                                          for s in {r["source"] for r in rows}},
        "last_training_date_by_source": {s: max(r["date"] for r in rows if r["source"] == s)
                                         for s in {r["source"] for r in rows}},
        "max_history_feature_day": max_feature_day,
        "chronological_invariant": all(r["latest_history_date"] < r["date"] for r in rows),
        "identity": "provider qualified stable IDs; no cross-source name joins",
        "coverage": "only fixtures with complete confirmed starters; cold-start players excluded from participation candidates",
        "training_release_status": "research_only",
    }
    return rows, labels, report


def write_csv(path: Path, fields: list[str], rows: list[dict], delimiter: str = ",") -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fields, delimiter=delimiter, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--statsbomb", type=Path, default=SOURCE_ROOT / "statsbomb/pl_hist_elapsed_v2")
    parser.add_argument("--sofascore", type=Path, default=SOURCE_ROOT / "sofascore_cfm")
    parser.add_argument("--out", type=Path, default=SOURCE_ROOT / "forecast/player_panel_v1")
    parser.add_argument("--only", choices=["statsbomb", "sofascore"])
    args = parser.parse_args()
    fixtures, hashes, exclusions = [], {}, Counter()
    if args.only != "sofascore":
        records, source_hashes = load_statsbomb(args.statsbomb)
        fixtures.extend(records)
        hashes.update(source_hashes)
    if args.only != "statsbomb":
        records, source_hashes, failures = load_sofascore(args.sofascore)
        fixtures.extend(records)
        hashes.update(source_hashes)
        exclusions.update(failures)
    rows, labels, report = build_panel(fixtures, max_age_days={"statsbomb": 120, "sofascore": 45})
    args.out.mkdir(parents=True, exist_ok=True)
    write_csv(args.out / "player_matches.csv", LABEL_FIELDS, labels)
    write_csv(args.out / "participation.tsv", TRAINING_FIELDS, rows, delimiter="\t")
    report["excluded_source_matches"] = dict(exclusions)
    report["source_sha256"] = hashes
    report["files"] = {
        "player_matches": sha256(args.out / "player_matches.csv"),
        "participation": sha256(args.out / "participation.tsv"),
    }
    (args.out / "manifest.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "source_sha256"}, indent=2))


if __name__ == "__main__":
    main()
