#!/usr/bin/env python3
"""Exhaustive FM26 search for official EPL registrations across all local exports.

Scans unfiltered FM26 identity rows, native full attributes, Sortitoutsi roster
and verifies existing UID assignments. Retains *candidate evidence* separately
from accepted identity. No CA/PA or youth filter, and never treats staff with
invented ratings as players.

This audit cannot prove a missing 2026/27 player is present in March 2026 FM.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import unicodedata

ROOT=Path(__file__).resolve().parents[3]
FULL=Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')
ORIGINAL=Path('/home/drewclaw/fm26-data/fm26_men.csv')
SI=Path('/home/drewclaw/fm26-data/sortitoutsi_fm26_roster.csv')
SQUAD=ROOT/'data/pl/squads_2026-10-10.json'


def tokens(v):
    return re.findall(r'[a-z0-9]+',''.join(x for x in
           unicodedata.normalize('NFKD',str(v or '').casefold())
           if not unicodedata.combining(x)))


def legal_parts(name):
    value=str(name).replace('*','')
    if ',' not in value:
        t=tokens(value)
        return t[:1],t[1:]
    family,given=value.split(',',1)
    return tokens(given),tokens(family)


def raw_sources(files, given_tokens):
    """Store only rows with a relevant first-name token; all raw rows checked."""
    hits=defaultdict(list)
    id_catalog=defaultdict(dict)
    counts=Counter()
    for label,path in files.items():
        with path.open(newline='',encoding='utf-8-sig') as f:
            reader=csv.DictReader(f)
            for row in reader:
                counts[label]+=1
                uid=str(row.get('uid') or '').strip()
                name=row.get('name') or row.get('Name') or ''
                if not uid or not name:
                    continue
                tt=tokens(name)
                if not tt or tt[0] not in given_tokens:
                    continue
                record={'uid':uid,'fm_name':name,'source':label,
                        'kind':row.get('kind','unspecified'),
                        'attr_source':row.get('attr_source','unavailable'),
                        'dob':row.get('dob') or row.get('DOB') or '',
                        'ca':row.get('ca','')}
                hits[tt[0]].append((tt,record))
                if label=='full':
                    id_catalog[uid][label]=record
    return hits,counts,id_catalog


def classify(person, tt):
    given,family=legal_parts(person)
    if not given or not family or not tt:
        return None
    exact=given+family
    if tt==exact:
        return (0,'complete_legal_name_order')
    if sorted(tt)==sorted(exact):
        return (1,'complete_legal_name_tokens')
    if tt[0]!=given[0]:
        return None
    overlap=len(set(tt[1:]) & set(family))
    if overlap==0:
        return None
    if set(family).issubset(set(tt)):
        return (2,'first_name_and_all_family_tokens')
    if family[-1] in tt:
        return (3,'first_name_and_final_family_token')
    return (4,'partial_legal_family_name_only')


def audit(squad,files):
    roster=[{'club':team,'official_name':p['official_name'],
             'registration':p.get('registration'),
             'fm_uid':str(p.get('fm_uid') or ''),
             'fm_verification':p.get('fm_attributes_verified')}
            for team,v in squad['teams'].items() for p in v['players']]
    first={legal_parts(p['official_name'])[0][0] for p in roster
           if legal_parts(p['official_name'])[0]}
    index,scanned,_=raw_sources(files,first)
    counts=Counter()
    result=[]
    for p in roster:
        given,_=legal_parts(p['official_name'])
        proposals={}
        if given:
            for tt,record in index[given[0]]:
                classified=classify(p['official_name'],tt)
                if classified is None:
                    continue
                rank,why=classified
                uid=record['uid']
                current=proposals.get(uid)
                if current is None or (rank,record['source'])<(current['rank'],current['source']):
                    proposals[uid]={'uid':uid,'name':record['fm_name'],
                                    'rank':rank,'match_type':why,
                                    'source':record['source'],
                                    'kind':record['kind'],
                                    'attr_source':record['attr_source']}
        ranked=sorted(proposals.values(),key=lambda c:(c['rank'],c['uid']))
        old=p['fm_uid']
        if old:
            status='already_assigned_uid'
        elif not ranked:
            status='no_compatible_identity_in_any_examined_local_export'
        elif len([r for r in ranked if r['rank']==ranked[0]['rank']])>1:
            status='multiple_best_name_candidates_unverified'
        else:
            status='one_best_name_candidate_requires_identity_proof'
        counts[status]+=1
        if old:
            counts['assigned_uid_also_name_search_hit' if old in proposals else
                   'assigned_uid_not_found_by_name_search']+=1
        result.append({
            'club':p['club'],'official_name':p['official_name'],
            'registration':p['registration'],
            'existing_fm_uid':old or None,'audit_status':status,
            'candidate_count':len(ranked),'best_candidate_ids':[v['uid'] for v in
                ranked if ranked and v['rank']==ranked[0]['rank']][:20],
            'candidates':ranked[:5],
        })
    return {
        'schema':'fm26_exhaustive_all_exports_candidate_audit_v1',
        'created_at_utc':datetime.now(timezone.utc).isoformat(),
        'official_total':len(roster),
        'no_ca_cutoff':True,'includes_u21':True,
        'files_scanned':{key:str(value) for key,value in files.items()},
        'source_row_counts':dict(scanned),
        'status':dict(counts),
        'registry':[{'club':r['club'],'official_name':r['official_name'],
                     'existing_fm_uid':r['existing_fm_uid'],
                     'audit_status':r['audit_status'],
                     'candidate_count':r['candidate_count'],
                     'candidates':r['candidates']} for r in result],
        'cautions':[
            'Name-compatible entries are not accepted FM identities without independent verified crosswalk.',
            'A March 2026 edition may genuinely omit later 2026-27 signings and youth players.',
            'FM26 raw exported attributes can be reconstructed, staff-kind, or null.',
            'A duplicate raw UID for different people must be quarantined.',
        ],
    }


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--squads',type=Path,default=SQUAD)
    p.add_argument('--full',type=Path,default=FULL)
    p.add_argument('--original',type=Path,default=ORIGINAL)
    p.add_argument('--sortitoutsi',type=Path,default=SI)
    p.add_argument('--output',type=Path,required=True)
    args=p.parse_args()
    if args.output.exists():
        p.error('immutable audit output exists')
    report=audit(json.loads(args.squads.read_text()),
                 {'full':args.full,'original':args.original,'sortitoutsi':args.sortitoutsi})
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
    print(json.dumps({'registered_total':report['official_total'],
                      'scanned':report['source_row_counts'],
                      'statuses':report['status']},indent=2))


if __name__=='__main__':
    main()

