#!/usr/bin/env python3
"""Build strictly causal historical Premier League inputs for the Rust match engine.

This produces an *observed-stat-only* feature contract for
`simulate_second_resolved_dynamic_mental_match`: no FM26 attributes, no current
squads, no present-day information. Every feature for a fixture is computed only
from StatsBomb event-covered matches with `date < fixture_date`; same-day
fixtures never contribute.

Outputs (under `data/statsbomb/pl_hist/` by default, all local/untracked):

- `fixtures.json`      covered fixtures, causal coverage, evaluability flags
- `team_rows.csv`      one row per 2015/16 match: scores always, shots/SOT/xG only
                       when the match is event-covered (NaN otherwise)
- `players/<id>.csv`   per-fixture candidate pool + causal per-90 rates
- `labels/<id>.json`   actual post-match labels for scoring (never a feature)
- `build_report.json`  counts and the no-lookahead audit

Definitions used in labels (documented, not invented):
- shots: StatsBomb `Shot` events
- SOT: shot outcome in {Goal, Saved, Saved To Post}
- goals: `Shot` outcome Goal; own goals are counted for the opposing team's
  score but never attributed to an attacker
- assists: `Pass.goal_assist == true` (a provider-registered primary assist)
- xG: sum of `shot.statsbomb_xg`
- xA: sum of the xG of shots whose `shot.key_pass_id` points at the player's pass
- progressive carry: carry gaining >= 10 yards toward goal (StatsBomb x
  increases toward goal) or ending inside the penalty area (x >= 102)
- minutes: summed lineup position intervals, capped at 90
- event minute index: `min(89, max(0, minute - 1))` so minute 15 falls in the
  first 0-15 window and stoppage time is conserved in the last bin
"""
from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DATA = ROOT / "data/statsbomb"
DEFAULT_OUT = ROOT / "data/statsbomb/pl_hist"

ON_TARGET = {"Goal", "Saved", "Saved To Post"}
RED_CARDS = {"Red Card", "Second Yellow"}


def clock_seconds(text):
    if not text:
        return None
    parts = text.split(":")
    if len(parts) != 2:
        return None
    try:
        return int(parts[0]) * 60 + int(parts[1])
    except ValueError:
        return None


def minute_index(event):
    minute = event.get("minute")
    if minute is None:
        return 89
    return min(89, max(0, int(minute) - 1))


def parse_lineups(payload, club):
    """player_id -> {name, started, minutes} for one club's lineup payload."""
    rows = {}
    for player in payload.get("lineup", []):
        pid = player.get("player_id")
        if pid is None:
            continue
        minutes = 0
        started = False
        for position in player.get("positions", []):
            if position.get("start_reason") == "Starting XI":
                started = True
            start = clock_seconds(position.get("from"))
            if start is None:
                continue
            end = clock_seconds(position.get("to"))
            if end is None:
                end = 90 * 60  # played to the final whistle
            minutes += max(0, end - start)
        rows[int(pid)] = {
            "name": player.get("player_name") or "",
            "started": started,
            "minutes": min(minutes / 60.0, 90.0),
        }
    return rows


