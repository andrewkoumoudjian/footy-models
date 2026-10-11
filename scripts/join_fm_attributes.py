"""Join FM26 attributes to SofaScore players by normalized name.

FM26 dataset has no club column, so we match on name (+ position class sanity).
Outputs data/sofascore_cfm/player_fm_join.csv with ca, pa, pos_cls and key FM attrs
mapped into engine attribute proxies: finishing/technical/defending + mental dims.
"""
import csv, json, re, unicodedata

csv.field_size_limit(10_000_000)
FM_PATH = "/home/drewclaw/fm26-data/fm26_final_men.csv"
OUT = "data/sofascore_cfm"

def norm(name):
    text = unicodedata.normalize("NFKD", name or "")
    text = "".join(ch for ch in text if not unicodedata.combining(ch))
    text = re.sub(r"[^a-z0-9 ]", " ", text.lower())
    return " ".join(text.split())

# Load FM index (name_n -> row). Prefer higher CA on collision.
fm_index = {}
with open(FM_PATH) as f:
    header = f.readline().strip().split(",")
    idx_name = header.index("name")
    idx_ca = header.index("ca")
    idx_pa = header.index("pa") if "pa" in header else None
    # fm26_final_men.csv lacks pos_cls; the match engine uses source position + selected FM attrs.
    attr_cols = [c for c in header if c.startswith("attr_")]
    for line in f:
        row = line.strip().split(",")
        if len(row) <= idx_ca:
            continue
        name_norm = norm(row[idx_name])
        try:
            ca = float(row[idx_ca] or 0)
        except ValueError:
            continue
        record = {"name": row[idx_name], "ca": ca,
                  "pa": row[idx_pa] if idx_pa is not None and idx_pa < len(row) else "",
                  **{c: row[header.index(c)] for c in attr_cols}}
        if name_norm not in fm_index or ca > fm_index[name_norm]["ca"]:
            fm_index[name_norm] = record

print("FM index:", len(fm_index))

players = list(csv.DictReader(open(f"{OUT}/player_aggregates.csv")))
joined = matched = 0
out_rows = []
KEY_ATTRS = {
    "attr_Finishing": "finishing", "attr_Composure": "composure",
    "attr_Passing": "passing", "attr_Technique": "technique",
    "attr_Tackling": "tackling", "attr_Marking": "marking",
    "attr_Positioning": "positioning", "attr_Pace": "pace",
    "attr_Acceleration": "acceleration", "attr_Strength": "strength",
    "attr_Consistency": "consistency", "attr_Dirtiness": "dirtiness",
    "attr_ImportantMatches": "important_matches", "attr_Teamwork": "teamwork",
    "attr_Leadership": "leadership", "attr_WorkRate": "work_rate",
}
base_cols = list(players[0].keys()) if players else []
fm_cols = ["fm_matched", "fm_ca", "fm_pa"] + [f"fm_{dst}" for dst in KEY_ATTRS.values()]
for player in players:
    out = dict(player)
    # Fixed schema: never derive the CSV header from whichever player happens to be first. The old
    # code did that, so an unmatched first row dropped every FM attribute column from the file even
    # when later players were successfully matched.
    for column in fm_cols:
        out[column] = ""
    hit = fm_index.get(norm(player["name"]))
    if hit:
        matched += 1
        out["fm_matched"] = 1
        out["fm_ca"] = round(hit["ca"])
        out["fm_pa"] = hit.get("pa", "")
        for src, dst in KEY_ATTRS.items():
            try:
                out[f"fm_{dst}"] = float(hit[src])
            except (KeyError, TypeError, ValueError):
                out[f"fm_{dst}"] = ""
    else:
        out["fm_matched"] = 0
    out_rows.append(out)

cols = base_cols + fm_cols
with open(f"{OUT}/player_fm_join.csv", "w") as f:
    f.write(",".join(cols) + "\n")
    for r in out_rows:
        f.write(",".join(str(r.get(c, "")) for c in cols) + "\n")

print(f"matched {matched}/{len(players)} players to FM26 attributes")

# Show a sample: CFM + Miami stars with their FM numbers
for team in ("CF Montréal", "Inter Miami CF"):
    rows = [r for r in out_rows if r["team"] == team and r.get("fm_matched")]
    rows.sort(key=lambda r: -int(r["minutes"]))
    print(f"\n{team} (FM-matched):")
    for r in rows[:6]:
        print(f"  {r['name']:<22} ca={r.get('fm_ca')} finish={r.get('fm_finishing')} "
              f"technique={r.get('fm_technique')} pace={r.get('fm_pace')} cons={r.get('fm_consistency')}")
