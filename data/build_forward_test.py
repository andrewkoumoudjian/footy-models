#!/usr/bin/env python3
"""Build forward-test candidates + realized oracle for the FM26 as-of anchor.

The FM26 database snapshot (start of FM26) is anchored at 2025-11-06 (T0).
The Transfermarkt overlay (read-only DuckDB) has realized valuations through
2026-06-12 (END). For every candidate player (forecast_candidates.csv for the
forecast model; surge/candidates.tsv for the surge model):

  - entry observation = last TM valuation with date <= T0 (mv_t at t <= T0)
  - features computed STRICTLY from observations before t (same extractor as
    the training pipeline -> identical 47-feature schema)
  - realized = last TM valuation in (t, END]; censored players (no valuation
    after t) are DROPPED, never encoded as zero
  - partial-window peak = max valuation in (t, END] (for partial surge labels)

Outputs (data/forward_test/):
  forecast_candidates.tsv / surge_candidates.tsv   (47 features + player_id,t,mv_t)
  forecast_realized.tsv   / surge_realized.tsv     (player_id,t,mv_t,mv_realized,
                                                    realized_date,horizon_days,peak_realized)
  forecast_meta.tsv       / surge_meta.tsv         (player_id, uid, name, club, competition,
                                                    country, position, pos_cls, age(as-of),
                                                    current_mv(entry), fm_ca, fm_pa, pa_eff,
                                                    attr_source, pa_gap, ca_age_pct, mins_365,
                                                    price_bucket)

All inputs are the exact repo files + the read-only Transfermarkt DuckDB.
DO NOT re-merge fm_tm_joined.csv; it is used as-is for FM features.
"""
import numpy as np
import pandas as pd
import duckdb
import os, datetime

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "forward_test")
os.makedirs(OUT, exist_ok=True)
DB = "/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb"
T0 = np.datetime64("2025-11-06")
T0_DAYS = int((T0 - np.datetime64("1970-01-01")) / np.timedelta64(1, "D"))
END = np.datetime64("2026-06-12")
END_DAYS = int((END - np.datetime64("1970-01-01")) / np.timedelta64(1, "D"))
print("T0_DAYS:", T0_DAYS, "END_DAYS:", END_DAYS)

# ---------------------------------------------------------------------------
# 1. Load source tables (identical to build_forecast_dataset.py)
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

# FM features keyed by TM player_id (from the FM<->TM join, used as-is)
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
# 3. Feature extractor (identical to build_forecast_dataset.py)
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

def extract_features(pid, t, mv_t, d, mv, comps_row, i):
    f = {}
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
    meta = pmeta.get(pid, {})
    dob = meta.get("dob", np.nan)
    age = (t - dob) / 365.25 if not np.isnan(dob) else np.nan
    if not np.isnan(age) and not (14.0 <= age <= 50.0):
        age = np.nan
    f["age"] = age
    f["age_sq"] = age * age if not np.isnan(age) else np.nan
    pos = meta.get("pos", "Missing")
    f["pos_gk"] = 1.0 if pos == "Goalkeeper" else 0.0
    f["pos_df"] = 1.0 if pos == "Defender" else 0.0
    f["pos_mf"] = 1.0 if pos == "Midfield" else 0.0
    f["height_cm"] = meta.get("height", np.nan)
    first_tier, uefa, sam = comp_info(comps_row[i])
    f["comp_mapped"] = 1.0 if comps_row[i] is not None and not pd.isna(comps_row[i]) else 0.0
    f["first_tier"] = first_tier
    f["confed_uefa"] = uefa
    f["confed_sam"] = sam
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

# PA band midpoint conversion (same table as footy_recruitment::surge)
PA_BAND = {-95:185.0,-85:175.0,-75:165.0,-65:155.0,-55:145.0,-45:135.0,-35:125.0,
           -25:115.0,-15:105.0,-10:185.0,-9:165.0,-8:145.0,-7:125.0,-6:105.0,-5:85.0,
           -4:65.0,-3:45.0,-2:30.0,-1:20.0}
def pa_eff(pa):
    if pd.isna(pa): return np.nan
    if pa < 0:
        return PA_BAND.get(int(pa), np.nan)
    return pa

