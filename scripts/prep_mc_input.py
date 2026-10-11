#!/usr/bin/env python3
"""Prepare MC input TSV from the forecast top-100 (drift + conformal vol)."""
import pandas as pd
import numpy as np

p = pd.read_csv("/home/drewclaw/footy-models/data/forecast_u23_top100.csv").sort_values("gap_eur", ascending=False)
p["mu_ann"] = np.log(p["predicted_12m_mv"] / p["current_mv"])
Z80 = 1.2816
p["sigma_ann"] = ((np.log(p["interval_90"]) - np.log(p["interval_10"])) / (2 * Z80)).clip(lower=0.05)
cols = ["player_id", "name", "club", "pos_cls", "age", "fm_ca", "fm_pa", "attr_source", "current_mv", "mu_ann", "sigma_ann"]
d = p[cols].copy()
for c in ("name", "club"):
    d[c] = d[c].str.replace("\t", " ").str.replace("\n", " ")
d.to_csv("/home/drewclaw/footy-models/data/mc_input.tsv", sep="\t", index=False)
print("mc_input.tsv:", d.shape)
print(d[["name", "current_mv", "mu_ann", "sigma_ann"]].head(3).to_string(index=False))
