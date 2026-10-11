p = 'scripts/prep_backtest.py'
s = open(p).read()

# Track last-appearance date per player during aggregation, and exclude players whose last
# appearance is >45 days before the fixture cutoff (transferred / injured / out of squad).
old = '''def agg_players(cutoff_date):
    players = defaultdict(lambda: {"matches": 0, "starts": 0, "minutes": 0.0})
    used = 0
    for mid, row in matches.items():
        if row["date"] >= cutoff_date:
            continue'''
new = '''def agg_players(cutoff_date):
    players = defaultdict(lambda: {"matches": 0, "starts": 0, "minutes": 0.0})
    last_appearance = {}
    used = 0
    for mid, row in matches.items():
        if row["date"] >= cutoff_date:
            continue
        match_date = row["date"]'''
assert old in s, "agg head"
s = s.replace(old, new)

old = '''                agg = players[pid]
                agg.setdefault("name", entry["player"]["name"])
                agg.setdefault("pos", entry.get("position") or entry["player"].get("position"))
                agg.setdefault("team_id", home_id if side == "home" else away_id)
                agg["matches"] += 1'''
new = '''                agg = players[pid]
                agg.setdefault("name", entry["player"]["name"])
                agg.setdefault("pos", entry.get("position") or entry["player"].get("position"))
                agg.setdefault("team_id", home_id if side == "home" else away_id)
                if pid not in last_appearance or match_date > last_appearance[pid]:
                    last_appearance[pid] = match_date
                agg["matches"] += 1'''
assert old in s, "appearance"
s = s.replace(old, new)

# Expose last_appearance to caller
old = '''        rows.append(out)
    return used, rows'''
new = '''        rows.append(out)
    return used, rows, last_appearance'''
assert old in s, "return"
s = s.replace(old, new)

old = '''    used, rows = agg_players(date)
    qualified = [r for r in rows if r["minutes"] >= MIN_MINUTES and r["team"]]'''
new = '''    used, rows, last_seen = agg_players(date)
    # Squad freshness: drop players not seen in the 45 days before the fixture (sold/injured/
    # out-of-favor). FM26 attributes are club-blind so this is the only staleness guard.
    from datetime import date as D

    def days_between(a, b):
        ya, ma, da = map(int, a.split("-"))
        yb, mb, db = map(int, b.split("-"))
        return (D(yb, mb, db) - D(ya, ma, da)).days

    qualified = [
        r for r in rows
        if r["minutes"] >= MIN_MINUTES and r["team"]
        and r["player_id"] in last_seen
        and days_between(last_seen[r["player_id"]], date) <= 45
    ]'''
assert old in s, "filter"
s = s.replace(old, new)

open(p, 'w').write(s)
print('prep_backtest staleness guard ok')