def build(pop, label):
    """pop: DataFrame with player_id + metadata columns (uid,name,club,competition,
    country,position,pos_cls,age,ca,pa,attr_source,pa_gap,ca_age_pct)."""
    rows = []
    rmeta = []
    n_no_entry = 0
    n_censored = 0
    for pid in pop["player_id"]:
        if pid not in pval:
            n_no_entry += 1
            continue
        d, cols = pval[pid]
        mv = cols["mv"]
        comps_row = cols["comp"]
        i = int(np.searchsorted(d, T0_DAYS, side="right")) - 1
        if i < 0:
            n_no_entry += 1
            continue
        t = int(d[i])
        mv_t = float(mv[i])
        if not np.isfinite(mv_t) or mv_t <= 0:
            continue
        feats = extract_features(pid, t, mv_t, d, mv, comps_row, i)
        # realized: last valuation in (t, END]
        k = int(np.searchsorted(d, END_DAYS, side="right")) - 1
        if k <= i:
            n_censored += 1
            continue
        mv_realized = float(mv[k])
        realized_date = int(d[k])
        peak_realized = float(np.nanmax(mv[i + 1:k + 1])) if (k - i) >= 1 else float(mv_realized)
        horizon_days = int(realized_date - t)
        rows.append(feats + [pid, t, mv_t])
        # metadata (age as-of t; mins_365 causal)
        m = pop.loc[pop["player_id"] == pid].iloc[0]
        ad, acols = papps.get(pid, (np.array([], dtype=np.int64), {}))
        i365 = np.searchsorted(ad, t - 365, side="right")
        i0 = np.searchsorted(ad, t, side="right")
        mins365 = float(acols["minutes_played"][i365:i0].sum()) if i0 > i365 else 0.0
        dob = pmeta.get(pid, {}).get("dob", np.nan)
        age = (t - dob) / 365.25 if not np.isnan(dob) else np.nan
        if not np.isnan(age) and not (14.0 <= age <= 50.0):
            age = np.nan
        price_bucket = 1 if mv_t > 400_000 else 0  # [10k,400k] vs (400k,1M]
        rmeta.append([
            int(pid), m["uid"], m["name"], m["current_club_name"], m["competition_name"],
            m["country_name"], m["position"], m["pos_cls"], age, mv_t,
            (datetime.date(1970,1,1) + datetime.timedelta(days=int(t))).isoformat(),
            m["ca"], m["pa"], pa_eff(m["pa"]), m["attr_source"], m["pa_gap"], m["ca_age_pct"],
            mins365, price_bucket,
        ])
    candf = pd.DataFrame(rows, columns=FEATURE_NAMES + ["player_id", "t", "mv_t"])
    candf.to_csv(os.path.join(OUT, f"{label}_candidates.tsv"), index=False, sep="\t", na_rep="NA")
    print(f"[{label}] candidates:", candf.shape, "no_entry:", n_no_entry, "censored:", n_censored)
    return rows, rmeta

def build_realized(rows, label):
    out = []
    for feats, pid, t, mv_t in [(r[:-3], r[-3], r[-2], r[-1]) for r in rows]:
        d, cols = pval[pid]
        mv = cols["mv"]
        k = int(np.searchsorted(d, END_DAYS, side="right")) - 1
        mv_realized = float(mv[k])
        realized_date = int(d[k])
        peak_realized = float(np.nanmax(mv[int(np.searchsorted(d, t, side="right")):k + 1]))
        horizon_days = int(realized_date - t)
        out.append([pid, t, mv_t, mv_realized, realized_date, horizon_days, peak_realized])
    rdf = pd.DataFrame(out, columns=["player_id", "t", "mv_t", "mv_realized", "realized_date", "horizon_days", "peak_realized"])
    rdf.to_csv(os.path.join(OUT, f"{label}_realized.tsv"), index=False, sep="\t")
    print(f"[{label}] realized rows:", rdf.shape)

# ---------------------------------------------------------------------------
# 4. Forecast forward-test population: forecast_candidates.csv
# ---------------------------------------------------------------------------
fc = pd.read_csv(os.path.join(os.path.dirname(os.path.abspath(__file__)), "forecast_candidates.csv"))
fc = fc.drop_duplicates("player_id")
print("forecast candidates:", len(fc))
rows, rmeta = build(fc, "forecast")
build_realized(rows, "forecast")
mdf = pd.DataFrame(rmeta, columns=["player_id","uid","name","current_club_name","competition_name",
    "country_name","position","pos_cls","age","current_mv","latest_val_date",
    "ca","pa","pa_eff","attr_source","pa_gap","ca_age_pct","mins_365","price_bucket"])
mdf.to_csv(os.path.join(OUT, "forecast_meta.tsv"), index=False, sep="\t", na_rep="NA")
print("forecast meta:", mdf.shape)

# ---------------------------------------------------------------------------
# 5. Surge forward-test population: surge/candidates.tsv
# ---------------------------------------------------------------------------
sc = pd.read_csv(os.path.join(os.path.dirname(os.path.abspath(__file__)), "surge", "candidates.tsv"), sep="\t")
sc = sc[["player_id"]].drop_duplicates()
# join metadata from forecast_candidates.csv where available
fcm = fc.set_index("player_id")
meta_cols = ["uid","name","current_club_name","competition_name","country_name","position","pos_cls","ca","pa","attr_source","pa_gap","ca_age_pct"]
sc = sc.merge(fcm[meta_cols], on="player_id", how="left")
print("surge candidates:", len(sc))
rows2, rmeta2 = build(sc, "surge")
build_realized(rows2, "surge")
mdf2 = pd.DataFrame(rmeta2, columns=["player_id","uid","name","current_club_name","competition_name",
    "country_name","position","pos_cls","age","current_mv","latest_val_date",
    "ca","pa","pa_eff","attr_source","pa_gap","ca_age_pct","mins_365","price_bucket"])
mdf2.to_csv(os.path.join(OUT, "surge_meta.tsv"), index=False, sep="\t", na_rep="NA")
print("surge meta:", mdf2.shape)
print("done")
