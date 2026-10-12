#!/usr/bin/env python3
"""Build UCL team-history tables from immutable cached ESPN boards.

Reads per-day scoreboard snapshots (data/ucl/raw_espn/history + MD1 boards),
emits an as-of team stats CSV and a results TSV in the exact shapes the
forecast bundle consumes. All cached fixtures precede the cutoff by
construction (callers only cache pre-cutoff dates).

Regulation-score rule: matches decided after extra time or penalties carry
final (ET-inclusive) board scores with no period linescores, so regulation
goals are unrecoverable from this source. Those fixtures are EXCLUDED from
the team-goal history (counted in provenance) -- missing stays missing,
never an imputed 90-minute score.

Optional --espn frozen summary files contribute measured team shots (player
shots aggregated per side); matches without a frozen summary keep empty
shots with kind=missing, exactly like the EPL CSV's xG columns.
"""
from __future__ import annotations

import argparse
import csv
import glob
import json
from collections import defaultdict
from pathlib import Path

EXCLUDED_DESCRIPTIONS = {
    "Final Score - After Extra Time",
    "Final Score - After Penalties",
}


def load_boards(patterns: list[str]) -> list[dict]:
    events = []
    for pattern in patterns:
        for path in sorted(glob.glob(pattern)):
            board = json.loads(Path(path).read_text())
            for event in board.get("events", []):
                events.append(event)
    return events


def result_of(event: dict):
    status = event.get("status", {}).get("type", {})
    if not status.get("completed"):
        return None
    comp = event.get("competitions", [{}])[0]
    sides = {c.get("homeAway"): c for c in comp.get("competitors", [])}
    if set(sides) != {"home", "away"}:
        raise ValueError(f"cannot resolve home/away for {event.get('id')}")
    try:
        home_goals = float(sides["home"]["score"])
        away_goals = float(sides["away"]["score"])
    except (TypeError, ValueError) as exc:
        raise ValueError(f"missing board score for {event.get('id')}") from exc
    return {
        "event_id": str(event.get("id")),
        "date": event.get("date", "")[:10],
        "description": status.get("description", ""),
        "home": sides["home"]["team"]["displayName"],
        "away": sides["away"]["team"]["displayName"],
        "home_goals": home_goals,
        "away_goals": away_goals,
    }


def team_shots(espn_paths: list[Path]) -> dict[str, dict]:
    shots: dict[str, dict] = {}
    for path in espn_paths:
        frozen = json.loads(path.read_text())
        for match in frozen.get("matches", []):
            by_club: dict[str, float] = defaultdict(float)
            for p in match["players"]:
                if p.get("appeared") and p.get("shots") is not None:
                    by_club[p["club"]] += float(p["shots"])
            shots[match["espn_match_id"]] = {
                match["home"]: by_club.get(match["home"]),
                match["away"]: by_club.get(match["away"]),
            }
    return shots


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--boards", nargs="+", required=True)
    ap.add_argument("--espn", nargs="*", default=[], type=Path)
    ap.add_argument("--stats-out", type=Path, required=True)
    ap.add_argument("--results-out", type=Path, required=True)
    a = ap.parse_args()
    if a.stats_out.exists() or a.results_out.exists():
        ap.error("refusing to overwrite immutable history tables")
    events = load_boards(a.boards)
    if not events:
        raise ValueError("no board events")
    seen: dict[str, dict] = {}
    for event in events:
        row = result_of(event)
        if row is None:
            continue
        if row["event_id"] in seen and seen[row["event_id"]] != row:
            raise ValueError(f"duplicate conflicting event {row['event_id']}")
        seen[row["event_id"]] = row
    shots = team_shots(a.espn)
    rows = sorted(seen.values(), key=lambda r: (r["date"], r["event_id"]))
    excluded = [r for r in rows if r["description"] in EXCLUDED_DESCRIPTIONS]
    kept = [r for r in rows if r["description"] not in EXCLUDED_DESCRIPTIONS]
    if len(kept) < 30:
        raise ValueError("insufficient dated historical fixtures")
    a.stats_out.parent.mkdir(parents=True, exist_ok=True)
    with a.stats_out.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["date", "home", "away", "home_goals", "away_goals",
                         "home_xg", "away_xg", "home_shots", "away_shots",
                         "home_sot", "away_sot", "source_provider",
                         "source_match_date", "source_published_at",
                         "home_xg_kind", "away_xg_kind", "home_shots_kind",
                         "away_shots_kind", "home_sot_kind", "away_sot_kind"])
        for r in kept:
            side = shots.get(r["event_id"], {})
            hs, aws = side.get(r["home"]), side.get(r["away"])
            writer.writerow([
                r["date"], r["home"], r["away"], f"{r['home_goals']:.1f}",
                f"{r['away_goals']:.1f}", "", "",
                "" if hs is None else f"{hs:.1f}",
                "" if aws is None else f"{aws:.1f}", "", "",
                "ESPN site.api scoreboard+summaries", r["date"], "",
                "missing", "missing",
                "measured" if hs is not None else "missing",
                "measured" if aws is not None else "missing",
                "missing", "missing"])
    with a.results_out.open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["day", "division", "home", "away", "home_goals", "away_goals"])
        for r in kept:
            writer.writerow([r["date"], "UCL", r["home"], r["away"],
                             int(r["home_goals"]), int(r["away_goals"])])
    print(f"UCL history v1: {len(kept)} regulation fixtures, "
          f"{len(excluded)} ET/penalty fixtures excluded (unrecoverable regulation score), "
          f"{sum(1 for r in kept if r['event_id'] in shots)} with measured shots")


if __name__ == "__main__":
    main()
