#!/usr/bin/env python3
"""Audit EPL registration -> FM26 identity coverage without inventing matches.

This is a read-only diagnostic of the full extracted FM person universe. It
does not reuse CA as an identity signal, auto-promote staff to players, or
assign same-name identities without provider DOB confirmation.
"""
import argparse
import csv
import json
from collections import Counter, defaultdict
from pathlib import Path

from match_premier_league_fm26 import norm, official_name


def audit(roster, fm_path):
    exact = defaultdict(list)
    with Path(fm_path).open(encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle):
            if row.get("uid") and row.get("name"):
                exact[norm(row["name"])].append({
                    "uid": row["uid"], "fm_name": row["name"],
                    "dob": row.get("dob", ""), "kind": row.get("kind", ""),
                    "attr_source": row.get("attr_source", ""),
                    "ca": float(row.get("ca") or 0),
                })
    status = Counter()
    examples = []
    group_counts = Counter()
    for team, block in roster["teams"].items():
        for player in block["players"]:
            registration = player["registration"]
            state = "fm_uid_assigned" if player.get("fm_uid") else "unresolved"
            quality = "direct_attribute_record" if player.get("fm_attributes_verified") else (
                "fm_record_without_trusted_attributes" if player.get("fm_uid") else "no_fm_identity"
            )
            group_counts[(registration, state, quality)] += 1
            status[state] += 1
            if state == "fm_uid_assigned":
                continue
            candidates = exact[norm(official_name(player["official_name"]))]
            # No UID is assigned here: registration names lack DOB and may be
            # ambiguous even if a single same-name FM record appears.
            if len(candidates) == 1:
                reason = "unique_full_name_not_identity_verified"
            elif len(candidates) > 1:
                reason = "multiple_same_name_fm_records"
            else:
                reason = "no_exact_full_name_in_fm_export"
            status[reason] += 1
            examples.append({
                "club": team, "registration": registration,
                "official_name": player["official_name"],
                "previous_match_method": player.get("match_method", ""),
                "reason": reason, "same_name_fm_candidates": candidates[:4],
                "resolution_needed": "official/player-provider date of birth and stable provider ID",
            })
    return {
        "as_of": roster["as_of"],
        "source_contract": "audit_only; never auto-assign UID by name or CA",
        "registration_group_counts": [
            {"registration": k[0], "state": k[1], "quality": k[2], "count": value}
            for k, value in sorted(group_counts.items())
        ],
        "identity_summary": dict(status),
        "unresolved_for_provider_id_review": examples,
        "notes": [
            "U21 registration lists include youth not necessarily present in this FM26 DB vintage",
            "The raw full FM export can omit expected first-team names as well as youth names",
            "Derived FM attributes are uncertainty-bearing statistical reconstructions, not observed ratings",
            "The FM source kind=staff is not independent proof of a current footballer's role",
            "Names or CA alone are not evidence of common identity; require DOB/verified provider crosswalk",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roster", type=Path, required=True)
    parser.add_argument("--fm-file", type=Path, default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite an existing identity audit")
    record = audit(json.loads(args.roster.read_text()), args.fm_file)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(record["identity_summary"], indent=2))


if __name__ == "__main__":
    main()
