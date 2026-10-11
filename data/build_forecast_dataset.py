#!/usr/bin/env python3
"""Build the 12-month value-forecast training dataset and candidate feature set.

Training rows (strictly causal):
  For every Transfermarkt valuation observation (player, t, mv_t) with
  10k <= mv_t <= 1M:
    - features are computed only from observations strictly before t
      (valuations, appearances, transfers, player/club/competition state);
    - FM26 attributes are attached by Transfermarkt player_id (missing for
      most historical players -> NA + fm_present indicator);
    - outcome: log_ratio = log(mv_12m / mv_t) where mv_12m is the market
      value at the valuation observation closest to t+365 within (t, t+365];
    - row kept iff the player's valuation series extends >= 365d past t AND
      at least one valuation is observed in (t, t+365] (no censored outcomes).

Candidate rows:
  For every player in forecast_candidates.csv (current_mv <= 1M, FM attrs
  present, attr_source in {db, derived}), features are computed as-of the
  player's latest valuation date (the "now" of scoring).
"""
import numpy as np
import pandas as pd
import duckdb
import os, sys, datetime

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "forecast")
os.makedirs(OUT, exist_ok=True)
DB = "/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb"

MIN_MV, MAX_MV = 10_000, 1_000_000
HORIZON = 365
MAX_TRAIN_T = pd.Timestamp("2025-06-12")  # last valuation date in DB is 2026-06-12

# ---------------------------------------------------------------------------
# 1. Load source tables
# ---------------------------------------------------------------------------
con = duckdb.connect(DB, read_only=True)
vals = con.execute("""
    SELECT player_id, date, market_value_in_eur AS mv,
           player_club_domestic_competition_id AS comp
    FROM player_valuations ORDER BY player_id, date
""").df()
vals["date"] = pd.to_datetime(vals["date"])
print("valuations:", len(vals))

players = con.execute("""
    SELECT player_id, date_of_birth, position, sub_position, height_in_cm FROM players
""").df()
players["dob"] = pd.to_datetime(players["date_of_birth"])
players["height_in_cm"] = players["height_in_cm"].astype(float)

comps = con.execute(
    "SELECT competition_id, sub_type, confederation FROM competitions"
).df()

apps = con.execute(
    "SELECT player_id, date, goals, assists, minutes_played FROM appearances"
).df()
apps["date"] = pd.to_datetime(apps["date"])

tr = con.execute(
    "SELECT player_id, transfer_date, transfer_fee, market_value_in_eur AS mv_at_transfer FROM transfers"
).df()
tr["transfer_date"] = pd.to_datetime(tr["transfer_date"])
tr["transfer_fee"] = tr["transfer_fee"].astype(float)
con.close()

# ---------------------------------------------------------------------------
# 2. Per-player sorted arrays for fast as-of lookups
# ---------------------------------------------------------------------------
def as_of_arrays(df, date_col, value_cols):
    out = {}
    for pid, grp in df.groupby("player_id"):
        d = grp[date_col].values.astype("datetime64[D]").astype(np.int64)
        order = np.argsort(d, kind="stable")
        out[pid] = (d[order], {c: grp[c].values[order] for c in value_cols})
    return out

pval = as_of_arrays(vals, "date", ["mv", "comp"])
papps = as_of_arrays(apps, "date", ["goals", "assists", "minutes_played"])
ptrans = as_of_arrays(tr, "transfer_date", ["transfer_fee", "mv_at_transfer"])

pmeta = {}
for pid, r in players.set_index("player_id").iterrows():
    pmeta[pid] = {
        # days since Unix epoch as float (original script used Timestamp.value
        # nanoseconds, producing garbage ages of ~-1e15 for 99.9% of rows)
        "dob": (r["dob"].value / 86400e9) if not pd.isna(r["dob"]) else np.nan,
        "pos": r["position"] if isinstance(r["position"], str) else "Missing",
        "height": r["height_in_cm"] if not pd.isna(r["height_in_cm"]) else np.nan,
    }

comp_idx = comps.set_index("competition_id")
def comp_info(cid):
    if cid is None or pd.isna(cid) or cid not in comp_idx.index:
        return (np.nan, np.nan, 0.0)
    r = comp_idx.loc[cid]
    st = r["sub_type"]
    conf = r["confederation"]
    return (1.0 if st == "first_tier" else 0.0,
            1.0 if conf == "europa" else 0.0,
            1.0 if conf == "amerika" else 0.0)

