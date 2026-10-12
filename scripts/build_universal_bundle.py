#!/usr/bin/env python3
"""Build a league-universal pre-match bundle for any EPL fixture set.

Covers all 20 canonical clubs from the official registration snapshot (senior
+ U21, loaned-out excluded) -- never a fixture shortlist. For each eligible
player the bundle carries an as-of shot-rate baseline plus the 11 pooled
residual features, so the Rust engine can apply the fitted EPL+MLS head
internally with no Python-side rate math at forecast time.

Per-player baseline contract (all strictly pre-cutoff):
- ESPN-linked players (verified crosswalk FM uid -> ESPN athlete history
  before as_of): empirical-Bayes (past_shots + 5*role_prior)/(past_min/90 + 5)
  with position-aware role priors (G0.02/D0.40/M0.90/F2.20, unknown 0.90).
- Everyone else: the cold-start role prior itself (documented weakness, never
  a fabricated history).
- xG is a neutral conversion proxy (baseline * 0.105); FM-differentiated
  conversion is a gated challenger and stays out of the bundle.
- FM traits enter only for squad-verified + union-usable rows
  (fm_row_usable gate); everything else is masked with fm_verified=0.
- minutes_prior is career historical minutes (experience weight for lineup
  sampling), not current-season form.

Team history comes from the curated as-of CSV (goals + observed shots) plus
Championship E1 priors. Output schema extends math_shot_goal_v1 with
pooled_feature_names / pooled_features / bundle_provenance so both the
existing and the heads-aware engine paths accept it. Refuses overwrite.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from fit_pooled_epl_mls_shot_model import POOLED_FEATURE_NAMES  # noqa: E402
from fit_unified_forecast_heads import (  # noqa: E402
    crosswalk_key,
    fm_feature_block,
    load_crosswalk,
    load_fm_union,
)

ROLE_PRIORS = {"goalkeeper": 0.02, "defender": 0.40, "midfield": 0.90,
               "attack": 2.20, "unknown": 0.90}
CONVERSION_NEUTRAL = 0.105
COLD_LAST_SEEN = math.log1p(120) / 5.0

# ESPN scoreboard names -> canonical squad names. Closed map; unknown names
# fail loudly instead of aliasing into the wrong club.
ESPN_TO_CANONICAL = {
    "AFC Bournemouth": "Bournemouth",
    "Brighton & Hove Albion": "Brighton",
    "Coventry City": "Coventry",
    "Hull City": "Hull",
    "Ipswich Town": "Ipswich",
    "Leeds United": "Leeds",
    "Manchester City": "Man City",
    "Manchester United": "Man United",
    "Newcastle United": "Newcastle",
    "Nottingham Forest": "Nott'm Forest",
    "Tottenham Hotspur": "Tottenham",
    "West Ham United": "West Ham",
    "Wolverhampton Wanderers": "Wolves",
    "Arsenal": "Arsenal", "Aston Villa": "Aston Villa", "Brentford": "Brentford",
    "Chelsea": "Chelsea", "Crystal Palace": "Crystal Palace",
    "Everton": "Everton", "Fulham": "Fulham", "Liverpool": "Liverpool",
    "Leicester City": "Leicester", "Southampton": "Southampton",
    "Sunderland": "Sunderland",
}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def fnv1a64(text: str) -> int:
    h = 0xCBF29CE484222325
    for byte in text.encode("utf-8"):
        h ^= byte
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def provisional_id(club: str, official_name: str) -> int:
    """Stable provisional u64 for players without an FM uid. Top bit set so
    the provisional space can never collide with native FM uids."""
    return fnv1a64(f"official:{club}:{official_name}") | (1 << 63)


def canonical_club(name: str) -> str:
    try:
        return ESPN_TO_CANONICAL[name.strip()]
    except KeyError:
        raise ValueError(f"unknown club identity {name!r}; refusing to alias")


def role_of(historical_position) -> str:
    role = (historical_position or "").strip().lower()
    return role if role in ROLE_PRIORS and role != "unknown" else "unknown"


def fixtures_from_tsv(path: Path, squads: dict, as_of: date) -> list[dict]:
    fixtures = []
    with path.open(encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            day = date.fromisoformat(row["date"].strip())
            if day <= as_of:
                raise ValueError(f"fixture {row} is not after cutoff {as_of}")
            home, away = row["home"].strip(), row["away"].strip()
            if home == away:
                raise ValueError("same home and away club")
            for club in (home, away):
                if club not in squads:
                    raise ValueError(f"fixture club has no registered squad: {club}")
            fixtures.append({"date": row["date"].strip(), "day": day.toordinal(),
                             "home": home, "away": away})
    if not fixtures:
        raise ValueError("no future fixtures")
    return fixtures


def fixtures_from_schedule(paths: list[Path], squads: dict, as_of: date) -> list[dict]:
    fixtures = []
    for path in paths:
        board = json.loads(path.read_text())
        for event in board.get("events", []):
            status = event.get("status", {}).get("type", {})
            if status.get("completed"):
                raise ValueError(f"schedule contains completed match: {event.get('name')}")
            if status.get("description") != "Scheduled":
                continue
            kickoff = event.get("date", "")
            day = date.fromisoformat(kickoff[:10])
            if day <= as_of:
                raise ValueError(f"scheduled fixture is not after cutoff: {event.get('name')}")
            competitors = event.get("competitions", [{}])[0].get("competitors", [])
            if len(competitors) != 2:
                raise ValueError(f"cannot resolve home/away for {event.get('name')}")
            sides = {c.get("homeAway"): c for c in competitors}
            try:
                home = canonical_club(sides["home"]["team"]["displayName"])
                away = canonical_club(sides["away"]["team"]["displayName"])
            except KeyError as exc:
                raise ValueError(f"unresolvable competitor in {event.get('name')}: {exc}") from exc
            for club in (home, away):
                if club not in squads:
                    raise ValueError(f"fixture club has no registered squad: {club}")
            fixtures.append({"date": day.isoformat(), "day": day.toordinal(),
                             "home": home, "away": away,
                             "espn_event_id": str(event.get("id")),
                             "kickoff_utc": kickoff})
    if not fixtures:
        raise ValueError("no scheduled future fixtures")
    seen = [(f["date"], f["home"], f["away"]) for f in fixtures]
    if len(set(seen)) != len(seen):
        raise ValueError("duplicate scheduled fixtures")
    return sorted(fixtures, key=lambda f: (f["date"], f["home"]))


def espn_histories(paths: list[Path], as_of: date) -> dict[tuple[str, str], dict]:
    history: dict[tuple[str, str], dict] = {}
    for path in paths:
        archive = json.loads(path.read_text())
        matches = archive.get("matches", [])
        for match in matches:
            day = date.fromisoformat(match["date"])
            if day >= as_of:
                continue
            for p in match["players"]:
                key = (p["club"], str(p["espn_id"]))
                st = history.setdefault(key, {"shots": 0, "minutes": 0.0,
                                              "last_date": None})
                st["shots"] += int(p["shots"]) if p["shots"] is not None else 0
                st["minutes"] += float(p["minutes"])
                st["last_date"] = day
    return history


def team_history(path: Path, as_of: date) -> tuple[list[dict], int]:
    rows = []
    with path.open(encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle):
            day = date.fromisoformat(row["date"])
            if day >= as_of or row["home"] == row["away"]:
                continue
            shots = [None, None]
            for i, key in enumerate(("home_shots", "away_shots")):
                raw = (row.get(key) or "").strip()
                if raw not in ("", "NaN"):
                    value = float(raw)
                    if math.isfinite(value) and value >= 0:
                        shots[i] = value
            rows.append({"day": day.toordinal(), "home": row["home"],
                         "away": row["away"],
                         "home_goals": float(row["home_goals"]),
                         "away_goals": float(row["away_goals"]),
                         "home_shots": shots[0], "away_shots": shots[1]})
    if len(rows) < 30:
        raise ValueError("insufficient dated historical fixtures")
    return rows, len({(r["day"], r["home"], r["away"]) for r in rows})


def championship(path: Path, first_day: int) -> list[dict]:
    rows = []
    with path.open(encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            if row["division"] != "E1":
                continue
            day = int(row["day"]) + 719163
            if day >= first_day or row["home"] == row["away"]:
                continue
            rows.append({"day": day, "home": row["home"], "away": row["away"],
                         "home_goals": float(row["home_goals"]),
                         "away_goals": float(row["away_goals"]),
                         "home_shots": None, "away_shots": None})
    return rows


def build_bundle(*, squads_path: Path, fixtures: list[dict], as_of: date,
                 espn_paths: list[Path], crosswalk_path: Path,
                 history_path: Path, results_path: Path) -> dict:
    squads_raw = json.loads(squads_path.read_text())
    snapshot_day = date.fromisoformat(squads_raw["as_of"])
    if snapshot_day > as_of:
        raise ValueError("squad snapshot is newer than the prediction cutoff")
    clubs: dict[str, list[dict]] = squads_raw["teams"]
    if len(clubs) != 20:
        raise ValueError(f"expected 20 registered clubs, got {len(clubs)}")

    crosswalk = load_crosswalk(crosswalk_path)
    fm_to_espn: dict[str, tuple[str, str]] = {}
    ambiguous: set[str] = set()
    history = espn_histories(espn_paths, as_of)
    for espn_key in history:
        fm_uid = crosswalk.get(crosswalk_key(espn_key[0], espn_key[1]))
        if fm_uid is None:
            continue
        if fm_uid in fm_to_espn:
            ambiguous.add(fm_uid)
        else:
            fm_to_espn[fm_uid] = espn_key
    wanted = {str(p.get("fm_uid")) for players in clubs.values()
              for p in players["players"]
              if p.get("fm_uid") and not p.get("loaned_out")}
    fm_usable = load_fm_union(sorted(wanted | set(crosswalk.values())))

    players: list[dict] = []
    seen_ids: set[int] = set()
    counts = {"corrected_link": 0, "cold_start": 0, "fm_verified": 0,
              "provisional_ids": 0, "ambiguous_skipped": 0}
    source_counts: dict[str, int] = {}
    kind_counts: dict[str, int] = {}
    trusted = 0
    for club in sorted(clubs):
        for entry in clubs[club]["players"]:
            if entry.get("loaned_out"):
                continue
            fm_uid = entry.get("fm_uid")
            pid = int(fm_uid) if fm_uid else provisional_id(club, entry["official_name"])
            if pid in seen_ids:
                raise ValueError(f"duplicate player id {pid} ({entry['official_name']})")
            seen_ids.add(pid)
            if fm_uid is None:
                counts["provisional_ids"] += 1
            role = role_of(entry.get("historical_position"))
            prior = ROLE_PRIORS[role]
            minutes_raw = entry.get("historical_minutes_2526")
            try:
                minutes_prior = float(minutes_raw) if minutes_raw not in (None, "") else 0.0
            except ValueError:
                raise ValueError(f"invalid historical minutes for {entry['official_name']}")
            if minutes_prior < 0 or not math.isfinite(minutes_prior):
                raise ValueError(f"invalid historical minutes for {entry['official_name']}")

            fm = fm_usable.get(str(fm_uid)) if fm_uid else None
            squad_verified = bool(entry.get("fm_attributes_verified"))
            usable = fm is not None and squad_verified
            if usable:
                trusted += 1
                counts["fm_verified"] += 1
            source = "db" if usable else "unverified_missing"
            source_counts[source] = source_counts.get(source, 0) + 1
            kind_counts["player/db" if usable else "unverified"] = \
                kind_counts.get("player/db" if usable else "unverified", 0) + 1

            st = None
            if fm_uid and str(fm_uid) not in ambiguous:
                espn_key = fm_to_espn.get(str(fm_uid))
                st = history.get(espn_key) if espn_key else None
            elif fm_uid:
                counts["ambiguous_skipped"] += 1
            if st is not None:
                baseline = ((st["shots"] + 5.0 * prior) / (st["minutes"] / 90.0 + 5.0))
                gap = math.log1p(max(0, (as_of - st["last_date"]).days)) / 5.0
                counts["corrected_link"] += 1
            else:
                baseline = prior
                gap = COLD_LAST_SEEN
                counts["cold_start"] += 1
            block = fm_feature_block(
                fm if usable else None, fm.get("dob", "") if usable else "", as_of)
            pooled = [0.0, block["age"], block["ca"], *block["attrs"], gap,
                      block["verified"]]
            if len(pooled) != len(POOLED_FEATURE_NAMES):
                raise ValueError("pooled feature contract drift")
            players.append({
                "id": pid, "club": club,
                "minutes_prior": minutes_prior,
                "shots_per90": baseline,
                "xg_per90": baseline * CONVERSION_NEUTRAL,
                "is_goalkeeper": role == "goalkeeper",
                "position_source": ("registered_historical_position" if role != "unknown"
                                    else "unknown_role_neutral"),
                "fm_source": "db" if usable else "unverified_missing",
                "fm_finishing": None, "fm_ca": None,
                "fm_uid": fm_uid, "official_name": entry["official_name"],
                "start_probability": None,
                "expected_minutes_if_start": None,
                "bench_appearance_probability": None,
                "pooled_features": pooled,
            })
    # Every fixture club needs a feasible 1GK+10OF pool at simulation time.
    for fixture in fixtures:
        for side in (fixture["home"], fixture["away"]):
            pool = [p for p in players if p["club"] == side]
            if sum(p["is_goalkeeper"] for p in pool) < 1 or \
                    sum(not p["is_goalkeeper"] for p in pool) < 10:
                raise ValueError(f"no feasible 1GK+10OF pool for {side}")

    history_rows, _ = team_history(history_path, as_of)
    first_day = min(f["day"] for f in fixtures)
    champ_rows = championship(results_path, first_day)
    return {
        "schema": "math_shot_goal_v1",
        "as_of": as_of.isoformat(),
        "fm_effective": "2026-03-01",
        "sources": {},
        "contracts": {
            "player_rates": "as-of EB baselines for ESPN-linked players; cold-start "
                            "role priors otherwise; xG is a neutral conversion proxy",
            "roles": "registered historical_position; unknown stays role-neutral",
            "history": "only observations strictly before as_of; no same-day results",
            "fm_traits": "squad-verified AND union-usable rows only; rest masked",
            "identity": "FM uids are squad-asserted; missing uids get stable "
                        "provisional ids (top bit set), never guessed FM uids",
        },
        "fm_source_counts": source_counts,
        "fm_record_kind_counts": kind_counts,
        "fm_trusted_attributes": trusted,
        "pooled_feature_names": list(POOLED_FEATURE_NAMES),
        "bundle_provenance": {
            "coverage": counts,
            "eligible_players": len(players),
            "squad_snapshot": squads_raw["as_of"],
        },
        "missing_shot_matches": sum(1 for r in history_rows
                                    if r["home_shots"] is None),
        "history": history_rows,
        "championship": champ_rows,
        "players": players,
        "fixtures": fixtures,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--squads", type=Path, default=ROOT / "data/pl/squads_2026-10-11_full.json")
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--fixtures-tsv", type=Path)
    group.add_argument("--schedule", type=Path, nargs="+")
    ap.add_argument("--as-of", required=True)
    ap.add_argument("--espn", type=Path, nargs="+", default=[
        ROOT / "data/ml/math_engine/espn_frozen_20261011.json",
        ROOT / "data/ml/math_engine/espn_extension_20261012.json"])
    ap.add_argument("--crosswalk", type=Path, default=
        ROOT / "data/ml/math_engine/espn_pipeline_final_oct12/crosswalk.json")
    ap.add_argument("--history", type=Path, default=ROOT / "data/pl/asof_team_stats.csv")
    ap.add_argument("--results", type=Path, default=ROOT / "data/pl/results.tsv")
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    if a.output.exists():
        ap.error("refusing to overwrite immutable bundle")
    as_of = date.fromisoformat(a.as_of)
    squads_raw = json.loads(a.squads.read_text())
    fixtures = (fixtures_from_tsv(a.fixtures_tsv, squads_raw["teams"], as_of)
                if a.fixtures_tsv else
                fixtures_from_schedule(a.schedule, squads_raw["teams"], as_of))
    bundle = build_bundle(squads_path=a.squads, fixtures=fixtures, as_of=as_of,
                          espn_paths=a.espn, crosswalk_path=a.crosswalk,
                          history_path=a.history, results_path=a.results)
    inputs = [a.squads, a.history, a.results, *(a.schedule or []), *a.espn,
              a.crosswalk]
    bundle["sources"] = {str(p): sha256(p) for p in inputs}
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(bundle, sort_keys=True, separators=(",", ":")) + "\n")
    print(f"{a.output}: {len(bundle['players'])} players, "
          f"{len(bundle['history'])} prior matches, "
          f"{len(bundle['championship'])} Championship games, "
          f"{len(bundle['fixtures'])} fixtures; {bundle['bundle_provenance']['coverage']}")


if __name__ == "__main__":
    main()
