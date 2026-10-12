#!/usr/bin/env python3
"""One shared, player-agnostic EPL/MLS structured GLM (no deep learning).

Joint conditional law:
    P(start | as-of features) = logistic(f·beta_start)
    shots | playing minutes, as-of f ~ Poisson(minutes/90 * exp(f·beta_shot))
    goals | shots, as-of f ~ Binomial(shots, logistic(f·beta_conversion))

A single fitted artifact owns one feature normalization, one regularizer and
all three statistically compatible heads. Teams/players appear only in audit
metadata; *no individual identity coefficients or learnable lookup tables*.

Labels are post-match and may enter ONLY fitting/scoring, never a pre-match
feature. Projection to 1GK+10 outfield and whole-match event conservation
belongs to the existing Rust mathematical simulation.
"""
from __future__ import annotations

import argparse
from datetime import date
import hashlib
import json
import math
from pathlib import Path

import numpy as np
from scipy.optimize import minimize

from unified_features import FEATURE_NAMES

TASKS = ("start", "shots", "conversion")
L2 = 1.0
MAX_DIMENSION = len(FEATURE_NAMES) * 2 + 1


def training_matrix(rows, means=None, scales=None, feature_names=FEATURE_NAMES):
    if not rows:
        raise ValueError("empty training/evaluation rows")
    width = len(feature_names)
    raw = np.full((len(rows), width), np.nan, dtype=float)
    for i, r in enumerate(rows):
        features = r.get("features") or {}
        if set(features) != set(feature_names):
            raise ValueError("exact frozen feature contract required")
        if any(key not in feature_names for key in features):
            raise ValueError("feature schema drift")
        for j, name in enumerate(feature_names):
            value = features[name]
            if value is not None:
                if not isinstance(value, (float, int)) or not math.isfinite(value):
                    raise ValueError("nonfinite feature")
                raw[i,j] = float(value)
    if means is None:
        with np.errstate(invalid="ignore"):
            counts = np.sum(np.isfinite(raw), axis=0)
            numerators = np.sum(np.where(np.isfinite(raw), raw, 0), axis=0)
            means = np.divide(numerators, counts, out=np.zeros(width), where=counts>0)
            var = np.divide(np.sum(np.where(np.isfinite(raw), (raw-means)**2,0),axis=0),
                            counts, out=np.ones(width), where=counts>1)
        scales = np.maximum(np.sqrt(var), 1e-9)
    elif len(means) != width or len(scales) != width:
        raise ValueError("saved transformation does not match frozen features")
    missing = (~np.isfinite(raw)).astype(float)
    normalized = np.where(np.isfinite(raw), (raw-means)/scales, 0.0)
    # Numerical stabilization outside the training support; the clipping is a
    # fixed operational policy, not chosen using sealed test labels.
    normalized = np.clip(normalized, -10.0, 10.0)
    return np.column_stack([np.ones(len(rows)), normalized, missing]), means, scales


def label_arrays(rows):
    selection = []
    minutes = []
    shots = []
    goals = []
    for r in rows:
        y = r.get("label")
        if not isinstance(y, dict):
            raise ValueError("training/evaluation rows must have independently observed labels")
        start = y.get("started")
        if start not in (True, False, 0, 1):
            raise ValueError("invalid binary start target")
        m, s, g = y.get("minutes"),y.get("shots"),y.get("goals")
        if m is None or s is None or g is None:
            raise ValueError("missing player event target cannot be coded as zero")
        m, s, g = float(m),float(s),float(g)
        if (not all(math.isfinite(v) for v in (m,s,g))
            or m<0 or m>130 or s<0 or g<0
            or int(s)!=s or int(g)!=g or g>s
            or (m==0 and (s!=0 or g!=0))):
            raise ValueError("invalid match exposure / shot-to-goal event conservation")
        selection.append(float(start))
        minutes.append(m)
        shots.append(s)
        goals.append(g)
    return tuple(np.asarray(x,dtype=float) for x in (selection, minutes, shots, goals))


