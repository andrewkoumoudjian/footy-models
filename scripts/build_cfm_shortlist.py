#!/usr/bin/env python3
"""CFM scout shortlist builder (STEERING_CFM.md sections 2-4).

Reads the enriched panel (data/cfm_scout_panel.csv) — NO re-merge — and:
  1. defines the CFM scout basket (age<23, 400k-2.5M, attr_source != staff, not at CFM)
  2. calibrates the psych archetype on the top-half-by-success alumni (fee>300k, success>=1.5)
  3. scores psych_fit (sigmoid of z-distance over mental + hidden attrs; hidden = bridge-club prior)
  4. yield (f36_gap_pct>0 AND predicted_12m>current_mv), spike (tail_tail_score)
  5. squad_fit (positional need + age fit + value band)
  6. combined rank z(psych_fit) + 0.5 z(spike) + 0.3 z(yield) + 0.4 z(squad_fit)
  7. selects the top-20 (coverage gate >=7/11 mental and >=10 scored; cap 3 per position slot;
     above psych-fit & spike medians; tail in top 40% of basket)

Writes data/cfm_top20_shortlist.csv (20 rows, pre-MC) and prints diagnostics.
"""
import pandas as pd
import numpy as np
import os

os.chdir("/home/drewclaw/footy-models")

MENTAL = ["attr_Determination", "attr_Bravery", "attr_Consistency", "attr_ImportantMatches",
          "attr_Composure", "attr_WorkRate", "attr_Teamwork", "attr_Leadership",
          "attr_Versatility", "attr_NaturalFitness", "attr_InjuryProneness"]
HIDDEN = ["psy_Pressure", "psy_Professionalism", "psy_Temperament", "psy_Controversy"]
NEG = {"attr_InjuryProneness": True, "psy_Controversy": True}

# Bridge-club prior for hidden attrs (all alumni lack usable hidden personality:
# only Koné has values and Ambition=0 / Adaptability=1 are placeholders outside
# FM's 1-20 scale). Steering-provided prior, Temperament set to 12 (consistent
# with the >=12 bridge-club cluster; not in the steering's prior list).
HIDDEN_ARCH = {"psy_Pressure": 12.0, "psy_Professionalism": 12.0,
               "psy_Temperament": 12.0, "psy_Controversy": 8.0}

def num(x):
    try:
        v = float(x)
        return v if v > 0 else np.nan
    except Exception:
        return np.nan

# ---------------- 1. basket ----------------
panel = pd.read_csv("data/cfm_scout_panel.csv")
basket = panel[
    (panel["age"] < 23) &
    (panel["current_mv"] >= 400000) & (panel["current_mv"] <= 2500000) &
    (panel["attr_source"] != "staff")
].copy()
basket = basket[~basket["club"].str.contains("ontr", case=False, na=False)].reset_index(drop=True)
print(f"basket: {len(basket)}")

# ---------------- 2. archetype (mental, from alumni) ----------------
alumni = pd.read_csv("data/cfm_alumni.tsv", sep="\t")
alumni["success"] = alumni["current_mv"] / alumni["transfer_fee"]
cal = alumni[(alumni["success"] >= 1.5) & (alumni["transfer_fee"] > 300000)].sort_values("success", ascending=False)
cal_set = cal.head(int(np.ceil(len(cal) / 2)))
print(f"alumni calibration set (top-half of {len(cal)} by success): "
      + ", ".join(f"{r.player_name} {r.success:.2f}x" for r in cal_set.itertuples()))

# full mental attrs for the calibration uids from FM26 attrs (read-only lookup)
attr_rows = {}
with open("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv") as f:
    import csv
    for row in csv.DictReader(f):
        attr_rows[row["uid"].strip()] = row

arch = {}
for a in MENTAL:
    vals = []
    for uid in cal_set["uid"].astype("Int64"):
        v = num(attr_rows.get(str(int(uid)), {}).get(a))
        if not np.isnan(v):
            vals.append(v)
    arch[a] = float(np.mean(vals)) if vals else np.nan
for a, v in HIDDEN_ARCH.items():
    arch[a] = v
assert all(not np.isnan(arch[a]) for a in MENTAL), "mental archetype must be complete"
print("mental archetype:", {a: round(arch[a], 2) for a in MENTAL})

# ---------------- 3. psych_fit ----------------
scales = {}
for a in MENTAL + HIDDEN:
    s = basket[a].apply(num).std()
    scales[a] = float(max(s, 1.0)) if not np.isnan(s) else 2.5

def z_dist(row, attrs):
    ds = []
    for a in attrs:
        v = num(row.get(a))
        if np.isnan(v):
            continue
        d = (arch[a] - v) if a in NEG else (v - arch[a])
        ds.append((d / scales[a]) ** 2)
    if not ds:
        return np.nan, 0
    return float(np.sqrt(np.mean(ds))), len(ds)

K, Z0 = 1.5, 1.1  # psych_fit = 1/(1+exp(K*(z-Z0))); calibrated so alumni >= 0.6
z, n = zip(*[z_dist(r, MENTAL + HIDDEN) for _, r in basket.iterrows()])
basket["psych_fit_z"] = z
basket["psy_attrs_used"] = n
basket["psych_fit"] = 1.0 / (1.0 + np.exp(K * (basket["psych_fit_z"] - Z0)))

