#!/usr/bin/env python3
"""Match pre-fixture EPL registered 2026/27 squads to original FM26 player identities.

Squad authority: Premier League's post-window registration article. FM authority:
FM26 database exact name and UID. No club inference from stale Transfermarkt.
Unique exact matches auto-accept; near-matches need high-confidence token and name
agreement; unresolved rows remain unresolved. Data snapshot is an as-of roster
source, not a confirmed starting XI or availability assessment.
"""
import argparse
import csv
import difflib
import html
import json
import re
import unicodedata
from collections import defaultdict, Counter
from datetime import date
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
SQUADS_URL='https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists'
FM_FILE=Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')
TEAMS={
    'Crystal Palace':'Crystal Palace','Nott\'m Forest':'Nottingham Forest',
    'Hull':'Hull City','Everton':'Everton','Liverpool':'Liverpool',
    'Man City':'Manchester City','Coventry':'Coventry City','Newcastle':'Newcastle United'
}

def norm(s):
    s=unicodedata.normalize('NFKD',s or '').casefold()
    s=''.join(ch for ch in s if not unicodedata.combining(ch))
    return ' '.join(re.findall(r'[a-z0-9]+',s))

def official_name(s):
    s=re.sub(r'\s*\(loan\).*','',s,flags=re.I).replace('*','').strip()
    if ',' in s:
        surname, given=s.split(',',1)
        return given.strip()+' '+surname.strip()
    return s

def fetch_official(cache,offline=False):
    if not offline:
        import requests
        response=requests.get(SQUADS_URL,timeout=25,headers={'User-Agent':'Mozilla/5.0'})
        response.raise_for_status()
        cache.parent.mkdir(parents=True,exist_ok=True)
        cache.write_text(response.text,encoding='utf-8')
    from bs4 import BeautifulSoup
    soup=BeautifulSoup(cache.read_text(encoding='utf-8'),'html.parser')
    body=soup.select_one('.article__content')
    if not body:raise ValueError('official squad HTML was not found')
    roster={}
    for h in body.find_all('h5'):
        club=h.get_text(' ',strip=True)
        if club not in TEAMS.values():continue
        buckets=[]
        group=None
        for paragraph in h.find_next_siblings():
            if paragraph.name=='h5':break
            if paragraph.name!='p':continue
            lines=[re.sub(r'\s+',' ',line).strip() for line in paragraph.get_text('\n').split('\n')]
            lines=[line for line in lines if line]
            for val in lines:
                low=val.lower()
                if '25 squad players' in low or low.startswith('squad players'):
                    group='registered';continue
                if 'u21 players' in low or 'under-21 players' in low:
                    group='u21';continue
                if group not in {'registered','u21'}:continue
                if low.startswith(('contract and scholars','home grown','squad players')):continue
                if val and not val.endswith(':'):
                    buckets.append({'official_name':val,'match_name':official_name(val),
                                    'registration':group})
        roster[club]=buckets
    if set(roster)!=set(TEAMS.values()):raise ValueError('incomplete official squad source')
    return roster

def best_match(src, exact, token_index, records):
    n=norm(src)
    if not n:return None,'unmatched',[]
    if n in exact:
        candidates=exact[n]
        # Prefer actual player record with CA, not duplicated non-player FM record.
        if len(candidates)==1:return candidates[0],'exact',[]
        cand=sorted(candidates,key=lambda x:(-x['ca'],x['uid']))
        if len(cand)>1 and cand[0]['ca'] == cand[1]['ca']:
            return None,'ambiguous exact',[{'name':x['name'],'uid':x['uid']} for x in cand[:3]]
        return cand[0],'exact',[]
    parts=n.split(); first=parts[0];last=parts[-1]
    candidates=set(token_index[last]) if last in token_index else set()
    if len(parts)>1:
        candidates|=set(token_index[parts[1]]) if len(parts[1])>=5 else set()
    # Require name-like match, avoid surname collisions and misidentification.
    ranks=[]
    for id in candidates:
        r=records[id];other=norm(r['name']); otherparts=other.split()
        if not otherparts:continue
        shared=set(parts)&set(otherparts)
        other_tokens=set(otherparts)
        # Uniquely recognize expanded official names: full legal given names vs
        # shortened FM display names. At least two tokens must align, with
        # the primary surname and first given name present.
        surname=parts[-1]
        complete=(len(other_tokens)>=2 and other_tokens.issubset(set(parts)) and first in other_tokens and surname in other_tokens)
        first_ok=first==otherparts[0] or (len(first)>=4 and first[:4]==otherparts[0][:4])
        similarity=difflib.SequenceMatcher(None,n,other).ratio()
        coverage=len(shared)/max(1,len(other_tokens))
        score=(1.0 if complete else 0.78*similarity+0.22*coverage)
        if (first_ok and len(shared)>=2 and score>=0.80):ranks.append((score,id))
    ranks.sort(key=lambda x:(-x[0],-records[x[1]]['ca'],records[x[1]]['uid']))
    suggestions=[{'name':records[i]['name'],'uid':records[i]['uid'],'similarity':round(v,4)} for v,i in ranks[:3]]
    if not ranks:return None,'unmatched',suggestions
    if ranks[0][0]>=0.96 and (len(ranks)<2 or ranks[0][0]-ranks[1][0]>=0.12):
        return records[ranks[0][1]],'unique expanded-name',suggestions
    if ranks[0][0]>=0.90 and (len(ranks)<2 or ranks[0][0]-ranks[1][0]>=0.10):
        return records[ranks[0][1]],'high-confidence name',suggestions
    return None,'review',suggestions

