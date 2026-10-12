#!/usr/bin/env python3
"""One-command, immutable, ESPN-as-of -> FM26 matchforecast research run.

ESPN files must have been retrieved *before* the forecast cutoff. A dated
official roster, verifiable future scoreboard and local FM26 extract are
required. Never silently fall back to invented current-season player histories.
"""
import argparse
from datetime import date, datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from espn_feed import parse_utc, stable_json
from attach_espn import verify_future_schedule

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
MODEL = ROOT / 'target/release/footy-matchforecast-math'
SOURCE_FILES = [
    'apps/matchforecast/src/math_engine/mod.rs',
    'apps/matchforecast/src/math_engine_main.rs',
    'apps/matchforecast/math_engine/build_inputs.py',
    'apps/matchforecast/math_engine/espn_feed.py',
    'apps/matchforecast/math_engine/attach_espn.py',
    'apps/matchforecast/math_engine/run_espn.py',
    'crates/match/src/lineup_entropy.rs',
    'crates/match/src/team_goals.rs',
    'Cargo.lock',
]


def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1<<20),b''):
            h.update(chunk)
    return h.hexdigest()


def verify_simulated_output(data, result, scenario):
    assert result['status']=='research_only_unvalidated'
    assert result['data_sources']==data['sources']
    assert result['feature_source_provenance']==data['feature_source_provenance']
    assert result['feature_cutoff_utc']==data['feature_cutoff_utc']
    assert result['fixture_kickoff_utc']==data['fixture_kickoff_utc']
    assert len(result['fixtures'])==1
    forecast=result['fixtures'][0]
    assert abs(sum(forecast[k] for k in ('home_win','draw','away_win'))-1)<1e-8
    assert abs(sum(x['probability'] for x in forecast['scorelines'])-1)<1e-8
    assert len(forecast['player_forecasts'])>=22
    assert (result['parameters']['use_selection_marginals'] is scenario)
    assert (result['parameters']['maximum_substitutions_per_team']==(5 if scenario else 0))
    for row in forecast['player_forecasts']:
        assert 0 <= row['p_start'] <= row['p_appearance'] <= 1
        assert 0 <= row['p_goal'] <= row['p_shot'] <= row['p_appearance']
        assert 0 <= row['expected_goals'] <= row['expected_shots']
        assert 0 <= row['expected_minutes'] <= 90 * row['p_appearance'] + 1e-8
    assert abs(sum(p['expected_goals'] for p in forecast['player_forecasts'])
               -forecast['expected_home_goals']-forecast['expected_away_goals'])<1e-8
    assert abs(sum(p['expected_shots'] for p in forecast['player_forecasts'])
               -forecast['expected_home_shots']-forecast['expected_away_shots'])<1e-8
    for side in ('home','away'):
        assert abs(sum(forecast[f'{side}_window_goals'])/result['paths']
                   -forecast[f'expected_{side}_goals'])<1e-8
    for path, digest in data['sources'].items():
        if sha(Path(path)) != digest:
            raise ValueError(f'model data input hash changed mid-run: {path}')


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--output-dir',type=Path,required=True)
    ap.add_argument('--as-of',required=True,help='team/roster calendar date YYYY-MM-DD')
    ap.add_argument('--forecast-cutoff-utc',required=True,
                    help='auditable moment this forecast was constructed in UTC')
    ap.add_argument('--fixture-date',required=True)
    ap.add_argument('--home',required=True)
    ap.add_argument('--away',required=True)
    ap.add_argument('--kickoff-utc',required=True)
    ap.add_argument('--squads',type=Path,required=True)
    ap.add_argument('--espn',type=Path,required=True)
    ap.add_argument('--schedule-board',type=Path,required=True)
    ap.add_argument('--fm',type=Path,default=Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv'))
    ap.add_argument('--fm-effective',default='2026-03-01')
    ap.add_argument('--paths',type=int,default=2500)
    ap.add_argument('--seed',type=int,default=42)
    ap.add_argument('--research-observed-only',action='store_true',
                    help='conditional scenario; excludes unverified candidates and is not publishable')
    args=ap.parse_args()
    if args.paths<1 or args.paths>1_000_000 or args.seed<0:
        ap.error('invalid MC paths or seed')
    cutoff=parse_utc(args.forecast_cutoff_utc)
    kickoff=parse_utc(args.kickoff_utc)
    if cutoff > datetime.now(timezone.utc):
        ap.error('forecast cutoff must not be future-dated')
    if kickoff<=cutoff:
        ap.error('cannot make a pre-match forecast after kickoff')
    match={'date':args.fixture_date,'home':args.home,'away':args.away}
    if date.fromisoformat(args.as_of)>cutoff.date():
        ap.error('base calendar feature date is later than forecast cutoff')
    board=json.loads(args.schedule_board.read_text())
    event_id=verify_future_schedule(board,match,kickoff)
    out=args.output_dir.resolve()
    if out.exists():
        ap.error(f'immutable research output directory already exists: {out}')
    out.mkdir(parents=True)
    fixtures=out/'fixtures.tsv'
    fixtures.write_text(f'date\thome\taway\n{args.fixture_date}\t{args.home}\t{args.away}\n')
    base=out/'base.json'
    enriched=out/'enriched.json'
    crosswalk=out/'crosswalk.json'
    predictions=out/'predictions.json'
    subprocess.run([sys.executable,str(HERE/'build_inputs.py'),'--fm',str(args.fm),
                    '--fm-effective',args.fm_effective,
                    '--as-of',args.as_of,'--fixtures',str(fixtures),
                    '--squads',str(args.squads),'--output',str(base)],
                   cwd=ROOT,check=True)
    command=[sys.executable,str(HERE/'attach_espn.py'),
             '--base-input',str(base),'--espn',str(args.espn),
             '--squads',str(args.squads),'--schedule-board',str(args.schedule_board),
             '--forecast-cutoff-utc',cutoff.isoformat(),'--kickoff-utc',kickoff.isoformat(),
             '--output',str(enriched),'--crosswalk-report',str(crosswalk)]
    if args.research_observed_only:
        command.append('--research-observed-only')
    subprocess.run(command,cwd=ROOT,check=True)
    subprocess.run(['cargo','build','--release','-p','footy-matchforecast',
                    '--bin','footy-matchforecast-math'],cwd=ROOT,check=True)
    sim=[str(MODEL),str(enriched),str(predictions),'--paths',str(args.paths),
         '--seed',str(args.seed)]
    if args.research_observed_only:
        sim+=['--selection-marginals','true','--max-subs','5']
    subprocess.run(sim,cwd=ROOT,check=True)
    data=json.loads(enriched.read_text())
    report=json.loads(predictions.read_text())
    verify_simulated_output(data,report,args.research_observed_only)
    manifest={
        'status':'unvalidated_research_scenario_not_for_publication',
        'candidate_scope':('matched_ESPN_observed_only_conditional_scenario'
                           if args.research_observed_only else
                           'all_FM_UID_matched_plus_observed_verified_non_FM_roster'),
        'all_official_registered_candidates_included':False,
        'verified_espn_event_id':event_id,
        'kickoff_utc':kickoff.isoformat(),'forecast_cutoff_utc':cutoff.isoformat(),
        'baseline_as_of':args.as_of,
        'source_coverage':data['feature_source_provenance'],
        'feature_source_hash':sha(args.espn),
        'prediction_sha256':sha(predictions),
        'enriched_sha256':sha(enriched),
        'binary_sha256':sha(MODEL),
        'implementation_sha256':{p:sha(ROOT/p) for p in SOURCE_FILES},
        'validation':'source hashes, strict chronology, official fixture, probability & event-count conservation passed',
    }
    (out/'manifest.json').write_text(stable_json(manifest))
    print(f'PASS: verified future event {event_id}; {len(data["players"])} candidate identities, '
          f'{args.paths} coherent paths; scenario={args.research_observed_only}; '
          f'frozen output={out}')


if __name__=='__main__':
    main()
