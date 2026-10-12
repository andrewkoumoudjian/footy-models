#!/usr/bin/env python3
"""Safely repair the EPL roster with native FM and verified Transfermarkt IDs.

The identity proof is an **observed prior Transfermarkt person record**,
matching the native FM UID's actual name and DOB, and the official league
player's given/family name. Current club match is required before promotion;
loaned-out registrations do not count as active club candidates. The joined
data source can contain bad FM/TM joins: mismatched names/DOB are quarantined.
The September 2026 official roster still has no native person-ID or DOB; mark
all promoted official-to-FM links as strongly supported, not absolute.
"""
from __future__ import annotations

import argparse
import csv
import json
from collections import defaultdict, Counter
from pathlib import Path

from match_premier_league_fm26 import TEAM_ALIASES, norm
from reconcile_epl_fm_sources import match_quality, FM

ROOT = Path(__file__).resolve().parents[1]


def normalized_club(club):
    words = norm(club).split()
    return ' '.join(w for w in words if w not in {'fc','afc','football','club','the'})


def club_matches(official, tm):
    a,b = normalized_club(official),normalized_club(tm)
    if a == b: return True
    equivalents = {
        'brighton hove albion':'brighton',
        'man city':'manchester city',
        'man united':'manchester united',
        'nott m forest':'nottingham forest',
        'spurs':'tottenham hotspur',
        'newcastle':'newcastle united',
        'hull':'hull city',
        'ipswich':'ipswich town',
    }
    return equivalents.get(a,a) == equivalents.get(b,b)


def name_consistent(first, second):
    """Demand common names; never accept matched DOB for different people."""
    a,b=norm(first).split(),norm(second).split()
    if not a or not b: return False
    initial=a[0] == b[0] or (len(a[0]) >= 4 and len(b[0]) >= 4
                             and a[0][:4] == b[0][:4])
    return bool(initial and set(a[1:]).intersection(b[1:]))


def transfermarkt_records(path, candidate_uids, *, reep_active=None, tm_players_path=None):
    records=defaultdict(list)
    with path.open(encoding='utf-8-sig',newline='') as handle:
        for row in csv.DictReader(handle):
            if row.get('uid') not in candidate_uids: continue
            row['identity_bridge_source']='previous_FM_TM_dated_name_DOB_join'
            records[row['uid']].append(row)
    if reep_active is not None and tm_players_path is not None:
        import duckdb
        active=json.loads(reep_active.read_text())
        with duckdb.connect(active['path'], read_only=True) as db:
            edges=db.execute('''
                SELECT DISTINCT fm.external_id, tm.external_id, fm.reep_id
                FROM bridges fm
                JOIN bridges tm ON fm.reep_id=tm.reep_id
                WHERE fm.provider='fm' AND fm.namespace='player'
                  AND tm.provider='transfermarkt' AND tm.namespace='spieler'
            ''').fetchall()
        by_tm=defaultdict(list)
        for uid,tm_id,rid in edges:
            if uid in candidate_uids:
                by_tm[str(tm_id)].append((str(uid),str(rid)))
        with tm_players_path.open(encoding='utf-8-sig',newline='') as handle:
            for row in csv.DictReader(handle):
                tid=row.get('player_id')
                if not tid or tid not in by_tm:continue
                for uid,rid in by_tm[tid]:
                    records[uid].append({
                        'uid':uid, 'player_id':tid,'name_tm':row.get('name',''),
                        'name':row.get('name',''),
                        'date_of_birth':(row.get('date_of_birth') or '')[:10],
                        'current_club_name':row.get('current_club_name') or '',
                        'latest_val_date':'', 'reep_id':rid,
                        'identity_bridge_source':'Reep_FM_to_Transfermarkt_typed_provider_ID',
                    })
    return records


def select_evidence(official, club, candidates, tm_rows):
    options=[]
    for c in candidates:
        if c.get('native_kind') != 'player' or not c.get('uid') or not c.get('native_dob'):
            continue
        if match_quality(official,c.get('fm_name') or '') is None:
            continue
        for tm in tm_rows.get(c['uid'],[]):
            if (tm.get('date_of_birth') != c['native_dob']
                or not name_consistent(c['fm_name'],tm.get('name_tm',''))
                or not name_consistent(tm.get('name_tm',''),tm.get('name',''))
                or not club_matches(TEAM_ALIASES[club],tm.get('current_club_name',''))):
                continue
            if tm.get('latest_val_date','') and tm['latest_val_date'] > '2026-10-11':
                continue
            options.append({
                'uid':c['uid'], 'fm_name':c['fm_name'],
                'fm_dob':c['native_dob'], 'kind':c['native_kind'],
                'attr_source':c.get('native_attr_source'),
                'transfermarkt_player_id':tm.get('player_id'),
                'tm_name':tm.get('name_tm'), 'tm_club':tm.get('current_club_name'),
                'tm_dob':tm.get('date_of_birth'), 'prior_tm_latest_value_date':tm.get('latest_val_date'),
                'identity_bridge_source':tm.get('identity_bridge_source'),
                'reep_id':tm.get('reep_id'),
            })
    ids={o['uid'] for o in options}
    if len(ids)==1 and len({o['transfermarkt_player_id'] for o in options})==1:
        return options[0], options
    return None, options


