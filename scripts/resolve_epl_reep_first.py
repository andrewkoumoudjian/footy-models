#!/usr/bin/env python3
"""Rebuild all 20 EPL rosters from Reep *before* considering old FM name guesses.

The official EPL registration feed has no source person IDs. Reep search must
therefore use its independently indexed canonical label and aliases as *name
evidence* to discover typed provider IDs; only then are the actual FM26 player
UIDs, local source kind, DOB, and other provider bridges inspected.

An official-registration -> Reep match is never an issuer-confirmed identifier:
even a unique Reep name and FM UID is recorded as independently corroborated
but provisional, and conflicting identities are never forced.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from datetime import datetime, timezone
from difflib import SequenceMatcher
import hashlib
import json
from pathlib import Path
import re
import unicodedata

ROOT=Path(__file__).resolve().parents[1]
ACTIVE=ROOT/"data/identity/reep/active.json"
ROSTER=ROOT/"data/pl/squads_2026-10-11_full.json"
FM=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv")
COMPLETE=Path("/home/drewclaw/fm26-data/fm26_complete_v6.csv")


def norm(value):
    value=(value or "").translate(str.maketrans(
        {"ø":"o","Ø":"O","ł":"l","Ł":"L","đ":"d","Đ":"D",
         "ß":"ss","æ":"ae","Æ":"Ae","œ":"oe","Œ":"Oe"}))
    return " ".join(re.findall("[a-z0-9]+","".join(
        c for c in unicodedata.normalize("NFKD",value).casefold()
        if not unicodedata.combining(c))))


def official_keys(name):
    """Two strong legal-name renderings and a *weak* short-name search key."""
    clean=re.sub(r"\s*\(Loan\).*","",name.replace("*",""),flags=re.I)
    if "," not in clean:
        parts=norm(clean).split()
        return {" ".join(parts)}, {(parts[0],parts[-1])} if len(parts)>1 else set()
    surname,given=clean.split(",",1)
    family=norm(surname).split()
    first=norm(given).split()
    if not family or not first:
        return set(),set()
    strong={" ".join(first+family), " ".join(first[:1]+family)}
    short={(first[0],family[-1])}
    # A many-part legal surname is not automatically shortened into a
    # high-confidence identity; it only widens fallback candidate search.
    return strong,short


def source_name_support(official, reep_name):
    """Name corroboration *after* an independently typed Reep/FM UID join.

    The official name is family-first, and Reep may omit middle names. A
    surname and given name must both agree, or a near-spelling must agree
    with otherwise substantial name overlap. This cannot certify identity.
    """
    official=re.sub(r"\s*\(Loan\).*","",official.replace("*",""),flags=re.I)
    if "," not in official:
        return False
    family, given=official.split(",",1)
    a=set(norm(family).split());b=norm(given).split()
    candidate=norm(reep_name).split()
    if not a or not b or not candidate:
        return False
    family_match=bool(a.intersection(candidate))
    first_match=(b[0] in candidate or any(
        SequenceMatcher(None,b[0],t).ratio()>=0.79 for t in candidate
    ))
    return family_match and first_match


def redirects_map(conn):
    return dict(conn.execute("SELECT from_id,to_id FROM redirects").fetchall())


def canonical(rid,redirects):
    visited=set()
    while rid in redirects:
        if rid in visited:
            raise ValueError("Reep redirect cycle")
        visited.add(rid)
        rid=redirects[rid]
    return rid


def candidates_from_reep(conn, registrations):
    """Scan Reep labels and aliases, independent of original FM UID guesses."""
    strong=defaultdict(set)
    weak=defaultdict(set)
    for i,p in enumerate(registrations):
        for name in p["strong_keys"]:
            strong[name].add(i)
        for pair in p["short_keys"]:
            weak[pair].add(i)
    redirects=redirects_map(conn)
    possible=defaultdict(dict)
    counts=Counter()
    for table,column in (("players","label"),("aliases","alias")):
        stream=conn.execute(f"SELECT reep_id,{column} FROM {table}")
        while True:
            rows=stream.fetchmany(50000)
            if not rows:
                break
            for rid,value in rows:
                if not value:
                    continue
                rid=canonical(rid,redirects)
                key=norm(value)
                if not key:
                    continue
                exact=strong.get(key,())
                tokens=key.split()
                broad=weak.get((tokens[0],tokens[-1]),()) if len(tokens)>1 else ()
                if not exact and not broad:
                    continue
                for i in set(exact) | set(broad):
                    method="label_or_alias_legal_name" if i in exact else "short_given_family_fallback"
                    rank=0 if i in exact else 1
                    existing=possible[i].get(rid)
                    if existing is None or rank<existing["rank"]:
                        possible[i][rid]={"reep_id":rid,"rank":rank,
                            "method":method,"evidence_name":value,"source_table":table}
                        counts[method]+=1
    return possible,counts


def provider_bridges(conn,relevant):
    if not relevant:
        return {}
    redirects=redirects_map(conn)
    # Reep v1 has merged canonical people; a historical bridge may still
    # reference the *pre-redirect* ID even when its current alias has moved.
    # Query both the canonical and redirected source IDs, then regroup all
    # typed provider bridges under their canonical identity.
    relevant_raw=set(relevant)
    for before in redirects:
        if canonical(before,redirects) in relevant:
            relevant_raw.add(before)
    conn.execute("CREATE TEMP TABLE wanted_reep_id(reep_id VARCHAR PRIMARY KEY)")
    conn.executemany("INSERT INTO wanted_reep_id VALUES (?)",[(rid,) for rid in sorted(relevant_raw)])
    bridges=defaultdict(lambda:defaultdict(set))
    for rid,provider,namespace,external,rung in conn.execute(
        "SELECT b.reep_id,b.provider,b.namespace,b.external_id,b.rung "
        "FROM bridges b JOIN wanted_reep_id w ON b.reep_id=w.reep_id "
        "WHERE (b.provider='fm' AND b.namespace='player') "
        "OR (b.provider='sofascore' AND b.namespace='player') "
        "OR (b.provider='fbref' AND b.namespace='person') "
        "OR (b.provider='transfermarkt' AND b.namespace='spieler')"
    ).fetchall():
        bridges[canonical(rid,redirects)][provider].add((str(external),str(rung)))
    return bridges


def read_fm_subset(path,ids,alternate_path=None):
    found=defaultdict(list)
    scanned=0
    with path.open(encoding="utf-8-sig",newline="") as file:
        for row in csv.DictReader(file):
            scanned+=1
            uid=str(row.get("uid") or "").strip()
            if uid in ids:
                found[uid].append({
                    "fm_uid":uid,"name":row.get("name",""),
                    "dob":row.get("dob",""),
                    "kind":row.get("kind",""),
                    "attr_source":row.get("attr_source",""),
                    "ca":row.get("ca",""),"pa":row.get("pa",""),
                    "record_source":"men_full_attrs",
                })
    if alternate_path is not None:
        # FM26 men+women+unknown binary frame has some major men's players
        # absent from the gender-partitioned 'men' export. Search ONLY UIDs
        # missing from the primary; never override a collision with v6 data.
        absent={uid for uid in ids if uid not in found}
        if absent:
            with alternate_path.open(encoding="utf-8-sig",newline="") as file:
                for row in csv.DictReader(file):
                    uid=str(row.get("uid") or "").strip()
                    if uid not in absent:
                        continue
                    valid=sum(
                        bool(v and v.replace(".","",1).isdigit() and
                             1<=float(v)<=20)
                        for k,v in row.items() if k.startswith("attr_")
                    )
                    found[uid].append({
                        "fm_uid":uid,"name":row.get("name",""),
                        "dob":row.get("dob",""),
                        "kind":row.get("kind",""),
                        "attr_source":"db" if valid else "none",
                        "ca":row.get("ca",""),"pa":row.get("pa",""),
                        "record_source":"complete_v6_raw_attribute_block",
                        "positive_attr_count":valid,
                    })
    return found,scanned


def resolve(registrations, candidates, bridges, fm_records):
    by_club=defaultdict(list)
    audit=Counter()
    for i,registration in enumerate(registrations):
        raw=candidates.get(i,{})
        alternatives=[]
        for rid,p in raw.items():
            providers=bridges.get(rid,{})
            uids=sorted(x[0] for x in providers.get("fm",()))
            eligible=[]
            for uid in uids:
                instances=fm_records.get(uid,[])
                if len(instances)==1 and instances[0]["kind"]=="player":
                    eligible.append(instances[0])
            # The Reep index anchors the person; corroboration from other
            # providers makes an ambiguous legal-name candidate less likely,
            # but cannot repair duplicate/conflicting FM IDs.
            external=sum(bool(providers.get(k)) for k in ("sofascore","fbref","transfermarkt"))
            row={**p,"fm_uids":uids,
                 "local_unique_fm_player_uids":[x["fm_uid"] for x in eligible],
                 "n_external_provider_namespaces":external,
                 "sofascore_ids":sorted(x[0] for x in providers.get("sofascore",())),
                 "fbref_ids":sorted(x[0] for x in providers.get("fbref",())),
                 "transfermarkt_ids":sorted(x[0] for x in providers.get("transfermarkt",()))}
            alternatives.append(row)
        alternatives.sort(key=lambda p:(p["rank"],-bool(p["local_unique_fm_player_uids"]),
            -p["n_external_provider_namespaces"],p["reep_id"]))
        # Strong name is required for automatic *provisional* Reep attachment.
        # Short-name matches are only review leads.
        strong=[p for p in alternatives if p["rank"]==0]
        if len(strong)==1:
            selected=strong[0]
            state="unique_reep_legal_name_with_provider_ids"
        elif len(strong)>1:
            # Existing name-only assignment is NEVER the search start.
            # If exactly one independently found Reep FM typed bridge
            # agrees with that provisional guess it can disambiguate,
            # with that evidence kept explicit and still not issuer-verified.
            agreement=[p for p in strong if registration["old_fm_uid"]
                       and registration["old_fm_uid"] in p["fm_uids"]]
            if len(agreement)==1:
                selected=agreement[0]
                state="multiple_alias_candidates_disambiguated_by_typed_fm_id_agreement"
            else:
                selected=None
                state="ambiguous_reep_legal_name"
        elif alternatives:
            corroborated=[p for p in alternatives if
                          p["rank"]==2 and len(p["fm_uids"])==1
                          and p["fm_uids"][0]==registration["old_fm_uid"]
                          and len(fm_records.get(p["fm_uids"][0],[]))==1]
            eligible=[p for p in alternatives if
                      len(p["fm_uids"])==1
                      and len(fm_records.get(p["fm_uids"][0],[]))==1
                      and p["n_external_provider_namespaces"]>=2]
            if len(corroborated)==1:
                selected=corroborated[0]
                state="reep_typed_fm_bridge_recovered_name_variant"
            elif len(alternatives)==1 and len(eligible)==1:
                selected=eligible[0]
                state="unique_short_given_family_multi_provider_candidate"
            else:
                selected=None
                state="only_weak_short_name_matches"
        else:
            selected=None
            state="not_found_in_reep_names_or_aliases"
        uid=None
        fm_data=None
        if selected:
            # The FM extractor sometimes classifies real players as staff.
            # Identity and mental attribute validity are DIFFERENT questions:
            # preserve the typed Reep fm:player bridge and disable any
            # staff-kind FM mental/technical attribute inputs.
            fmids=selected["fm_uids"]
            if len(fmids)==1 and len(fm_records.get(fmids[0],[]))==1:
                uid=fmids[0]
                fm_data=fm_records[uid][0]
                if fm_data["kind"]=="player":
                    state="reep_to_local_fm_player_id_unique"
                else:
                    state="reep_to_local_fm_uid_nonplayer_kind_no_trusted_attributes"
            elif len(fmids)==1:
                state="reep_fm_uid_missing_or_duplicated_in_export"
            elif len(fmids)>1:
                state="multiple_fm_uids_for_one_reep_person"
            else:
                state="unique_reep_person_but_no_fm_player_bridge"
        audit[state]+=1
        by_club[registration["club"]].append({
            "official_name":registration["official_name"],
            "registration":registration["registration"],
            "original_guess_fm_uid":registration["old_fm_uid"],
            "new_reep_id":selected["reep_id"] if selected else None,
            "new_fm_uid":uid,
            "new_fm_kind":fm_data["kind"] if fm_data else None,
            "new_fm_attr_source":fm_data["attr_source"] if fm_data else None,
            "new_fm_record_source":fm_data["record_source"] if fm_data else None,
            "new_fm_attributes_trusted":bool(fm_data and fm_data["kind"]=="player"
                                             and fm_data["attr_source"]=="db"),
            "new_fm_name":fm_data["name"] if fm_data else None,
            "new_fm_dob":fm_data["dob"] if fm_data else None,
            "identity_state":state,
            "official_to_reep_identity_issuer_verified":False,
            "match_name_evidence":selected["evidence_name"] if selected else None,
            "match_source_table":selected["source_table"] if selected else None,
            "provider_ids":{
                "sofascore":selected["sofascore_ids"] if selected else [],
                "fbref":selected["fbref_ids"] if selected else [],
                "transfermarkt":selected["transfermarkt_ids"] if selected else [],
            },
            "alternatives":alternatives[:8],
            "alternatives_truncated":max(0,len(alternatives)-8),
        })
    # Reep aliases can also merge *two different current official players*.
    # Example in this vintage: Harrison Steven Gray (Coventry) and Harry
    # Andrew Gray (Leeds). A same-Reep collision is NOT two verified persons.
    by_reep=defaultdict(list)
    for players in by_club.values():
        for player in players:
            if player["new_reep_id"]:
                by_reep[player["new_reep_id"]].append(player)
    for rid,people in by_reep.items():
        if len(people)<2:
            continue
        same_fm={p["new_fm_uid"] for p in people}
        surface={norm(re.sub(r"\s*\(Loan\).*","",p["official_name"].replace("*",""),flags=re.I))
                 for p in people}
        if len(same_fm)==1 and None not in same_fm and len(surface)==1 and len({
                p["registration"] for p in people})<=2:
            # Same footballer appears on parent club and loan club official
            # lists. It is not evidence Reep has merged two different humans.
            # Preserve both *registration rows* with one canonical person.
            # The active-playing-club selector must separately gate their
            # matchday eligibility.
            for person in people:
                audit[person["identity_state"]]-=1
                audit["same_person_in_multiple_club_registrations"]+=1
                person["identity_state"]="same_person_in_multiple_club_registrations"
                person["multi_club_eligibility_unresolved"]=True
            continue
        for player in people:
            audit[player["identity_state"]]-=1
            audit["cross_registration_reep_collision"]+=1
            player["identity_state"]="cross_registration_reep_collision"
            player["colliding_reep_id_not_accepted"]=rid
            player["new_reep_id"]=None
            player["new_fm_uid"]=None
            player["new_fm_kind"]=None
            player["new_fm_attr_source"]=None
            player["new_fm_record_source"]=None
            player["provider_ids"]={"sofascore":[],"fbref":[],"transfermarkt":[]}
    return by_club,dict(audit)


def build(conn,roster,fm_path,alternate_path=None):
    registrations=[]
    for club,group in roster["teams"].items():
        for p in group["players"]:
            strong,short=official_keys(p["official_name"])
            registrations.append({
                "club":club,"official_name":p["official_name"],
                "registration":p["registration"],
                "old_fm_uid":str(p.get("fm_uid") or "") or None,
                "strong_keys":strong,"short_keys":short,
            })
    if len(registrations)!=1576 or len(roster["teams"])!=20:
        raise ValueError("require entire verified twenty-club 1576-registration snapshot")
    candidates,scan_counts=candidates_from_reep(conn,registrations)
    # Second pass: an existing name matcher may have an FM UID. Resolve that
    # ID back into Reep's typed bridge register, instead of treating the
    # original guessed UID as a confirmed identity. The Reep person label and
    # aliases must separately support the official name.
    old_ids={p["old_fm_uid"] for p in registrations if p["old_fm_uid"]}
    from_ids=defaultdict(set)
    if old_ids:
        conn.execute("CREATE TEMP TABLE old_fm_candidates(id VARCHAR PRIMARY KEY)")
        conn.executemany("INSERT INTO old_fm_candidates VALUES (?)",[(uid,) for uid in sorted(old_ids)])
        redirects=redirects_map(conn)
        for fm_id,rid in conn.execute(
            "SELECT b.external_id,b.reep_id FROM bridges b "
            "JOIN old_fm_candidates old ON b.external_id=old.id "
            "WHERE b.provider='fm' AND b.namespace='player'"
        ).fetchall():
            from_ids[fm_id].add(canonical(rid,redirects))
        potential={rid for rids in from_ids.values() for rid in rids}
        if potential:
            conn.execute("CREATE TEMP TABLE typed_old_candidates(reep_id VARCHAR PRIMARY KEY)")
            conn.executemany("INSERT INTO typed_old_candidates VALUES (?)",[(x,) for x in sorted(potential)])
            names=defaultdict(set)
            for rid,name in conn.execute(
                "SELECT e.reep_id,e.label FROM entities e "
                "JOIN typed_old_candidates t USING (reep_id)"
            ).fetchall():
                if name:names[rid].add(name)
            for rid,name in conn.execute(
                "SELECT a.reep_id,a.alias FROM aliases a "
                "JOIN typed_old_candidates t USING (reep_id)"
            ).fetchall():
                if name:names[rid].add(name)
            for i,p in enumerate(registrations):
                uid=p["old_fm_uid"]
                if not uid:
                    continue
                choices=from_ids.get(uid,set())
                if len(choices)!=1:
                    continue
                rid=next(iter(choices))
                if rid in candidates[i]:
                    continue
                qualifying=[name for name in names[rid]
                            if source_name_support(p["official_name"],name)]
                if qualifying:
                    candidates[i][rid]={
                        "reep_id":rid,"rank":2,
                        "method":"typed_fm_bridge_plus_independent_reep_name",
                        "evidence_name":sorted(qualifying,key=len)[-1],
                        "source_table":"fm_bridge_reverse_lookup",
                    }
                    scan_counts["secondary_provider_id_search"]+=1
    wanted={rid for options in candidates.values() for rid in options}
    edges=provider_bridges(conn,wanted)
    ids={x[0] for provider in edges.values() for x in provider.get("fm",())}
    fm_records,scanned=read_fm_subset(fm_path,ids,alternate_path)
    by_club,status=resolve(registrations,candidates,edges,fm_records)
    return {
        "schema":"reep_first_full_epl_identity_audit_v1",
        "forecast_date":roster["as_of"],
        "created_utc":datetime.now(timezone.utc).isoformat(),
        "source_principle":"Reep canonical person/aliases before any existing FM roster guesses",
        "official_registration_issuer_ids_present":False,
        "identity_caveat":"Reep alias evidence never proves official EPL list's person ID; results are provisional",
        "counts":{
            "official_players":len(registrations),
            "original_name_guess_uids":sum(p["old_fm_uid"] is not None for p in registrations),
            "reep_candidate_entities":len(wanted),
            "local_fm_rows_scanned":scanned,
            "binary_complete_v6_fallback_matches":sum(
                bool(x["record_source"]=="complete_v6_raw_attribute_block")
                for records in fm_records.values() for x in records),
            "reep_search_hits":dict(scan_counts),
            "resolution":status,
            "new_unique_fm_player_uid":sum(p["new_fm_uid"] is not None
                                            for v in by_club.values() for p in v),
            "with_reep_person_id":sum(p["new_reep_id"] is not None
                                      for v in by_club.values() for p in v),
            "not_resolved_to_unique_fm_player":sum(p["new_fm_uid"] is None
                                                    for v in by_club.values() for p in v),
            "by_club":{club:{
                "registered":len(p),
                "with_reep":sum(x["new_reep_id"] is not None for x in p),
                "fm_player_uid":sum(x["new_fm_uid"] is not None for x in p)}
                for club,p in sorted(by_club.items())},
        },
        "clubs":dict(by_club),
    }


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--active",type=Path,default=ACTIVE)
    ap.add_argument("--roster",type=Path,default=ROSTER)
    ap.add_argument("--fm",type=Path,default=FM)
    ap.add_argument("--complete-fm",type=Path,default=COMPLETE)
    ap.add_argument("--output",type=Path,required=True)
    a=ap.parse_args()
    if a.output.exists():
        ap.error("will not overwrite frozen audit")
    import duckdb
    active=json.loads(a.active.read_text())
    with duckdb.connect(active["path"],read_only=True) as db:
        result=build(db,json.loads(a.roster.read_text()),a.fm,a.complete_fm)
    result["reep_source_stamp"]=active["stamp"]
    result["reep_sha256"]=active["sha256"]
    a.output.parent.mkdir(parents=True,exist_ok=True)
    a.output.write_text(json.dumps(result,sort_keys=True,ensure_ascii=False)+"\n")
    print(json.dumps(result["counts"],indent=2,sort_keys=True))


if __name__=="__main__":
    main()