# FM features keyed by TM player_id (from the FM<->TM join)
fmj = pd.read_csv(os.path.join(os.path.dirname(os.path.abspath(__file__)), "fm_tm_joined.csv"),
                  usecols=["player_id", "ca", "pa", "pa_gap", "ca_age_pct", "attr_source",
                           "sig_technical", "sig_physical", "sig_mental", "sig_defensive",
                           "sig_attacking", "sig_creative"])
fmj = fmj.drop_duplicates("player_id").set_index("player_id")
FM_COLS = ["ca", "pa", "pa_gap", "ca_age_pct", "sig_technical", "sig_physical",
           "sig_mental", "sig_defensive", "sig_attacking", "sig_creative"]
fm_mat = fmj[FM_COLS].values.astype(float)
fm_src = fmj["attr_source"].values
fm_ids = fmj.index.values
fm_index = {pid: i for i, pid in enumerate(fm_ids)}

# ---------------------------------------------------------------------------
# 3. Feature extractor (shared by training rows and candidates)
# ---------------------------------------------------------------------------
FEATURE_NAMES = [
    "log_mv", "log_mv_180d_ratio", "log_mv_365d_ratio", "log_peak_prior_ratio",
    "log_min_prior_ratio", "log_n_vals_prior", "log_days_since_first_val", "val_freq",
    "log_days_since_last_val", "last_change_dir", "mv_vol_log",
    "perf_present_730", "log_mins_365", "log_apps_365", "g_per90_365", "a_per90_365",
    "log_mins_180", "mins_trend", "g_per90_180",
    "ever_transferred", "log_days_since_transfer", "log_transfer_fee", "free_transfer",
    "log_mv_transfer_ratio",
    "age", "age_sq", "pos_gk", "pos_df", "pos_mf", "height_cm",
    "comp_mapped", "first_tier", "confed_uefa", "confed_sam",
    "fm_present", "fm_ca", "fm_pa", "fm_pa_gap", "fm_ca_age_pct",
    "fm_sig_technical", "fm_sig_physical", "fm_sig_mental", "fm_sig_defensive",
    "fm_sig_attacking", "fm_sig_creative", "fm_attr_db", "fm_attr_derived",
]
N_FEAT = len(FEATURE_NAMES)
print("features:", N_FEAT)

