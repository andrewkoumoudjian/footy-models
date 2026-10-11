#!/usr/bin/env python3
"""
Build scout.parquet for the footy-models Scout demo.

Sources:
  - data/fm_universe.csv           563,382 FM26 players (CA/PA/sigs/attr_source)
  - data/fm_tm_joined.csv          TM overlay (club/league/country/mv) for 14,783
  - data/demo/candidates_v2_mean.csv  2,250 v2 12m forecast rows
  - data/demo/candidates_v3_tail.csv  7,969 v3 expectile-0.9 spike rows
  - data/forecast_36m_all.csv        949 basket with 36m gain + drift (mu_12)

Output:
  - data/demo/scout.parquet  (public HF repo: akks88/footy-models-demo)
  - data/demo/scout_meta.json
"""
import json
import math
import pyarrow as pa
import pyarrow.parquet as pq
import csv

DATA = "/home/drewclaw/footy-models/data"
SIGMA_DEFAULT = 0.461  # calibrated conformal sigma from the 36m MC run


def read_csv(path, delim=","):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f, delimiter=delim))


def read_tsv(path):
    return read_csv(path, delim="\t")


def f(x, default=None):
    try:
        return float(x)
    except (TypeError, ValueError):
        return default


print("loading universe...")
universe = read_csv(f"{DATA}/fm_universe.csv")
print(f"  universe rows: {len(universe)}")

print("loading TM overlay...")
tm = {}
for r in read_csv(f"{DATA}/fm_tm_joined.csv"):
    key = str(r.get("uid") or "").strip()
    if key:
        tm[key] = r
print(f"  TM overlay rows: {len(tm)}")

print("loading player_id -> uid map...")
pid2uid = {}
for r in read_tsv(f"{DATA}/forecast/candidates_meta.tsv"):
    pid = str(r.get("player_id") or "").strip()
    uid = str(r.get("uid") or "").strip()
    if pid and uid:
        pid2uid[pid] = uid
print(f"  meta map rows: {len(pid2uid)}")

print("loading v2 forecast...")
v2 = {}
for r in read_csv(f"{DATA}/demo/candidates_v2_mean.csv"):
    uid = pid2uid.get(str(r["player_id"]).strip(), "")
    if uid:
        v2[uid] = r
print(f"  v2 rows (uid-keyed): {len(v2)}")

print("loading v3 tail...")
v3 = {}
for r in read_csv(f"{DATA}/demo/candidates_v3_tail.csv"):
    uid = pid2uid.get(str(r["player_id"]).strip(), "")
    if uid:
        v3[uid] = r
print(f"  v3 rows (uid-keyed): {len(v3)}")

print("loading 36m basket...")
b36 = {}
for r in read_csv(f"{DATA}/forecast_36m_all.csv"):
    uid = pid2uid.get(str(r["player_id"]).strip(), "")
    if uid:
        b36[uid] = r
print(f"  36m rows (uid-keyed): {len(b36)}")

cols = {
    "uid": [], "name": [], "name_n": [], "dob_d": [], "age": [], "ca": [], "pa": [],
    "pa_gap": [], "ca_age_pct": [], "pos_cls": [], "height": [], "home_rep": [],
    "cur_rep": [], "world_rep": [], "attr_source": [],
    "sig_technical": [], "sig_physical": [], "sig_mental": [],
    "sig_defensive": [], "sig_attacking": [], "sig_creative": [],
    "club": [], "league": [], "country": [], "current_mv": [],
    "v2_forecast_12m_pct": [], "v2_interval_10": [], "v2_interval_90": [],
    "spike_score": [], "gain_36m_pct": [], "mu_12": [],
}
INT = {"age", "ca", "pa", "pa_gap", "height", "home_rep", "cur_rep", "world_rep"}
FLOAT = {"ca_age_pct", "sig_technical", "sig_physical", "sig_mental",
         "sig_defensive", "sig_attacking", "sig_creative", "current_mv",
         "v2_forecast_12m_pct", "v2_interval_10", "v2_interval_90",
         "spike_score", "gain_36m_pct", "mu_12"}

