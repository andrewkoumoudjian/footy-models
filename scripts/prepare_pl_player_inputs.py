#!/usr/bin/env python3
"""Prepare dated Premier League match and FM26 player inputs for the existing matchforecast engine.

Player identities and clubs are from the official registered squads; the FM
attribute prior comes from UID-matched data. Match-level player event rates and
participation weights remain explicitly imputed due absent 2026/27 PL player
appearance/event feed. Never present those inputs as observed player statistics.
"""
import argparse
import csv
import hashlib
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

def observed_number(row,key):
    """Return a measured field or None; malformed measurements are never zero."""
    value=row.get(key)
    if value is None or str(value).strip()=='':return None
    try:
        number=float(value)
        if not (0<=number<float('inf')):raise ValueError('nonfinite or negative')
        return number
    except (ValueError,TypeError) as error:
        raise ValueError(f'invalid source measurement {key}={value!r}') from error

def position_group(raw):
    pos=(raw or '').strip().casefold()
    if any(p in pos for p in ('goalkeeper','keeper','goalie')):return 'GK'
    if any(p in pos for p in ('back','defender','defence','defense')):return 'DEF'
    if any(p in pos for p in ('midfield','wing-back')):return 'MID'
    if any(p in pos for p in ('striker','forward','winger','attacker','attack')):return 'FWD'
    return 'UNKNOWN'

FM_SI_POSITION_GROUP={'g':'GK','gk':'GK','dm':'MID','wb':'DEF','wbr':'DEF','wbl':'DEF',
                      'st':'FWD','f':'FWD'}
def fm_si_position_group(raw):
    """Sortitoutsi FM26 Position strings are FM role codes: 'GK', 'D C, DM', 'AM/F'."""
    for component in (raw or '').split(','):
        token=component.strip().casefold().split(' ')[0]
        if not token:continue
        if token in FM_SI_POSITION_GROUP:return FM_SI_POSITION_GROUP[token]
        if token[0]=='g':return 'GK'
        if token[0]=='d':return 'DEF'
        if token[0]=='m' or token[0]=='a':return 'MID'
        if token[0]=='s':return 'FWD'
    return 'UNKNOWN'

TM_POSITION_GROUP={'goalkeeper':'GK','defender':'DEF','midfielder':'MID','attack':'FWD'}

def documented_position(p):
    """First available dated provider position evidence; never invented.

    Priority: registered historical position, then the Sortitoutsi FM26 roster
    role code joined through the verified SI->binary UID mapping, then the
    Transfermarkt player position of the identity-corroborated TM row.
    Returns (group, source_name); ('UNKNOWN','unknown') is honest missingness.
    """
    group=position_group(p.get('historical_position',''))
    if group!='UNKNOWN':return group,'historical_2025_26'
    group=fm_si_position_group(p.get('sortitoutsi_position',''))
    if group!='UNKNOWN':return group,'sortitoutsi_fm26_roster'
    group=TM_POSITION_GROUP.get((p.get('transfermarkt_position') or '').strip().casefold(),'UNKNOWN')
    if group!='UNKNOWN':return group,'transfermarkt_players'
    return 'UNKNOWN','unknown'

def official_candidate_id(team, official_name):
    """Stable provisional identity within a club/season, NOT a person join."""
    season=CUTOFF.year if CUTOFF.month>=7 else CUTOFF.year-1
    key=f'pl-roster:{season}:{team}:{official_name.casefold()}'.encode('utf-8')
    return str(int.from_bytes(hashlib.sha256(key).digest()[:8],'big') | (1<<63))