def extract_features(pid, t, mv_t, d, mv, comps_row, i):
    """Compute the feature vector for player `pid` at date `t` (days since epoch),
    where (d, mv, comps_row) is the player's full sorted valuation series and
    `i` is the index of the current observation (strictly-prior data = indices < i)."""
    f = {}
    # --- market state (strictly prior valuations) ---
    f["log_mv"] = np.log(mv_t)
    def mv_at(cut):
        c = np.searchsorted(d, t - cut, side="right")
        return mv[c - 1] if c > 0 else np.nan
    mv180, mv365 = mv_at(180), mv_at(365)
    f["log_mv_180d_ratio"] = np.log(mv180 / mv_t) if not np.isnan(mv180) else np.nan
    f["log_mv_365d_ratio"] = np.log(mv365 / mv_t) if not np.isnan(mv365) else np.nan
    if i > 0:
        f["log_peak_prior_ratio"] = np.log(mv[:i].max() / mv_t)
        f["log_min_prior_ratio"] = np.log(mv[:i].min() / mv_t)
        days_first = t - d[0]
        days_last = t - d[i - 1]
        last_mv = mv[i - 1]
        f["log_n_vals_prior"] = np.log1p(i)
        f["log_days_since_first_val"] = np.log1p(days_first)
        f["val_freq"] = i / max(1.0, days_first)
        f["log_days_since_last_val"] = np.log1p(days_last)
        f["last_change_dir"] = 1.0 if mv_t > last_mv else (-1.0 if mv_t < last_mv else 0.0)
        w = np.log(mv[max(0, i - 10):i])
        f["mv_vol_log"] = w.std() if len(w) >= 2 else np.nan
    else:
        f.update(log_peak_prior_ratio=np.nan, log_min_prior_ratio=np.nan,
                 log_n_vals_prior=0.0, log_days_since_first_val=np.nan,
                 val_freq=np.nan, log_days_since_last_val=np.nan,
                 last_change_dir=0.0, mv_vol_log=np.nan)
    # --- performance (appearances) ---
    ad, acols = papps.get(pid, (np.array([], dtype=np.int64), {}))
    i365 = np.searchsorted(ad, t - 365, side="right")
    i180 = np.searchsorted(ad, t - 180, side="right")
    i0 = np.searchsorted(ad, t, side="right")
    i730 = np.searchsorted(ad, t - 730, side="right")
    perf_present = 1.0 if (i0 - i730) > 0 else 0.0
    mins365 = acols["minutes_played"][i365:i0].sum() if i0 > i365 else 0.0
    apps365 = float(i0 - i365)
    g365 = acols["goals"][i365:i0].sum() if i0 > i365 else 0.0
    a365 = acols["assists"][i365:i0].sum() if i0 > i365 else 0.0
    mins180 = acols["minutes_played"][i180:i0].sum() if i0 > i180 else 0.0
    mins_prior180 = acols["minutes_played"][i365:i180].sum() if i180 > i365 else 0.0
    g180 = acols["goals"][i180:i0].sum() if i0 > i180 else 0.0
    f["perf_present_730"] = perf_present
    f["log_mins_365"] = np.log1p(mins365)
    f["log_apps_365"] = np.log1p(apps365)
    f["g_per90_365"] = g365 * 90.0 / mins365 if mins365 > 0 else (np.nan if perf_present else 0.0)
    f["a_per90_365"] = a365 * 90.0 / mins365 if mins365 > 0 else (np.nan if perf_present else 0.0)
    f["log_mins_180"] = np.log1p(mins180)
    f["mins_trend"] = np.log((mins180 + 1.0) / (mins_prior180 + 1.0))
    f["g_per90_180"] = g180 * 90.0 / mins180 if mins180 > 0 else (np.nan if perf_present else 0.0)
    # --- transfers (strictly before t) ---
    td, tcols = ptrans.get(pid, (np.array([], dtype=np.int64), {}))
    it = np.searchsorted(td, t, side="right")
    if it > 0:
        fee = tcols["transfer_fee"][it - 1]
        mvt = tcols["mv_at_transfer"][it - 1]
        free = 1.0 if (np.isnan(fee) or fee <= 0) else 0.0
        fee_ = 0.0 if free else fee
        f["ever_transferred"] = 1.0
        f["log_days_since_transfer"] = np.log1p(t - td[it - 1])
        f["log_transfer_fee"] = np.log1p(fee_)
        f["free_transfer"] = free
        f["log_mv_transfer_ratio"] = np.log(mv_t / mvt) if (not np.isnan(mvt) and mvt > 0) else np.nan
    else:
        f.update(ever_transferred=0.0, log_days_since_transfer=np.nan,
                 log_transfer_fee=np.nan, free_transfer=np.nan, log_mv_transfer_ratio=np.nan)
    # --- player state ---
    meta = pmeta.get(pid, {})
    dob = meta.get("dob", np.nan)
    age = (t - dob) / 365.25 if not np.isnan(dob) else np.nan
    # plausibility window (pro guards against any residual bad DOB records)
    if not np.isnan(age) and not (14.0 <= age <= 50.0):
        age = np.nan
    f["age"] = age
    f["age_sq"] = age * age if not np.isnan(age) else np.nan
    pos = meta.get("pos", "Missing")
    f["pos_gk"] = 1.0 if pos == "Goalkeeper" else 0.0
    f["pos_df"] = 1.0 if pos == "Defender" else 0.0
    f["pos_mf"] = 1.0 if pos == "Midfield" else 0.0
    f["height_cm"] = meta.get("height", np.nan)
    # --- competition (as-of observation) ---
    first_tier, uefa, sam = comp_info(comps_row[i])
    f["comp_mapped"] = 1.0 if comps_row[i] is not None and not pd.isna(comps_row[i]) else 0.0
    f["first_tier"] = first_tier
    f["confed_uefa"] = uefa
    f["confed_sam"] = sam
    # --- FM state (missing -> NA + indicator) ---
    if pid in fm_index:
        fi = fm_index[pid]
        f["fm_present"] = 1.0
        for j, name in enumerate(FM_COLS):
            v = fm_mat[fi, j]
            f["fm_" + name] = v if (not np.isnan(v) and v > -1) else np.nan
        src = fm_src[fi]
        f["fm_attr_db"] = 1.0 if src == "db" else 0.0
        f["fm_attr_derived"] = 1.0 if src == "derived" else 0.0
    else:
        f["fm_present"] = 0.0
        for name in FM_COLS:
            f["fm_" + name] = np.nan
        f["fm_attr_db"] = np.nan
        f["fm_attr_derived"] = np.nan
    return [f[n] for n in FEATURE_NAMES]