scored = 0
for i, u in enumerate(universe):
    uid = u["uid"].strip()
    t = tm.get(uid)
    a = v2.get(uid)
    s = v3.get(uid)
    m = b36.get(uid)

    cols["uid"].append(uid)
    cols["name"].append(u.get("name") or "")
    cols["name_n"].append(u.get("name_n") or "")
    cols["dob_d"].append(u.get("dob_d") or "")
    cols["age"].append(f(u.get("age")))
    cols["ca"].append(f(u.get("ca")))
    cols["pa"].append(f(u.get("pa")))
    cols["pa_gap"].append(f(u.get("pa_gap")))
    cols["ca_age_pct"].append(f(u.get("ca_age_pct")))
    cols["pos_cls"].append(u.get("pos_cls") or "")
    cols["height"].append(f(u.get("height")))
    cols["home_rep"].append(f(u.get("home_rep")))
    cols["cur_rep"].append(f(u.get("cur_rep")))
    cols["world_rep"].append(f(u.get("world_rep")))
    cols["attr_source"].append(u.get("attr_source") or "")
    for sig in ("sig_technical", "sig_physical", "sig_mental",
                "sig_defensive", "sig_attacking", "sig_creative"):
        cols[sig].append(f(u.get(sig)))

    # TM overlay
    cols["club"].append((t or {}).get("current_club_name") or "")
    cols["league"].append((t or {}).get("competition_name") or "")
    cols["country"].append((t or {}).get("country_name") or "")
    cols["current_mv"].append(f((t or {}).get("current_mv")))

    # v2 12m forecast (gap_pct = fractional 12m gain)
    if a:
        cols["v2_forecast_12m_pct"].append(f(a.get("gap_pct")))
        cols["v2_interval_10"].append(f(a.get("interval_10")))
        cols["v2_interval_90"].append(f(a.get("interval_90")))
    else:
        cols["v2_forecast_12m_pct"].append(None)
        cols["v2_interval_10"].append(None)
        cols["v2_interval_90"].append(None)

    # v3 spike score
    if s:
        cols["spike_score"].append(f(s.get("tail_score")))
        scored += 1
    else:
        cols["spike_score"].append(None)

    # 36m gain + drift (basket only)
    if m:
        cols["gain_36m_pct"].append(f(m.get("gap_36m_pct")))
        cols["mu_12"].append(f(m.get("mu_12")))
    else:
        cols["gain_36m_pct"].append(None)
        cols["mu_12"].append(None)

print(f"rows with spike score: {scored} / {len(universe)}")

arrays = []
for name in cols:
    values = cols[name]
    if name in INT:
        arrays.append(pa.array([v if v is not None else None for v in values], pa.int32()))
    elif name in FLOAT:
        arrays.append(pa.array([v if v is not None else None for v in values], pa.float64()))
    else:
        arrays.append(pa.array(values, pa.string()))

table = pa.table(dict(zip(cols.keys(), arrays)))
out = f"{DATA}/demo/scout.parquet"
pq.write_table(table, out, compression="zstd", row_group_size=65536)
print(f"wrote {out} rows={table.num_rows} cols={table.num_columns} size={__import__('os').path.getsize(out) / 1e6:.1f}MB")

meta = {
    "name": "footy-models scout demo",
    "as_of": "2025-11-06",
    "realized_through": "2026-06-12",
    "models": {
        "v2_forecast": {"file": "data/forecast/model.bin", "objective": "ridge mean", "forward_ic": 0.5100},
        "v3_spike": {"file": "data/tail/model_exp090_r1e6.bin", "objective": "expectile-0.9", "forward_ic": 0.4858},
    },
    "sigma_mc_default": SIGMA_DEFAULT,
    "rows": table.num_rows,
    "scored_rows": scored,
    "generated": "2026-08-10",
}
meta_out = f"{DATA}/demo/scout_meta.json"
with open(meta_out, "w") as mh:
    json.dump(meta, mh, indent=2)
print(f"wrote {meta_out}")
