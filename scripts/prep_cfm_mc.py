#!/usr/bin/env python3
"""Build the CFM MC input TSVs (STEERING_CFM.md section 5).

Base run:  mu_ann = ln(f36_predicted_12m_mv / current_mv),  sigma_ann = 0.4688
Spike run: mu_ann = tail_tail_score (v3 expectile-0.9 12m log-ratio), sigma_ann = 0.4688
"""
import pandas as pd
import numpy as np

top = pd.read_csv("/home/drewclaw/footy-models/data/cfm_top20_shortlist.csv").sort_values("rank")
cols = ["player_id", "name", "club", "pos_cls", "age", "fm_ca", "fm_pa", "attr_source", "current_mv", "mu_ann", "sigma_ann"]

base = top[["player_id", "name", "club", "pos_cls", "age", "fm_ca", "fm_pa", "attr_source", "current_mv"]].copy()
base["mu_ann"] = np.log(top["f36_predicted_12m_mv"] / top["current_mv"])
base["sigma_ann"] = 0.4688

spike = base.copy()
spike["mu_ann"] = top["tail_tail_score"]

for d, path in [(base, "data/cfm_mc_input.tsv"), (spike, "data/cfm_mc_input_spike.tsv")]:
    d = d[cols].copy()
    for c in ("name", "club"):
        d[c] = d[c].astype(str).str.replace("\t", " ").str.replace("\n", " ")
    d.to_csv(path, sep="\t", index=False)
    print(path, d.shape)
    print(d[["name", "current_mv", "mu_ann", "sigma_ann"]].head(3).to_string(index=False))
