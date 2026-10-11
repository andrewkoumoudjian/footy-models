"""Precompute strictly as-of player aggregates + FM join for each backtest fixture.

For each of the last N CF Montréal matches, builds data/sofascore_cfm/bk/<mid>_players.csv
using ONLY lineups from matches strictly before that fixture's date.
"""
import csv, glob, json, os, sys, time, unicodedata, re
from collections import defaultdict

csv.field_size_limit(10_000_000)
OUT = "data/sofascore_cfm"
BK = f"{OUT}/bk"
os.makedirs(BK, exist_ok=True)

N = int(sys.argv[1]) if len(sys.argv) > 1 else 12
MIN_MINUTES = 0.0

# ---------- load match index ----------
matches = {}
with open(f"{OUT}/asof_team_stats.csv") as f:
    header = f.readline().strip().split(",")
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        matches[row["match_id"]] = row

cfm_fixtures = sorted(
    [r for r in matches.values() if "CF Montréal" in (r["home"], r["away"])],
    key=lambda r: r["date"],
)
targets = cfm_fixtures[-N:]
print(f"backtesting last {len(targets)} CFM fixtures "
      f"({targets[0]['date']} .. {targets[-1]['date']})")

TEAMS = {v: k for k, v in json.load(open(f"{OUT}/teams.json")).items()}

FIELDS = {
    "totalShots": "shots", "expectedGoals": "xg", "goalAssist": "assists",
    "expectedAssists": "xa", "totalPass": "passes", "accuratePass": "passes_acc",
    "ballCarriesCount": "carries", "progressiveBallCarriesCount": "prog_carries",
    "totalTackle": "tackles", "interception": "interceptions", "clearances": "clearances",
}

def norm(name):
    text = unicodedata.normalize("NFKD", name or "")
    text = "".join(ch for ch in text if not unicodedata.combining(ch))
    text = re.sub(r"[^a-z0-9 ]", " ", text.lower())
    return " ".join(text.split())

# ---------- FM index once ----------
fm_index = {}
FM_PATH = "/home/drewclaw/fm26-data/fm26_final_men.csv"
with open(FM_PATH) as fh:
    header = fh.readline().strip().split(",")
    idx_name, idx_ca = header.index("name"), header.index("ca")
    attr_cols = [c for c in header if c.startswith("attr_")]
    col_idx = {c: header.index(c) for c in attr_cols}
    for line in fh:
        row = line.strip().split(",")
        if len(row) <= idx_ca:
            continue
        try:
            ca = float(row[idx_ca] or 0)
        except ValueError:
            continue
        key = norm(row[idx_name])
        if key not in fm_index or ca > fm_index[key]["ca"]:
            fm_index[key] = {"ca": ca, **{c: row[col_idx[c]] for c in attr_cols}}
print("FM index:", len(fm_index))

KEY_ATTRS = {
    "attr_Finishing": "finishing", "attr_Composure": "composure",
    "attr_Passing": "passing", "attr_Technique": "technique",
    "attr_Tackling": "tackling", "attr_Marking": "marking",
    "attr_Consistency": "consistency", "attr_Dirtiness": "dirtiness",
}

