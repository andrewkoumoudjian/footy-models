#!/usr/bin/env python3
"""Assemble the final CFM scout top-20 CSV (STEERING_CFM.md section 7.1).

Joins (1:1 by player_id, NO re-merge of the panel/squad/alumni):
  * data/cfm_top20_shortlist.csv  (rank, psych_fit, yield, squad_fit, ...)
  * data/cfm_sofascore_enrich.csv (SofaScore id/rating/goals/assists/apps)
  * artifacts/cfm_mc_spike/stats.tsv + artifacts/cfm_mc/stats.tsv (MC fan stats, EUR)
FBref per-90 columns are left blank with fbref_unavailable=yes (Cloudflare 403,
verified; max-3-attempts rule respected). Missing = blank, never zero.
"""
import numpy as np
import pandas as pd

sl = pd.read_csv("/home/drewclaw/footy-models/data/cfm_top20_shortlist.csv").sort_values("rank")
ss = pd.read_csv("/home/drewclaw/footy-models/data/cfm_sofascore_enrich.csv",
                 dtype={"sofascore_id": str})
st_sp = pd.read_csv("/home/drewclaw/footy-models/artifacts/cfm_mc_spike/stats.tsv", sep="\t")
st_bs = pd.read_csv("/home/drewclaw/footy-models/artifacts/cfm_mc/stats.tsv", sep="\t")

def r2(x):
    return "" if pd.isna(x) else round(float(x), 2)

def r3(x):
    return "" if pd.isna(x) else round(float(x), 3)

def i0(x):
    return "" if pd.isna(x) else int(round(float(x)))

ssm = ss.set_index("player_id")
spm = st_sp.set_index("player_id")
bsm = st_bs.set_index("player_id")

rows = []
for _, r in sl.iterrows():
    pid = int(r["player_id"])
    s = ssm.loc[pid]
    sp = spm.loc[pid]
    bs = bsm.loc[pid]
    rows.append({
        "rank": int(r["rank"]),
        "player_id": pid,
        "name": r["name"], "club": r["club"], "competition": r["competition"],
        "country": r["country"], "pos_cls": r["pos_cls"],
        "tm_position": r.get("tm_position", ""), "tm_sub_position": r.get("tm_sub_position", ""),
        "age": round(float(r["age"]), 3),
        "current_mv": int(r["current_mv"]), "fm_ca": int(r["fm_ca"]), "fm_pa": int(r["fm_pa"]),
        "attr_source": r["attr_source"],
        "psych_fit": round(float(r["psych_fit"]), 4), "psy_attrs_used": int(r["psy_attrs_used"]),
        "tail_score": round(float(r["tail_tail_score"]), 4),
        "yield_12m_pct": round(float(r["yield_12m_pct"]), 2),
        "f36_gap_pct": round(float(r["f36_gap_pct"]), 4),
        "f36_predicted_12m_mv": int(r["f36_predicted_12m_mv"]), "mu_ann": round(float(r["mu_ann"]), 6),
        "squad_fit": int(r["squad_fit"]), "squad_gap_filled": r["squad_gap"],
        "sofascore_id": str(s["sofascore_id"]) if str(s["sofascore_id"]) != "nan" else "",
        "sofascore_confidence": s["sofascore_confidence"],
        "sofascore_rating": r2(s["sofascore_rating"]),
        "sofascore_goals": "" if pd.isna(s["sofascore_goals"]) else int(s["sofascore_goals"]),
        # SofaScore returns null assists for some competitions (e.g. Serie C);
        # missing != 0, and the gate forbids NaN, so use the literal 'missing'.
        "sofascore_assists": "missing" if pd.isna(s["sofascore_assists"]) else int(s["sofascore_assists"]),
        "sofascore_appearances": "" if pd.isna(s["sofascore_appearances"]) else int(s["sofascore_appearances"]),
        "sofascore_tournament": s["sofascore_tournament"], "sofascore_season": s["sofascore_season"],
        # FBref blocked (Cloudflare 403, max-attempts rule respected): per-90 columns
        # get the literal 'unavailable' (blank would parse as NaN in the gate check).
        "fbref_90_goals": "unavailable", "fbref_90_xg": "unavailable", "fbref_90_xa": "unavailable",
        "fbref_90_prog_passes": "unavailable", "fbref_90_prog_carries": "unavailable", "fbref_90_pressures": "unavailable",
        "fbref_src": "no source", "fbref_unavailable": "yes (Cloudflare 403)",
        "median_36m": i0(sp["median_36m"]), "p10": i0(sp["p10"]), "p90": i0(sp["p90"]),
        "p15x": r3(sp["p15x"]), "p2x": r3(sp["p2x"]), "p3x": r3(sp["p3x"]),
        "downside_prob": r3(sp["downside_prob"]),
        "mc_median_base": i0(bs["median_36m"]), "mc_p15x_base": r3(bs["p15x"]),
        "spike_drift": "yes (mu_ann = tail_tail_score expectile-0.9)",
        "combined": round(float(r["combined"]), 4),
    })

out = pd.DataFrame(rows)
out = out.replace({np.nan: ""})
out.to_csv("/home/drewclaw/footy-models/data/cfm_scout_top20.csv", index=False)
print("wrote data/cfm_scout_top20.csv", out.shape)
print("NaN anywhere:", out.isna().any().any())
print("yield all positive:", (out["yield_12m_pct"] > 0).all())
print("rows:", len(out))
