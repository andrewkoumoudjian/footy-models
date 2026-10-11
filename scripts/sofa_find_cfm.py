"""Find CF Montréal's upcoming fixtures via SofaScore (curl_cffi, chrome impersonation)."""
from curl_cffi import requests
import json

s = requests.Session(impersonate="chrome")

# 1. search team
r = s.get("https://api.sofascore.com/api/v1/search/all?q=CF%20Montreal", timeout=30)
data = r.json()
teams = [t for t in data.get("results", []) if t.get("type") == "team"]
for t in teams[:5]:
    ent = t.get("entity", {})
    print("team:", ent.get("id"), ent.get("name"), ent.get("sport"), ent.get("country", {}).get("name") if isinstance(ent.get("country"), dict) else "")
json.dump(data, open("/tmp/sofa_search.json", "w"))
