#!/usr/bin/env python3
"""Chronological calibration falsification for P(recorded assist | scored non-own goal).

StatsBomb registered primary assists and non-own-goal shots only. This estimates
*assisted-goal frequency*, NOT which player assists, scoring, or current FM26
rates. Strictly earlier matchdays only. The final 24 covered matches are sealed
for evaluation, never parameter selection.
"""
from __future__ import annotations
import argparse
import json
import math
import random
from pathlib import Path
from backtest_pl_event_oos import load_games

ROOT = Path(__file__).resolve().parents[1]

def counts(game):
    goals=sum(game['teams'][t]['counts'].get('goals',0) for t in (game['home'],game['away']))
    assists=sum(game['teams'][t]['counts'].get('assists',0) for t in (game['home'],game['away']))
    if not 0<=assists<=goals:
        raise ValueError(f"inconsistent assist labels in {game['id']}: assists={assists}, goals={goals}")
    return goals,assists

def posterior(history, alpha=2.,beta=2.):
    goals,assists=zip(*(counts(g) for g in history))
    return (alpha+sum(assists))/(alpha+beta+sum(goals))

def log_loss(prob, goal_count, assisted_count):
    if not 0<prob<1:raise ValueError('probability should be strictly within (0,1)')
    return -assisted_count*math.log(prob)-(goal_count-assisted_count)*math.log1p(-prob)

def run(source,events_dir,holdout=24,seed=42):
    games,missing=load_games(source,events_dir)
    if not 5<=holdout<=len(games)-15:raise ValueError('holdout too large for calibration sample')
    evaluation=games[-holdout:]
    # The last 24 events may share an exact date with one or more earlier
    # fixtures. Drop those same-date matches from fitting, never silently
    # allow within-day leakage into a pre-kickoff forecast.
    training=[g for g in games[:-holdout] if g['date']<evaluation[0]['date']]
    if len(training)<15 or any(g['date']>=evaluation[0]['date'] for g in training):
        raise ValueError('insufficient strictly earlier calibration matches')
    sample_goals=sum(counts(g)[0] for g in training)
    sample_assists=sum(counts(g)[1] for g in training)
    fixed68=0.68
    trained=posterior(training)
    methods={'historical_posterior':trained,'preexisting_0_68':fixed68,'uniform_0_50':0.5}
    rows=[]
    for game in evaluation:
        goals,assists=counts(game)
        rows.append({'match_id':game['id'],'date':game['date'],'home':game['home'],'away':game['away'],
                     'non_own_goals':goals,'registered_primary_assists':assists,
                     'per_method_logloss':{key:log_loss(p,goals,assists) for key,p in methods.items()}})
    n=sum(r['non_own_goals'] for r in rows)
    if n==0:raise ValueError('holdout contains no scored goals')
    loss={key:sum(r['per_method_logloss'][key] for r in rows)/n for key in methods}
    rng=random.Random(seed)
    contrasts={}
    for model in ('historical_posterior','preexisting_0_68'):
        values=[]
        for _ in range(3000):
            batch=[rows[rng.randrange(len(rows))] for _ in rows]
            goals=sum(r['non_own_goals'] for r in batch)
            if goals==0:continue
            values.append(sum(r['per_method_logloss'][model]-r['per_method_logloss']['uniform_0_50']
                              for r in batch)/goals)
        values.sort()
        contrasts[model]={'vs_uniform_logloss_delta':loss[model]-loss['uniform_0_50'],
           'paired_fixture_bootstrap_95': [values[int(.025*len(values))],values[min(len(values)-1,int(.975*len(values)))]],
           'beats_uniform_on_sealed_holdout': loss[model]<loss['uniform_0_50'],
           'significantly_beats_uniform':values[min(len(values)-1,int(.975*len(values)))]<0}
    paired_existing=[]
    for _ in range(3000):
        batch=[rows[rng.randrange(len(rows))] for _ in rows]
        goals=sum(r['non_own_goals'] for r in batch)
        if goals==0: continue
        paired_existing.append(sum(r['per_method_logloss']['historical_posterior']-
                                   r['per_method_logloss']['preexisting_0_68'] for r in batch)/goals)
    paired_existing.sort()
    existing_contrast={
        'model_minus_existing_0_68_logloss':loss['historical_posterior']-loss['preexisting_0_68'],
        'paired_fixture_bootstrap_95':[paired_existing[int(.025*len(paired_existing))],
                                       paired_existing[min(len(paired_existing)-1,int(.975*len(paired_existing)))]],
        'clearly_better_than_existing':paired_existing[min(len(paired_existing)-1,int(.975*len(paired_existing)))]<0,
    }
    promoted=(contrasts['historical_posterior']['significantly_beats_uniform']
              and existing_contrast['clearly_better_than_existing'])
    return {'schema_version':1,'target':'assisted_scored_goal_probability',
       'definition':'P(StatsBomb Pass.goal_assist=true | non-own Goal shot)',
       'training_matches':len(training),'holdout_matches':len(evaluation),
       'training_through':training[-1]['date'],'oos_from':evaluation[0]['date'],
       'oos_to':evaluation[-1]['date'],'covered_games':len(games),'uncaptured_season_games':missing,
       'training_goals':sample_goals,'training_primary_assists':sample_assists,
       'training_observed_assist_ratio':sample_assists/sample_goals,
       'training_only_beta_2_2_posterior':trained,
       'holdout_goals':n,'holdout_primary_assists':sum(r['registered_primary_assists'] for r in rows),
       'holdout_observed_ratio':sum(r['registered_primary_assists'] for r in rows)/n,
       'mean_binary_log_loss_per_goal':loss,'contrasts':contrasts,
       'historical_prior_vs_existing':existing_contrast,
       'promotion':{'approve_historical_prior':promoted,'reason':
         'accept only when paired fixture-bootstrap upper 95% loss differences vs both 0.50 and existing 0.68 are negative; do not calibrate FM26 from this incompatible historical sample'},
       'limitations':['StatsBomb 2015/16 PL events cover only subset of fixture calendar',
         'The holdout cannot identify which player will assist or validate xA-like attributes',
         'This is not a compatible historical version of 2026 FM26 and cannot calibrate that specific model'],
       'evaluated_games':rows}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--matches',default='data/statsbomb/matches/pl_2015_16.json')
    p.add_argument('--events',default='data/statsbomb/events')
    p.add_argument('--holdout-matches',type=int,default=24)
    p.add_argument('--output',default='artifacts/validation/primary-assist-prior-2015-16-oos.json')
    args=p.parse_args()
    result=run(ROOT/args.matches,ROOT/args.events,args.holdout_matches)
    path=ROOT/args.output;path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(result,indent=2)+'\n')
    print('train',result['training_matches'],'games',result['training_goals'],'goals',result['training_primary_assists'],'assists')
    print('test',result['holdout_matches'],'games',result['holdout_goals'],'goals',result['holdout_primary_assists'],'assists')
    print('posterior',round(result['training_only_beta_2_2_posterior'],5))
    print('losses', {k:round(v,5) for k,v in result['mean_binary_log_loss_per_goal'].items()})
    print('promotion',result['promotion'])
    print('wrote',path)

if __name__=='__main__':main()
