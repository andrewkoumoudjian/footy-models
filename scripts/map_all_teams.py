"""Add all teams appearing in asof_team_stats to teams.json so aggregates cover every opponent."""
import csv, json
from curl_cffi import requests

s = requests.Session(impersonate="chrome")

teams = json.load(open("data/sofascore_cfm/teams.json"))

names = set()
with open("data/sofascore_cfm/asof_team_stats.csv") as f:
    header = f.readline().strip().split(",")
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        names.add(row["home"])
        names.add(row["away"])

for name in sorted(names):
    if name in teams:
        continue
    r = s.get("https://api.sofascore.com/api/v1/search/all", params={"q": name}, timeout=30)
    found = None
    for res in r.json().get("results", []):
        if res.get("type") == "team":
            e = res["entity"]
            if e.get("sport", {}).get("slug") == "football" and e["name"] == name:
                found = e["id"]
                break
    if found:
        teams[name] = found
        print(f"{name} -> {found}")
    else:
        print(f"{name} -> NOT FOUND")

json.dump(teams, open("data/sofascore_cfm/teams.json", "w"), indent=1)
print(len(teams), "teams mapped")
