"""CF Montréal: recent results + upcoming fixtures from SofaScore."""
from curl_cffi import requests
import json, time

s = requests.Session(impersonate="chrome")
TEAM = 22006

# last events (results)
r = s.get(f"https://api.sofascore.com/api/v1/team/{TEAM}/events/last/0", timeout=30)
last = r.json()
events = last.get("events", [])
print("=== last matches ===")
for e in events[:8]:
    ts = time.strftime("%Y-%m-%d", time.gmtime(e["startTimestamp"]))
    ht, at = e["homeTeam"]["name"], e["awayTeam"]["name"]
    hs = e.get("homeScore", {}).get("current")
    as_ = e.get("awayScore", {}).get("current")
    tour = e["tournament"]["name"]
    print(ts, f"{ht} {hs}-{as_} {at}", "|", tour, "| id", e["id"], "| slug", e.get("slug"))

json.dump(last, open("/tmp/cfm_last.json", "w"))

# next events (upcoming)
r2 = s.get(f"https://api.sofascore.com/api/v1/team/{TEAM}/events/next/0", timeout=30)
nxt = r2.json()
print("\n=== next matches ===")
for e in nxt.get("events", []):
    ts = time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime(e["startTimestamp"]))
    print(ts, e["homeTeam"]["name"], "vs", e["awayTeam"]["name"], "|", e["tournament"]["name"], "| id", e["id"])
json.dump(nxt, open("/tmp/cfm_next.json", "w"))