def read_native(path,uids):
    data={}
    with path.open(encoding='utf-8-sig',newline='') as handle:
        for row in csv.DictReader(handle):
            if row['uid'] in uids:
                if row['uid'] in data:
                    raise ValueError('duplicated native UID: '+row['uid'])
                data[row['uid']]=row
    return data


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--roster',type=Path,default=ROOT/'data/identity/reep/epl_full_reep_first_v3.json')
    ap.add_argument('--audit',type=Path,default=ROOT/'data/identity/reep/epl_unmatched_all_sources_v1.json')
    ap.add_argument('--tm-join',type=Path,default=ROOT/'data/fm_tm_joined.csv')
    ap.add_argument('--native',type=Path,default=FM/'fm26_men_full_attrs.csv')
    ap.add_argument('--reep-active',type=Path,default=ROOT/'data/identity/reep/active.json')
    ap.add_argument('--tm-players',type=Path,default=ROOT/'data/tm_players.csv')
    ap.add_argument('--output',type=Path,required=True)
    ap.add_argument('--review',type=Path,required=True)
    args=ap.parse_args()
    if args.output.exists() or args.review.exists():ap.error('refuse overwrite existing evidence')
    roster=json.loads(args.roster.read_text());audit=json.loads(args.audit.read_text())
    if len(roster['teams'])!=20:raise ValueError('full league required')
    requested={(p['club'],p['official_name']):p for p in audit['players']}
    wanted={e['uid'] for p in audit['players'] for e in p['evidence'] if e.get('uid')}
    tm=transfermarkt_records(args.tm_join,wanted,reep_active=args.reep_active,
                            tm_players_path=args.tm_players)
    decisions={}
    counts=Counter()
    for (club,official),item in requested.items():
        selected,evidence=select_evidence(official,club,item['evidence'],tm)
        decisions[(club,official)]={'accepted':selected,'transfermarkt_candidates':evidence}
        if selected: counts['name_dob_tm_club_matches']+=1
    owners=defaultdict(list)
    for key,decision in decisions.items():
        if decision['accepted']:
            owners[decision['accepted']['uid']].append(key)
    existing_owners=defaultdict(set)
    for club,block in roster['teams'].items():
        for player in block['players']:
            if player.get('fm_uid'):
                existing_owners[str(player['fm_uid'])].add((club,player['official_name']))
    valid={}
    for uid,locations in owners.items():
        candidates=[loc for loc in locations if not any(p['official_name']==loc[1]
                     and (p.get('loaned_out') or '(loan)' in p['official_name'].lower())
                     for p in roster['teams'][loc[0]]['players'])]
        # Never assign a native person to 2 active EPL registrations.
        if len(candidates)!=1 or existing_owners[uid]:
            counts['cross_club_or_loan_or_existing_collision_quarantined']+=len(locations)
            continue
        valid[candidates[0]]=uid
    native=read_native(args.native,set(valid.values()))
    added=[]
    for club,block in roster['teams'].items():
        for player in block['players']:
            key=(club,player['official_name'])
            uid=valid.get(key)
            if uid is None:continue
            observed=native.get(uid)
            if not observed or observed.get('kind')!='player' or observed.get('dob')!=decisions[key]['accepted']['fm_dob']:
                counts['missing_or_inconsistent_native_record']+=1
                continue
            approved=observed.get('attr_source')=='db'
            attrs={key[5:]:float(value) for key,value in observed.items() if key.startswith('attr_')
                   and value and value.replace('.','',1).isdigit()} if approved else {}
            player.update({
                'fm_uid':uid,'fm_name':observed['name'],'fm_kind':'player',
                'attr_source':observed['attr_source'],
                'fm_player_record_verified':True,
                'fm_attributes_verified':approved,
                'ca':float(observed['ca']) if approved and observed.get('ca') else None,
                'pa':float(observed['pa']) if approved and observed.get('pa') else None,
                'attributes':attrs,
                'match_method':'multi_source_FM_UID_Transfermarkt_name_DOB_club',
                'identity_verification':'official_name_FM_TM_same_DOB_asof_club_provisional',
                'source_quality':'native_db_attributes_and_TM_DOB_club' if approved else 'reconstructed_or_missing_attributes',
                'transfermarkt_evidence':decisions[key]['accepted'],
            })
            counts['promoted']+=1
            added.append({'club':club,'official_name':player['official_name'],'fm_uid':uid,
                          'tm_player_id':decisions[key]['accepted']['transfermarkt_player_id'],
                          'fm_attr_source':observed['attr_source']})
    for club,team in roster['teams'].items():
        rows=team['players']
        team['matched_count']=sum(bool(p.get('fm_uid')) for p in rows)
        team['matched_25']=sum(bool(p.get('fm_uid')) and p['registration']=='registered' for p in rows)
        team['matched_u21']=sum(bool(p.get('fm_uid')) and p['registration']=='u21' for p in rows)
        team['unmatched']=[p['official_name'] for p in rows if not p.get('fm_uid')]
    report={'schema':'epl_tm_identity_corroboration_v1','decisions':[
        {'club':club,'official_name':name,**decision} for (club,name),decision in decisions.items()],
        'promoted':added,'counts':dict(counts),
        'remaining_unmatched':sum(len(t['unmatched']) for t in roster['teams'].values()),
        'limitations':'TM join is historical, not real-time official asof team confirmation; roster-to-provider link remains provisional'}
    args.review.parent.mkdir(parents=True,exist_ok=True)
    args.review.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    roster['cross_source_identity_recovery']=dict(counts)
    roster['cross_source_identity_evidence']=str(args.review)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(roster,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'promoted':added,'counts':dict(counts),
                      'matched':sum(v['matched_count'] for v in roster['teams'].values()),
                      'unmatched':report['remaining_unmatched']},indent=2))

if __name__=='__main__':main()
