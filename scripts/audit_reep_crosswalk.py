#!/usr/bin/env python3
"""Measure verified Reep FM26/SofaScore/FBref bridges for EPL and MLS.

Reads a pinned, checksum-verified local DuckDB release. Never assigns missing
official EPL name-only registrations a person ID, never assumes provider IDs
are in the same namespace, and quarantines conflicting Reep identities.

Run: uv run --no-project --with duckdb python scripts/audit_reep_crosswalk.py
"""
from __future__ import annotations

import argparse
import csv
import json
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def sofa_players(root: Path) -> dict[str, dict]:
    identities: dict[str, dict] = {}
    for path in sorted((root / "player_matches").glob("*.json")):
        sheet = json.loads(path.read_text())
        if sheet.get("confirmed") is not True:
            continue
        for side in ("home", "away"):
            for event in (sheet.get(side) or {}).get("players", []):
                person = event.get("player") or {}
                if person.get("id") is None:
                    continue
                pid = str(person["id"])
                born = person.get("dateOfBirthTimestamp")
                dob = datetime.fromtimestamp(born, timezone.utc).date().isoformat() if isinstance(born, int) else None
                prior = identities.get(pid)
                if prior and prior["dob"] and dob and prior["dob"] != dob:
                    raise ValueError(f"SofaScore ID {pid} has conflicting birthdates")
                if not prior or not prior["dob"]:
                    identities[pid] = {"name": person.get("name", ""), "dob": dob}
    return identities


def resolve_redirect(reep_id: str, redirects: dict[str, str]) -> str:
    seen = set()
    while reep_id in redirects:
        if reep_id in seen:
            raise ValueError(f"Reep redirect cycle from {reep_id}")
        seen.add(reep_id)
        reep_id = redirects[reep_id]
    return reep_id


def read_bridges(conn, provider: str, namespace: str, redirects: dict[str, str]):
    edges = defaultdict(set)
    methods = defaultdict(set)
    for external, reep, rung in conn.execute(
        "SELECT external_id, reep_id, rung FROM bridges WHERE provider=? AND namespace=?",
        [provider, namespace],
    ).fetchall():
        edges[external].add(resolve_redirect(reep, redirects))
        methods[external].add(str(rung))
    return edges, methods


def single(edges: dict[str, set[str]], external: str) -> str | None:
    values = edges.get(str(external), set())
    return next(iter(values)) if len(values) == 1 else None


def fm_records(path: Path, uids: set[str]) -> dict[str, dict]:
    """Inspect raw FM person kind, DOB and attribute provenance for bridge targets."""
    records = {}
    with path.open(newline="", encoding="utf-8-sig") as handle:
        for row in csv.DictReader(handle):
            uid = row.get("uid")
            if uid not in uids:
                continue
            if uid in records:
                raise ValueError(f"duplicate FM UID {uid} in local export")
            records[uid] = {"name": row.get("name", ""), "dob": row.get("dob", ""),
                            "kind": row.get("kind", ""), "attr_source": row.get("attr_source", "")}
    return records


