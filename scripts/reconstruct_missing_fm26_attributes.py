#!/usr/bin/env python3
"""Explicit probabilistic FM26 attribute reconstruction, using FM's fitted curves.

Source: akks88/football-manager-data attr_generation_curves.json, fitted from
196,014 actual FM players; /home/drewclaw/fm26-data/work/generate_attrs.py.
The original generator samples random simulated save-specific traits. Here we
provide conditional means, uncertainty, and deterministic *optional* draws
for a statistical forecast. We NEVER assert generated values are the original
person's observed FM 1–20 traits.

When CA is missing, the correct answer is NOT fabricated individual 1–20
attributes: use only a (very broad) position+population prior, and mark
individual FM CA/mental evidence absent.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import random

BASE=Path("/home/drewclaw/fm26-data")
CURVES=BASE/"hf_upload/data/attr_generation_curves.json"
FULL=BASE/"fm26_men_full_attrs.csv"
SI=BASE/"sortitoutsi_uid_map.csv"
SI_ROSTER=BASE/"sortitoutsi_fm26_roster.csv"
GK_ATTRIBUTES=frozenset((
    "Handling","AerialReach","CommandOfArea","Communication","Kicking",
    "Throwing","OneOnOnes","Reflexes","RushingOut","Punching","Eccentricity",
))
POSITION_CLASSES=("GK","DF","MF","FW")


def sha(path):
    digest=hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda:stream.read(1<<20),b""):
            digest.update(chunk)
    return digest.hexdigest()


def position_class(value):
    value=(value or "").strip().upper()
    if not value:
        return None
    if value.startswith("GK"):
        return "GK"
    if value.startswith(("D","W")):
        return "DF"
    if value.startswith(("ST","F")):
        return "FW"
    return "MF"


def collect_roles(uid_set, roster_path, bridge_path):
    """Only source-published Sortitoutsi positions, mapped with verified IDs."""
    id_map=defaultdict(set)
    with bridge_path.open(encoding="utf-8-sig",newline="") as fh:
        for row in csv.DictReader(fh):
            if row.get("match_type") not in {"full","surname"}:
                continue
            uid=row.get("binary_uid","")
            if uid in uid_set:
                id_map[uid].add((row["si_id"],position_class(row.get("position"))))
    roster_roles=defaultdict(set)
    with roster_path.open(encoding="utf-8-sig",newline="") as fh:
        for row in csv.DictReader(fh):
            roster_roles[row["uid"]].add(position_class(row.get("Position")))
    confirmed={}
    for uid,edges in id_map.items():
        classes={cls for sid,cls in edges
                 if cls is not None and cls in roster_roles.get(sid,set())}
        if len(classes)==1:
            confirmed[uid]=next(iter(classes))
    return confirmed


def records_for_uids(path,needed):
    selected=defaultdict(list)
    with path.open(encoding="utf-8-sig",newline="") as fh:
        for row in csv.DictReader(fh):
            if row.get("uid") in needed:
                selected[row["uid"]].append(row)
    return selected


def curve_moments(curves, cls, ca):
    if cls not in curves:
        raise ValueError("unknown position model")
    ca=int(round(ca))
    if not 1<=ca<=200:
        raise ValueError("CA outside FM's 1-200 interval")
    sample={}
    for attr,points in curves[cls].items():
        if ca>=len(points):
            raise ValueError("CA outside fitted curves")
        mean,sd=map(float,points[ca])
        if not math.isfinite(mean) or not math.isfinite(sd) or sd<0:
            raise ValueError("bad learned attribute-distribution statistics")
        if cls!="GK" and attr in GK_ATTRIBUTES:
            mean,sd=0.0,0.0
        else:
            # Our predictions are conditional expected ratings, not actual
            # saved integers. The published source sampler clips to 1..20.
            mean=max(1,min(20,mean))
            sd=max(0.6,sd)
        sample[attr]={"mean":round(mean,4),"std":round(sd,4),
                      "p10":round(max(0 if attr in GK_ATTRIBUTES and cls!="GK" else 1,
                                       mean-1.2815515655*sd),4),
                      "p90":round(min(20,mean+1.2815515655*sd),4)}
    return sample


def cohort_prior(curves,cls,ca_samples):
    """Empirical overall FM player cohort; not the subject's personal CA."""
    group=ca_samples.get(cls) or []
    if len(group)<25:
        return None
    # Stable subsampling across *observed* FM player CA in confirmed SI roles.
    # Do not use generated attributes to fit these priors.
    samples=sorted(group)
    chosen=samples[::max(1,len(samples)//250)]
    combined={}
    for ca in chosen:
        for attr,values in curve_moments(curves,cls,ca).items():
            combined.setdefault(attr,[]).append(values)
    out={}
    for attr,points in combined.items():
        mu=sum(x["mean"] for x in points)/len(points)
        variance=sum(x["std"]**2+(x["mean"]-mu)**2 for x in points)/len(points)
        sd=math.sqrt(variance)
        out[attr]={"mean":round(mu,4),"std":round(sd,4),
                   "p10":round(max(0 if attr in GK_ATTRIBUTES and cls!="GK" else 1,
                                    mu-1.2815515655*sd),4),
                   "p90":round(min(20,mu+1.2815515655*sd),4)}
    return out


def ca_position_mixture(curves, ca, position_weights):
    """CA is known, but the specific role is not: marginalize over roles.

    Published FM position population fractions come from the real FM
    generator's pos_pop.json. The between-role variance is explicitly
    carried into the uncertainty rather than pretending everyone is MF.
    """
    if set(position_weights)!=set(POSITION_CLASSES):
        raise ValueError("published position-population weights are incomplete")
    total=sum(float(v) for v in position_weights.values())
    if total<=0:
        raise ValueError("invalid source position weights")
    distributions={cls:curve_moments(curves,cls,ca) for cls in POSITION_CLASSES}
    result={}
    for attr in distributions["MF"]:
        parts=[(float(position_weights[cls])/total,distributions[cls][attr])
               for cls in POSITION_CLASSES]
        mu=sum(w*v["mean"] for w,v in parts)
        variance=sum(w*(v["std"]**2+(v["mean"]-mu)**2) for w,v in parts)
        sd=math.sqrt(variance)
        result[attr]={"mean":round(mu,4),"std":round(sd,4),
                      "p10":round(max(0,mu-1.2815515655*sd),4),
                      "p90":round(min(20,mu+1.2815515655*sd),4)}
    return result


def generate(audit, curves, fm_source, si_roster, si_bridge, position_weights):
    if audit.get("schema")!="reep_first_full_epl_identity_audit_v1":
        raise ValueError("require Reep-first identity source")
    people=[p for group in audit["clubs"].values() for p in group]
    if len(people)!=1576:
        raise ValueError("not full 20-club registered universe")
    ids={str(p["new_fm_uid"]) for p in people if p.get("new_fm_uid")}
    known=records_for_uids(fm_source,ids)
    confirmed_role=collect_roles(ids,si_roster,si_bridge)
    # CA-based generation must only be used if the actual FM UID is present
    # exactly once. Unknown/ambiguous rows cannot donate ratings.
    cohort=defaultdict(list)
    for uid,records in known.items():
        if len(records)!=1:
            continue
        row=records[0]
        try:ca=int(float(row["ca"]))
        except (ValueError,KeyError):continue
        if row.get("kind")=="player" and row.get("attr_source")=="db" and 1<=ca<=200:
            cls=confirmed_role.get(uid)
            if cls:
                cohort[cls].append(ca)
    prior={cls:cohort_prior(curves,cls,cohort) for cls in POSITION_CLASSES}
    results=[]
    counts=Counter()
    for club,group in audit["clubs"].items():
        for entry in group:
            uid=str(entry["new_fm_uid"] or "")
            rows=known.get(uid,[])
            # There is no reason to generate a new realization if the player's
            # actual attribute block or a source-flagged synthetic block
            # already exists in the native FM data.
            if not uid or len(rows)!=1:
                reason=("unresolved_identity" if not uid else
                        "duplicated_or_missing_native_uid")
                counts[reason]+=1
                continue
            row=rows[0]
            # Two native staff-classified player IDs contain a nominal DB
            # block, but none of the twelve reliability-check attributes is
            # actually populated on the 1–20 scale. Treat that sparse block
            # as unusable and reconstruct conservatively from CA instead.
            trustworthy_fields=("Anticipation","Decisions","Composure",
                "Concentration","Leadership","Teamwork","WorkRate","Bravery",
                "Finishing","Pace","Passing","Technique")
            valid_evidence=sum(
                1 for field in trustworthy_fields
                if row.get("attr_"+field) and
                1<=float(row["attr_"+field])<=20)
            if row["attr_source"]=="db" and (
                    row["kind"]=="player" or valid_evidence>0):
                counts[("observed_database_block_recovered_from_staff_kind"
                        if row["kind"]!="player" else
                        "observed_database_block_no_generation")]+=1
                continue
            if row["kind"]=="player" and row["attr_source"]=="derived":
                counts["existing_source_generated_attributes_no_redraw"]+=1
                continue
            if entry["new_reep_id"] is None:
                counts["no_unique_canonical_reep_identity"]+=1
                continue
            cls=confirmed_role.get(uid)
            try:ca=int(float(row["ca"]))
            except (TypeError,ValueError):ca=None
            if ca is not None and not 1<=ca<=200:ca=None
            if cls is not None and ca is not None:
                trait_distributions=curve_moments(curves,cls,ca)
                state="ca_and_verified_position_conditional"
                conditioning={"fm_uid":uid,"ca":ca,"ca_evidence":"FM raw header staff-kind/untrusted",
                              "position":cls,"position_evidence":"Sortitoutsi verified UID and Position"}
            elif cls is not None and prior.get(cls) is not None:
                trait_distributions=prior[cls]
                state="verified_position_population_prior_no_individual_ca"
                conditioning={"fm_uid":uid,"ca":None,"ca_evidence":"missing",
                              "position":cls,"position_evidence":"Sortitoutsi verified UID and Position"}
            elif ca is not None:
                trait_distributions=ca_position_mixture(curves,ca,position_weights)
                state="ca_only_position_mixture_with_wide_role_uncertainty"
                conditioning={"fm_uid":uid,"ca":ca,
                              "ca_evidence":"FM raw header staff-kind/untrusted",
                              "position":None,
                              "position_evidence":"unknown; population-weighted FM mixture"}
            else:
                counts["missing_ca_and_verified_position"]+=1
                continue
            values={
                "official_name":entry["official_name"],"club":club,
                "reep_id":entry["new_reep_id"],"fm_uid":uid,
                "raw_kind":row["kind"],"raw_attr_source":row["attr_source"],
                "generation_source":"akks88_fm26_smoothed_empirical_CA_position_curves",
                "generation_method":"expected_conditional_trait_distribution_no_imaginary_save_draw",
                "state":state,"conditioning":conditioning,
                "traits":trait_distributions,
                "observed_FM_trait_values":False,
                "medical_or_dynamic_mental_traits_observed":False,
                "usable_for_predictive_model_as":("conditional_distribution_with_uncertainty_not_observed_player_rating"),
            }
            results.append(values)
            counts[state]+=1
    return {
        "schema":"fm26_missing_trait_conditional_priors_v1",
        "source_vintage":"FM26_base_26.0.0_dated_2026-03-01",
        "forecast_cutoff":"2026-10-11",
        "generated_at_utc":datetime.now(timezone.utc).isoformat(),
        "generation_and_data_quality_note":(
            "FM generates un-stored attribute blocks stochastically; not a canonical "
            "per-player FM rating. Published CA-position smoothed curves are fitted "
            "from real player distributions; no CA regression tuning (previously "
            "rejected for bias). Missing CA uses only broad cohort priors."
        ),
        "cohort_ca_support_counts":{k:len(v) for k,v in cohort.items()},
        "counts":dict(counts),
        "priors":results,
    }


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--identity",type=Path,required=True)
    ap.add_argument("--fm",type=Path,default=FULL)
    ap.add_argument("--curves",type=Path,default=CURVES)
    ap.add_argument("--si-roster",type=Path,default=SI_ROSTER)
    ap.add_argument("--si-map",type=Path,default=SI)
    ap.add_argument("--pos-pop",type=Path,default=BASE/"work/pos_pop.json")
    ap.add_argument("--output",type=Path,required=True)
    a=ap.parse_args()
    if a.output.exists():
        ap.error("immutable attribute prior snapshot already exists")
    result=generate(json.loads(a.identity.read_text()),json.loads(a.curves.read_text()),
                    a.fm,a.si_roster,a.si_map,json.loads(a.pos_pop.read_text()))
    result["input_sha256"]={str(p):sha(p) for p in
                             (a.identity,a.curves,a.fm,a.si_roster,a.si_map,a.pos_pop)}
    a.output.parent.mkdir(parents=True,exist_ok=True)
    a.output.write_text(json.dumps(result,sort_keys=True)+"\n")
    print(json.dumps({"counts":result["counts"],"cohort_CA_support":result["cohort_ca_support_counts"],
                      "priors_written":len(result["priors"])},indent=2))


if __name__=="__main__":
    main()