def write_results():
    rows={}
    for path in sorted((OUT/'raw').glob('*_E0.csv')):
        with path.open(encoding='utf-8-sig') as stream:
            for row in csv.DictReader(stream):
                try:d=datetime.strptime(row['Date'],'%d/%m/%Y').date()
                except (KeyError,ValueError):continue
                # Match dates have no verified result-publication clock; require an
                # earlier calendar day to avoid same-day pre-kickoff leakage.
                if d>=CUTOFF or row.get('FTHG') in ('',None) or row.get('FTAG') in ('',None):continue
                home,away=canon(row['HomeTeam']),canon(row['AwayTeam'])
                rows[(d,home,away)]={**row,'_provider':'football-data.co.uk'}
    for r in CONFIG.get('manual_results',[]):
        d=date.fromisoformat(r['date'])
        if d>=CUTOFF:continue
        key=(d,canon(r['home']),canon(r['away']))
        if key in rows:
            same=fl(rows[key].get('FTHG'))==r['home_goals'] and fl(rows[key].get('FTAG'))==r['away_goals']
            if not same:raise ValueError(f'manual result contradicts source {key}')
        else:rows[key]={'FTHG':str(r['home_goals']),'FTAG':str(r['away_goals']),
                        '_provider':'dated_manual_results'}
    path=OUT/'asof_team_stats.csv'
    fields=['date','home','away','home_goals','away_goals','home_xg','away_xg','home_shots',
            'away_shots','home_sot','away_sot','source_provider','source_match_date',
            'source_published_at','home_xg_kind','away_xg_kind','home_shots_kind',
            'away_shots_kind','home_sot_kind','away_sot_kind']
    with path.open('w',newline='') as out:
        writer=csv.DictWriter(out,fieldnames=fields);writer.writeheader()
        for (d,h,a),r in sorted(rows.items()):
            g1,g2=observed_number(r,'FTHG'),observed_number(r,'FTAG')
            if g1 is None or g2 is None:raise ValueError(f'missing scored result for {d,h,a}')
            measurements={key:observed_number(r,source) for key,source in (
                ('home_xg','HxG'),('away_xg','AxG'),('home_shots','HS'),
                ('away_shots','AS'),('home_sot','HST'),('away_sot','AST'))}
            # Empty means genuinely unobserved. Previously goals were silently
            # relabeled xG and generic match shot totals were invented.
            writer.writerow({'date':d.isoformat(),'home':h,'away':a,
                'home_goals':g1,'away_goals':g2,
                **{k: v if v is not None else '' for k,v in measurements.items()},
                'source_provider':r.get('_provider','unknown'),
                'source_match_date':d.isoformat(),
                'source_published_at':'', # cannot substantiate exact provider publication time
                **{f'{k}_kind':'measured' if v is not None else 'missing'
                   for k,v in measurements.items()}})
    return len(rows)

