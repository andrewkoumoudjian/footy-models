"""Fetch per-player season stats per team via the team-scoped endpoint (correct shape)."""
from curl_cffi import requests
import json, os

s = requests.Session(impersonate="chrome")
OUT = "data/sofascore_cfm"
UT, SEASON = 242, 86668
TEAMS = json.load(open(f"{OUT}/teams.json"))

# The league endpoint returns rows WITHOUT statistics unless ordered by a stat.
# Try ordering variants and check which include 'statistics'.
r = s.get(f"https://api.sofascore.com/api/v1/unique-tournament/{UT}/season/{SEASON}/statistics",
          params={"limit": 3, "order": "goalsScored"}, timeout=30)
row = r.json()["results"][0]
print("has statistics key:", "statistics" in row, "| keys:", list(row.keys()))
if "statistics" in row:
    print(json.dumps(row["statistics"], indent=1)[:600])

# Also probe team-scoped player season stats
TEAM = TEAMS["CF Montréal"]
for url in [
    f"https://api.sofascore.com/api/v1/team/{TEAM}/unique-tournament/{UT}/season/{SEASON}/players",
    f"https://api.sofascore.com/api/v1/team/{TEAM}/unique-tournament/{UT}/season/{SEASON}/statistics/overall",
    f"https://api.sofascore.com/api/v1/team/{TEAM}/performance/unique-tournament/{UT}/season/{SEASON}",
]:
    rr = s.get(url, timeout=30)
    print(rr.status_code, url.split(str(TEAM) + "/")[1][:60], "|", rr.text[:80].replace("\n", " "))
