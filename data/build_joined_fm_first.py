#!/usr/bin/env python3
"""FM-first data prep: FM26 men = universe; Transfermarkt = market overlay.
Produces:
- data/fm_universe.csv      : all FM men with CA/PA + derived development signals
- data/fm_tm_joined.csv     : FM x TM joined (expanded matching) + FM-derived features
"""
import duckdb, os, re, unicodedata, datetime
import pandas as pd
import numpy as np

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
    try: return datetime.date.fromisoformat(s)
    except Exception: return None

# ---------- FM universe ----------
print("loading FM...")
fm = pd.read_csv(FM_CSV, low_memory=False)
fm = fm[fm["gender"] == "male"].copy()
fm["name_n"] = fm["name"].apply(norm_name)
fm["dob_d"] = fm["dob"].apply(norm_dob)
fm["birth_year"] = fm["dob_d"].apply(lambda d: d.year if d else None)
fm["age"] = fm["dob_d"].apply(lambda d: (datetime.date(2026,6,12) - d).days / 365.25 if d else None)

# FM-derived features
GK = ["Handling","AerialReach","CommandOfArea","Communication","Kicking","Throwing","OneOnOnes","Reflexes","RushingOut","Punching","Eccentricity"]
def pos_cls(r):
    if pd.isna(r.get("ca")): return None
    gk = sum(r.get("attr_"+a) or 0 for a in GK)
    if gk >= 60: return "GK"
    fin = (r.get("attr_Finishing") or 0) + (r.get("attr_OffTheBall") or 0) + (r.get("attr_Anticipation") or 0)
    dfn = (r.get("attr_Tackling") or 0) + (r.get("attr_Marking") or 0) + (r.get("attr_Positioning") or 0)
    if fin >= dfn + 10: return "FW"
    if dfn >= fin + 8: return "DF"
    return "MF"
attr_cols = [c for c in fm.columns if c.startswith("attr_") and c != "attr_source"]
for c in attr_cols:
    fm[c] = pd.to_numeric(fm[c], errors="coerce")
fm["pos_cls"] = fm.apply(pos_cls, axis=1)
# composite signals (only for players with real/generated attrs)
def comp(r, cols):
    vals = [r.get("attr_"+c) for c in cols]
    vals = [v for v in vals if pd.notna(v)]
    return float(np.mean(vals)) if vals else np.nan
tech_cols = ["Crossing","Dribbling","Finishing","Heading","LongShots","Passing","Technique","FirstTouch"]
phys_cols = ["Acceleration","Agility","Balance","JumpingReach","Pace","Stamina","Strength","NaturalFitness"]
ment_cols = ["Anticipation","Bravery","Composure","Concentration","Decisions","Determination","Flair","Leadership","OffTheBall","Positioning","Teamwork","Vision","WorkRate","Aggression"]
def_c_cols = ["Tackling","Marking","Positioning","Concentration","Anticipation","Teamwork","WorkRate","Strength"]
att_cols = ["Finishing","LongShots","OffTheBall","Anticipation","Composure","Technique","Dribbling","Pace","Acceleration"]
cre_cols = ["Vision","Passing","FirstTouch","Dribbling","Technique","Flair","OffTheBall","Crossing","LongShots","FreeKicks","Corners"]
fm["sig_technical"] = fm.apply(lambda r: comp(r, tech_cols), axis=1)
fm["sig_physical"] = fm.apply(lambda r: comp(r, phys_cols), axis=1)
fm["sig_mental"] = fm.apply(lambda r: comp(r, ment_cols), axis=1)
fm["sig_defensive"] = fm.apply(lambda r: comp(r, def_c_cols), axis=1)
fm["sig_attacking"] = fm.apply(lambda r: comp(r, att_cols), axis=1)
fm["sig_creative"] = fm.apply(lambda r: comp(r, cre_cols), axis=1)
fm["ca"] = pd.to_numeric(fm["ca"], errors="coerce")
fm["pa"] = pd.to_numeric(fm["pa"], errors="coerce")
fm["pa_gap"] = fm["pa"] - fm["ca"]
# age-relative CA percentile within FM population (development signal)
fm["ca_age_pct"] = np.nan
for band in [(15,18),(18,21),(21,24),(24,28),(28,40)]:
    m = (fm["age"] >= band[0]) & (fm["age"] < band[1]) & fm["ca"].notna()
    fm.loc[m, "ca_age_pct"] = fm.loc[m, "ca"].rank(pct=True)
fm["attr_source"] = fm["attr_source"].fillna("")
keep = ["uid","name","name_n","dob_d","birth_year","age","ca","pa","pa_gap","ca_age_pct",
        "pos_cls","height","home_rep","cur_rep","world_rep","attr_source",
        "sig_technical","sig_physical","sig_mental","sig_defensive","sig_attacking","sig_creative"]
