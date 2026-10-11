import csv

rows = list(csv.DictReader(open("data/fm_tm_joined.csv")))
print("total joined:", len(rows))

# Messi check by name
for r in rows:
    if "messi" in (r["name"] or "").lower():
        print("Messi:", r["name"], "ca=", r["ca"], "pa=", r["pa"], "pos=", r["pos_cls"],
              "tm_club=", r.get("current_club_name"))

import collections
targets = ["Inter Miami", "Montr", "Vancouver Whitecaps", "Philadelphia Union"]
found = collections.Counter()
for r in rows:
    club = ((r.get("current_club_name") or "") + " " + (r.get("club_name") or "")).lower()
    for t in targets:
        if t.lower() in club:
            found[t] += 1
print(found)
