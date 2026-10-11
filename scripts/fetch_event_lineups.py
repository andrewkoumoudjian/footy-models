"""Player season stats: use event lineups (per-match player stats) for the last N matches
of each team -> per-player per-90 aggregates. This is the underlying player data we need.

For each of the 4 teams: iterate their finished 2026 matches, GET /event/{id}/lineups,
which includes all players with minutes + statistics for that match.
"""
from curl_cffi import requests
import json, os, time

s = requests.Session(impersonate="chrome")
OUT = "data/sofascore_cfm"
os.makedirs(f"{OUT}/player_matches", exist_ok=True)

TEAMS = json.load(open(f"{OUT}/teams.json"))

def load_history(team_name):
    if team_name == "CF Montréal":
        return json.load(open(f"{OUT}/history_manifest.json"))["events"]
    opp = json.load(open(f"{OUT}/opponent_history_manifest.json"))
    key = team_name if team_name in opp else {"Vancouver Whitecaps": "Vancouver Whitecaps FC"}.get(team_name)
    return [e | {"has_stats": True} for e in opp[key]]

total = ok = 0
for name, tid in TEAMS.items():
    events = load_history(name)
    fetched = 0
    for e in events:
        mid = e["id"]
        dest = f"{OUT}/player_matches/{mid}.json"
        total += 1
        if os.path.exists(dest) and os.path.getsize(dest) > 200:
            fetched += 1
            continue
        try:
            r = s.get(f"https://api.sofascore.com/api/v1/event/{mid}/lineups", timeout=30)
            if r.status_code == 200:
                json.dump(r.json(), open(dest, "w"))
                fetched += 1
            else:
                # lineups may be missing for older matches; confirmed is fine, skip quietly
                pass
        except Exception:
            pass
    print(f"{name}: {fetched}/{len(events)} lineups")
    ok += fetched
print("total lineups cached:", ok)
