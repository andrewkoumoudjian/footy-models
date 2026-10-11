#!/usr/bin/env python3
"""Prepare dated Premier League match and FM26 player inputs for the existing matchforecast engine.

Player identities and clubs are from the official registered squads; the FM
attribute prior comes from UID-matched data. Match-level player event rates and
participation weights remain explicitly imputed due absent 2026/27 PL player
appearance/event feed. Never present those inputs as observed player statistics.
"""
import argparse
import csv
import json
from datetime import date,datetime
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'data/pl'
CONFIG_PATH=ROOT/'configs/pl_2026-10-11.json'
CONFIG=json.loads(CONFIG_PATH.read_text())
CUTOFF=date.fromisoformat(CONFIG['as_of'])
SQUADS=None  # loaded only after CLI fixture date and paths are validated
AVAILABILITY=None  # never silently reuse an old injury report
ALIASES={'Nottingham Forest':"Nott'm Forest",'Hull City':'Hull','Newcastle United':'Newcastle',
         'Manchester City':'Man City','Manchester United':'Man United','Liverpool FC':'Liverpool'}

def canon(s):return ALIASES.get(s,s)
def fl(x,default=0.0):
 try:return float(x) if x not in ('',None) else default
 except (ValueError,TypeError):return default

def write_results():
    rows={}
    for path in sorted((OUT/'raw').glob('*_E0.csv')):
        for row in csv.DictReader(path.open(encoding='utf-8-sig')):
            try:d=datetime.strptime(row['Date'],'%d/%m/%Y').date()
            except (KeyError,ValueError):continue
            if d>CUTOFF or not row.get('FTHG') or not row.get('FTAG'):continue
            home,away=canon(row['HomeTeam']),canon(row['AwayTeam'])
            rows[(d,home,away)]=dict(row)
    for r in CONFIG.get('manual_results',[]):
        d=date.fromisoformat(r['date'])
        if d>CUTOFF:continue
        key=(d,canon(r['home']),canon(r['away']))
        if key in rows:
            same=fl(rows[key].get('FTHG'))==r['home_goals'] and fl(rows[key].get('FTAG'))==r['away_goals']
            if not same:raise ValueError(f'manual result contradicts source {key}')
        else:rows[key]={'FTHG':str(r['home_goals']),'FTAG':str(r['away_goals'])}
    path=OUT/'asof_team_stats.csv'
    fields=['date','home','away','home_goals','away_goals','home_xg','away_xg','home_shots',
            'away_shots','home_sot','away_sot']
    with path.open('w',newline='') as out:
        writer=csv.DictWriter(out,fieldnames=fields);writer.writeheader()
        for (d,h,a),r in sorted(rows.items()):
            g1,g2=fl(r['FTHG']),fl(r['FTAG'])
            # For seasons lacking real xG, the conceded-goals statistic is an
            # outcome proxy, not a falsely measured expected-goals observation.
            writer.writerow({'date':d.isoformat(),'home':h,'away':a,
                'home_goals':g1,'away_goals':g2,'home_xg':fl(r.get('HxG'),g1),
                'away_xg':fl(r.get('AxG'),g2),'home_shots':fl(r.get('HS'),12),
                'away_shots':fl(r.get('AS'),11),'home_sot':fl(r.get('HST'),4),
                'away_sot':fl(r.get('AST'),4)})
    return len(rows)