def classify_player_bridge(item: dict, fm: dict[str, dict]) -> tuple[str, dict | None]:
    """Do not promote staff records or mismatched birthdates to model inputs."""
    if len(item["fm_uids"]) != 1:
        return "ambiguous_fm_ids", None
    uid = item["fm_uids"][0]
    record = fm.get(uid)
    if record is None:
        return "not_in_local_fm_export", None
    if item["sofascore_dob"] and record["dob"] and item["sofascore_dob"] != record["dob"]:
        return "date_of_birth_conflict", record
    if record["kind"] != "player":
        return "fm_record_classified_nonplayer", record
    if not item["sofascore_dob"] or not record["dob"]:
        return "dob_not_independently_checked", record
    if record["attr_source"] == "db":
        return "linked_player_with_db_attributes_and_matching_dob", record
    if record["attr_source"] == "derived":
        return "linked_player_with_reconstructed_attributes_and_matching_dob", record
    return "linked_player_without_usable_attributes", record


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--active", type=Path, default=ROOT / "data/identity/reep/active.json")
    parser.add_argument("--squads", type=Path, default=ROOT / "data/identity/reep/epl_full_all_sources_v1.json")
    parser.add_argument("--sofascore", type=Path, default=ROOT / "data/sofascore_cfm")
    parser.add_argument("--fm", type=Path, default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--output", type=Path, default=ROOT / "data/identity/reep/coverage_full_all_sources_v1.json")
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite previous coverage audit; use a new output path")
    import duckdb

    active = json.loads(args.active.read_text())
    duckdb_path = Path(active["path"])
    with duckdb.connect(str(duckdb_path), read_only=True) as conn:
        redirects = dict(conn.execute("SELECT from_id, to_id FROM redirects").fetchall())
        bridges = {}
        methods = {}
        for provider, namespace in (
            ("fm", "player"), ("sofascore", "player"),
            ("fbref", "person"), ("transfermarkt", "spieler"),
        ):
            key = provider
            bridges[key], methods[key] = read_bridges(conn, provider, namespace, redirects)
    squads = json.loads(args.squads.read_text())
    candidate_players = [(club, p) for club, team in squads["teams"].items() for p in team["players"]]
    sofaplayers = sofa_players(args.sofascore)
    fm_assigned = [(club, p) for club, p in candidate_players if p.get("fm_uid")]
    fm_mapped = [(club, p, single(bridges["fm"], p["fm_uid"])) for club, p in fm_assigned]
    sofamapped = {sid: single(bridges["sofascore"], sid) for sid in sofaplayers}

    # A Reep entity can hold multiple provider IDs. Preserve all of them;
    # never choose an arbitrary FM UID merely because a Sofa ID is bridged.
    reep_to_fm = defaultdict(set)
    for uid, identities in bridges["fm"].items():
        for rid in identities:
            reep_to_fm[rid].add(uid)
    reep_to_fbref = defaultdict(set)
    for pid, identities in bridges["fbref"].items():
        for rid in identities:
            reep_to_fbref[rid].add(pid)
    reep_to_tm = defaultdict(set)
    for pid, identities in bridges["transfermarkt"].items():
        for rid in identities:
            reep_to_tm[rid].add(pid)
    sofa_all = [
        {"sofascore_id": sid, "reep_id": rid,
         "fbref_person_ids": sorted(reep_to_fbref[rid]),
         "transfermarkt_player_ids": sorted(reep_to_tm[rid]),
         "fm_uids": sorted(reep_to_fm[rid])}
        for sid, rid in sorted(sofamapped.items()) if rid
    ]
    joined, conflicts = [], []
    for sid, reep in sofamapped.items():
        if reep is None:
            continue
        fmids = sorted(reep_to_fm[reep])
        fbrefids = sorted(reep_to_fbref[reep])
        item = {"sofascore_id": sid, "name": sofaplayers[sid]["name"],
                "sofascore_dob": sofaplayers[sid]["dob"], "reep_id": reep,
                "fm_uids": fmids, "fbref_person_ids": fbrefids,
                "fm_identity_method": sorted(methods["fm"].get(fmids[0], [])) if len(fmids) == 1 else [],
                "sofascore_identity_method": sorted(methods["sofascore"].get(sid, [])),
                "usable_single_fm_uid": len(fmids) == 1}
        if len(fmids) > 1:
            conflicts.append(item)
        if fmids:
            joined.append(item)

    local_fm = fm_records(args.fm, {uid for item in joined for uid in item["fm_uids"]})
    quality_counts = Counter()
    dob_conflicts = []
    for item in joined:
        status, record = classify_player_bridge(item, local_fm)
        item["local_fm_status"] = status
        item["local_fm_name"] = record["name"] if record else None
        item["local_fm_dob"] = record["dob"] if record else None
        item["local_fm_kind"] = record["kind"] if record else None
        item["local_attribute_source"] = record["attr_source"] if record else None
        item["allowed_as_exact_attributes"] = status == "linked_player_with_db_attributes_and_matching_dob"
        item["allowed_as_distributional_attributes"] = status == "linked_player_with_reconstructed_attributes_and_matching_dob"
        quality_counts[status] += 1
        if status == "date_of_birth_conflict":
            dob_conflicts.append(item)

    result = {
        "source": "Reep public identity register DuckDB / CC0",
        "stamp": active["stamp"],
        "sha256": active["sha256"],
        "source_contract": "direct typed provider-ID bridges only; redirects resolved; no name-only IDs",
        "provider_bridge_total": {name: sum(len(s) for s in edges.values()) for name, edges in bridges.items()},
        "epl_registrations": len(candidate_players),
        "epl_fm_uids_already_assigned": len(fm_assigned),
        "epl_fm_uids_with_one_reep_id": sum(bool(rep) for _, _, rep in fm_mapped),
        "epl_fm_uids_with_conflicting_reep_ids": sum(len(bridges["fm"].get(str(p["fm_uid"]), [])) > 1 for _, p in fm_assigned),
        "epl_registration_rows_still_unresolved": len(candidate_players) - len(fm_assigned),
        "sofascore_distinct_local_players": len(sofaplayers),
        "sofascore_ids_with_one_reep_id": sum(bool(v) for v in sofamapped.values()),
        "sofascore_ids_with_multiple_reep_ids": sum(len(bridges["sofascore"].get(sid, [])) > 1 for sid in sofaplayers),
        "sofascore_players_with_reep_and_fm": len(joined),
        "sofascore_players_with_reep_and_one_fm_uid": sum(item["usable_single_fm_uid"] for item in joined),
        "sofascore_players_with_reep_and_fbref_id": sum(bool(item["fbref_person_ids"]) for item in joined),
        "sofascore_to_fm_ambiguous_entities": len(conflicts),
        "sofascore_fm_local_quality_status": dict(quality_counts),
        "sofascore_dob_conflicts": dob_conflicts,
        "sofascore_reep_links_all": sofa_all,
        "sofascore_fm_links": joined,
        "sofascore_conflicts": conflicts,
        "epl_fm_reep_links": [{"club": club, "official_name": p["official_name"],
                                "fm_uid": p["fm_uid"], "reep_id": reep,
                                "fbref_person_ids": sorted(reep_to_fbref[reep]),
                                "transfermarkt_player_ids": sorted(reep_to_tm[reep]),
                                "identity_caveat": "official_registration_to_FM_was_name_based"}
                               for club, p, reep in fm_mapped if reep],
        "limitations": [
            "An official EPL registration list has no provider person IDs; Reep cannot assign all 600 automatically",
            "A Reep bridge identifies a person, not the reliability or correctness of FM attributes",
            "Staff-kind records, conflicting DOB and missing local FM UIDs are not accepted as footballer attribute observations",
            "An unmatched SofaScore ID remains unresolved; aliases are proposals, not verified joins",
            "This audit does not use name/DOB to infer additional IDs or infer current club membership",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({k: v for k, v in result.items() if not isinstance(v, list)}, indent=2))


if __name__ == "__main__":
    main()
