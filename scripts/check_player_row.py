"""Check why team filter matched 0 rows: inspect the row shape."""
import json

d = json.load(open("data/sofascore_cfm/players/league_probe.json")) if False else None
from curl_cffi import requests
s = requests.Session(impersonate="chrome")
r = s.get("https://api.sofascore.com/api/v1/unique-tournament/242/season/86668/statistics",
          params={"limit": 3, "order": "rating", "page": 0}, timeout=30)
row = r.json()["results"][0]
print(json.dumps(row, indent=1)[:1500])