def objective(beta_flat, x, labels, penalty):
    starts, minutes, shots, goals = labels
    n, k = x.shape
    beta = beta_flat.reshape(3,k)
    z_start = np.clip(x @ beta[0], -35,35)
    z_shot = np.clip(x @ beta[1], -18,12)
    z_conv = np.clip(x @ beta[2], -35,35)

    exposure = minutes / 90.0
    valid_shots = exposure > 0
    valid_conv = shots > 0
    mu = exposure[valid_shots]*np.exp(z_shot[valid_shots])
    count_scales = np.maximum(1, np.sum(valid_shots))
    conv_scales = np.maximum(1, np.sum(shots[valid_conv]))
    # Normalize each conditional component so the most frequent events do not
    # automatically dominate the joint optimization.
    start_ll = np.mean(np.logaddexp(0,z_start) - starts*z_start)
    shot_ll = (np.sum(mu - shots[valid_shots]*z_shot[valid_shots])
               / count_scales)
    conv_ll = (np.sum(shots[valid_conv]*np.logaddexp(0,z_conv[valid_conv])
                      - goals[valid_conv]*z_conv[valid_conv])
               / conv_scales)
    ridge = penalty*np.sum(beta[:,1:]**2)
    total = start_ll + shot_ll + conv_ll + ridge

    grads = np.zeros_like(beta)
    grads[0] = x.T @ (np.exp(-np.logaddexp(0,-z_start))-starts) / n
    if np.any(valid_shots):
        residual = mu-shots[valid_shots]
        grads[1] = x[valid_shots].T @ residual / count_scales
    if np.any(valid_conv):
        p = np.exp(-np.logaddexp(0,-z_conv[valid_conv]))
        grads[2] = x[valid_conv].T @ (shots[valid_conv]*p-goals[valid_conv]) / conv_scales
    grads[:,1:] += 2*penalty*beta[:,1:]
    return float(total),grads.ravel()


def validate_splits(rows):
    if not rows:
        raise ValueError("training dataset empty")
    groups = {k:[] for k in ("train","validation","test")}
    seen = set()
    for r in rows:
        s=r.get("split")
        if s not in groups:
            raise ValueError("train/validation/test split required")
        try:
            dt=date.fromisoformat(r["date"])
        except (KeyError,ValueError) as exc:
            raise ValueError("full fixture date required") from exc
        if r.get("league") not in ("EPL","MLS"):
            raise ValueError("only EPL or MLS are eligible")
        identity = (r["league"],r["fixture_id"],r["canonical_player_id"])
        if identity in seen:
            raise ValueError("duplicate player/fixture label")
        seen.add(identity)
        # Pre-match feature cutoff must be STRICTLY before match date; no
        # ingestion of full-season or same-match features here.
        if not r.get("feature_cutoff_utc") or r["feature_cutoff_utc"][:10] >= r["date"]:
            raise ValueError("as-of features are not earlier than target fixture")
        groups[s].append(r)
    if not all(groups.values()):
        raise ValueError("three nonempty chronological splits required")
    maxima={k:max(date.fromisoformat(r["date"]) for r in rows) for k,rows in groups.items()}
    minima={k:min(date.fromisoformat(r["date"]) for r in rows) for k,rows in groups.items()}
    if maxima["train"]>=minima["validation"] or maxima["validation"]>=minima["test"]:
        raise ValueError("chronological split overlap")
    return groups


def predict_matrix(x, coefficients):
    beta=np.asarray(coefficients,dtype=float).reshape(3,x.shape[1])
    score=np.clip(x@beta.T,[-35,-18,-35],[35,12,35])
    start=1/(1+np.exp(-score[:,0]))
    shots=np.exp(score[:,1])
    convert=1/(1+np.exp(-score[:,2]))
    return start,shots,convert


def proper_loss(rows,transform,coefficients,feature_names=FEATURE_NAMES):
    x,_,_=training_matrix(rows,*transform,feature_names=feature_names)
    y,minutes,shots,goals=label_arrays(rows)
    p_start, rate, conversion=predict_matrix(x,coefficients)
    p_start=np.clip(p_start,1e-12,1-1e-12)
    conversion=np.clip(conversion,1e-12,1-1e-12)
    exposure=minutes/90
    predicted=np.maximum(exposure*rate,1e-12)
    scores={
        "start_log_loss":float(np.mean(-y*np.log(p_start)-(1-y)*np.log(1-p_start))),
        "shot_poisson_log_loss":float(np.mean(
            predicted-shots*np.log(predicted)+
            np.asarray([math.lgamma(s+1) for s in shots]))),
        "conversion_binomial_log_loss":float(
            np.sum(-goals*np.log(conversion)-(shots-goals)*np.log(1-conversion))
            / max(1,np.sum(shots))),
    }
    return scores