# alumni validation
alumni_fits = []
for uid in cal_set["uid"].astype("Int64"):
    row = attr_rows.get(str(int(uid)), {})
    zz, nn = z_dist(row, MENTAL)
    alumni_fits.append(1.0 / (1.0 + np.exp(K * (zz - Z0))))
print(f"alumni psych_fit validation: min {min(alumni_fits):.3f}, mean {np.mean(alumni_fits):.3f} (need >= 0.6)")

# ---------------- 4. yield + spike ----------------
basket["yield_12m_pct"] = basket["f36_gap_pct"] * 100.0
basket["spike"] = basket["tail_tail_score"].fillna(basket["f36_gap_36m_pct"])
basket["positive_yield"] = (basket["f36_gap_pct"] > 0) & (basket["f36_predicted_12m_mv"] > basket["current_mv"])
basket["mu_ann"] = np.log(basket["f36_predicted_12m_mv"] / basket["current_mv"])

# ---------------- 5. squad fit ----------------
NEED = {"Left Winger": 3, "Left Midfield": 3, "Left-Back": 3,
        "Attacking Midfield": 2, "Central Midfield": 2, "Defensive Midfield": 2,
        "Right Winger": 1, "Right Midfield": 1, "Centre-Forward": 1,
        "Second Striker": 1, "Centre-Back": 1, "Right-Back": 1, "Goalkeeper": 1}
GAP = {"Left Winger": "LW", "Left Midfield": "LM", "Left-Back": "LB",
       "Attacking Midfield": "AM", "Central Midfield": "CM", "Defensive Midfield": "DM",
       "Right Winger": "RW", "Right Midfield": "RM", "Centre-Forward": "CF",
       "Second Striker": "CF", "Centre-Back": "CB", "Right-Back": "RB", "Goalkeeper": "GK"}

def squad_fit_row(r):
    sub = r.get("tm_sub_position")
    need = NEED.get(sub, 1)
    age_fit = 2 if r["age"] < 22 else 1
    vb = 2 if 400000 <= r["current_mv"] <= 1200000 else 1
    return need + age_fit + vb, GAP.get(sub, "?")

basket["squad_fit"], basket["squad_gap"] = zip(*[squad_fit_row(r) for _, r in basket.iterrows()])
basket["n_mental"] = [sum(1 for a in MENTAL if not np.isnan(num(r[a]))) for _, r in basket.iterrows()]

# ---------------- 6. combined rank (over the coverage-gated pool) ----------------
pool = basket[basket["positive_yield"] & basket["tail_tail_score"].notna() &
              (basket["tail_tail_score"] > 0) &
              (basket["n_mental"] >= 7) & (basket["psy_attrs_used"] >= 10)].copy()
for col in ["psych_fit", "spike", "yield_12m_pct", "squad_fit"]:
    m, s = pool[col].mean(), pool[col].std()
    pool["z_" + col] = (pool[col] - m) / s
pool["combined"] = pool["z_psych_fit"] + 0.5 * pool["z_spike"] + 0.3 * pool["z_yield_12m_pct"] + 0.4 * pool["z_squad_fit"]
pool = pool.sort_values("combined", ascending=False)
print(f"coverage-gated pool: {len(pool)}")

# ---------------- 7. top-20 selection ----------------
med_psych = basket["psych_fit"].median()
med_spike = basket["spike"].median()
tail40 = basket["tail_tail_score"].quantile(0.60)
print(f"basket medians: psych_fit {med_psych:.3f}, spike {med_spike:.3f}; tail top-40% >= {tail40:.3f}")

selected, counts = [], {}
for _, r in pool.iterrows():
    g = r["squad_gap"]
    if counts.get(g, 0) >= 3:
        continue
    if r["psych_fit"] <= med_psych or r["spike"] <= med_spike or r["tail_tail_score"] < tail40:
        continue
    selected.append(r["player_id"])
    counts[g] = counts.get(g, 0) + 1
    if len(selected) >= 20:
        break
print("position caps used:", counts)

top = pool[pool["player_id"].isin(selected)].sort_values("combined", ascending=False).copy()
assert len(top) == 20, f"expected 20, got {len(top)}"
top.insert(0, "rank", range(1, 21))

out_cols = ["rank", "player_id", "name", "club", "competition", "country", "pos_cls",
            "tm_position", "tm_sub_position", "age", "current_mv", "fm_ca", "fm_pa",
            "attr_source", "psych_fit", "psych_fit_z", "psy_attrs_used", "n_mental",
            "spike", "tail_tail_score", "yield_12m_pct", "f36_gap_pct",
            "f36_predicted_12m_mv", "f36_gap_36m_pct", "mu_ann", "squad_fit", "squad_gap",
            "combined"]
top[out_cols].to_csv("data/cfm_top20_shortlist.csv", index=False)
print("wrote data/cfm_top20_shortlist.csv")
print(top[["rank", "name", "club", "tm_sub_position", "age", "current_mv", "psych_fit",
           "spike", "yield_12m_pct", "squad_gap", "combined"]].to_string(index=False))
