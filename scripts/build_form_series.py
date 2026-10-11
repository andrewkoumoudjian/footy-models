"""Build per-player per-match output series for causal next-match rate/form models.

For each backtest fixture, emits data/sofascore_cfm/bk_form/<mid>_form.csv:
player_id, name, team, date, minutes, shots, xg, xa, passes, prog_carries -- one row per player-match,
strictly before the fixture cutoff. The Rust side fits pooled next-match rate models
from prior-only histories and retains the 2-state HMM as a sparse-history fallback.
"""
import argparse, csv, json, glob, os
from collections import defaultdict

OUT = "data/sofascore_cfm"
os.makedirs(f"{OUT}/bk_form", exist_ok=True)
os.makedirs(f"{OUT}/asof_form", exist_ok=True)

parser = argparse.ArgumentParser()
parser.add_argument(
    "--live-cutoff",
    help="Optional YYYY-MM-DD cutoff for a production/as-of artifact; rows on or after it are excluded.",
)
args = parser.parse_args()

# match metadata from asof table
match_meta = {}
with open(f"{OUT}/asof_team_stats.csv") as f:
    header = f.readline().strip().split(",")
    for line in f:
        row = dict(zip(header, line.strip().split(",")))
        match_meta[row["match_id"]] = {
            "date": row["date"],
            "home": row["home_id"],
            "away": row["away_id"],
        }

fixtures = json.load(open(f"{OUT}/bk/fixtures.json"))
TEAMS = json.load(open(f"{OUT}/teams.json"))


def write_form_series(cutoff, destination):
    series = defaultdict(list)
    for path in glob.glob(f"{OUT}/player_matches/*.json"):
        m = os.path.basename(path)[:-5]
        meta = match_meta.get(m)
        if not meta or meta["date"] >= cutoff:
            continue
        try:
            d = json.load(open(path))
        except Exception:
            continue
        home_name = next((t for t, i in TEAMS.items() if str(i) == str(meta["home"])), None)
        away_name = next((t for t, i in TEAMS.items() if str(i) == str(meta["away"])), None)
        for side, team_name in (("home", home_name), ("away", away_name)):
            if not team_name:
                continue
            block = d.get(side) or {}
            for entry in block.get("players", []):
                st = entry.get("statistics") or {}
                mins = float(st.get("minutesPlayed") or 0)
                if mins <= 0:
                    continue
                pid = entry["player"]["id"]
                series[pid].append({
                    "player_id": pid,
                    "name": entry["player"]["name"],
                    "team": team_name,
                    "date": meta["date"],
                    "minutes": mins,
                    # SofaScore's player payload uses `totalShots` for attempts.  The old
                    # `shots` lookup silently produced zero for most rows, which effectively
                    # removed the highest-weight component of the Rust form signal.
                    "shots": float(st.get("totalShots") or st.get("shots") or st.get("shotsOnTarget") or 0),
                    "xg": float(st.get("expectedGoals") or 0),
                    "xa": float(st.get("expectedAssists") or 0),
                    "passes": float(st.get("totalPass") or 0),
                    "prog_carries": float(st.get("progressiveBallCarriesCount") or 0),
                })
    out_rows = []
    for pid, rows in series.items():
        rows.sort(key=lambda r: r["date"])
        out_rows.extend(rows)
    with open(destination, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=["player_id","name","team","date","minutes","shots","xg","xa","passes","prog_carries"])
        writer.writeheader()
        writer.writerows(out_rows)
    return len(out_rows)


for fx in fixtures:
    mid = str(fx["match_id"])
    cutoff = fx["date"]
    count = write_form_series(cutoff, f"{OUT}/bk_form/{mid}_form.csv")
    print(f"{mid} ({cutoff}): {count} player-match rows")

if args.live_cutoff:
    destination = f"{OUT}/asof_form/{args.live_cutoff}_form.csv"
    live_count = write_form_series(args.live_cutoff, destination)
    print(f"as-of {args.live_cutoff}: {live_count} player-match rows -> {destination}")

print("done")
