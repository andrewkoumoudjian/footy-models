#!/usr/bin/env python3
"""Join FM26 attributes with Transfermarkt features (name + DOB)."""
import duckdb, os, re, unicodedata, datetime
import pandas as pd

OUT = "/home/drewclaw/footy-models/data"
FM_CSV = "/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"
con = duckdb.connect("/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb", read_only=True)

def norm_name(s):
    if not s or (isinstance(s, float)): return ""
    s = unicodedata.normalize("NFKD", str(s))
    s = "".join(c for c in s if not unicodedata.combining(c))
    s = re.sub(r"[^a-z0-9]+", " ", s.lower()).strip()
    return s

def norm_dob(d):
    if not d or (isinstance(d, float)): return None
    if isinstance(d, datetime.datetime): return d.date()
    if isinstance(d, datetime.date): return d
    s = str(d).split(" ")[0]
    try:
        return datetime.date.fromisoformat(s)
    except Exception:
        return None

# 1) TM active players + latest valuation
print("loading TM...")
players = con.execute("""
    SELECT player_id, name, date_of_birth, position, sub_position, foot, height_in_cm,
           current_club_id, current_club_name, current_club_domestic_competition_id,
           last_season, international_caps, international_goals, contract_expiration_date
    FROM players WHERE CAST(last_season AS INTEGER) >= 2025
""").df()
print("active players:", len(players))

vals = con.execute("""
    WITH latest AS (
        SELECT player_id, MAX(date) AS d FROM player_valuations GROUP BY player_id
    )
    SELECT pv.player_id, pv.market_value_in_eur AS current_mv, pv.date AS latest_val_date,
           pv.current_club_name AS val_club
    FROM player_valuations pv JOIN latest l ON pv.player_id = l.player_id AND pv.date = l.d
""").df()
players = players.merge(vals, on="player_id", how="left")

# momentum (6m / 12m before the latest valuation date)
mom = con.execute("""
    WITH back6 AS (
        SELECT pv.player_id, pv.market_value_in_eur AS mv_6m
        FROM player_valuations pv
        JOIN (SELECT player_id, MAX(date) d FROM player_valuations WHERE date <= DATE '2025-12-12' GROUP BY player_id) b
          ON pv.player_id = b.player_id AND pv.date = b.d
    ), back12 AS (
        SELECT pv.player_id, pv.market_value_in_eur AS mv_12m
        FROM player_valuations pv
        JOIN (SELECT player_id, MAX(date) d FROM player_valuations WHERE date <= DATE '2025-06-12' GROUP BY player_id) b
          ON pv.player_id = b.player_id AND pv.date = b.d
    )
    SELECT b6.player_id, b6.mv_6m, b12.mv_12m
    FROM back6 b6 LEFT JOIN back12 b12 ON b6.player_id = b12.player_id
""").df()
players = players.merge(mom, on="player_id", how="left")

# valuation history stats
hist = con.execute("""
    SELECT player_id, COUNT(*) AS n_valuations, MIN(date) AS first_val_date, MAX(market_value_in_eur) AS peak_mv
    FROM player_valuations GROUP BY player_id
""").df()
players = players.merge(hist, on="player_id", how="left")

# 2) appearances aggregated: career + 24/25 + 25/26 seasons
apps = con.execute("""
    SELECT player_id,
           COUNT(*) FILTER (WHERE date >= '2023-07-01') AS apps_2324,
           SUM(goals)   FILTER (WHERE date >= '2023-07-01') AS goals_2324,
           SUM(assists) FILTER (WHERE date >= '2023-07-01') AS assists_2324,
           SUM(minutes_played) FILTER (WHERE date >= '2023-07-01') AS mins_2324,
           COUNT(*) FILTER (WHERE date >= '2024-07-01') AS apps_2425,
           SUM(goals)   FILTER (WHERE date >= '2024-07-01') AS goals_2425,
           SUM(assists) FILTER (WHERE date >= '2024-07-01') AS assists_2425,
           SUM(minutes_played) FILTER (WHERE date >= '2024-07-01') AS mins_2425,
           COUNT(*) FILTER (WHERE date >= '2025-07-01') AS apps_2526,
           SUM(goals)   FILTER (WHERE date >= '2025-07-01') AS goals_2526,
           SUM(assists) FILTER (WHERE date >= '2025-07-01') AS assists_2526,
           SUM(minutes_played) FILTER (WHERE date >= '2025-07-01') AS mins_2526
    FROM appearances GROUP BY player_id
""").df()
players = players.merge(apps, on="player_id", how="left")

# 3) most recent transfer
tr = con.execute("""
    SELECT player_id, transfer_date, transfer_season, from_club_name, to_club_name,
           transfer_fee, market_value_in_eur AS mv_at_transfer
    FROM (
        SELECT *, ROW_NUMBER() OVER (PARTITION BY player_id ORDER BY transfer_date DESC) rn
        FROM transfers
    ) WHERE rn = 1
""").df()
players = players.merge(tr, on="player_id", how="left")

# 4) clubs + competitions (league level)
clubs = con.execute("SELECT club_id, name AS club_name, domestic_competition_id FROM clubs").df()
comps = con.execute("SELECT competition_id, name AS competition_name, country_name, confederation, sub_type, type FROM competitions").df()
players = players.merge(clubs, left_on="current_club_id", right_on="club_id", how="left")
players = players.merge(comps, left_on="current_club_domestic_competition_id", right_on="competition_id", how="left")

players.to_csv(f"{OUT}/tm_active_features.csv", index=False)
print("tm active features:", players.shape)

# 5) FM men (name, dob, ca, pa, attrs)
print("loading FM...")
fm = pd.read_csv(FM_CSV, low_memory=False, usecols=lambda c: c.startswith("attr_") or c in ("uid","name","dob","ca","pa","height","attr_source","gender","kind"))
fm = fm[fm["gender"] == "male"].copy()
fm["name_n"] = fm["name"].apply(norm_name)
fm["dob_d"] = fm["dob"].apply(norm_dob)
fm = fm.dropna(subset=["name_n"])
print("fm men:", len(fm))

# 6) join
tm = players.copy()
tm["name_n"] = tm["name"].apply(norm_name)
tm["dob_d"] = tm["date_of_birth"].apply(norm_dob)
joined = tm.merge(fm, left_on=["name_n","dob_d"], right_on=["name_n","dob_d"], how="inner", suffixes=("","_fm"))
print("joined (name+dob exact):", len(joined))

# name-only fallback for unmatched TM players with high value (unique names)
unmatched = tm[~tm["player_id"].isin(joined["player_id"])].copy()
unmatched = unmatched[unmatched["current_mv"] >= 300000]
by_name = unmatched.merge(fm, on="name_n", how="inner", suffixes=("","_fm"))
# dedupe: keep best match by name length closeness
by_name["name_len_diff"] = (by_name["name"].str.len() - by_name["name_fm"].str.len()).abs()
by_name = by_name.sort_values("name_len_diff").drop_duplicates("player_id")
joined2 = pd.concat([joined, by_name], ignore_index=True)
print("joined (+name-only):", len(joined2))

joined2.to_csv(f"{OUT}/fm_tm_joined.csv", index=False)
print("saved fm_tm_joined.csv")
print("MV distribution of joined (current_mv):")
print(joined2["current_mv"].describe())
print("in 400k-1M:", ((joined2["current_mv"] >= 400000) & (joined2["current_mv"] <= 1000000)).sum())
