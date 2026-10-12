#!/usr/bin/env python3
"""Reconcile two dated leagues to one 248-column, ID-agnostic scoring matrix.

Feature snapshots have no results/labels and MUST NOT be misused to replay
previous games. Existing MLS bundle predates final serializer: use the exact
existing serializer when its player rows have no model_input.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[3]


def sha(path):
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1<<20),b""):
            h.update(chunk)
    return h.hexdigest()


def prepare(bundles,paths):
    names=None
    rows=[]
    seen=set()
    coverage=Counter()
    counts=Counter()
    identity=Counter()
    source=Counter()
    for bundle,path in zip(bundles,paths):
        if bundle.get("schema")!="unified_epl_mls_player_set_v1" or bundle.get("league") not in ("EPL","MLS"):
            raise ValueError("unknown pre-match feature bundle")
        league=bundle["league"]
        if league=="EPL" and (bundle["roster_scope"]!="full_league" or bundle["team_count"]!=20):
            raise ValueError("eight-club EPL subset cannot be full-league training input")
        for club,data in bundle["teams"].items():
            for person in data["players"]:
                flat=person.get("model_input")
                if flat is None:
                    # The older MLS snapshot was created with the same
                    # source-neutral builder before model_input was embedded.
                    sys.path.insert(0,str(ROOT/"scripts"))
                    from build_unified_prematch_bundle import flatten_model_features
                    flat=flatten_model_features(person,data["inputs"])
                f=flat["features"]
                if names is None:
                    names=sorted(f)
                if len(names)!=248 or sorted(f)!=names:
                    raise ValueError("EPL/MLS 248-column feature schema mismatch")
                if flat["observed_mask"]!={k:v is not None for k,v in f.items()}:
                    raise ValueError("incorrect per-feature observed/missing mask")
                key=(league,person["canonical_player_id"])
                if key in seen:
                    raise ValueError("duplicate canonical identity across teams")
                seen.add(key)
                for k,v in f.items():
                    if v is not None:
                        coverage[k]+=1
                identity[(league,person["identity_state"])]+=1
                for k,v in person["source_coverage"].items():
                    source[(league,k,bool(v))]+=1
                counts[league]+=1
                rows.append({
                    "league":league,
                    "club":club,
                    "canonical_player_id":person["canonical_player_id"],
                    "feature_cutoff_date":bundle["forecast_cutoff_date"],
                    "features":f,
                    "identity_state":person["identity_state"],
                    "source_coverage":person["source_coverage"],
                    "model_input_only_no_labels":True,
                })
        if counts[league]!=bundle["player_count"]:
            raise ValueError("source bundle player count differs from actual rows")
    if set(counts)!={"EPL","MLS"}:
        raise ValueError("both EPL and MLS are required")
    cutoffs={b["league"]:b["forecast_cutoff_date"] for b in bundles}
    manifest={
        "schema":"unified_epl_mls_shared_scoring_inputs_248_v1",
        "status":"feature_snapshot_only_no_labels_no_trained_predictor",
        "feature_count":len(names),
        "feature_names":names,
        "feature_cutoffs_by_league":cutoffs,
        "mixed_vintage_sources_are_not_a_single_fixture":True,
        "league_counts":dict(counts),
        "identity_status":[{"league":a,"identity":b,"n":c} for (a,b),c in sorted(identity.items())],
        "source_coverage":[{"league":a,"source":b,"available":v,"n":c}
                           for (a,b,v),c in sorted(source.items())],
        "column_coverage":{k:{"available":coverage[k],
                              "missing":len(rows)-coverage[k]} for k in names},
        "source_sha256":{str(p):sha(p) for p in paths},
    }
    return manifest,rows


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--epl",type=Path,required=True)
    p.add_argument("--mls",type=Path,required=True)
    p.add_argument("--manifest",type=Path,required=True)
    p.add_argument("--rows",type=Path,required=True)
    p.add_argument("--feature-schema",type=Path,required=True)
    a=p.parse_args()
    if any(x.exists() for x in (a.manifest,a.rows,a.feature_schema)):
        p.error("refusing to overwrite versioned scoring inputs")
    paths=(a.epl,a.mls)
    manifest,rows=prepare([json.loads(path.read_text()) for path in paths],paths)
    for target in (a.manifest,a.rows,a.feature_schema):
        target.parent.mkdir(parents=True,exist_ok=True)
    a.manifest.write_text(json.dumps(manifest,sort_keys=True,indent=2)+"\n")
    a.rows.write_text("".join(json.dumps(x,sort_keys=True,separators=(",",":"))+"\n" for x in rows))
    a.feature_schema.write_text(json.dumps(manifest["feature_names"])+"\n")
    print(json.dumps({"players":manifest["league_counts"],
                      "features":manifest["feature_count"],
                      "source_coverage":manifest["source_coverage"]},indent=2))


if __name__=="__main__":
    main()
