#!/usr/bin/env python3
"""Feasibility: count U23 players with 400k <= current TM value <= 1.2M."""
import pandas as pd

j = pd.read_csv("/home/drewclaw/footy-models/data/fm_tm_joined.csv")
print("joined cols:", [c for c in j.columns][:25])
print("rows:", len(j))
# find the value column
for c in j.columns:
    if "mv" in c.lower() or "value" in c.lower() or "price" in c.lower():
        print("candidate col:", c, j[c].dtype)