# ---------------------------------------------------------------------------
# 4. Build training rows
# ---------------------------------------------------------------------------
rows = []
n_kept_band = 0
n_no_followup = 0
n_truncated = 0
import time
t0 = time.time()
for pid, (d, cols) in pval.items():
    mv = cols["mv"]
    comps_row = cols["comp"]
    last_date = d[-1]
    for i in range(len(d)):
        if not (MIN_MV <= mv[i] <= MAX_MV):
            continue
        n_kept_band += 1
        t = d[i]
        mv_t = mv[i]
        j = np.searchsorted(d, t, side="right")
        k = np.searchsorted(d, t + HORIZON, side="right")
        if j >= k:
            n_no_followup += 1
            continue
        if last_date < t + HORIZON:
            n_truncated += 1
            continue
        # market value at the observation closest to t+365 within (t, t+365]
        m = j + int(np.argmin(np.abs(d[j:k] - (t + HORIZON))))
        mv_12m = mv[m]
        log_ratio = float(np.log(mv_12m / mv_t))
        feats = extract_features(pid, t, mv_t, d, mv, comps_row, i)
        rows.append(feats + [pid, t, mv_t, mv_12m, log_ratio, mv_12m / mv_t - 1.0])
        if len(rows) % 100000 == 0:
            print("  rows", len(rows), "elapsed", round(time.time() - t0, 1))
print("in-band obs:", n_kept_band, "no follow-up:", n_no_followup,
      "truncated window:", n_truncated, "kept:", len(rows))

train = pd.DataFrame(rows, columns=FEATURE_NAMES + ["player_id", "t", "mv_t", "mv_12m", "log_ratio", "realized_return"])
print("mean log_ratio:", train["log_ratio"].mean(), "median:", train["log_ratio"].median(), "rows:", len(train))
train.to_csv(os.path.join(OUT, "train_rows.csv"), index=False)
# TSV with NA for Rust
train.to_csv(os.path.join(OUT, "train.tsv"), index=False, sep="\t", na_rep="NA")
print("wrote", os.path.join(OUT, "train.tsv"))

# quick summary by year
print(train.groupby(pd.to_datetime(train["t"], unit="D").dt.year)["log_ratio"].agg(["count", "mean"]).tail(12))

# ---------------------------------------------------------------------------
# 5. Candidate rows (as-of each candidate's latest valuation date)
# ---------------------------------------------------------------------------
cand = pd.read_csv(os.path.join(os.path.dirname(os.path.abspath(__file__)), "forecast_candidates.csv"))
cand = cand.drop_duplicates("player_id", keep="first").sort_values("player_id")
print("candidates:", len(cand))

crow = []
cmeta = []
n_no_series = 0
for pid in cand["player_id"]:
    if pid not in pval:
        n_no_series += 1
        continue
    d, cols = pval[pid]
    mv = cols["mv"]
    comps_row = cols["comp"]
    i = len(d) - 1
    t = d[i]
    mv_t = mv[i]
    feats = extract_features(pid, t, mv_t, d, mv, comps_row, i)
    crow.append(feats + [pid, t, mv_t])
print("candidates without valuation series:", n_no_series, "kept:", len(crow))

candf = pd.DataFrame(crow, columns=FEATURE_NAMES + ["player_id", "t", "mv_t"])
candf.to_csv(os.path.join(OUT, "candidates.tsv"), index=False, sep="\t", na_rep="NA")
print("wrote", os.path.join(OUT, "candidates.tsv"), candf.shape)

# candidate metadata for proposals (FM quality-gate attributes joined by uid)
keep_meta = ["player_id","uid","name","current_club_name","competition_name","country_name",
             "position","pos_cls","age","current_mv","latest_val_date","ca","pa",
             "attr_source","pa_gap","ca_age_pct"]
cmeta = cand[keep_meta].copy()
fm_all = pd.read_csv("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv",
                     usecols=["uid","attr_Finishing","attr_Dribbling","attr_Passing",
                              "attr_Vision","attr_Tackling","attr_Marking",
                              "attr_Handling","attr_Reflexes","attr_FirstTouch",
                              "attr_Acceleration","attr_Pace","attr_Stamina",
                              "attr_WorkRate","attr_Anticipation","attr_Positioning",
                              "attr_Composure","attr_Technique","attr_OffTheBall"])
fm_all = fm_all.drop_duplicates("uid")
cmeta = cmeta.merge(fm_all, on="uid", how="left")
cmeta.to_csv(os.path.join(OUT, "candidates_meta.tsv"), index=False, sep="\t", na_rep="NA")
print("wrote", os.path.join(OUT, "candidates_meta.tsv"), cmeta.shape)
