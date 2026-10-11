"""Audit: does any player in our SofaScore squads appear in FM26 under a DIFFERENT club?

FM26 (fm26_final_men.csv) has NO club column, so a stale-club assignment cannot enter the
pipeline through attributes - the join is name-keyed and club-blind. The real staleness risks:
  1. Player left the team -> still has minutes from OLD matches in the as-of window.
     (Handled by walk-forward cutoffs; for FUTURE forecasts we must check current squad.)
  2. Name-match collision: FM26 row belongs to a different person with the same name.

This script quantifies risk (1) by comparing each squad's most recent appearance date, and
risk (2) by listing ambiguous names (multiple FM26 players sharing a normalized name).
"""
import csv, json, glob, os, re, unicodedata
from collections import defaultdict

csv.field_size_limit(10_000_000)
OUT = "data/sofascore_cfm"

def norm(name):
    text = unicodedata.normalize("NFKD", name or "")
    text = "".join(ch for ch in text if not unicodedata.combining(ch))
    text = re.sub(r"[^a-z0-9 ]", " ", text.lower())
    return " ".join(text.split())

# ---- Risk 2: FM26 name collisions among our matched players -------------------------------
fm_counts = defaultdict(int)
with open("/home/drewclaw/fm26-data/fm26_final_men.csv") as f:
    header = f.readline().strip().split(",")
    idx_name = header.index("name")
    for line in f:
        parts = line.strip().split(",")
        if len(parts) > idx_name:
            fm_counts[norm(parts[idx_name])] += 1

players = list(csv.DictReader(open(f"{OUT}/player_fm_join.csv")))
matched = [p for p in players if p.get("fm_matched") == "1"]
collisions = [p["name"] for p in matched if fm_counts.get(norm(p["name"]), 0) > 1]
print(f"FM-matched players: {len(matched)}")
print(f"ambiguous name matches (>1 FM26 player with that name): {len(collisions)}")
for name in sorted(collisions):
    print("  COLLISION:", name)

# ---- Risk 1: stale squad members (no appearance in last 60 days of data) -------------------
# Latest match date per team in the as-of table
last_date = {}
with open(f"{OUT}/asof_team_stats.csv") as f:
    header = f.readline().strip().split(",")
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        for side in ("home", "away"):
            team = row[side]
            if team not in last_date or row["date"] > last_date[team]:
                last_date[team] = row["date"]

# Last appearance per player from lineup files
last_seen = {}
match_dates = {}
with open(f"{OUT}/asof_team_stats.csv") as f:
    header = f.readline().strip().split(",")
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        match_dates[row["match_id"]] = row["date"]

for path in glob.glob(f"{OUT}/player_matches/*.json"):
    mid = os.path.basename(path)[:-5]
    date = match_dates.get(mid)
    if not date:
        continue
    try:
        d = json.load(open(path))
    except Exception:
        continue
    for side in ("home", "away"):
        block = d.get(side) or {}
        for entry in block.get("players", []):
            st = entry.get("statistics") or {}
            if float(st.get("minutesPlayed") or 0) <= 0:
                continue
            pid = entry["player"]["id"]
            if pid not in last_seen or date > last_seen[pid][0]:
                last_seen[pid] = (date, entry["player"]["name"])

TEAMS = set(json.load(open(f"{OUT}/teams.json")))
stale = []
for p in players:
    team = p.get("team") or ""
    if team not in TEAMS or team == "None":
        continue
    try:
        pid = int(p["player_id"])
    except ValueError:
        continue
    seen = last_seen.get(pid)
    cutoff = last_date.get(team, "9999")
    # stale = hasn't appeared within 45 days of the team's latest fixture in data
    if seen is None:
        continue
    from datetime import date as D
    y1, m1, d1 = map(int, seen[0].split("-"))
    y2, m2, d2 = map(int, cutoff.split("-"))
    gap = (D(y2, m2, d2) - D(y1, m1, d1)).days
    if gap > 45:
        stale.append((team, p["name"], seen[0], cutoff, gap))

print(f"\nsquad members with no appearance in final 45 days of window (transfer/out candidates): {len(stale)}")
for team, name, seen, cutoff, gap in sorted(stale):
    print(f"  {team:<24} {name:<24} last={seen} window_end={cutoff} gap={gap}d")

json.dump({"ambiguous_names": collisions,
           "stale": [{"team": t, "name": n, "last": s, "window_end": c, "gap_days": g}
                     for t, n, s, c, g in stale]},
          open(f"{OUT}/squad_staleness_audit.json", "w"), indent=1)
print("\nwrote squad_staleness_audit.json")
