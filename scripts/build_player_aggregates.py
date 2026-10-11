"""Build per-player season aggregates from cached SofaScore event lineups.

Outputs data/sofascore_cfm/player_aggregates.csv with per-90 rates:
player_id, name, team, position, matches, starts, minutes,
shots, goals, xg, assists, xa, passes, pass_acc, key_passes,
carries, progressive_carries, progression_m, dribbles, dribbles_won,
tackles, interceptions, clearances, ball_recovery, fouls, yellow, red,
touches, dispossessed, rating
"""
import json, glob, os
from collections import defaultdict

OUT = "data/sofascore_cfm"

# match -> (home_id, away_id) to know each player's team
asof = {}
with open(f"{OUT}/asof_team_stats.csv") as f:
    header = f.readline().strip().split(",")
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        asof[row["match_id"]] = (int(row["home_id"]), int(row["away_id"]),
                                 row["home"], row["away"],
                                 float(row["home_goals"] or 0), float(row["away_goals"] or 0),
                                 row["date"])

TEAMS = {v: k for k, v in json.load(open(f"{OUT}/teams.json")).items()}

FIELDS = {
    "totalShots": "shots", "goals": "goals", "expectedGoals": "xg",
    "goalAssist": "assists", "expectedAssists": "xa",
    "totalPass": "passes", "accuratePass": "passes_acc", "keyPasses": "key_passes",
    "ballCarriesCount": "carries", "progressiveBallCarriesCount": "prog_carries",
    "totalProgression": "progression_m", "dribbles": "dribbles", "successfulDribbles": "dribbles_won",
    "totalTackle": "tackles", "interception": "interceptions", "clearances": "clearances",
    "ballRecovery": "ball_recovery", "fouls": "fouls", "yellowCards": "yellow", "redCards": "red",
    "touches": "touches", "dispossessed": "dispossessed", "rating": "rating_sum",
    "bigChancesCreated": "big_chances", "onTargetScoringAttempts": "sot",
}

players = defaultdict(lambda: {"matches": 0, "starts": 0, "minutes": 0.0})
selection = defaultdict(lambda: {
    "listings": 0, "appearances": 0, "starts": 0, "minutes": 0.0, "start_minutes": 0.0,
    "first_date": None,
})
team_fixture_dates = defaultdict(list)
last_listed = {}
last_appearance = {}
for path in glob.glob(f"{OUT}/player_matches/*.json"):
    mid = os.path.basename(path)[:-5]
    if mid not in asof:
        continue
    home_id, away_id, match_date = asof[mid][0], asof[mid][1], asof[mid][6]
    d = json.load(open(path))
    # Only count a fixture as selection evidence when the player-level source exists. Missing
    # lineup coverage is unknown, not evidence that a player was omitted from the squad.
    team_fixture_dates[home_id].append(match_date)
    team_fixture_dates[away_id].append(match_date)
    for side in ("home", "away"):
        block = d.get(side)
        if not block:
            continue
        for entry in block.get("players", []):
            pid = entry["player"]["id"]
            st = entry.get("statistics") or {}
            minutes = float(st.get("minutesPlayed") or 0)
            agg = players[pid]
            agg.setdefault("name", entry["player"]["name"])
            agg.setdefault("position", entry.get("position") or entry["player"].get("position"))
            team_id = home_id if side == "home" else away_id
            if pid not in last_listed or match_date > last_listed[pid]:
                last_listed[pid] = match_date
                agg["team_id"] = team_id
            sel = selection[(pid, team_id)]
            if sel["first_date"] is None or match_date < sel["first_date"]:
                sel["first_date"] = match_date
            sel["listings"] += 1
            if minutes <= 0:
                continue
            if pid not in last_appearance or match_date > last_appearance[pid]:
                last_appearance[pid] = match_date
            agg["matches"] += 1
            started = not entry.get("substitute")
            if started:
                agg["starts"] += 1
            agg["minutes"] += minutes
            sel["appearances"] += 1
            sel["minutes"] += minutes
            if started:
                sel["starts"] += 1
                sel["start_minutes"] += minutes
            for src, dst in FIELDS.items():
                value = st.get(src)
                if value is None:
                    continue
                try:
                    agg[dst] = agg.get(dst, 0.0) + float(value)
                except (TypeError, ValueError):
                    continue

rows = []
for pid, agg in players.items():
    m90 = max(agg["minutes"] / 90.0, 0.1)
    team_id = agg.get("team_id")
    sel = selection.get((pid, team_id), {})
    first_current_club_date = sel.get("first_date")
    current_club_fixtures = sum(
        1 for fixture_date in team_fixture_dates.get(team_id, [])
        if first_current_club_date is not None and fixture_date >= first_current_club_date
    )
    row = {
        "player_id": pid, "name": agg.get("name"), "team": TEAMS.get(team_id),
        "pos": agg.get("position"), "matches": agg["matches"], "starts": agg["starts"],
        "minutes": round(agg["minutes"]),
        "first_listed_date": first_current_club_date or "",
        "last_listed_date": last_listed.get(pid, ""),
        "last_appearance_date": last_appearance.get(pid, ""),
        "selection_listings": sel.get("listings", 0),
        "selection_appearances": sel.get("appearances", 0),
        "selection_starts": sel.get("starts", 0),
        "selection_minutes": round(sel.get("minutes", 0.0)),
        "selection_start_minutes": round(sel.get("start_minutes", 0.0)),
        "team_fixtures": current_club_fixtures,
    }
    for field in ["shots", "goals", "xg", "assists", "xa", "passes", "passes_acc", "key_passes",
                  "carries", "prog_carries", "dribbles_won", "tackles", "interceptions",
                  "clearances", "ball_recovery", "big_chances", "sot"]:
        total = agg.get(field, 0.0)
        row[f"{field}_p90"] = round(total / m90, 3)
    row["pass_pct"] = round(100 * agg.get("passes_acc", 0) / max(agg.get("passes", 1), 1), 1)
    rows.append(row)

rows.sort(key=lambda r: (r["team"] or "", -(r["minutes"])))
cols = list(rows[0].keys())
import io
with open(f"{OUT}/player_aggregates.csv", "w") as f:
    f.write(",".join(cols) + "\n")
    for r in rows:
        f.write(",".join(str(r[c]) for c in cols) + "\n")
print(f"wrote {len(rows)} players -> {OUT}/player_aggregates.csv")

# quick view of CFM's top players by minutes
cfm = [r for r in rows if r["team"] == "CF Montréal"][:12]
print("\nCFM top by minutes:")
for r in cfm:
    print(f"  {r['name']:<24} {r['pos']} min={r['minutes']:>4} shots={r['shots_p90']:.2f} "
          f"xg={r['xg_p90']:.2f} xa={r['xa_p90']:.2f} passes={r['passes_p90']:.0f} progC={r['prog_carries_p90']:.1f}")
