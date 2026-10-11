"""Diagnose why Charlotte players got team=None in aggregates: the TEAMS map lacks Charlotte."""
import csv
from collections import Counter

rows = list(csv.DictReader(open("data/sofascore_cfm/player_aggregates.csv")))
print(Counter(r["team"] for r in rows))