def run():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--offline',action='store_true')
    parser.add_argument('--fm-file',default=str(FM_FILE),help='path to FM26 full men attributes CSV')
    parser.add_argument('--output',default='data/pl/squads_2026-10-10.json')
    a=parser.parse_args()
    roster=fetch_official(ROOT/'data/pl/raw/registered_pl_2026_27.html',a.offline)
    legacy={}
    with (ROOT/'data/fm_tm_joined.csv').open(encoding='utf-8-sig') as f:
        for row in csv.DictReader(f):
            if row.get('uid') and row.get('current_club_name'):
                legacy[row['uid']]=row
    roster_tokens = {part for squad in roster.values() for p in squad
                     for part in norm(p['match_name']).split() if len(part) >= 4}
    records=[];exact=defaultdict(list);token_index=defaultdict(list)
    with Path(a.fm_file).open(encoding='utf-8-sig') as f:
        for row in csv.DictReader(f):
            try: ca=float(row.get('ca') or 0.)
            except ValueError:continue
            if ca<50 or not row.get('uid'):continue
            if len(set(norm(row['name']).split()) & roster_tokens) < 2: continue
            record={'uid':row['uid'],'name':row['name'],'ca':ca,'pa':float(row.get('pa') or 0),
                'attrs':{k[5:]:float(v) for k,v in row.items() if k.startswith('attr_') and v and re.fullmatch(r'[0-9]+(?:\.[0-9]+)?',v)},
                'tm': legacy.get(row['uid'],{})}
            idx=len(records);records.append(record)
            n=norm(row['name']);exact[n].append(record)
            for token in set(n.split()):
                if len(token)>3:token_index[token].append(idx)
    output={'as_of':'2026-10-10','registration_source':SQUADS_URL,'fm_source':'FM26 binary-derived complete men attributes','teams':{}}
    for short,club in TEAMS.items():
        entries=[]
        seen=set()
        for p in roster[club]:
            candidate,method,suggestions=best_match(p['match_name'],exact,token_index,records)
            row={'official_name':p['official_name'],'registration':p['registration'],'match_method':method}
            if candidate:
                if candidate['uid'] in seen:
                    row['match_method']='duplicate: unassigned'
                else:
                    seen.add(candidate['uid'])
                    row.update({'fm_uid':candidate['uid'],'fm_name':candidate['name'],'ca':candidate['ca'],
                                'pa':candidate['pa'],'attributes':candidate['attrs']})
                    if candidate['tm']:
                        tm=candidate['tm']
                        row['historical_position']=tm.get('position','')
                        row['historical_minutes_2526']=tm.get('mins_2526','')
                        row['historical_apps_2526']=tm.get('apps_2526','')
            if 'fm_uid' not in row and suggestions:row['suggestions']=suggestions
            entries.append(row)
        matched=[x for x in entries if 'fm_uid' in x]
        counts=Counter(x['registration'] for x in matched)
        unresolved=[x['official_name'] for x in entries if 'fm_uid' not in x]
        output['teams'][short]={'official_club':club,'registered_count':len(entries),
            'matched_count':len(matched),'matched_25':counts['registered'],'matched_u21':counts['u21'],
            'unmatched':unresolved,'players':entries}
        print(f'{short:20s} {len(matched):2d}/{len(entries):2d} matched; 25={counts["registered"]} U21={counts["u21"]} | unmatched {unresolved[:8]}')
    outfile=ROOT/a.output
    outfile.parent.mkdir(parents=True,exist_ok=True)
    outfile.write_text(json.dumps(output,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    print('WROTE',outfile)

if __name__=='__main__':run()