fm_out = fm[keep].copy()
fm_out.to_csv(f"{OUT}/fm_universe.csv", index=False)
print("fm universe:", fm_out.shape, "with CA:", fm_out["ca"].notna().sum())

# ---------- TM features (same as before) ----------
print("loading TM...")
players = con.execute("""
    SELECT player_id, name, date_of_birth, position, sub_position, foot, height_in_cm,
           current_club_id, current_club_name, current_club_domestic_competition_id,
           last_season, international_caps, international_goals, contract_expiration_date
    FROM players WHERE CAST(last_season AS INTEGER) >= 2025
""").df()
vals = con.execute("""
    WITH latest AS (SELECT player_id, MAX(date) AS d FROM player_valuations GROUP BY player_id)
    SELECT pv.player_id, pv.market_value_in_eur AS current_mv, pv.date AS latest_val_date,
           pv.current_club_name AS val_club
    FROM player_valuations pv JOIN latest l ON pv.player_id = l.player_id AND pv.date = l.d
""").df()
players = players.merge(vals, on="player_id", how="left")
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
    SELECT b6.player_id, b6.mv_6m, b12.mv_12m FROM back6 b6 LEFT JOIN back12 b12 ON b6.player_id = b12.player_id
""").df()
players = players.merge(mom, on="player_id", how="left")
hist = con.execute("""
    SELECT player_id, COUNT(*) AS n_valuations, MIN(date) AS first_val_date,
           MAX(market_value_in_eur) AS peak_mv, MIN(market_value_in_eur) AS min_mv
    FROM player_valuations GROUP BY player_id
""").df()
players = players.merge(hist, on="player_id", how="left")
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
tr = con.execute("""
    SELECT player_id, transfer_date, transfer_season, from_club_name, to_club_name,
           transfer_fee, market_value_in_eur AS mv_at_transfer
    FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY player_id ORDER BY transfer_date DESC) rn FROM transfers)
    WHERE rn = 1
""").df()
players = players.merge(tr, on="player_id", how="left")
clubs = con.execute("SELECT club_id, name AS club_name, domestic_competition_id FROM clubs").df()
comps = con.execute("SELECT competition_id, name AS competition_name, country_name, confederation, sub_type, type FROM competitions").df()
players = players.merge(clubs, left_on="current_club_id", right_on="club_id", how="left")
players = players.merge(comps, left_on="current_club_domestic_competition_id", right_on="competition_id", how="left")
players["name_n"] = players["name"].apply(norm_name)
players["dob_d"] = players["date_of_birth"].apply(norm_dob)
players["birth_year"] = players["dob_d"].apply(lambda d: d.year if d else None)
print("tm active:", players.shape)

# ---------- Join: FM first ----------
# pass 1: exact name + dob
j1 = fm_out.merge(players, on=["name_n","dob_d"], how="inner", suffixes=("","_tm"))
print("pass1 (name+dob):", j1.shape[0])
# pass 2: surname + dob (last token of name) for unmatched TM players
tm_un = players[~players["player_id"].isin(j1["player_id"])].copy()
tm_un["surname"] = tm_un["name_n"].apply(lambda s: s.split(" ")[-1] if s else "")
fm2 = fm_out.copy()
fm2["surname"] = fm2["name_n"].apply(lambda s: s.split(" ")[-1] if s else "")
j2 = tm_un.merge(fm2, on=["surname","dob_d"], how="inner", suffixes=("_tm",""))
# dedupe by player keeping best (shortest name mismatch)
j2 = j2.drop_duplicates("player_id", keep="first")
print("pass2 (surname+dob):", j2.shape[0])
# pass 3: name + birth_year (for unmatched, dob missing on one side)
tm_un2 = players[~players["player_id"].isin(j1["player_id"].tolist() + j2["player_id"].tolist())].copy()
j3 = tm_un2[tm_un2["birth_year"].notna()].merge(
    fm2[fm2["birth_year"].notna()], on=["name_n","birth_year"], how="inner", suffixes=("_tm",""))
j3 = j3.drop_duplicates("player_id", keep="first")
print("pass3 (name+birthyear):", j3.shape[0])

joined = pd.concat([j1, j2, j3], ignore_index=True)
# cleanup duplicate columns from merge suffixes
joined = joined.loc[:, ~joined.columns.duplicated()]
print("TOTAL JOINED:", joined.shape[0])
joined.to_csv(f"{OUT}/fm_tm_joined.csv", index=False)
print("saved fm_tm_joined.csv")
print("joined with current_mv:", joined["current_mv"].notna().sum())
print("in 400k-1M:", ((joined["current_mv"] >= 400000) & (joined["current_mv"] <= 1000000)).sum())
