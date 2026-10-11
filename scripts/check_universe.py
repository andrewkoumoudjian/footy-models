"""Check FM26 universe for MLS players (the joined table may only have 14.7k of the best)."""
import csv

# fm_universe.csv is the full 563k player set
with open("data/fm_universe.csv") as f:
    header = f.readline().strip().split(",")
    print("universe columns:", [c for c in header[:25]])
    count = 0
    messi = None
    mls_like = []
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        count += 1
        name = (row.get("name") or "").lower()
        if "messi" in name and messi is None:
            messi = {c: row.get(c) for c in ("uid", "name", "ca", "pa", "pos_cls")}
        if count <= 600000:
            # club column names unknown yet; just capture Messi
            pass
    print("rows:", count)
    print("messi:", messi)