def parse_events(meta, events):
    """Per-match observations and labels from one StatsBomb event payload."""
    home = meta["home_team"]["home_team_name"]
    away = meta["away_team"]["away_team_name"]
    teams = {
        home: {"shots": 0, "sot": 0, "xg": 0.0, "goals": 0, "own_goals_for": 0},
        away: {"shots": 0, "sot": 0, "xg": 0.0, "goals": 0, "own_goals_for": 0},
    }
    players = {}

    def row_for(team, event):
        player = event.get("player") or {}
        pid = player.get("id")
        if pid is None:
            return None
        pid = int(pid)
        row = players.setdefault(pid, {
            "team": team, "name": player.get("name") or "",
            "shots": 0, "sot": 0, "goals": 0, "assists": 0,
            "xg": 0.0, "xa": 0.0, "passes": 0, "prog_carries": 0,
        })
        return row

    passes_by_id = {}
    goals, assists, reds, subs = [], [], [], []
    for event in events:
        team = (event.get("team") or {}).get("name")
        if team not in teams:
            continue
        kind = (event.get("type") or {}).get("name")
        if kind == "Pass":
            passes_by_id[event.get("id")] = event
            row = row_for(team, event)
            if row is not None:
                row["passes"] += 1
        elif kind == "Carry":
            row = row_for(team, event)
            if row is not None:
                location = event.get("location")
                end = (event.get("carry") or {}).get("end_location")
                if (location and end and (end[0] - location[0] >= 10.0 or end[0] >= 102.0)):
                    row["prog_carries"] += 1
        elif kind == "Shot":
            shot = event.get("shot") or {}
            xg = float(shot.get("statsbomb_xg") or 0.0)
            outcome = (shot.get("outcome") or {}).get("name")
            teams[team]["shots"] += 1
            teams[team]["xg"] += xg
            row = row_for(team, event)
            if row is not None:
                row["shots"] += 1
                row["xg"] += xg
                if outcome in ON_TARGET:
                    row["sot"] += 1
                    teams[team]["sot"] += 1
                if outcome == "Goal":
                    row["goals"] += 1
                    teams[team]["goals"] += 1
                    goals.append({
                        "team": team, "player_id": row and int(event["player"]["id"]),
                        "minute_index": minute_index(event), "own_goal": False,
                    })
            key_pass = shot.get("key_pass_id")
            pass_event = passes_by_id.get(key_pass)
            if pass_event is not None:
                passer = row_for(team, pass_event)
                if passer is not None:
                    passer["xa"] += xg
        elif kind == "Own Goal Against":
            other = away if team == home else home
            teams[other]["own_goals_for"] += 1
            goals.append({
                "team": other,
                "player_id": (event.get("player") or {}).get("id"),
                "minute_index": minute_index(event), "own_goal": True,
            })
        elif kind == "Bad Behaviour":
            card = ((event.get("bad_behaviour") or {}).get("card") or {}).get("name")
            if card in RED_CARDS:
                reds.append({
                    "team": team, "player_id": (event.get("player") or {}).get("id"),
                    "minute_index": minute_index(event),
                })
        elif kind == "Foul Committed":
            card = ((event.get("foul_committed") or {}).get("card") or {}).get("name")
            if card in RED_CARDS:
                reds.append({
                    "team": team, "player_id": (event.get("player") or {}).get("id"),
                    "minute_index": minute_index(event),
                })
        elif kind == "Substitution":
            substitution = event.get("substitution") or {}
            subs.append({
                "team": team,
                "player_off": (substitution.get("outgoing") or {}).get("id"),
                "player_on": (substitution.get("incoming") or {}).get("id"),
                "minute_index": minute_index(event),
            })
        elif kind == "Pass" and (event.get("pass") or {}).get("goal_assist") is True:
            pass
    # Registered assists (distinct pass events from the same scan).
    for event in events:
        team = (event.get("team") or {}).get("name")
        if team not in teams or (event.get("type") or {}).get("name") != "Pass":
            continue
        if (event.get("pass") or {}).get("goal_assist") is True:
            row = row_for(team, event)
            if row is not None:
                row["assists"] += 1
                assists.append({
                    "team": team, "player_id": int(event["player"]["id"]),
                    "minute_index": minute_index(event),
                })
    return {"teams": teams, "players": players, "goals": goals, "assists": assists, "reds": reds, "subs": subs}


def normalise_manifest(manifest):
    covered = []
    for entry in manifest.get("history", []):
        covered.append({
            "match_id": int(entry["match_id"]),
            "date": entry["date"],
            "home": entry["home"],
            "away": entry["away"],
            "home_goals": int(entry.get("home_goals") or 0),
            "away_goals": int(entry.get("away_goals") or 0),
        })
    fixture = manifest.get("fixture")
    if fixture:
        covered.append({
            "match_id": int(fixture["match_id"]),
            "date": fixture["date"],
            "home": fixture["home"],
            "away": fixture["away"],
            "home_goals": int(fixture.get("actual_home_goals") or 0),
            "away_goals": int(fixture.get("actual_away_goals") or 0),
        })
    covered.sort(key=lambda value: (value["date"], value["match_id"]))
    return covered


