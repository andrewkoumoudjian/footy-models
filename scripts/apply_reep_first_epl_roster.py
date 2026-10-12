#!/usr/bin/env python3
"""Build 20-club EPL model roster from Reep-first audit, not old FM name guesses.

Preserve each official registered/U21 candidate even when no identity is
resolved. No fallback to previous 1,051 proposed FM UIDs is permitted.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]


def transform(roster,audit):
    if roster.get("roster_scope")!="full_league" or len(roster["teams"])!=20:
        raise ValueError("requires full 20-club EPL roster")
    if audit["schema"]!="reep_first_full_epl_identity_audit_v1":
        raise ValueError("not a Reep-first all-club identity audit")
    if audit["counts"]["official_players"]!=1576 or audit["forecast_date"]!=roster["as_of"]:
        raise ValueError("identity audit is from another league/vintage")
    source=json.loads(json.dumps(roster))
    counter=Counter()
    for club,block in source["teams"].items():
        original=block["players"]
        matching=audit["clubs"].get(club)
        if matching is None or len(original)!=len(matching):
            raise ValueError("team missing or wrong player count in audit")
        by_name={x["official_name"]:x for x in matching}
        if len(by_name)!=len(matching):
            raise ValueError("ambiguous original same-name registration")
        for p in original:
            resolution=by_name.get(p["official_name"])
            if not resolution:
                raise ValueError(f"unmatched official roster member: {p['official_name']}")
            # Crucial: DELETE old name-matcher associations and any traits
            # derived from them; even if Reep finds no match, the person's
            # official registration is not dropped.
            for field in (
                "fm_uid","fm_name","fm_kind","attr_source","ca","pa",
                "attributes","fm_player_record_verified","fm_attributes_verified",
                "source_quality","match_method"):
                p.pop(field,None)
            p["match_method"]="reep_first_search"
            p["identity_state"]=resolution["identity_state"]
            p["official_to_reep_identity_issuer_verified"]=False
            p["reep_id_candidate"]=resolution["new_reep_id"]
            p["provider_ids_candidate"]=resolution["provider_ids"]
            p["identity_name_evidence"]=resolution["match_name_evidence"]
            p["fm_uid"]=resolution["new_fm_uid"] if resolution["new_fm_uid"] else None
            p["fm_identity_accepted_as_candidate_only"]=bool(resolution["new_fm_uid"])
            p["fm_attr_source_reep_candidate"]=resolution["new_fm_attr_source"]
            p["fm_kind_reep_candidate"]=resolution["new_fm_kind"]
            p["fm_binary_record_source_reep_candidate"]=resolution.get("new_fm_record_source")
            p["multi_club_eligibility_unresolved"]=bool(
                resolution.get("multi_club_eligibility_unresolved"))
            counter[p["identity_state"]]+=1
        block["matched_count"]=sum(bool(p["fm_uid"]) for p in original)
        # Preserve the original roster's schema: unmatched is a list, not
        # a count. Other agents and validation tools consume this field.
        block["unmatched"]=[p["official_name"] for p in original if not p["fm_uid"]]
        block["matched_25"]=sum(bool(p["fm_uid"]) and p["registration"]=="registered"
                                for p in original)
        block["matched_u21"]=sum(bool(p["fm_uid"]) and p["registration"]=="u21"
                                 for p in original)
    source["fm_matching_note"]=(
        "Reep v1 alias/label -> typed fm:player UID before original name matcher. "
        "All official registrations retained, current official person IDs unavailable. "
        "Provisional attributes must keep identity source and FM kind provenance.")
    source["identity_rebuild"]="reep_first_all_20_clubs_v1"
    source["identity_summary"]=dict(counter)
    source["reep_release_stamp"]=audit["reep_source_stamp"]
    source["reep_register_sha256"]=audit["reep_sha256"]
    if sum(len(t["players"]) for t in source["teams"].values())!=1576:
        raise ValueError("registered player dropped from full roster")
    return source


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--official",type=Path,default=ROOT/"data/pl/squads_2026-10-11_full.json")
    p.add_argument("--reep-audit",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    a=p.parse_args()
    if a.output.exists():
        p.error("immutable result already exists")
    source=transform(json.loads(a.official.read_text()),
                     json.loads(a.reep_audit.read_text()))
    source["identity_source_file_sha256"]=hashlib.sha256(a.reep_audit.read_bytes()).hexdigest()
    a.output.parent.mkdir(parents=True,exist_ok=True)
    a.output.write_text(json.dumps(source,sort_keys=True,ensure_ascii=False)+"\n")
    print(json.dumps({
        "total":sum(len(x["players"]) for x in source["teams"].values()),
        "teams":len(source["teams"]),"identity_status":source["identity_summary"],
        "FM_candidate_uids":sum(x["matched_count"] for x in source["teams"].values())},
        indent=2))


if __name__=="__main__":
    main()
