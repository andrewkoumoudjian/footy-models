"""Build strictly causal next-match player participation training rows.

Each row asks: given only a player's selection history for this club before a covered fixture,
will they appear/start in that fixture and, if starting, how many minutes will they play?

Cold-start players are deliberately absent from the row set for their first club listing. They
become candidates only after that listing, preventing future roster membership from leaking
backward. A fixture contributes non-appearance labels only when its player-level payload exists.
"""

import csv
import glob
import json
import os
from collections import defaultdict
from datetime import date

OUT = "data/sofascore_cfm"
DEST = f"{OUT}/selection_training.tsv"
FRESH_DAYS = 45


def parse_date(value):
    year, month, day = map(int, value.split("-"))
    return date(year, month, day)


matches = []
with open(f"{OUT}/asof_team_stats.csv") as handle:
    for row in csv.DictReader(handle):
        path = f"{OUT}/player_matches/{row['match_id']}.json"
        if os.path.exists(path):
            matches.append((row["date"], row, path))
matches.sort(key=lambda item: (item[0], item[1]["match_id"]))

# Number of covered team fixtures already processed. This denominator advances only when the
# player-level source exists, so missing provider coverage is never treated as a non-selection.
team_fixture_count = defaultdict(int)
states = {}
current_team_by_player = {}
rows = []


def target_players(block):
    result = {}
    for entry in (block or {}).get("players", []):
        player = entry.get("player") or {}
        player_id = player.get("id")
        if player_id is None:
            continue
        stats = entry.get("statistics") or {}
        minutes = float(stats.get("minutesPlayed") or 0.0)
        appeared = minutes > 0.0
        started = appeared and not bool(entry.get("substitute", True))
        result[int(player_id)] = {
            "name": player.get("name") or "",
            "appeared": appeared,
            "started": started,
            "minutes": minutes,
        }
    return result


for match_date_text, match, path in matches:
    match_date = parse_date(match_date_text)
    try:
        payload = json.load(open(path))
    except Exception:
        continue

    for side, team_key, team_name_key in (
        ("home", "home_id", "home"),
        ("away", "away_id", "away"),
    ):
        block = payload.get(side)
        if block is None:
            continue
        team_id = int(match[team_key])
        team_name = match[team_name_key]
        actual = target_players(block)
        covered_before = team_fixture_count[team_id]

        # Score only players whose membership was knowable before kickoff. A first appearance for
        # this club is a cold start and becomes evidence only for later fixtures.
        for (state_team, player_id), state in list(states.items()):
            if state_team != team_id or current_team_by_player.get(player_id) != team_id:
                continue
            days_since_listed = (match_date - state["last_listed"]).days
            if days_since_listed < 0 or days_since_listed > FRESH_DAYS:
                continue
            appearance_anchor = state["last_appearance"] or state["first_listed"]
            days_since_appearance = (match_date - appearance_anchor).days
            club_fixtures = covered_before - state["first_fixture_index"] + 1
            if club_fixtures < 2:
                continue
            outcome = actual.get(player_id)
            appeared = bool(outcome and outcome["appeared"])
            started = bool(outcome and outcome["started"])
            minutes = float(outcome["minutes"] if outcome else 0.0)
            rows.append({
                "date": match_date_text,
                "match_id": match["match_id"],
                "team_id": team_id,
                "team": team_name,
                "player_id": player_id,
                "club_fixtures": club_fixtures,
                "prior_listings": state["listings"],
                "prior_appearances": state["appearances"],
                "prior_starts": state["starts"],
                "prior_minutes": round(state["minutes"], 3),
                "prior_start_minutes": round(state["start_minutes"], 3),
                "days_since_last_appearance": days_since_appearance,
                "days_since_last_listed": days_since_listed,
                "appeared": int(appeared),
                "started": int(started),
                "minutes": round(minutes, 3),
            })

        # The target can update history only after its labels have been emitted.
        team_fixture_count[team_id] += 1
        current_index = team_fixture_count[team_id]
        for player_id, outcome in actual.items():
            # Once a player has been listed for a different club, that is causal evidence that the old
            # club state should no longer generate future non-selection labels. Keep the historical
            # state for auditability but move the player's active membership pointer.
            current_team_by_player[player_id] = team_id
            key = (team_id, player_id)
            state = states.get(key)
            if state is None:
                state = {
                    "first_fixture_index": current_index,
                    "first_listed": match_date,
                    "listings": 0,
                    "appearances": 0,
                    "starts": 0,
                    "minutes": 0.0,
                    "start_minutes": 0.0,
                    "last_listed": match_date,
                    "last_appearance": None,
                }
                states[key] = state
            state["listings"] += 1
            state["last_listed"] = match_date
            if not outcome["appeared"]:
                continue
            state["appearances"] += 1
            state["minutes"] += outcome["minutes"]
            if outcome["started"]:
                state["starts"] += 1
                state["start_minutes"] += outcome["minutes"]
            state["last_appearance"] = match_date

columns = [
    "date", "match_id", "team_id", "team", "player_id", "club_fixtures", "prior_listings",
    "prior_appearances", "prior_starts", "prior_minutes", "prior_start_minutes",
    "days_since_last_appearance", "days_since_last_listed", "appeared", "started", "minutes",
]
with open(DEST, "w", newline="") as handle:
    writer = csv.DictWriter(handle, fieldnames=columns, delimiter="\t")
    writer.writeheader()
    writer.writerows(rows)

starts = sum(row["started"] for row in rows)
appearances = sum(row["appeared"] for row in rows)
print(
    f"wrote {len(rows)} causal participation rows -> {DEST}; "
    f"appearance rate={appearances / max(len(rows), 1):.3f}, "
    f"start rate={starts / max(len(rows), 1):.3f}"
)
