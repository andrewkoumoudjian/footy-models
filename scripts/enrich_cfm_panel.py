#!/usr/bin/env python3
"""Enrich data/cfm_scout_panel.csv with forecast + spike + TM position columns.

The orchestrator's prep_cfm_panel.py dropped the f36_*/tail_* columns because
it derived fieldnames from rows[0] (whose forecast match was empty). This is a
1:1 enrichment by player_id from the SAME source files prep_cfm_panel.py used
(forecast_36m_all.csv, candidates_v3_tail.csv, fm_tm_joined.csv) — no re-merge
of the TM x FM x hidden joins, so no Cartesian blowup. Existing rows/columns
are untouched; only new columns are added.
"""
import pandas as pd
import numpy as np

PANEL = "/home/drewclaw/footy-models/data/cfm_scout_panel.csv"
F36 = "/home/drewclaw/footy-models/data/forecast_36m_all.csv"
TAIL = "/home/drewclaw/footy-models/data/demo/candidates_v3_tail.csv"
TM = "/home/drewclaw/footy-models/data/fm_tm_joined.csv"

panel = pd.read_csv(PANEL)
f36 = pd.read_csv(F36)
tail = pd.read_csv(TAIL)
tm = pd.read_csv(TM, usecols=["player_id", "position", "sub_position"]).drop_duplicates("player_id")

panel["player_id"] = panel["player_id"].astype("Int64")
f36_map = f36.set_index("player_id")
tail_map = tail.set_index("player_id")
tm_map = tm.set_index("player_id")

for col, src in [
    ("f36_predicted_12m_mv", "predicted_12m_mv"), ("f36_gap_pct", "gap_pct"),
    ("f36_gap_36m_pct", "gap_36m_pct"), ("f36_mu_12", "mu_12"),
    ("f36_predicted_36m_mv", "predicted_36m_mv"),
    ("f36_interval_10", "interval_10"), ("f36_interval_90", "interval_90"),
]:
    panel[col] = panel["player_id"].map(f36_map[src])
for col, src in [
    ("tail_tail_score", "tail_score"), ("tail_predicted_12m_mv", "predicted_12m_mv"),
    ("tail_gap_pct", "gap_pct"),
]:
    panel[col] = panel["player_id"].map(tail_map[src])
panel["tm_position"] = panel["player_id"].map(tm_map["position"])
panel["tm_sub_position"] = panel["player_id"].map(tm_map["sub_position"])

panel.to_csv(PANEL, index=False)
n = len(panel)
print(f"enriched panel: {n} rows x {panel.shape[1]} cols")
print(f"f36 coverage {panel['f36_gap_pct'].notna().sum()} ({panel['f36_gap_pct'].notna().mean()*100:.1f}%)")
print(f"tail coverage {panel['tail_tail_score'].notna().sum()} ({panel['tail_tail_score'].notna().mean()*100:.1f}%)")
print(f"tm_position coverage {panel['tm_sub_position'].notna().sum()} ({panel['tm_sub_position'].notna().mean()*100:.1f}%)")
