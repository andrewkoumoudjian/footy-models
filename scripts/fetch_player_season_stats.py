"""Fetch per-player season statistics for the four teams via the league-season endpoint,
filtered to each team, plus team squads. Saves JSON for the calibration step."""
from curl_cffi import requests
import json, os

s = requests.Session(impersonate="chrome")
OUT = "data/sofascore_cfm"
os.makedirs(f"{OUT}/players", exist_ok=True)

UT, SEASON = 242, 86668
TEAMS = {"CF Montréal": 22006, "Inter Miami CF": 337602, "Vancouver Whitecaps": 22010, "Philadelphia Union": 39833}

# League-wide player season stats (paginated), then filter by team id.
all_players = []
page = 0
while page < 12:
    url = f"https://api.sofascore.com/api/v1/unique-tournament/{UT}/season/{SEASON}/statistics"
    r = s.get(url, params={"limit": 100, "order": "rating", "page": page}, timeout=30)
    if r.status_code != 200:
        print("stop at page", page, r.status_code)
        break
    results = r.json().get("results", [])
    if not results:
        break
    all_players.extend(results)
    page += 1
print("league player rows:", len(all_players))

by_team = {tid: [] for tid in TEAMS.values()}
for row in all_players:
    tid = row.get("player", {}).get("team", {}).get("id")
    if tid in by_team:
        by_team[tid].append(row)

for name, tid in TEAMS.items():
    rows = by_team[tid]
    json.dump({"rows": rows}, open(f"{OUT}/players/{tid}.json", "w"))
    print(name, "->", len(rows), "players with season stats")

json.dump(TEAMS, open(f"{OUT}/teams.json", "w"), indent=1)

# squads (current roster incl. players without stats yet)
for name, tid in TEAMS.items():
    r = s.get(f"https://api.sofascore.com/api/v1/team/{tid}/players", timeout=30)
    if r.status_code == 200:
        json.dump(r.json(), open(f"{OUT}/players/{tid}_squad.json", "w"))
        print(name, "squad size:", len(r.json().get("players", [])))
