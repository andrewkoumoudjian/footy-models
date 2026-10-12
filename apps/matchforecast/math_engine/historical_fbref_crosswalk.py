#!/usr/bin/env python3
"""ID-linked, historical FBref-season + Understat research sidecar.

This enriches *candidate identity evidence*, not a legacy 248-feature bundle.
The available HF FBref table has season aggregates, NOT ten prior match logs.
Reep-linked official EPL identities remain provisional pending verification.
No historical table here contains MLS, and no matching records are relabelled.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from datetime import date, datetime, timezone
import hashlib
import json
import math
from pathlib import Path


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_csv(path):
    with path.open(encoding="utf-8-sig", newline="") as handle:
        yield from csv.DictReader(handle)


def numeric(value):
    if value is None or value == "":
        return None
    x = float(value.replace(",", ""))
    if not math.isfinite(x) or x < 0:
        raise ValueError("impossible historical numeric observation")
    return x


def end_year(season):
    value = season.split("-")
    if len(value) != 2 or len(value[0]) != 4 or len(value[1]) != 4:
        raise ValueError("expected YYYY-YYYY source season")
    lo, hi = map(int, value)
    if hi != lo + 1:
        raise ValueError("nonconsecutive source season")
    return hi


def source_clock(paths, at):
    """A locally acquired file cannot have been used before it existed here."""
    for path in paths:
        when = datetime.fromtimestamp(path.stat().st_mtime, timezone.utc)
        if when >= at:
            raise ValueError(f"local historical archive/bundle postdates feature cutoff: {path}")


def build(bundle, rows, understat, cutoff):
    if bundle.get("schema") != "unified_epl_mls_player_set_v1":
        raise ValueError("unsupported input bundle schema")
    if bundle.get("league") != "EPL" or bundle.get("roster_scope") != "full_league":
        raise ValueError("source is historical EPL; requires full 20-club EPL bundle")
    if bundle["team_count"] != 20 or bundle["player_count"] < 1500:
        raise ValueError("historical EPL source cannot substitute an eight-club roster")
    if date.fromisoformat(bundle["forecast_cutoff_date"]) >= cutoff.date():
        raise ValueError("this is a LATER-vintage sidecar, not a retroactive modification")
    fbref = defaultdict(list)
    for row in rows:
        if row.get("league") != "EPL":
            continue
        year = end_year(row["season"])
        if date(year, 7, 1) >= cutoff.date():
            continue
        ident = str(row.get("player_id") or "").strip()
        if not ident:
            continue
        fbref[ident].append({
            "season": row["season"],
            "provider_squad": row["squad"],
            "matches": numeric(row["mp"]),
            "starts": numeric(row["starts"]),
            "minutes": numeric(row["min"]),
            "goals": numeric(row["gls"]),
            "assists": numeric(row["ast"]),
            "identity_source": "FBref provider player_id",
        })
    # This second source is a prior full-season, explicit same-provider-FBref-ID
    # and same club join already verified on dated historical aggregates.
    quality = defaultdict(list)
    if understat is not None:
        if understat.get("schema") != "fbref_understat_season_v1":
            raise ValueError("unverified Understat-FBref join")
        for x in understat["players"]:
            if x["league"] == "EPL" and date.fromisoformat(x["available_from"]) < cutoff.date():
                quality[(x["fbref_player_id"], x["season"])].append(x)
    counts = Counter()
    sidecar = []
    attached = {}
    for club_name, club in sorted(bundle["teams"].items()):
        for candidate in club["players"]:
            ids = candidate["provider_ids"].get("fbref_person_ids_candidate") or []
            cid = candidate["canonical_player_id"]
            if cid in attached:
                raise ValueError("duplicate original EPL roster candidate")
            seen = sorted(set(map(str, ids)))
            if not seen:
                counts["no_reep_fbref_id_candidate"] += 1
                continue
            if len(seen) != 1:
                counts["multiple_reep_fbref_ids_ambiguous"] += 1
                continue
            person_id = seen[0]
            seasons = fbref.get(person_id, [])
            if not seasons:
                counts["linked_id_no_prior_season_rows"] += 1
                continue
            latest = max(x["season"] for x in seasons)
            source_rows = [x for x in seasons if x["season"] == latest]
            # A full season can have multi-club rows. Keep ALL such rows
            # separate rather than double counting combined "2 Squads" rows.
            if any(x["starts"] is not None and x["matches"] is not None
                   and x["starts"] > x["matches"] for x in source_rows):
                raise ValueError("FBref historical starts exceed appearances")
            prior_xg = quality.get((person_id, latest), [])
            xg_rows = [{
                "historic_club": q["club"],
                "shots": q["understat_shots"],
                "xg": q["understat_xg"],
                "npxg": q["understat_npxg"],
                "verified_prior_season_club_and_provider_id": True,
            } for q in prior_xg]
            if len({q["historic_club"] for q in xg_rows}) != len(xg_rows):
                raise ValueError("duplicate historical provider season-club identity")
            record = {
                "canonical_player_id": cid,
                "current_official_club": club_name,
                "fbref_person_id": person_id,
                "source_retrieval_condition": "feature_cutoff_must_follow_local_archive",
                "identity_for_current_registration": "CANDIDATE_ONLY_UNVERIFIED",
                "historical_season": latest,
                "historical_season_available_conservatively_from": f"{end_year(latest)}-07-01",
                "prior_full_season_rows_not_recent_10_match_logs": source_rows,
                "prior_understat_crossverified_club_season_rows": xg_rows,
                "training_feature_admission": False,
                "reason_training_admission_false": (
                    "official-registration to FM/Reep person identity still name-based; "
                    "historical FBref data licensing and retrospective availability require review"),
            }
            attached[cid] = record
            sidecar.append(record)
            counts["matched_provider_fbref_id_previous_season"] += 1
            if xg_rows:
                counts["with_prior_understat_xg_rows"] += 1
    return {
        "schema": "historical_fbref_candidate_sidecar_v1",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "feature_cutoff_utc": cutoff.isoformat(),
        "bundle_vintage": bundle["forecast_cutoff_date"],
        "leagues_supported_by_source": ["EPL"],
        "retains_original_248_feature_schema": True,
        "source_is_match_log": False,
        "reasons_not_fit_ready": [
            "Season-level aggregates are not recent ten-match statistics.",
            "Official EPL registration to Reep identity is not yet fully verified.",
            "Source permission for model-training and public reuse must be reviewed.",
            "Historical data snapshot was acquired after the original October 11 bundle.",
            "No MLS rows appear in the provided historical table.",
        ],
        "full_league_player_count": bundle["player_count"],
        "candidate_coverage": dict(sorted(counts.items())),
        "historical_candidate_records": sidecar,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--fbref", required=True, type=Path)
    parser.add_argument("--understat-joined", type=Path)
    parser.add_argument("--cutoff-utc", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("immutable audit already exists")
    cutoff = datetime.fromisoformat(args.cutoff_utc.replace("Z", "+00:00"))
    if cutoff.tzinfo is None:
        parser.error("cutoff requires an explicit timezone")
    cutoff = cutoff.astimezone(timezone.utc)
    if cutoff > datetime.now(timezone.utc):
        parser.error("cannot assert availability at a future time")
    files = [args.bundle, args.fbref]
    if args.understat_joined:
        files.append(args.understat_joined)
    source_clock(files, cutoff)
    result = build(json.loads(args.bundle.read_text()), read_csv(args.fbref),
                   json.loads(args.understat_joined.read_text()) if args.understat_joined else None,
                   cutoff)
    result["file_sha256"] = {str(p): sha(p) for p in files}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    print(json.dumps({"players": result["full_league_player_count"],
                     "candidate_coverage": result["candidate_coverage"]}, indent=2))


if __name__ == "__main__":
    main()
