"""Pull CF Montréal 2026 MLS season history: all finished events + per-match team stats."""
from curl_cffi import requests
import json, time

s = requests.Session(impersonate="chrome")
TEAM = 22006
OUT = "data/sofascore_cfm"

import os
os.makedirs(f"{OUT}/events", exist_ok=True)
os.makedirs(f"{OUT}/teamstats", exist_ok=True)
os.makedirs(f"{OUT}/lineups", exist_ok=True)

# page through ALL past events (last/N pages)
all_events = []
page = 0
while True:
    r = s.get(f"https://api.sofascore.com/api/v1/team/{TEAM}/events/last/{page}", timeout=30)
    if r.status_code != 200:
        break
    events = r.json().get("events", [])
    if not events:
        break
    all_events.extend(events)
    if len(events) < 40:  # page size heuristic
        break
    page += 1
    if page > 10:
        break

print("total past events fetched:", len(all_events))

# keep FINISHED MLS (+Canadian Championship) matches from the 2026 season window
finished = []
for e in all_events:
    ts = e["startTimestamp"]
    date = time.strftime("%Y-%m-%d", time.gmtime(ts))
    if date < "2026-02-01":
        continue
    status = e.get("status", {}).get("type")
    comp = e["tournament"]["name"]
    if status != "finished" or comp not in ("MLS", "Canadian Championship", "CONCACAF Champions Cup"):
        continue
    finished.append(e)

print("2026 finished competitive:", len(finished))
for e in sorted(finished, key=lambda x: x["startTimestamp"]):
    d = time.strftime("%Y-%m-%d", time.gmtime(e["startTimestamp"]))
    print(d, f'{e["homeTeam"]["name"]} {e.get("homeScore",{}).get("current")}-{e.get("awayScore",{}).get("current")} {e["awayTeam"]["name"]}', "| id", e["id"])

json.dump({"events": sorted(finished, key=lambda x: x["startTimestamp"])},
          open(f"{OUT}/history_manifest.json", "w"), indent=1)

# fetch per-match event statistics for each
ok = fail = 0
for e in finished:
    mid = e["id"]
    try:
        rs = s.get(f"https://api.sofascore.com/api/v1/event/{mid}/statistics", timeout=30)
        if rs.status_code == 200:
            json.dump(rs.json(), open(f"{OUT}/teamstats/{mid}.json", "w"))
            ok += 1
        else:
            fail += 1
    except Exception:
        fail += 1
print("teamstats ok/fail:", ok, fail)
