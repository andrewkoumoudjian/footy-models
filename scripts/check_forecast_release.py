#!/usr/bin/env python3
"""Fail-closed release eligibility audit for the versioned football forecast targets.

Never infer that a historical *different* model validated the current FM26
second-resolved engine. A missing label stream or no exact-model OOS means
research_only. The 200-fixture and 2500-path thresholds are explicit operator
policies, not mathematical guarantees of model accuracy.
"""
import argparse
import json
import math
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

def same_engine_oos_ok(snapshot, report):
    """Only matching engine/version, strict temporal holdout evidence can qualify.

    A research baseline on 2015 StatsBomb data is NEVER a compatible report.
    """
    if not isinstance(report,dict) or report.get('schema_version')!=1:
        return False
    fingerprint=snapshot.get('model_code_sha256')
    if not (isinstance(fingerprint,str) and len(fingerprint)==64
            and report.get('model_code_sha256')==fingerprint):
        return False
    if report.get('model')!=snapshot.get('model'):
        return False
    try:
        return (report['protocol']=='same_engine_walk_forward_frozen_pre_kickoff'
                and report['fit_end_date']<report['oos_start_date']
                and report['oos_end_date']<snapshot['as_of']
                and report.get('tested_without_selection_or_tuning') is True
                and report.get('same_fixture_engine_and_output_contract') is True)
    except (KeyError,TypeError):
        return False


def audit(contract, snapshot, events=None, assist=None, matching_oos=None):
    if contract['schema_version']!=1:raise ValueError('unknown target contract')
    if len({x['id'] for x in contract['targets']})!=len(contract['targets']):
        raise ValueError('duplicate forecast target ids')
    fixtures=snapshot['fixtures']
    if not fixtures:raise ValueError('no fixtures')
    min_paths=min(int(f['paths']) for f in fixtures)
    if min_paths<=0:raise ValueError('paths must be positive')
    halfwidth=1.96*math.sqrt(.25/min_paths)
    minimum=contract['minimum_independent_paths_for_public_result_probability']
    compatible=same_engine_oos_ok(snapshot,matching_oos)
    labels=[]
    for t in contract['targets']:
        detail={**t,'publication_status':'research_only','gate_reasons':[]}
        if t['release']=='descriptive_only':
            detail['publication_status']='descriptive_only'
        elif t['release']=='not_in_simulator':
            detail['publication_status']='unavailable'
            detail['gate_reasons'].append('not produced by current simulation')
        else:
            if min_paths<minimum:
                detail['gate_reasons'].append(f'{min_paths} simulated full-match paths < {minimum} required by precision policy')
            if t['id']=='time_goal_1min' and any(len(f.get('minutes_1min',[]))!=90 for f in fixtures):
                detail['publication_status']='unavailable'
                detail['gate_reasons'].append('frozen snapshot predates native 90-minute export')
            if t['id'] in ('player_goal','player_primary_assist','player_goal_plus_assist') and any(
                'player_minute_events' not in f for f in fixtures):
                detail['publication_status']='unavailable'
                detail['gate_reasons'].append('missing same-path player-event credit in this frozen snapshot')
            evidence = (matching_oos.get('targets',{}).get(t['id'])
                        if compatible else None)
            if not isinstance(evidence,dict):
                evidence=None
            if not evidence:
                detail['gate_reasons'].append('no version-matched, pre-match OOS evaluation of the current player simulator')
            else:
                n = evidence.get('distinct_oos_fixtures',0)
                if not isinstance(n,int) or n < contract['minimum_distinct_oos_matches_for_release']:
                    detail['gate_reasons'].append('insufficient held-out fixture count')
                if evidence.get('primary_score')!=t['primary_score']:
                    detail['gate_reasons'].append('evaluation used wrong primary scoring rule')
                if evidence.get('label_coverage',0)<.95:
                    detail['gate_reasons'].append('verified target-label coverage below 95%')
                if not (isinstance(evidence.get('matched_day_bootstrap_95'),list)
                        and len(evidence['matched_day_bootstrap_95'])==2
                        and all(isinstance(x,(float,int)) and math.isfinite(x) for x in evidence['matched_day_bootstrap_95'])
                        and evidence['matched_day_bootstrap_95'][1]<0):
                    detail['gate_reasons'].append('proper-score improvement over causal reference not demonstrated on matchday blocks')
            if t['id']=='player_primary_assist' and assist is not None and not assist['promotion']['approve_historical_prior']:
                detail['historical_assist_prior_warning']='2015/16 candidate failed; unrelated 2026-style engine evidence is required'
            if detail['publication_status']!='unavailable' and not detail['gate_reasons']:
                detail['publication_status']='calibrated_for_scored_target'
        labels.append(detail)
    return {'schema_version':1,'model':snapshot.get('model'),
      'as_of':snapshot.get('as_of'),'path_count_min':min_paths,
      'worst_case_mc_halfwidth_95':halfwidth,
      'all_match_targets_calibrated':all(
          t['publication_status']=='calibrated_for_scored_target'
          for t in labels if t['level']=='match'),
      'historical_event_data_cannot_validate_current_player_engine':True,
      'matching_engine_oos_valid':compatible,
      'independent_historical_event_oos_matches':events['split']['holdout_games'] if events else None,
      'historical_assist_prior_approved':assist['promotion']['approve_historical_prior'] if assist else None,
      'targets':labels}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--contract',default='configs/forecast_targets_v1.json')
    p.add_argument('--snapshot',default='artifacts/premier-league-2026-10-11/player_paths.json')
    p.add_argument('--event-oos',default='artifacts/validation/premier-league-events-2015-16-oos.json')
    p.add_argument('--assist-oos',default='artifacts/validation/primary-assist-prior-2015-16-oos.json')
    p.add_argument('--matching-engine-oos',default=None,
                   help='separately provided, versioned same-engine OOS evidence; never substitute the historical baseline')
    p.add_argument('--output',default='artifacts/validation/forecast-release-gates-2026-10-11.json')
    a=p.parse_args()
    def read(path):
        full=ROOT/path
        return json.loads(full.read_text()) if full.is_file() else None
    contract=read(a.contract);snapshot=read(a.snapshot)
    if contract is None or snapshot is None:raise ValueError('missing required target contract or snapshot')
    result=audit(contract,snapshot,read(a.event_oos),read(a.assist_oos),
                 read(a.matching_engine_oos) if a.matching_engine_oos else None)
    target=ROOT/a.output;target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(json.dumps(result,indent=2)+'\n')
    print(f'Checked {len(result["targets"])} targets; paths={result["path_count_min"]}; max MC 95% halfwidth=±{result["worst_case_mc_halfwidth_95"]:.1%}')
    for t in result['targets']:
        print(t['id'],t['publication_status'],'; '.join(t['gate_reasons'])[:140])
    print('saved',target)

if __name__=='__main__':main()
