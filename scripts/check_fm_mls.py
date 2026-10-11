"""Check the FM26 full dataset (fm26_final_men.csv) for MLS club coverage + Messi attributes."""
import csv, sys

csv.field_size_limit(10_000_000)
PATH = "/home/drewclaw/fm26-data/fm26_final_men.csv"

mls_names = {"inter miami", "cf montréal", "cf montreal", "montreal impact",
             "vancouver whitecaps", "philadelphia union"}

with open(PATH) as f:
    header = f.readline().strip().split(",")
    club_col = None
    for i, c in enumerate(header):
        if c.lower() in ("club", "team", "current_club"):
            club_col = i
            break
    print("columns:", len(header), "| club col:", header[club_col] if club_col is not None else "NONE")
    if club_col is None:
        # print candidate columns
        print([c for c in header if "club" in c.lower() or "league" in c.lower() or "division" in c.lower()])
        sys.exit(0)
    hits = []
    count = 0
    for line in f:
        row = line.strip().split(",")
        count += 1
        if len(row) <= club_col:
            continue
        club = row[club_col].strip().lower()
        if any(t in club for t in mls_names):
            hits.append((row[1], row[club_col]))
    print("scanned:", count, "| MLS-club players found:", len(hits))
    for name, club in hits[:15]:
        print("  ", name, "|", club)