def write_csv(path, header, rows):
    lines = [",".join(header)]
    for row in rows:
        cells = []
        for key in header:
            value = row.get(key, "")
            if isinstance(value, float):
                if math.isnan(value):
                    cells.append("NaN")
                else:
                    cells.append(repr(value))
            else:
                cells.append(str(value))
        lines.append(",".join(cells))
    path.write_text("\n".join(lines) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--data", default=str(DEFAULT_DATA))
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    args = parser.parse_args()
    data_dir, out_dir = Path(args.data), Path(args.out)
    if not (data_dir / "matches/pl_2015_16.json").is_file():
        raise SystemExit(
            f"missing {data_dir/'matches/pl_2015_16.json'}; "
            "run python3 scripts/fetch_statsbomb_history.py first"
        )
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "players").mkdir(exist_ok=True)
    (out_dir / "labels").mkdir(exist_ok=True)

    matches = json.loads((data_dir / "matches/pl_2015_16.json").read_text())
    meta_by_id = {int(m["match_id"]): m for m in matches}
    covered = normalise_manifest(json.loads((data_dir / "manifest.json").read_text()))

    observations = {}
    audit_warnings = []
    for entry in covered:
        mid = entry["match_id"]
        meta = meta_by_id.get(mid)
        if meta is None:
            raise SystemExit(f"covered fixture {mid} is missing from matches/pl_2015_16.json")
        events = json.loads((data_dir / f"events/{mid}.json").read_text())
        lineups = json.loads((data_dir / f"lineups/{mid}.json").read_text())
        parsed = parse_events(meta, events)
        lineup_rows = {}
        for team_payload in lineups:
            lineup_rows[team_payload["team_name"]] = parse_lineups(team_payload, team_payload["team_name"])
        for club in (entry["home"], entry["away"]):
            if club not in lineup_rows:
                audit_warnings.append(f"match {mid}: no lineup payload for {club}")
                lineup_rows[club] = {}
        event_players = set(parsed["players"])
        for club in (entry["home"], entry["away"]):
            for pid in lineup_rows[club]:
                if pid not in event_players:
                    continue
                if parsed["players"][pid]["team"] != club:
                    audit_warnings.append(f"match {mid}: player {pid} event team != lineup club {club}")
        observations[mid] = {
            "entry": entry, "parsed": parsed, "lineups": lineup_rows,
        }

    # team_rows.csv: all 380 season results; shot columns only where covered.
    team_rows = []
    for meta in sorted(matches, key=lambda value: (value["match_date"], int(value["match_id"]))):
        mid = int(meta["match_id"])
        home = meta["home_team"]["home_team_name"]
        away = meta["away_team"]["away_team_name"]
        row = {
            "date": meta["match_date"], "home": home, "away": away,
            "home_goals": int(meta["home_score"]), "away_goals": int(meta["away_score"]),
            "home_shots": float("nan"), "away_shots": float("nan"),
            "home_sot": float("nan"), "away_sot": float("nan"),
            "home_xg": float("nan"), "away_xg": float("nan"),
        }
        if mid in observations:
            teams = observations[mid]["parsed"]["teams"]
            for side, club in (("home", home), ("away", away)):
                row[f"{side}_shots"] = float(teams[club]["shots"])
                row[f"{side}_sot"] = float(teams[club]["sot"])
                row[f"{side}_xg"] = float(teams[club]["xg"])
        team_rows.append(row)
    write_csv(
        out_dir / "team_rows.csv",
        ["date", "home", "away", "home_goals", "away_goals", "home_shots", "away_shots",
         "home_sot", "away_sot", "home_xg", "away_xg"],
        team_rows,
    )

    # Per-fixture causal player pools and labels.
    fixture_records = []
    max_feature_dates = []
    for entry in covered:
        mid = entry["match_id"]
        date_text = entry["date"]
        label = {
            "match_id": mid, "date": date_text, "home": entry["home"], "away": entry["away"],
            "home_goals": entry["home_goals"], "away_goals": entry["away_goals"],
            "goals": observations[mid]["parsed"]["goals"],
            "assists": observations[mid]["parsed"]["assists"],
            "reds": observations[mid]["parsed"]["reds"],
            "subs": observations[mid]["parsed"]["subs"],
            "teams": observations[mid]["parsed"]["teams"],
            "players": [],
        }
        coverage = {}
        all_candidate_rows = []
        for club in (entry["home"], entry["away"]):
            prior = [
                other for other in covered
                if other["date"] < date_text and club in (other["home"], other["away"])
            ]
            aggregate = {}
            for other in prior:
                other_mid = other["match_id"]
                lineup = observations[other_mid]["lineups"].get(club, {})
                stats = observations[other_mid]["parsed"]["players"]
                for pid, lrow in lineup.items():
                    row = aggregate.setdefault(pid, {
                        "name": lrow["name"], "listings": 0, "appearances": 0, "starts": 0,
                        "minutes": 0.0, "start_minutes": 0.0,
                        "shots": 0.0, "goals": 0.0, "assists": 0.0,
                        "xg": 0.0, "xa": 0.0, "passes": 0.0, "prog_carries": 0.0,
                        "first_listed": other["date"], "last_listed": other["date"],
                        "last_appearance": None,
                    })
                    row["listings"] += 1
                    if lrow["minutes"] > 0:
                        row["appearances"] += 1
                        row["last_appearance"] = other["date"]
                    if lrow["started"]:
                        row["starts"] += 1
                        row["start_minutes"] += lrow["minutes"]
                    row["minutes"] += lrow["minutes"]
                    row["last_listed"] = max(row["last_listed"], other["date"])
                    player_stats = stats.get(pid)
                    if player_stats is not None:
                        for key in ("shots", "goals", "assists", "xg", "xa", "passes", "prog_carries"):
                            row[key] += float(player_stats.get(key, 0.0))
            max_feature_date = max((other["date"] for other in prior), default="")
            if max_feature_date:
                assert max_feature_date < date_text, f"lookahead: {max_feature_date} >= {date_text}"
            max_feature_dates.append(max_feature_date or "none")
            coverage[club] = {"prior_matches": len(prior), "candidates": len(aggregate)}
            rows = []
            for pid, row in sorted(aggregate.items()):
                minutes = row["minutes"]
                per90 = lambda value: (value * 90.0 / minutes) if minutes > 0 else 0.0
                rows.append({
                    "player_id": pid, "name": row["name"].replace(",", " "), "team": club,
                    "minutes": minutes,
                    "selection_listings": row["listings"],
                    "selection_appearances": row["appearances"],
                    "selection_starts": row["starts"],
                    "selection_minutes": row["minutes"],
                    "selection_start_minutes": row["start_minutes"],
                    "team_fixtures": len(prior),
                    "first_listed_date": row["first_listed"],
                    "last_listed_date": row["last_listed"],
                    "last_appearance_date": row["last_appearance"] or "",
                    "shots_p90": per90(row["shots"]),
                    "goals_p90": per90(row["goals"]),
                    "assists_p90": per90(row["assists"]),
                    "xg_p90": per90(row["xg"]),
                    "xa_p90": per90(row["xa"]),
                    "passes_p90": per90(row["passes"]),
                    "prog_carries_p90": per90(row["prog_carries"]),
                })
            all_candidate_rows.extend(rows)
        write_csv(
            out_dir / f"players/{mid}.csv",
            ["player_id", "name", "team", "minutes", "selection_listings", "selection_appearances",
             "selection_starts", "selection_minutes", "selection_start_minutes", "team_fixtures",
             "first_listed_date", "last_listed_date", "last_appearance_date",
             "shots_p90", "goals_p90", "assists_p90", "xg_p90", "xa_p90", "passes_p90",
             "prog_carries_p90"],
            all_candidate_rows,
        )
        # Actual labels: same fixture's own events, used for scoring only.
        lineup_home = observations[mid]["lineups"].get(entry["home"], {})
        lineup_away = observations[mid]["lineups"].get(entry["away"], {})
        stats = observations[mid]["parsed"]["players"]
        for club, lineup in ((entry["home"], lineup_home), (entry["away"], lineup_away)):
            for pid, lrow in sorted(lineup.items()):
                player_stats = stats.get(pid, {})
                label["players"].append({
                    "player_id": pid, "team": club, "name": lrow["name"],
                    "minutes": lrow["minutes"], "started": lrow["started"],
                    "shots": float(player_stats.get("shots", 0.0)),
                    "sot": float(player_stats.get("sot", 0.0)),
                    "goals": float(player_stats.get("goals", 0.0)),
                    "assists": float(player_stats.get("assists", 0.0)),
                    "xg": float(player_stats.get("xg", 0.0)),
                    "xa": float(player_stats.get("xa", 0.0)),
                    "passes": float(player_stats.get("passes", 0.0)),
                    "prog_carries": float(player_stats.get("prog_carries", 0.0)),
                })
        (out_dir / f"labels/{mid}.json").write_text(json.dumps(label, indent=1) + "\n")
        fixture_records.append({
            "match_id": mid, "date": date_text, "home": entry["home"], "away": entry["away"],
            "home_goals": entry["home_goals"], "away_goals": entry["away_goals"],
            "coverage": coverage,
            "evaluable": coverage[entry["home"]]["candidates"] >= 11 and coverage[entry["away"]]["candidates"] >= 11,
        })

    (out_dir / "fixtures.json").write_text(json.dumps({
        "schema_version": 1,
        "source": "StatsBomb open data, English Premier League 2015/16 (local event JSON)",
        "feature_contract": "observed_stats_only_no_fm26_no_current_squads",
        "no_lookahead_rule": "all features from event-covered matches with date < fixture date; same-day excluded",
        "fixtures": fixture_records,
    }, indent=1) + "\n")
    report = {
        "schema_version": 1,
        "covered_matches": len(covered),
        "season_metadata_matches": len(matches),
        "evaluable_fixtures": sum(1 for f in fixture_records if f["evaluable"]),
        "feature_contract": "observed_stats_only_no_fm26_no_current_squads",
        "max_feature_date_before_fixture": all(
            value == "none" or value < fixture["date"]
            for value, fixture in zip(max_feature_dates, fixture_records)
        ),
        "warnings": audit_warnings,
    }
    (out_dir / "build_report.json").write_text(json.dumps(report, indent=1) + "\n")
    print(f"built {len(covered)} covered fixtures; evaluable={report['evaluable_fixtures']}")
    print(f"no-lookahead audit: {report['max_feature_date_before_fixture']}; warnings={len(audit_warnings)}")
    print(f"outputs under {out_dir}")


if __name__ == "__main__":
    main()
