#!/usr/bin/env python3
"""Read-only aggregate audit of the dated EPL forecasting input boundary.

Never exports player names, raw FM attributes or licensed observations. The
presence of an FM UID alone is not proof of a verified historical player.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def audit(squads: Path, players: Path, team_rows: Path, fm: Path, raw_results: Path) -> dict:
    roster = json.loads(squads.read_text())
    with players.open(newline="", encoding="utf-8-sig") as stream:
        predicted = list(csv.DictReader(stream))
    with team_rows.open(newline="", encoding="utf-8-sig") as stream:
        matches = list(csv.DictReader(stream))
    if not predicted or not matches:
        raise ValueError("missing player inputs or team history")

    simulated_ids = [row["player_id"] for row in predicted]
    if len(simulated_ids) != len(set(simulated_ids)):
        raise ValueError("duplicate FM UID in the simulated player pool")

    declared = [row for club in roster["teams"].values() for row in club["players"]]
    matched = [row for row in declared if row.get("fm_uid")]
    attributes_by_id: dict[str, tuple[str, str]] = {}
    simulated_id_set = set(simulated_ids)
    with fm.open(newline="", encoding="utf-8-sig") as stream:
        for row in csv.DictReader(stream):
            uid = row.get("uid", "")
            if uid not in simulated_id_set:
                continue
            provenance = (row.get("kind", "unknown"), row.get("attr_source", "unknown"))
            if uid in attributes_by_id and attributes_by_id[uid] != provenance:
                raise ValueError("multiple inconsistent FM records for one simulated player")
            attributes_by_id[uid] = provenance
    source_classes = Counter(
        f"{attributes_by_id.get(uid, ('missing', 'missing'))[0]}:"
        f"{attributes_by_id.get(uid, ('missing', 'missing'))[1]}"
        for uid in simulated_ids
    )
    empty_selection = sum(
        all(float(row[field]) == 0.0 for field in
            ("selection_listings", "selection_starts", "team_fixtures"))
        for row in predicted
    )
    zeroed_appearance_dates = sum(not row.get("last_appearance_date") for row in predicted)

    # The live prep script uses only *_E0 result files for team history and
    # substitutes observed final goals for a source-missing xG. Count the true
    # measured xG rows, not how plausible the final proxy values look.
    measured_xg = 0
    n_complete_source_matches = 0
    for path in sorted(raw_results.glob("*_E0.csv")):
        with path.open(newline="", encoding="utf-8-sig") as stream:
            for row in csv.DictReader(stream):
                if not row.get("FTHG") or not row.get("FTAG"):
                    continue
                date = row.get("Date", "")
                if not date:
                    continue
                from datetime import datetime
                day = datetime.strptime(date, "%d/%m/%Y").date().isoformat()
                if day >= roster["as_of"]:
                    continue
                n_complete_source_matches += 1
                measured_xg += bool(row.get("HxG") and row.get("AxG"))

    source_dates = [row["date"] for row in matches]
    has_quality_flags = "home_xg_kind" in matches[0] and "away_xg_kind" in matches[0]
    if has_quality_flags:
        source_measured = sum(
            row["home_xg_kind"] == "measured" and row["away_xg_kind"] == "measured"
            for row in matches
        )
        source_missing = sum(
            row["home_xg_kind"] == "missing" or row["away_xg_kind"] == "missing"
            for row in matches
        )
        if source_measured + source_missing != len(matches):
            raise ValueError("mixed or unsupported xG observation kinds")
        if any(
            (row["home_xg_kind"] == "missing" and row["home_xg"])
            or (row["away_xg_kind"] == "missing" and row["away_xg"])
            for row in matches
        ):
            raise ValueError("xG measurement says missing but contains a numeric proxy")
    else:
        source_measured, source_missing = measured_xg, 0
    summary = {
        "schema_version": 2 if has_quality_flags else 1,
        "forecast_input_as_of": roster["as_of"],
        "scope": "dated 8-club EPL forecast candidate set, not the full league",
        "registered_and_u21_listings": len(declared),
        "roster_rows_with_fm_uid": len(matched),
        "roster_unique_fm_uids": len({str(row["fm_uid"]) for row in matched}),
        "simulated_player_rows": len(predicted),
        "simulated_player_classification": dict(sorted(source_classes.items())),
        "player_rows_without_observed_start_history": empty_selection,
        "player_rows_without_last_appearance_date": zeroed_appearance_dates,
        "team_result_rows": len(matches),
        "team_result_min_date": min(source_dates),
        "team_result_max_date": max(source_dates),
        "original_completed_E0_results": n_complete_source_matches,
        "additional_dated_results_not_in_cached_E0": len(matches) - n_complete_source_matches,
        "original_E0_results_with_both_measured_xg": measured_xg,
        "team_history_xg_measured_rows": source_measured,
        "team_history_xg_missing_rows": source_missing,
        "team_history_xg_proxy_rows": 0 if has_quality_flags else len(matches) - measured_xg,
        "source_hashes": {
            "registered_squads": digest(squads),
            "generated_player_inputs": digest(players),
            "generated_team_inputs": digest(team_rows),
            "raw_fm": digest(fm),
        },
        "not_a_calibration_result": True,
        "provenance_limitations": [
            "FM staff/derived records require independent person/role verification",
            "No player event/lineup observations enter current live player inputs",
            "Measured xG coverage is limited; missing xG must be marginalized by the forecasting adapter" if has_quality_flags
            else "Legacy source-missing xG was imputed from realized goals without a flag",
            "Raw external source publish-time and past roster-availability are not audited here",
        ],
    }
    if n_complete_source_matches > len(matches) or measured_xg > len(matches):
        raise ValueError("raw E0 matches and generated team rows are inconsistent")
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--squads", type=Path, default=ROOT / "data/pl/squads_2026-10-10.json")
    parser.add_argument("--players", type=Path, default=ROOT / "data/pl/player_fm_join.csv")
    parser.add_argument("--team-rows", type=Path, default=ROOT / "data/pl/asof_team_stats.csv")
    parser.add_argument("--raw-results", type=Path, default=ROOT / "data/pl/raw")
    parser.add_argument("--fm", type=Path, default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = audit(args.squads, args.players, args.team_rows, args.fm, args.raw_results)
    output = json.dumps(result, indent=2) + "\n"
    if args.output:
        if args.output.exists():
            raise FileExistsError("refusing to overwrite a coverage audit")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output)
    print(output, end="")


if __name__ == "__main__":
    main()
