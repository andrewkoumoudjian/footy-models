"""Fetch Charlotte FC 2026 history (team stats + lineups) so the CFM home fixture can run."""
from curl_cffi import requests
import json, os, time

s = requests.Session(impersonate="chrome")
OUT = "data/sofascore_cfm"
TEAM = 404108

os.makedirs(f"{OUT}/teamstats", exist_ok=True)
os.makedirs(f"{OUT}/player_matches", exist_ok=True)

events = []
page = 0
while page < 8:
    r = s.get(f"https://api.sofascore.com/api/v1/team/{TEAM}/events/last/{page}", timeout=30)
    if r.status_code != 200:
        break
    evs = r.json().get("events", [])
    if not evs:
        break
    events.extend(evs)
    if len(evs) < 40:
        break
    page += 1

kept = []
for e in events:
    date = time.strftime("%Y-%m-%d", time.gmtime(e["startTimestamp"]))
    if date >= "2026-08-23":  # as-of cutoff: only matches BEFORE the forecast date
        continue
    if e.get("status", {}).get("type") != "finished":
        continue
    if e["tournament"]["name"] not in ("MLS", "Canadian Championship", "CONCACAF Champions Cup"):
        continue
    kept.append(e)

print("Charlotte finished matches pre-cutoff:", len(kept))

stats_ok = lineup_ok = 0
for e in kept:
    mid = e["id"]
    rs = s.get(f"https://api.sofascore.com/api/v1/event/{mid}/statistics", timeout=30)
    if rs.status_code == 200 and rs.json():
        json.dump(rs.json(), open(f"{OUT}/teamstats/{mid}.json", "w"))
        stats_ok += 1
    rl = s.get(f"https://api.sofascore.com/api/v1/event/{mid}/lineups", timeout=30)
    if rl.status_code == 200 and rl.json():
        json.dump(rl.json(), open(f"{OUT}/player_matches/{mid}.json", "w"))
        lineup_ok += 1

json.dump([e | {"has_stats": True} for e in kept], open(f"{OUT}/charlotte_history_manifest.json", "w"))
print("stats:", stats_ok, "lineups:", lineup_ok)
