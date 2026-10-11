"""Extract per-match team stat vectors from SofaScore statistics for CFM history.

Outputs data/sofascore_cfm/team_match_stats.tsv:
match_id date home_team away_team home_goals away_goals
  then per-side metric columns: shots, sot, xg, possession, passes, pass_acc, big_chances,
  corners, fouls, yellow, red, tackles, interceptions, clearances, aerials_won
"""
import json, time

OUT = "data/sofascore_cfm"
manifest = json.load(open(f"{OUT}/history_manifest.json"))["events"]

KEYMAP = {
    "expectedGoals": "xg",
    "totalShotsOnGoal": "sot",
    "totalShotsOffGoal": "shots_off",
    "blockedShots": "shots_blocked",
    "ballPossession": "possession",
    "totalPasses": "passes",
    "accuratePasses": "passes_acc",
    "bigChanceCreated": "big_chances",
    "cornerKicks": "corners",
    "fouls": "fouls",
    "yellowCards": "yellow",
    "redCards": "red",
    "totalTackles": "tackles",
    "interceptions": "interceptions",
    "clearances": "clearances",
    "aerialsWon": "aerials_won",
    "goals": "goals",
    "goalkeeperSaves": "gk_saves",
}

def extract(path):
    try:
        d = json.load(open(path))
    except Exception:
        return None
    stats = {}
    for group in d.get("statistics", []):
        if group.get("period") != "ALL":
            continue
        for g in group.get("groups", []):
            for item in g.get("statisticsItems", []):
                key = KEYMAP.get(item.get("key"))
                if not key:
                    continue
                hv = item.get("homeValue")
                av = item.get("awayValue")
                if hv is None or av is None:
                    # parse from strings like "62%"
                    def parse(x):
                        if isinstance(x, (int, float)):
                            return float(x)
                        try:
                            return float(str(x).replace("%", ""))
                        except Exception:
                            return None
                    hv, av = parse(item.get("home")), parse(item.get("away"))
                if hv is not None and av is not None:
                    stats[key] = (hv, av)
    return stats or None

rows = []
for e in manifest:
    mid = e["id"]
    st = extract(f"{OUT}/teamstats/{mid}.json")
    if not st:
        print("no stats for", mid)
        continue
    date = time.strftime("%Y-%m-%d", time.gmtime(e["startTimestamp"]))
    hg = e.get("homeScore", {}).get("current")
    ag = e.get("awayScore", {}).get("current")
    rows.append({
        "match_id": mid, "date": date,
        "home": e["homeTeam"]["name"], "away": e["awayTeam"]["name"],
        "home_id": e["homeTeam"]["id"], "away_id": e["awayTeam"]["id"],
        "home_goals": hg, "away_goals": ag, **{f"home_{k}": v[0] for k, v in st.items()},
        **{f"away_{k}": v[1] for k, v in st.items()},
    })

cols = sorted({c for r in rows for c in r})
with open(f"{OUT}/team_match_stats.tsv", "w") as f:
    f.write("\t".join(cols) + "\n")
    for r in rows:
        f.write("\t".join(str(r.get(c, "")) for c in cols) + "\n")
print("wrote", len(rows), "matches ->", f"{OUT}/team_match_stats.tsv")
