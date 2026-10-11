"""Extract opponent team stats into the same TSV format, then build the as-of calibration table.

Produces data/sofascore_cfm/asof_team_stats.csv: one row per (team, match) with metrics,
covering CFM + the three upcoming opponents.
"""
import json, time

OUT = "data/sofascore_cfm"
KEYMAP = {
    "expectedGoals": "xg",
    # SofaScore's historical key names are counter-intuitive: `totalShotsOnGoal` is the
    # displayed "Total shots" statistic, while `shotsOnGoal` is the displayed "Shots on target".
    "totalShotsOnGoal": "shots",
    "shotsOnGoal": "sot",
    "totalShotsOffGoal": "shots_off",
    "shotsOffGoal": "shots_off",
    "blockedShots": "shots_blocked",
    "blockedScoringAttempt": "shots_blocked",
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
    "goalkeeperSaves": "gk_saves",
}

def extract(mid):
    try:
        d = json.load(open(f"{OUT}/teamstats/{mid}.json"))
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
                def parse(x):
                    if isinstance(x, (int, float)):
                        return float(x)
                    try:
                        return float(str(x).replace("%", ""))
                    except Exception:
                        return None
                hv = item.get("homeValue", parse(item.get("home")))
                av = item.get("awayValue", parse(item.get("away")))
                if isinstance(hv, str):
                    hv = parse(hv)
                if isinstance(av, str):
                    av = parse(av)
                if hv is not None and av is not None:
                    stats[key] = (hv, av)
    return stats or None

rows = []
sources = [json.load(open(f"{OUT}/history_manifest.json"))["events"]]
opp = json.load(open(f"{OUT}/opponent_history_manifest.json"))
for _, events in opp.items():
    sources.append([e for e in events if e.get("has_stats")])

seen = set()
for events in sources:
    for e in events:
        mid = e["id"]
        if mid in seen:
            continue
        seen.add(mid)
        st = extract(mid)
        if not st:
            continue
        date = time.strftime("%Y-%m-%d", time.gmtime(e["startTimestamp"]))
        hg, ag = e.get("homeScore", {}).get("current"), e.get("awayScore", {}).get("current")
        rows.append({
            "match_id": mid, "date": date,
            "home": e["homeTeam"]["name"], "home_id": e["homeTeam"]["id"],
            "away": e["awayTeam"]["name"], "away_id": e["awayTeam"]["id"],
            "home_goals": hg, "away_goals": ag, **{f"home_{k}": v[0] for k, v in st.items()},
            **{f"away_{k}": v[1] for k, v in st.items()},
        })

rows.sort(key=lambda r: r["date"])
cols = sorted({c for r in rows for c in r})
with open(f"{OUT}/asof_team_stats.csv", "w") as f:
    f.write(",".join(cols) + "\n")
    for r in rows:
        f.write(",".join(str(r.get(c, "")) for c in cols) + "\n")

# per-team summary
from collections import defaultdict
team_rows = defaultdict(list)
for r in rows:
    team_rows[r["home"]].append(("home", r))
    team_rows[r["away"]].append(("away", r))

print(f"{len(rows)} unique matches with stats")
print("\n=== as-of 2026-08-23 team windows ===")
for name in sorted(team_rows):
    side_rows = team_rows[name]
    if len(side_rows) < 4:
        continue
    gf = [float(r["home_goals"]) if s == "home" else float(r["away_goals"]) for s, r in side_rows]
    ga = [float(r["away_goals"]) if s == "home" else float(r["home_goals"]) for s, r in side_rows]
    xg = [float(r[f"{s}_xg"]) for s, r in side_rows]
    sot = [float(r[f"{s}_sot"]) for s, r in side_rows]
    poss = [float(r[f"{s}_possession"]) for s, r in side_rows]
    n = len(gf)
    print(f"{name:<22} n={n:>2} GF/m={sum(gf)/n:.2f} GA/m={sum(ga)/n:.2f} xG/m={sum(xg)/n:.2f} xGA~ SOT/m={sum(sot)/n:.1f} poss%={sum(poss)/n:.0f}")