def agg_players(cutoff_date):
    players = defaultdict(lambda: {"matches": 0, "starts": 0, "minutes": 0.0})
    # Selection behavior is club-specific. Keep this separate from the all-history performance
    # aggregate so a transfer does not make starts for the old club inflate the new club prior.
    selection = defaultdict(lambda: {
        "listings": 0, "appearances": 0, "starts": 0, "minutes": 0.0, "start_minutes": 0.0,
        "first_date": None,
    })
    team_fixture_dates = defaultdict(list)
    last_listed = {}
    last_appearance = {}
    used = 0
    for mid, row in matches.items():
        if row["date"] >= cutoff_date:
            continue
        match_date = row["date"]
        path = f"{OUT}/player_matches/{mid}.json"
        if not os.path.exists(path):
            continue
        d = json.load(open(path))
        used += 1
        home_id, away_id = int(row["home_id"]), int(row["away_id"])
        team_fixture_dates[home_id].append(match_date)
        team_fixture_dates[away_id].append(match_date)
        for side in ("home", "away"):
            block = d.get(side)
            if not block:
                continue
            for entry in block.get("players", []):
                pid = entry["player"]["id"]
                st = entry.get("statistics") or {}
                minutes = float(st.get("minutesPlayed") or 0)
                agg = players[pid]
                agg.setdefault("name", entry["player"]["name"])
                agg.setdefault("pos", entry.get("position") or entry["player"].get("position"))
                team_id = home_id if side == "home" else away_id
                if pid not in last_listed or match_date > last_listed[pid]:
                    last_listed[pid] = match_date
                    # A matchday listing is stronger roster-membership evidence than waiting for a
                    # player to receive minutes. This keeps unused substitutes and recent arrivals
                    # in the causal candidate pool while still fitting performance rates on minutes.
                    agg["team_id"] = team_id
                sel = selection[(pid, team_id)]
                if sel["first_date"] is None or match_date < sel["first_date"]:
                    sel["first_date"] = match_date
                sel["listings"] += 1
                if minutes <= 0:
                    continue
                if pid not in last_appearance or match_date > last_appearance[pid]:
                    last_appearance[pid] = match_date
                agg["matches"] += 1
                started = not entry.get("substitute")
                if started:
                    agg["starts"] += 1
                agg["minutes"] += minutes
                sel["appearances"] += 1
                sel["minutes"] += minutes
                if started:
                    sel["starts"] += 1
                    sel["start_minutes"] += minutes
                for src, dst in FIELDS.items():
                    v = st.get(src)
                    if v is None:
                        continue
                    try:
                        agg[dst] = agg.get(dst, 0.0) + float(v)
                    except (TypeError, ValueError):
                        pass
    rows = []
    for pid, agg in players.items():
        m90 = max(agg["minutes"] / 90.0, 0.1)
        team_id = agg.get("team_id")
        sel = selection.get((pid, team_id), {})
        first_current_club_date = sel.get("first_date")
        current_club_fixtures = sum(
            1 for fixture_date in team_fixture_dates.get(team_id, [])
            if first_current_club_date is not None and fixture_date >= first_current_club_date
        )
        out = {
            "player_id": pid, "name": agg.get("name"),
            "team": TEAMS.get(team_id), "pos": agg.get("pos"),
            "matches": agg["matches"], "minutes": round(agg["minutes"]),
            "first_listed_date": first_current_club_date or "",
            "last_listed_date": last_listed.get(pid, ""),
            "last_appearance_date": last_appearance.get(pid, ""),
            "selection_listings": sel.get("listings", 0),
            "selection_appearances": sel.get("appearances", 0),
            "selection_starts": sel.get("starts", 0),
            "selection_minutes": round(sel.get("minutes", 0.0)),
            "selection_start_minutes": round(sel.get("start_minutes", 0.0)),
            "team_fixtures": current_club_fixtures,
        }
        for field in ["shots", "xg", "xa", "passes", "passes_acc", "prog_carries",
                      "tackles", "interceptions", "clearances"]:
            out[f"{field}_p90"] = round(agg.get(field, 0.0) / m90, 3)
        rows.append(out)
    return used, rows, last_listed, last_appearance

for t in targets:
    mid, date = t["match_id"], t["date"]
    dest = f"{BK}/{mid}_players.csv"
    if os.path.exists(dest):
        os.remove(dest)
    used, rows, last_listed, last_appearance = agg_players(date)
    # Squad freshness is based on being listed in a covered matchday payload, not receiving minutes.
    # That preserves unused substitutes while still aging out sold/inactive players.
    from datetime import date as D

    def days_between(a, b):
        ya, ma, da = map(int, a.split("-"))
        yb, mb, db = map(int, b.split("-"))
        return (D(yb, mb, db) - D(ya, ma, da)).days

    qualified = [
        r for r in rows
        if r["minutes"] >= MIN_MINUTES and r["team"]
        and r["player_id"] in last_listed
        and days_between(last_listed[r["player_id"]], date) <= 45
    ]
    for r in qualified:
        pid = r["player_id"]
        r["days_since_last_listed"] = days_between(last_listed[pid], date)
        # For a listed player with no appearance yet, use time since first current-club listing;
        # prior_appearances=0 tells the model this is a cold-start rather than a recent appearance.
        appearance_anchor = last_appearance.get(pid) or r.get("first_listed_date") or last_listed[pid]
        r["days_since_last_appearance"] = days_between(appearance_anchor, date)
    joined = 0
    cols = list(qualified[0].keys()) if qualified else ["player_id"]
    fm_cols = [f"fm_{d}" for d in KEY_ATTRS.values()] + ["fm_ca", "fm_matched"]
    cols += fm_cols
    for r in qualified:
        hit = fm_index.get(norm(r["name"]))
        r["fm_matched"] = 1 if hit else 0
        if hit:
            joined += 1
            r["fm_ca"] = round(hit["ca"])
            for src, dst in KEY_ATTRS.items():
                try:
                    r[f"fm_{dst}"] = float(hit[src])
                except (TypeError, ValueError):
                    r[f"fm_{dst}"] = ""
        else:
            for dst in KEY_ATTRS.values():
                r[f"fm_{dst}"] = ""
            r["fm_ca"] = ""
    with open(dest, "w") as f:
        f.write(",".join(cols) + "\n")
        for r in qualified:
            f.write(",".join(str(r.get(c, "")) for c in cols) + "\n")
    json.dump({"match_id": mid, "date": date}, open(f"{BK}/{mid}_meta.json", "w"))
    print(f"  {mid} ({date}): {used} prior matches -> {len(qualified)} players, {joined} FM-matched")

json.dump([{"match_id": t["match_id"], "date": t["date"],
            "home": t["home"], "away": t["away"],
            "home_goals": t["home_goals"], "away_goals": t["away_goals"]}
           for t in targets], open(f"{BK}/fixtures.json", "w"), indent=1)
print("done")
