#!/usr/bin/env python3
"""One-command, auditable Premier League FM26 -> match engine -> timelines pipeline.

- full: historical result refresh/refit, authoritative registered club matching,
  dated injuries, player priors, stateful match simulation, snapshot & timeline.
- render-only: regenerate charts/report from a committed snapshot without raw data.
- dry-run: print exactly what would be executed; do not write files.

Do not use later information in pre-match snapshots; full runs require a dated
availability file. This does *not* claim the EPL player model is well calibrated.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from datetime import date,timedelta
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

MODEL_SOURCE_DIRS=(
    'crates/match/src',
    'crates/core/src',
    'crates/forecast/src',
    'crates/state/src',
    'apps/matchforecast/src',
    'apps/plforecast/src',
)
MODEL_SOURCE_FILES=(
    'Cargo.lock',
    'Cargo.toml',
    'crates/match/Cargo.toml',
    'apps/matchforecast/Cargo.toml',
    'apps/plforecast/Cargo.toml',
    'scripts/forecast_premier_league.py',
    'scripts/match_premier_league_fm26.py',
    'scripts/fm26_complete.py',
    'scripts/recover_epl_remaining_identities.py',
    'scripts/resolve_epl_reep.py',
    'scripts/reconcile_epl_fm_sources.py',
    'scripts/promote_epl_fm_tm_evidence.py',
    'scripts/prepare_pl_player_inputs.py',
)

def model_code_files():
    """All relevant domain, model, simulator, and adapter source files."""
    files={ROOT/name for name in MODEL_SOURCE_FILES}
    for name in MODEL_SOURCE_DIRS:
        directory=ROOT/name
        if not directory.is_dir():
            raise FileNotFoundError(f'missing model source directory: {name}')
        files.update(directory.rglob('*.rs'))
    return tuple(sorted(str(src.relative_to(ROOT)) for src in files))

def model_code_fingerprint():
    """Fingerprint the match+feature implementation, independent of Git dirt."""
    h=hashlib.sha256()
    for name in model_code_files():
        src=ROOT/name
        if not src.is_file():
            raise FileNotFoundError(f'missing model implementation file: {name}')
        h.update(name.encode('utf-8')+b'\0')
        h.update(src.read_bytes())
        h.update(b'\0')
    return h.hexdigest()

def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda:f.read(1<<20),b''):h.update(block)
    return h.hexdigest()

def execute(args, dry=False):
    print('+',' '.join(str(x) for x in args),flush=True)
    if not dry:subprocess.run([str(x) for x in args],cwd=ROOT,check=True)

def snapshot_audit(src,dest):
    data=json.loads(src.read_text())
    teams={}
    for team,r in data['teams'].items():
        teams[team]={
            'official_club':r['official_club'],
            'registered_players':sum(p['registration']=='registered' for p in r['players']),
            'official_listed_players':r['registered_count'],
            'fm_matches':r['matched_count'],
            'fm_matched_registered':r['matched_25'],
            'fm_matched_u21':r['matched_u21'],
            'matched':[{'name':p['official_name'],'fm_name':p.get('fm_name'),'fm_uid':p['fm_uid'],
                        'registration':p['registration'],'matching_method':p['match_method'],
                        'fm_kind':p.get('fm_kind','unknown'),
                        'attr_source':p.get('attr_source','unknown'),
                        'fm_attributes_verified':p.get('fm_attributes_verified',False),
                        'identity_verification':p.get('identity_verification'),
                        'native_fm_person_evidence':p.get('fm_native_record_trusted_for_this_player')}
                       for p in r['players'] if p.get('fm_uid')],
            'unmatched':r['unmatched']}
    dest.write_text(json.dumps({'as_of':data['as_of'],'registration_source':data['registration_source'],
                                'model':'Reep-first typed FM identity bridge + native/SI/TM evidence','teams':teams},
                               indent=2,ensure_ascii=False)+'\n')

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--fixtures',default='configs/pl_2026-10-11.json')
    ap.add_argument('--availability',default='configs/pl_availability_2026-10-11.json')
    ap.add_argument('--fm-file',help='raw FM26 men attributes CSV (full pipeline only)')
    ap.add_argument('--registration-url',help='official registered squad source for the season')
    ap.add_argument('--html-cache',help='optional season-specific cached official roster HTML')
    ap.add_argument('--offline',action='store_true',help='use cached football results + squad HTML')
    ap.add_argument('--paths',type=int,default=72)
    ap.add_argument('--seed',type=int,default=42)
    ap.add_argument('--snapshot-dir',help='override artifacts/<matchweek>')
    ap.add_argument('--images-dir',help='override assets/predictions/<matchweek>')
    ap.add_argument('--report',help='override docs/premier-league-player-forecasts-<date>.md')
    ap.add_argument('--render-only',action='store_true',help='no FM/raw results necessary; use saved player_paths.json')
    ap.add_argument('--historical-event-oos',action='store_true',
                    help='also score separate 2015/16 StatsBomb event-rate baseline on held-out matches')
    ap.add_argument('--historical-same-engine-oos',action='store_true',
                    help='also run the observed-stat-only same-engine 2015/16 walk-forward backtest '
                         '(build inputs, run footy-matchforecast-pl-hist, score OOS)')
    ap.add_argument('--historical-paths',type=int,default=200,
                    help='paths per fixture for the same-engine historical backtest')
    ap.add_argument('--historical-jobs',type=int,default=0,
                    help='worker threads for the historical backtest (0 = auto)')
    ap.add_argument('--matching-engine-oos',
                    help='optional fully verified same-Rust-engine historical OOS report (no cross-model substitution)')
    ap.add_argument('--polymarket-spec',
                    help='verified regulation-time 1X2 event and YES token mapping; descriptive sidecar only')
    ap.add_argument('--polymarket-histories',
                    help='optional frozen three-token price histories for offline/reproducible market analysis')
    ap.add_argument('--polymarket-output',
                    help='output for optional market comparison (defaults to local untracked data, never frozen forecast JSON)')
    ap.add_argument('--dry-run',action='store_true',help='show plan; change nothing')
    opt=ap.parse_args()
    config=json.loads((ROOT/opt.fixtures).read_text())
    cutoff=date.fromisoformat(config['as_of'])
    if opt.polymarket_histories and not opt.polymarket_spec:
        ap.error('--polymarket-histories requires --polymarket-spec')
    if opt.polymarket_spec and opt.render_only:
        ap.error('market comparisons require a new pre-match run, never a frozen render-only artifact')
    if opt.polymarket_spec and not opt.dry_run:
        market_spec=json.loads((ROOT/opt.polymarket_spec).read_text())
        if market_spec.get('snapshot_utc','')[:10] > str(cutoff):
            ap.error('market snapshot cannot postdate the model as-of cutoff')
    dates=[date.fromisoformat(f['date']) for f in config['fixtures']]
    if not dates or any(d<=cutoff for d in dates):
        ap.error('all fixture kickoffs must be strictly after as-of cutoff')
    stamp=min(dates).isoformat()
    snapshot=Path(opt.snapshot_dir or f'artifacts/premier-league-{stamp}')
    images=Path(opt.images_dir or f'assets/predictions/{stamp}')
    artifact=ROOT/snapshot
    source=snapshot/'player_paths.json'
    season_year=cutoff.year if cutoff.month>=7 else cutoff.year-1
    season_cache=opt.html_cache or f'data/pl/raw/registered_pl_{season_year}_{(season_year+1)%100:02d}.html'
    roster=Path(f'data/pl/squads_{cutoff}_canonical_v2.json')
    benchmark=Path('data/pl/forecasts.json')
    script=Path('scripts')
    python=sys.executable
    run=lambda command:execute(command,opt.dry_run)
    if not opt.render_only:
        if opt.offline and opt.registration_url and not opt.html_cache:
            ap.error('custom season registration URL in offline mode requires --html-cache for that season')
        if not opt.fm_file or not Path(opt.fm_file).is_file():
            ap.error('full pipeline requires an existing --fm-file; use --render-only for saved snapshot')
        if opt.paths < 1 or opt.paths > 10000:ap.error('--paths must be 1..10000')
        if opt.seed < 0:ap.error('--seed must be nonnegative')
        availability=json.loads((ROOT/opt.availability).read_text())
        if availability.get('as_of')!=str(cutoff):
            ap.error('availability file must be explicitly date-frozen to the selected as-of')
        if not opt.dry_run:artifact.mkdir(parents=True,exist_ok=True)
        train=[python,script/'forecast_premier_league.py','--fixtures',opt.fixtures,'--release']
        if opt.offline:train.append('--offline')
        run(train)
        # The first-party canonical resolver requires DuckDB. Never fall back
        # to the retired name-first FM matcher if a dependency is missing.
        match=['uv','run','--no-project','--with','duckdb','--with','beautifulsoup4','python',
               script/'match_premier_league_fm26.py','--fixtures',opt.fixtures,
               '--fm-file',opt.fm_file,'--as-of',str(cutoff),'--html-cache',season_cache,'--output',roster]
        if opt.registration_url:match+=['--registration-url',opt.registration_url]
        if opt.offline:match.append('--offline')
        run(match)
        run([python,script/'prepare_pl_player_inputs.py','--fixtures',opt.fixtures,
             '--squads',roster,'--availability',opt.availability])
        run(['cargo','run','--quiet','--release','-p','footy-matchforecast',
             '--bin','footy-matchforecast-pl','--','--paths',str(opt.paths),
             '--seed',str(opt.seed),'--as-of',str(cutoff+timedelta(days=1)),
             '--snapshot-as-of',str(cutoff),'--results','data/pl/asof_team_stats.csv',
             '--players','data/pl/player_fm_join.csv','--baseline',benchmark,
             '--output',source])
        if not opt.dry_run:
            # Persist the exact model/feature source fingerprint into every
            # *new* simulation. The publication gate can then require exact
            # fingerprint equality with a genuinely held-out OOS run.
            model_fingerprint=model_code_fingerprint()
            result_path=ROOT/source
            result=json.loads(result_path.read_text())
            result['model_code_sha256']=model_fingerprint
            staged=result_path.with_suffix('.json.tmp')
            staged.write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
            staged.replace(result_path)
            shutil.copyfile(ROOT/benchmark,artifact/'team_benchmark.json')
            shutil.copyfile(ROOT/'data/pl/provenance.json',artifact/'team_benchmark_provenance.json')
            snapshot_audit(ROOT/roster,artifact/'fm_matched_squads.json')
            local_sources={'team_results':ROOT/'data/pl/asof_team_stats.csv',
               'player_inputs':ROOT/'data/pl/player_fm_join.csv',
               'official_candidate_roster':ROOT/'data/pl/official_candidate_roster.json',
               'player_input_provenance':ROOT/'data/pl/player_input_provenance.json',
               'official_roster_match':ROOT/roster,'availability':ROOT/opt.availability,
               'baseline':ROOT/benchmark,'raw_fm26':Path(opt.fm_file)}
            hashes={key:digest(path) for key,path in local_sources.items()}
            manifest={'as_of':str(cutoff),'source_contract_version':2,'fixtures':config['fixtures'],
                'paths_per_fixture':opt.paths,'seed':opt.seed,
                'engine':'footy-match::simulate_second_resolved_dynamic_mental_match',
                'model_code_sha256':model_fingerprint,
                'model_code_files':list(model_code_files()),
                'source_sha256':hashes,'availability_source':availability.get('source'),
                'git_revision':subprocess.run(['git','rev-parse','HEAD'],cwd=ROOT,text=True,capture_output=True,check=True).stdout.strip(),
                'raw_data_in_git':False,
                'limitations':'No confirmed starting XIs or contemporaneous per-player event tracking; FM-derived scenario priors'}
            (artifact/'run_manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    elif not opt.dry_run and not (ROOT/source).is_file():
        ap.error(f'no committed simulation artifact at {source}')
    run([python,script/'validate_pl_snapshot.py','--source',source,'--audit',snapshot/'fm_matched_squads.json'])
    run([python,script/'render_pl_timeline.py','--source',source,'--out-dir',images])
    report=opt.report or f'docs/premier-league-player-forecasts-{stamp}.md'
    run([python,script/'write_pl_timeline_report.py','--source',source,'--derived',images/'momentum_15min.json',
         '--output',report])
    if opt.dry_run or (not opt.dry_run and (ROOT/source).is_file()):
        # 1-minute player-level events are native to the upgraded Rust engine.
        # Frozen 2026-10-10 72-path snapshots predate this addition; never
        # interpolate missing minute/assists from the six older 15-minute bins.
        minute_ready=not opt.render_only
        if opt.render_only and not opt.dry_run:
            minute_ready='minutes_1min' in json.loads((ROOT/source).read_text())['fixtures'][0]
        if minute_ready:
            run([python,script/'render_pl_player_minute.py','--source',source,
                 '--out-dir',images/'minute-level'])
        elif opt.render_only:
            print('SKIP player minute charts: source snapshot predates native 90-minute output (no interpolation)')
    if opt.historical_event_oos:
        # This is a distinct past-season event-rate benchmark, never fit the
        # future FM26 player model on or label it as FM26 calibration.
        run([python,script/'backtest_pl_event_oos.py','--holdout-matches','24'])
        run([python,script/'write_pl_oos_report.py'])
        run([python,script/'calibrate_assist_prior_oos.py','--holdout-matches','24'])
    if opt.historical_same_engine_oos:
        # Observed-stat-only same-engine walk-forward backtest of the Rust match engine on the
        # cached StatsBomb 2015/16 subset. Local raw data required; the report records the engine
        # fingerprint so it can never be confused with a different model family.
        run([python,script/'build_pl_hist_inputs.py'])
        run([str(ROOT/'target/release/footy-matchforecast-pl-hist'),
             '--paths',str(opt.historical_paths),'--jobs',str(opt.historical_jobs),
             '--output','artifacts/validation/pl-hist-same-engine-paths.json'])
        run([python,script/'score_pl_hist_backtest.py'])
    if not opt.render_only:
        release=[python,script/'check_forecast_release.py',
                 '--snapshot',source,'--output',snapshot/'release_gates.json']
        if opt.matching_engine_oos:
            release+=['--matching-engine-oos',opt.matching_engine_oos]
        run(release)
    if opt.polymarket_spec:
        market_output=opt.polymarket_output or f'data/pl/polymarket_reference_{cutoff}.json'
        market_cmd=[python,script/'polymarket_prematch.py','--spec',opt.polymarket_spec,
                    '--snapshot',source,'--output',market_output]
        if opt.polymarket_histories: market_cmd+=['--histories',opt.polymarket_histories]
        run(market_cmd)
    print(f'SNAPSHOT {source} | IMAGES {images} | REPORT {report}')

if __name__=='__main__':main()