def prepare_players():
    fields=['player_id','name','team','minutes','first_listed_date','last_listed_date',
            'last_appearance_date','selection_listings','selection_appearances','selection_starts',
            'selection_minutes','selection_start_minutes','team_fixtures','shots_p90','xg_p90','xa_p90',
            'passes_p90','prog_carries_p90','fm_ca','fm_finishing','fm_technique','fm_tackling',
            'fm_marking','fm_consistency','fm_dirtiness']
    statuses={};outrows=[]
    for team,squad in SQUADS['teams'].items():
        excluded=set(AVAILABILITY['excluded_by_club'].get(team,[]))
        matched=[p for p in squad['players'] if p.get('fm_uid')
                 and '(Loan)' not in p['official_name'] and p['fm_uid'] not in excluded]
        assert len({p['fm_uid'] for p in matched})==len(matched)
        # More recently registered senior players get an expected involvement
        # advantage, still NOT observed starts or injury availability.
        for p in matched:
            attrs=p['attributes'];ca=p['ca'];tm_minutes=fl(p.get('historical_minutes_2526'))
            actual_prior=tm_minutes>50
            # Legacy 'minutes' field is an exposure proxy for rate shrinkage and
            # line-up rank, *never* a report of observed 2026/27 participation.
            exposure=(min(2300,tm_minutes) if actual_prior else max(100.,(ca-85.)*17.))
            if p['registration']=='u21':exposure *= 0.46
            exposure=max(50.,exposure)
            pos=p.get('historical_position','')
            finishing=fl(attrs.get('Finishing'),10)
            passing=fl(attrs.get('Passing'),10)
            vision=fl(attrs.get('Vision'),10)
            technique=fl(attrs.get('Technique'),10)
            decisions=fl(attrs.get('Decisions'),10)
            dribbling=fl(attrs.get('Dribbling'),10)
            creativity=(passing+vision+technique+decisions)/4
            attacking=(finishing+fl(attrs.get('OffTheBall'),10)+fl(attrs.get('FirstTouch'),10))/3
            if pos=='Goalkeeper' or (fl(attrs.get('Reflexes'))>=12 and fl(attrs.get('Handling'))>=11):
                role='GK';shots=0.015;xg=0.005;xa=0.005;passes=20.;carries=.02
            else:
                role='outfield';shots=max(.18,min(3.4,.75+.13*(attacking-10)+.045*(dribbling-10)))
                xg=max(.015,min(.65,shots*(.10+.008*(finishing-10))))
                xa=max(.018,min(.5,.09+.013*(creativity-10)))
                passes=max(15.,min(78.,30+2.1*(passing-10)))
                carries=max(.2,min(5.,1.1+.16*(dribbling-10)))
            # No selection data is available for the 2026/27 PL season. The
            # participation model must use fallback, not fabricated starts.
            outrows.append({'player_id':p['fm_uid'],'name':p['fm_name'],'team':team,
                'minutes':round(exposure,3),'first_listed_date':'',
                'last_listed_date':CUTOFF.isoformat(),'last_appearance_date':'',
                'selection_listings':0,'selection_appearances':0,'selection_starts':0,
                'selection_minutes':0,'selection_start_minutes':0,'team_fixtures':0,
                'shots_p90':round(shots,4),'xg_p90':round(xg,4),'xa_p90':round(xa,4),
                'passes_p90':round(passes,4),'prog_carries_p90':round(carries,4),
                'fm_ca':ca,'fm_finishing':finishing,'fm_technique':technique,
                'fm_marking':fl(attrs.get('Marking'),10),'fm_tackling':fl(attrs.get('Tackling'),10),
                'fm_consistency':fl(attrs.get('Consistency'),10),'fm_dirtiness':fl(attrs.get('Dirtiness'),10)})
        statuses[team]={'registered':squad['registered_count'],
            'fm_matched':len(matched),
            'matched_senior':sum(p['registration']=='registered' for p in matched),
            'fm_unmatched':squad['unmatched'],
            'excluded_registered_loans':[p['official_name'] for p in squad['players'] if '(Loan)' in p['official_name']],
            'excluded_known_injured_or_suspended':[p['official_name'] for p in squad['players'] if p.get('fm_uid') in excluded],
            'historical_minutes_priors':sum(fl(p.get('historical_minutes_2526'))>50 for p in matched)}
    with (OUT/'player_fm_join.csv').open('w',newline='') as out:
        w=csv.DictWriter(out,fieldnames=fields);w.writeheader();w.writerows(outrows)
    (OUT/'player_input_provenance.json').write_text(json.dumps({
        'snapshot_as_of':CUTOFF.isoformat(),'source':SQUADS['registration_source'],
        'player_rate_inputs':'FM attribute-derived heuristics; NOT fitted on actual PL per-player events',
        'participation_inputs':'CA-ranked fallback plus available pre-season 2025/26 historical minute exposure; no observed current appearances',
        'squad_availability':'registered squad and specified dated availability exclusions; doubtful lineups remain uncertain',
        'availability_source':AVAILABILITY['source'],'availability_source_published':AVAILABILITY['source_published'],
        'teams':statuses},indent=2,ensure_ascii=False)+'\n')
    return len(outrows),statuses

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures',default='configs/pl_2026-10-11.json')
    parser.add_argument('--squads',help='UID-matched official roster JSON')
    parser.add_argument('--availability',help='dated availability exclusions JSON; required to avoid implicitly reusing old team news')
    parser.add_argument('--data-dir',default='data/pl')
    args=parser.parse_args()
    OUT=ROOT/args.data_dir
    CONFIG=json.loads((ROOT/args.fixtures).read_text())
    CUTOFF=date.fromisoformat(CONFIG['as_of'])
    if not args.availability:
        raise SystemExit('Refusing to reuse stale availability. Pass --availability dated JSON explicitly.')
    AVAILABILITY=json.loads((ROOT/args.availability).read_text())
    if date.fromisoformat(AVAILABILITY['as_of'])!=CUTOFF:
        raise SystemExit('Availability as-of must match fixture as-of; refresh it for a new date')
    SQUADS=json.loads((ROOT/(args.squads or f'data/pl/squads_{CUTOFF}.json')).read_text())
    if date.fromisoformat(SQUADS['as_of'])!=CUTOFF:
        raise SystemExit('Squad mapping as-of does not match fixture as-of')
    OUT.mkdir(parents=True,exist_ok=True)
    matches=write_results();players,s=prepare_players();print('matches',matches,'FM player rows',players)
    for team,x in s.items():print(team,x['fm_matched'],x['matched_senior'],'unmatched',len(x['fm_unmatched']))
