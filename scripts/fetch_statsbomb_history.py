import json, os, urllib.request

BASE = "https://raw.githubusercontent.com/statsbomb/open-data/master/data"
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "statsbomb")

def fetch(path, dest):
    dest = os.path.join(ROOT, dest)
    if os.path.exists(dest) and os.path.getsize(dest) > 100:
        return dest
    url = f"{BASE}/{path}"
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=60) as r:
                data = r.read()
            with open(dest, "wb") as f:
                f.write(data)
            return dest
        except Exception as e:
            if attempt == 2:
                raise
    return None

matches = json.load(open(fetch("matches/2/27.json", "matches/pl_2015_16.json")))
teams = {}
for m in matches:
    teams[m["home_team"]["home_team_id"]] = m["home_team"]["home_team_name"]
    teams[m["away_team"]["away_team_id"]] = m["away_team"]["away_team_name"]

targets = {i: n for i, n in teams.items() if n in ("Manchester United", "AFC Bournemouth")}
print("target teams:", targets)

# Fixture to forecast: final day 2016-05-17 Man Utd vs Bournemouth
fixture = [m for m in matches if m["match_date"] >= "2016-05-16"
           and m["home_team"]["home_team_id"] in targets
           and m["away_team"]["away_team_id"] in targets]
assert len(fixture) == 1, fixture
fx = fixture[0]
print("fixture:", fx["match_date"], fx["home_team"]["home_team_name"], "vs",
      fx["away_team"]["away_team_name"], fx["match_id"])

HOME_ID, AWAY_ID = fx["home_team"]["home_team_id"], fx["away_team"]["away_team_id"]

# All PRIOR season fixtures involving either club -> history for calibration (strictly as-of)
prior = [m for m in sorted(matches, key=lambda x: x["match_date"])
         if m["match_date"] < fx["match_date"]
         and (m["home_team"]["home_team_id"] in targets or m["away_team"]["away_team_id"] in targets)]
print(len(prior), "prior fixtures")

manifest = {"fixture": {"match_id": fx["match_id"], "date": fx["match_date"],
                        "home": fx["home_team"]["home_team_name"], "away": fx["away_team"]["away_team_name"],
                        "home_id": HOME_ID, "away_id": AWAY_ID,
                        "actual_home_goals": fx["home_score"], "actual_away_goals": fx["away_score"]},
            "history": []}

for m in prior:
    mid = m["match_id"]
    ev = fetch(f"events/{mid}.json", f"events/{mid}.json")
    lu = fetch(f"lineups/{mid}.json", f"lineups/{mid}.json")
    manifest["history"].append({
        "match_id": mid, "date": m["match_date"],
        "competition": m["competition"]["competition_name"],
        "season": m["season"]["season_name"],
        "home": m["home_team"]["home_team_name"], "away": m["away_team"]["away_team_name"],
        "home_id": m["home_team"]["home_team_id"], "away_id": m["away_team"]["away_team_id"],
        "home_goals": m["home_score"], "away_goals": m["away_score"],
        "events_file": os.path.basename(ev), "lineups_file": os.path.basename(lu),
        "home_lineup_available": m.get("last_updated") is not None,
    })

json.dump(manifest, open(os.path.join(ROOT, "manifest.json"), "w"), indent=1)
print("cached", len(manifest["history"]), "historical matches")