def fit(rows, penalty=L2, feature_names=FEATURE_NAMES):
    splits=validate_splits(rows)
    train=splits["train"]
    if len(feature_names) != len(set(feature_names)) or not feature_names:
        raise ValueError("duplicate or empty canonical feature columns")
    x,means,scales=training_matrix(train,feature_names=feature_names)
    labels=label_arrays(train)
    if not all(np.isfinite(x.flat)):
        raise ValueError("nonfinite training matrix")
    initial=np.zeros(3*x.shape[1],dtype=float)
    initial[x.shape[1]*1]=math.log(1.0)
    initial[x.shape[1]*2]=math.log(0.1/0.9)
    result=minimize(objective,initial,args=(x,labels,penalty),method="L-BFGS-B",
                    jac=True,options={"maxiter":700,"gtol":1e-8})
    if not np.isfinite(result.fun) or not np.all(np.isfinite(result.x)):
        raise RuntimeError("non-finite joint model fit")
    if not result.success and np.linalg.norm(result.jac)>1e-4:
        raise RuntimeError(f"multivariate joint fit failed: {result.message}")
    beta=result.x.reshape(3,x.shape[1])
    transform=(means,scales)
    return {
        "schema":"shared_epl_mls_math_glm_v1",
        "model_family":"pooled_regularized_conditional_start_shot_goal_likelihood",
        "feature_contract":("unified_epl_mls_248_v1" if len(feature_names)==248
                            else "unified_math_features_v1"),
        "feature_names":list(feature_names),
        "hyperparameter_l2":penalty,
        "fit_data_cutoff_exclusive":min(r["date"] for r in splits["validation"]),
        "fit_support":{"train":len(train),"validation":len(splits["validation"]),
                       "test":len(splits["test"])},
        "transform":{"means":means.tolist(),"scales":scales.tolist(),
                     "missing_features_double_width":True},
        "coefficients":{task:beta[i].tolist() for i,task in enumerate(TASKS)},
        "loss":{
            part:proper_loss(splits[part],transform,beta,feature_names)
            for part in ("train","validation","test")
        },
        "assessment":"research_only; no promotion on inspected or unsealed fixtures",
        "identity_features_in_numeric_training":False,
        "parameter_sharing":"same 3 task heads for all players in EPL and MLS; no player lookup",
        "simulation_contract":"calibrate exact-cardinality XI using shared p(start); conditional shot and conversion means must be consumed by one Rust event path",
    }


def score(model,rows):
    if model.get("schema")!="shared_epl_mls_math_glm_v1":
        raise ValueError("unknown model format")
    features = model.get("feature_names")
    if not isinstance(features,list) or not features or len(features)!=len(set(features)):
        raise ValueError("feature schema drift")
    transform=model["transform"]
    x,_,_=training_matrix(rows,transform["means"],transform["scales"],features)
    beta=np.asarray([model["coefficients"][s] for s in TASKS])
    p,shots,conversion=predict_matrix(x,beta)
    return [{
        "canonical_player_id":r["canonical_player_id"],"league":r["league"],
        "p_start_unprojected":float(p[i]),
        "shot_attempts_per90_given_playing":float(shots[i]),
        "p_goal_per_shot_given_playing":float(conversion[i]),
        "probabilities_not_published_without_same_engine_oos":True,
    } for i,r in enumerate(rows)]


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--training-jsonl",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--l2",type=float,default=L2)
    p.add_argument("--feature-schema",type=Path,
                   help="optional JSON ordered feature_names for the canonical 248-feature path")
    args=p.parse_args()
    if args.output.exists():
        p.error("refuse overwrite frozen fit")
    if not math.isfinite(args.l2) or args.l2<=0:
        p.error("invalid l2")
    rows=[json.loads(line) for line in args.training_jsonl.read_text().splitlines() if line.strip()]
    names=json.loads(args.feature_schema.read_text()) if args.feature_schema else list(FEATURE_NAMES)
    if not isinstance(names,list) or any(not isinstance(k,str) for k in names):
        p.error("feature schema is not an ordered list of column names")
    report=fit(rows,args.l2,tuple(names))
    report["training_sha256"]=hashlib.sha256(args.training_jsonl.read_bytes()).hexdigest()
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n")
    print(json.dumps({"model":report["schema"],"n":report["fit_support"],
                      "proper_losses":report["loss"]},indent=2))


if __name__=="__main__":
    main()
