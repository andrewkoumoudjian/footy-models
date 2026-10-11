"""Fetch CFM player season stats + upcoming opponent (Inter Miami) stats from SofaScore.

- CFM squad player season statistics for 2026 MLS
- Inter Miami's 2026 match results/team stats (needed to forecast their fixture vs CFM)
"""
from curl_cffi import requests
import json, time, os

s = requests.Session(impersonate="chrome")
OUT = "data/sofascore_cfm"
os.makedirs(f"{OUT}/players", exist_ok=True)

CFM = 22006
MIAMI = None

# unique season id from an event
ev = json.load(open("/tmp/cfm_next.json"))["events"][0]
print("next event:", ev["homeTeam"]["name"], "vs", ev["awayTeam"]["name"])
season = ev["season"]
print("season:", season)
tournament = ev["tournament"]["id"]

# Opponents of interest for the next 3 fixtures
NEXT = [
    {"name": "Inter Miami CF", "date": "2026-08-29", "event_id": 15168339},
    {"name": "Vancouver Whitecaps", "date": "2026-09-01", "event_id": 16851667},
    {"name": "Philadelphia Union", "date": "2026-09-05", "event_id": 15168358},
]
json.dump(NEXT, open(f"{OUT}/upcoming.json", "w"), indent=1)

# search team ids for opponents
def team_id(name):
    r = s.get("https://api.sofascore.com/api/v1/search/all", params={"q": name}, timeout=30)
    for res in r.json().get("results", []):
        if res.get("type") == "team":
            ent = res["entity"]
            if ent.get("sport", {}).get("slug") == "football" and ent["name"].lower().startswith(name.lower()[:12]):
                return ent["id"], ent["name"]
    return None, None

opponents = {}
for n in ["Inter Miami CF", "Vancouver Whitecaps FC", "Philadelphia Union"]:
    tid, tname = team_id(n)
    opponents[n] = tid
    print(n, "->", tid, tname)

json.dump(opponents, open(f"{OUT}/opponent_ids.json", "w"), indent=1)

# past events pages of each opponent (for as-of history)
def team_history(team_id, since="2026-02-01"):
    events = []
    page = 0
    while page < 8:
        r = s.get(f"https://api.sofascore.com/api/v1/team/{team_id}/events/last/{page}", timeout=30)
        if r.status_code != 200:
            break
        evs = r.json().get("events", [])
        if not evs:
            break
        events.extend(evs)
        if len(evs) < 40:
            break
        page += 1
    out = []
    for e in events:
        date = time.strftime("%Y-%m-%d", time.gmtime(e["startTimestamp"]))
        if date >= since and e.get("status", {}).get("type") == "finished" and e["tournament"]["name"] in ("MLS", "Canadian Championship", "CONCACAF Champions Cup"):
            st = {}
            try:
                rs = s.get(f"https://api.sofascore.com/api/v1/event/{e['id']}/statistics", timeout=30)
                if rs.status_code == 200:
                    st = rs.json()
                    json.dump(st, open(f"{OUT}/teamstats/{e['id']}.json", "w"))
            except Exception:
                pass
            out.append({"event": e, "has_stats": bool(st)})
    return out

histories = {}
for n, tid in opponents.items():
    if tid is None:
        continue
    h = team_history(tid)
    histories[n] = h
    print(n, len(h), "finished matches")

json.dump({k: [x["event"] | {"has_stats": x["has_stats"]} for x in v] for k, v in histories.items()},
          open(f"{OUT}/opponent_history_manifest.json", "w"))
