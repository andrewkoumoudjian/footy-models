#!/usr/bin/env python3
"""Verify scout.parquet content."""
import pyarrow.parquet as pq

p = pq.read_table("/home/drewclaw/footy-models/data/demo/scout.parquet")
print("rows:", p.num_rows, "cols:", p.num_columns)
df = p.to_pandas()
nonempty = lambda s: (s.fillna("") != "").sum()
print("club non-empty:", nonempty(df["club"]))
print("league non-empty:", nonempty(df["league"]))
print("with mv:", df["current_mv"].notna().sum())
print("with v2:", df["v2_forecast_12m_pct"].notna().sum())
print("with spike:", df["spike_score"].notna().sum())
print("with 36m:", df["gain_36m_pct"].notna().sum())
print("with mu_12:", df["mu_12"].notna().sum())
print("attr_source counts:", df["attr_source"].value_counts().to_dict())
print("PA>=140 U23:", ((df["pa"] >= 140) & (df["age"] <= 23)).sum())
print()
star = df[df["name"].str.contains("Masanka", na=False)]
print(star[["uid", "name", "club", "age", "ca", "pa", "current_mv", "spike_score", "v2_forecast_12m_pct", "gain_36m_pct"]].to_string(index=False))