def prepare_players(observed_priors=None):
    fields=['player_id','name','team','minutes','first_listed_date','last_listed_date',
            'last_appearance_date','selection_listings','selection_appearances','selection_starts',
            'selection_minutes','selection_start_minutes','team_fixtures','shots_p90','xg_p90','xa_p90',
            'passes_p90','prog_carries_p90','fm_ca','fm_finishing','fm_technique','fm_tackling',
            'fm_marking','fm_consistency','fm_dirtiness','fm_kind','fm_attr_source',
            'fm_attributes_verified','identity_status','registered_club','membership_observed_as_of',
            'registration_type','position_group','position_source','exposure_kind',
            'event_rate_kind','source_published_at','observed_espn_id','observed_appearances',
            'observed_minutes','observed_shots']
    statuses={};outrows=[];candidate_rows=[]
    observed_by_uid={}
    if observed_priors is not None:
        if observed_priors.get('schema')!='espn_observed_shot_priors_v1':
            raise ValueError('incompatible observed-shot-priors schema')
        if date.fromisoformat(observed_priors['observed_window_exclusive_end'])!=CUTOFF:
            raise ValueError('observed-shot priors window must end exactly at the prediction cutoff')
        for person in observed_priors['people']:
            if person['fm_uid'] is None:continue
            key=(person['club'],str(person['fm_uid']))
            if key in observed_by_uid:
                raise ValueError(f'duplicate observed prior for {key}')
            observed_by_uid[key]=person
    for team,squad in SQUADS['teams'].items():
        excluded=set(AVAILABILITY['excluded_by_club'].get(team,[]))
        for p in squad['players']:
            if p.get('fm_uid') and 'fm_attributes_verified' not in p:
                raise ValueError('stale roster has no FM kind/attribute provenance; rerun identity matching')
            loan=p.get('loaned_out',bool('(Loan)' in p['official_name']))
            unavailable=bool(p.get('fm_uid') in excluded)
            candidate_rows.append({
                'canonical_candidate_id':str(p.get('fm_uid') or official_candidate_id(team,p['official_name'])),
                'identity_namespace':'fm26_uid' if p.get('fm_uid') else 'official_roster_provisional',
                'official_name':p['official_name'],'club':team,'registration':p['registration'],
                'membership_observed_as_of':p.get('membership_observed_as_of',CUTOFF.isoformat()),
                'candidate_status':'loaned_out' if loan else 'confirmed_unavailable' if unavailable else 'eligible_unknown_selection',
                'match_method':p['match_method'],'fm_kind':p.get('fm_kind','unknown'),
                'attr_source':p.get('attr_source','unknown'),
                'fm_attributes_verified':p.get('fm_attributes_verified',False),
                'historical_position':p.get('historical_position',''),
                'sortitoutsi_position':p.get('sortitoutsi_position',''),
                'transfermarkt_position':p.get('transfermarkt_position',''),
                'historical_position_group':documented_position(p)[0],
                'position_source':documented_position(p)[1],
                'start_observations':None,'minutes_observations':None,
                'source_published_at':None})
        matched=[p for p in squad['players'] if p.get('fm_uid')
                 and '(Loan)' not in p['official_name'] and p['fm_uid'] not in excluded]
        assert len({p['fm_uid'] for p in matched})==len(matched)
        # More recently registered senior players get an expected involvement
        # advantage, still NOT observed starts or injury availability.
        for p in matched:
            verified=p.get('fm_attributes_verified') is True
            attrs=p.get('attributes',{}) if verified else {}
            ca=p.get('ca') if verified else None
            tm_minutes=fl(p.get('historical_minutes_2526'))
            actual_prior=tm_minutes>50
            # Legacy 'minutes' field is an exposure proxy for rate shrinkage and
            # line-up rank, *never* a report of observed 2026/27 participation.
            exposure=(min(2300,tm_minutes) if actual_prior else
                      max(100.,(ca-85.)*17.) if ca is not None else 500.)
            if p['registration']=='u21':exposure *= 0.46
            exposure=max(50.,exposure)
            finishing=fl(attrs.get('Finishing'),10)
            passing=fl(attrs.get('Passing'),10)
            vision=fl(attrs.get('Vision'),10)
            technique=fl(attrs.get('Technique'),10)
            decisions=fl(attrs.get('Decisions'),10)
            dribbling=fl(attrs.get('Dribbling'),10)
            creativity=(passing+vision+technique+decisions)/4
            attacking=(finishing+fl(attrs.get('OffTheBall'),10)+fl(attrs.get('FirstTouch'),10))/3
            role,position_source_name=documented_position(p)
            # Verified-FM goalkeeper attributes still document a keeper when
            # no provider position evidence exists; do not guess outfield roles.
            if role=='UNKNOWN' and verified and (
                    fl(attrs.get('Reflexes'))>=12 and fl(attrs.get('Handling'))>=11):
                role='GK';position_source_name='verified_fm_goalkeeper_attributes'
            if role=='GK':
                shots=0.015;xg=0.005;xa=0.005;passes=20.;carries=.02
            else:
                shots=max(.18,min(3.4,.75+.13*(attacking-10)+.045*(dribbling-10)))
                xg=max(.015,min(.65,shots*(.10+.008*(finishing-10))))
                xa=max(.018,min(.5,.09+.013*(creativity-10)))
                passes=max(15.,min(78.,30+2.1*(passing-10)))
                carries=max(.2,min(5.,1.1+.16*(dribbling-10)))
            # No selection data is available for the 2026/27 PL season. The
            # participation model must use fallback, not fabricated starts.
            observed=observed_by_uid.get((team,str(p['fm_uid']))) if p.get('fm_uid') else None
            outrows.append({'player_id':p['fm_uid'],
                'name':p.get('fm_name') or p['official_name'],'team':team,
                'minutes':round(exposure,3),'first_listed_date':'',
                'last_listed_date':CUTOFF.isoformat(),'last_appearance_date':'',
                'selection_listings':0,'selection_appearances':0,'selection_starts':0,
                'selection_minutes':0,'selection_start_minutes':0,'team_fixtures':0,
                'shots_p90':round(observed['gamma_poisson_shots_per90'],4) if observed else round(shots,4),
                'xg_p90':round(xg,4),'xa_p90':round(xa,4),
                'passes_p90':round(passes,4),'prog_carries_p90':round(carries,4),
                'fm_ca':ca if ca is not None else '',
                'fm_finishing':finishing if verified else '',
                'fm_technique':technique if verified else '',
                'fm_marking':fl(attrs.get('Marking'),10) if verified else '',
                'fm_tackling':fl(attrs.get('Tackling'),10) if verified else '',
                'fm_consistency':fl(attrs.get('Consistency'),10) if verified else '',
                'fm_dirtiness':fl(attrs.get('Dirtiness'),10) if verified else '',
                'fm_kind':p.get('fm_kind','unknown'),'fm_attr_source':p.get('attr_source','unknown'),
                'fm_attributes_verified':str(verified).lower(),
                'identity_status':'fm_player_record' if p.get('fm_player_record_verified') else 'unverified_record_kind',
                'registered_club':team,'membership_observed_as_of':p.get('membership_observed_as_of',CUTOFF.isoformat()),
                'registration_type':p['registration'],'position_group':role,
                'position_source':position_source_name,
                'exposure_kind':'prior_2025_26_minutes' if actual_prior else 'fm_heuristic' if verified else 'population_prior',
                'event_rate_kind':('espn_observed_gamma_poisson_5_exposure_role_prior' if observed
                                   else 'fm_heuristic_not_observed' if verified else 'population_prior_not_observed'),
                'source_published_at':'',
                'observed_espn_id':observed['espn_id'] if observed else '',
                'observed_appearances':observed['observed_appearances'] if observed else '',
                'observed_minutes':observed['observed_minutes'] if observed else '',
                'observed_shots':observed['observed_shots'] if observed else ''})
        statuses[team]={'registered':squad['registered_count'],
            'fm_matched':len(matched),
            'matched_senior':sum(p['registration']=='registered' for p in matched),
            'fm_unmatched':squad['unmatched'],
            'excluded_registered_loans':[p['official_name'] for p in squad['players'] if '(Loan)' in p['official_name']],
            'excluded_known_injured_or_suspended':[p['official_name'] for p in squad['players'] if p.get('fm_uid') in excluded],
            'historical_minutes_priors':sum(fl(p.get('historical_minutes_2526'))>50 for p in matched)}
    with (OUT/'player_fm_join.csv').open('w',newline='') as out:
        w=csv.DictWriter(out,fieldnames=fields);w.writeheader();w.writerows(outrows)
    # Full official candidate universe is staged independently of the existing
    # 18-player simulator adapter; unmatched identities must not disappear, but
    # they must not be silently promoted to FM-derived rates either.
    (OUT/'official_candidate_roster.json').write_text(json.dumps({
        'schema_version':1,'as_of':CUTOFF.isoformat(),
        'registration_source':SQUADS['registration_source'],
        'usage':'candidate universe for a future calibrated selection model, not inferred lineups',
        'players':candidate_rows},indent=2,ensure_ascii=False)+'\n')
    (OUT/'player_input_provenance.json').write_text(json.dumps({
        'snapshot_as_of':CUTOFF.isoformat(),'source':SQUADS['registration_source'],
        'player_rate_inputs':('mix of ESPN observed Gamma-Poisson shot-rate posteriors and FM attribute-derived heuristics'
                              if observed_priors else 'FM attribute-derived heuristics; NOT fitted on actual PL per-player events'),
        'observed_shot_priors':({
            'source':observed_priors['espn_archive']['path'],
            'source_sha256':observed_priors['espn_archive']['sha256'],
            'window_exclusive_end':observed_priors['observed_window_exclusive_end'],
            'people':observed_priors['people_count'],
            'reconciliation':observed_priors['identity_reconciliation_counts']} if observed_priors
            else 'not supplied; all player event rates remain unobserved heuristics'),
        'unverified_fm_records':'kept as candidates without FM ratings; population-prior event/exposure values carry high epistemic uncertainty',
        'missing_team_measurements':'blank numeric observations and explicit measured/missing flags; never substitute scored goals as observed xG',
        'full_candidate_roster':'official_candidate_roster.json includes unmatched names with provisional IDs; no FM attrs or observed selections imputed',
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
    parser.add_argument('--observed-shot-priors',help='dated ESPN observed shot-rate priors JSON '
                        '(window must end exactly at the fixture as-of)')
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
    if AVAILABILITY.get('source_published','')[:10] > CUTOFF.isoformat():
        raise SystemExit('Availability source publication time follows forecast cutoff')
    OUT.mkdir(parents=True,exist_ok=True)
    observed_priors=None
    if args.observed_shot_priors:
        observed_priors=json.loads((ROOT/args.observed_shot_priors).read_text())
    matches=write_results();players,s=prepare_players(observed_priors)
    print('matches',matches,'FM player rows',players)
    if observed_priors:
        print('observed shot priors applied:',observed_priors['people_count'],'people from',
              observed_priors['espn_archive']['path'],'window ends',observed_priors['observed_window_exclusive_end'])
    for team,x in s.items():print(team,x['fm_matched'],x['matched_senior'],'unmatched',len(x['fm_unmatched']))
