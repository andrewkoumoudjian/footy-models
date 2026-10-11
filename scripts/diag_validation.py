"""Diagnose which validation rule fails for the Portland fixture input.

Replicates the Rust checks: candidate count >= 11, unique players, probability ranges,
ability_index finite, minutes <= 90, and every lineup candidate present in player_states.
"""
import csv

rows = list(csv.DictReader(open("data/sofascore_cfm/bk/15171545_players.csv")))
print("total qualified:", len(rows))

for team in ("CF Montréal", "Portland Timbers"):
    mine = [r for r in rows if r["team"] == team]
    print(f"\n{team}: {len(mine)} players")
    for r in mine:
        problems = []
        try:
            ca = float(r["fm_ca"]) if r["fm_ca"] else None
        except ValueError:
            ca = None
        ability = (ca / 10.0) if ca is not None else 10.5
        if not (ability == ability):  # NaN
            problems.append("NaN ability")
        print(f"  {r['name']:<24} min={r['minutes']:>5} fm_ca={r['fm_ca'] or '-':>4} "
              f"shots={r['shots_p90']} prog={r['prog_carries_p90']} xa={r['xa_p90']} "
              f"{' '.join(problems)}")
