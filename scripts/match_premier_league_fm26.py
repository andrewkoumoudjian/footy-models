#!/usr/bin/env python3
"""Official EPL registrations -> Reep-first typed FM identity discovery.

Run-time does NOT assign native FM UIDs by display-name similarity. The Reep
identity register, cross-provider IDs, FM26 raw entries, and independent
Sortitoutsi/TM DOB evidence are resolved by one canonical matching function.
Weak evidence is retained as a review lead; only corroborated *player-specific*
native attributes may enter model inputs. Squad membership != matchday XI.
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
TEAM_ALIASES={
    'Arsenal':'Arsenal','Aston Villa':'Aston Villa','Bournemouth':'AFC Bournemouth',
    'Brentford':'Brentford','Brighton':'Brighton & Hove Albion','Chelsea':'Chelsea',
    'Fulham':'Fulham','Ipswich':'Ipswich Town','Leeds':'Leeds United',
    'Man United':'Manchester United','Sunderland':'Sunderland','Tottenham':'Tottenham Hotspur',
    'Crystal Palace':'Crystal Palace','Nott\'m Forest':'Nottingham Forest',
    'Hull':'Hull City','Everton':'Everton','Liverpool':'Liverpool',
    'Man City':'Manchester City','Coventry':'Coventry City','Newcastle':'Newcastle United'
}

def norm(s):
    # NFKD does not decompose letters such as Ø, Ł, Đ and Æ. Dropping these
    # characters made Ødegaard, e.g., impossible to discover by surname.
    s=(s or '').casefold().translate(str.maketrans({
        'ø':'o','ł':'l','đ':'dj','ð':'d','þ':'th','æ':'ae','œ':'oe',
        'ß':'ss','ı':'i','ħ':'h','ŧ':'t','ŋ':'n',
    }))
    # N'Diaye, O'Brien and similar patronymics are one surname token;
    # dropping an apostrophe as a separator previously broke exact discovery.
    s=re.sub(r"(?<=[a-z])['’](?=[a-z])",'',s)
    s=unicodedata.normalize('NFKD',s)
    s=''.join(ch for ch in s if not unicodedata.combining(ch))
    return ' '.join(re.findall(r'[a-z0-9]+',s))

def official_name(s):
    s=re.sub(r'\s*\(loan\).*','',s,flags=re.I).replace('*','').strip()
    if ',' in s:
        surname, given=s.split(',',1)
        return given.strip()+' '+surname.strip()
    return s

def fetch_official(cache,offline=False,teams=None,url=SQUADS_URL):
    if not offline:
        import requests
        response=requests.get(url,timeout=25,headers={'User-Agent':'Mozilla/5.0'})
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
        if club not in (teams or TEAM_ALIASES).values():continue
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
    if set(roster)!=set((teams or TEAM_ALIASES).values()):raise ValueError(f'incomplete official squad source: missing {set((teams or TEAM_ALIASES).values())-set(roster)}')
    return roster

def best_match(src, exact, token_index, records):
    """Deprecated name-only scorer, retained for legacy falsification tests.

    NEVER use to populate any current model roster. See run() below.
    """
    n=norm(src)
    if not n:return None,'unmatched',[]
    if n in exact:
        candidates=exact[n]
        # An identical name is not an identity key. Never disambiguate a
        # player/staff collision (or two players) with a higher FM CA rating.
        if len(candidates)==1:return candidates[0],'exact',[]
        cand=sorted(candidates,key=lambda x:x['uid'])
        return None,'ambiguous exact',[{'name':x['name'],'uid':x['uid'],
                                        'kind':x['kind'],'attr_source':x['attr_source']} for x in cand[:3]]
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
    ranks.sort(key=lambda x:(-x[0],records[x[1]]['uid']))
    suggestions=[{'name':records[i]['name'],'uid':records[i]['uid'],'similarity':round(v,4),
                  'kind':records[i]['kind'],'attr_source':records[i]['attr_source']}
                 for v,i in ranks[:3]]
    if not ranks:return None,'unmatched',suggestions
    if ranks[0][0]>=0.96 and (len(ranks)<2 or ranks[0][0]-ranks[1][0]>=0.12):
        return records[ranks[0][1]],'unique expanded-name',suggestions
    if ranks[0][0]>=0.90 and (len(ranks)<2 or ranks[0][0]-ranks[1][0]>=0.10):
        return records[ranks[0][1]],'high-confidence name',suggestions
    return None,'review',suggestions

def run():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--offline',action='store_true')
    parser.add_argument('--fm-file',default=str(FM_FILE),help='men-provenance sidecar (attr_source/height_source); complete-everyone primary comes from --fm-complete-file')
    parser.add_argument('--fm-men-file',default=None,help='explicit men-provenance path (defaults to --fm-file when it points at a men extract, else the bundled men path)')
    parser.add_argument('--fixtures',help='optional fixture subset; omit to build all 20 EPL clubs')
    parser.add_argument('--as-of',default='2026-10-11',help='as-of YYYY-MM-DD')
    parser.add_argument('--registration-url',default=SQUADS_URL)
    parser.add_argument('--html-cache',default='data/pl/raw/registered_pl_2026_27.html')
    parser.add_argument('--output',default='data/pl/squads_2026-10-11_canonical.json')
    parser.add_argument('--identity-audit',help='optional full candidate/ambiguity review JSON')
    parser.add_argument('--reep-active',default='data/identity/reep/active.json')
    parser.add_argument('--tm-players',default='data/tm_players.csv')
    parser.add_argument('--sortitoutsi-roster',default='/home/drewclaw/fm26-data/sortitoutsi_fm26_roster.csv')
    parser.add_argument('--sortitoutsi-map',default='/home/drewclaw/fm26-data/sortitoutsi_uid_map.csv')
    parser.add_argument('--fm-complete-file',default='/home/drewclaw/fm26-data/fm26_complete_v6.csv',
                        help='complete-everyone FM26 primary: all genders/kinds, no CA pre-filter (unioned with men provenance)')
    a=parser.parse_args()
    cutoff=date.fromisoformat(a.as_of)
    if not date(2026,9,1)<=cutoff<date(2027,7,1) and a.registration_url==SQUADS_URL:
        raise ValueError('2026/27 official squad list cannot be assumed valid in another season; provide --registration-url')
    clubs=TEAM_ALIASES.copy()
    if a.fixtures:
        config=json.loads((ROOT/a.fixtures).read_text())
        if config.get('as_of')!=a.as_of:
            raise ValueError('fixture config as-of must equal requested official roster as-of')
        requested={name for f in config['fixtures'] for name in (f['home'],f['away'])}
        unsupported=requested-set(clubs)
        if unsupported:raise ValueError(f'unknown Premier League team aliases: {sorted(unsupported)}')
        clubs={k:v for k,v in clubs.items() if k in requested}
    # FULL LEAGUE first. Fixture-only matching obscured collisions across
    # clubs and left the other twelve EPL teams invisible. Filter *after*
    # resolving all official registrations.
    registration=fetch_official(ROOT/a.html_cache,a.offline,None,a.registration_url)
    from recover_epl_remaining_identities import resolve_official_roster
    official={'as_of':str(cutoff),'registration_source':a.registration_url,
              'registration_source_published':'2026-09-03' if a.registration_url==SQUADS_URL else None,
              'roster_scope':'full_league','expected_club_count':20,
              'fm_source':'FM26 complete-everyone union (complete v6 primary + men provenance)','teams':{}}
    for club,club_name in TEAM_ALIASES.items():
        entries=[{'official_name':p['official_name'],'registration':p['registration'],
                  'registered_club':club,'membership_observed_as_of':str(cutoff),
                  'loaned_out':bool(re.search(r'\(loan\)',p['official_name'],re.I))}
                 for p in registration[club_name]]
        official['teams'][club]={'official_club':club_name,'registered_count':len(entries),
            'players':entries,'matched_count':0,'matched_25':0,'matched_u21':0,
            'unmatched':[p['official_name'] for p in entries]}
    men_path=Path(a.fm_men_file) if a.fm_men_file else (
        Path(a.fm_file) if Path(a.fm_file).name.startswith('fm26_men') else FM_FILE)
    output,ledger=resolve_official_roster(official,ROOT/a.reep_active,
        men_path,ROOT/a.tm_players,Path(a.sortitoutsi_roster),Path(a.sortitoutsi_map),
        Path(a.fm_complete_file))
    if a.fixtures:
        output['teams']={club:output['teams'][club] for club in clubs}
        output['roster_scope']='fixture_subset'
        output['expected_club_count']=len(clubs)
    outfile=ROOT/a.output
    review=ROOT/a.identity_audit if a.identity_audit else outfile.with_name(outfile.stem+'_identity_evidence.json')
    serial=json.dumps(output,indent=2,ensure_ascii=False)+'\n'
    review_serial=json.dumps({'as_of':str(cutoff),
        'resolver':'canonical_reep_first_v3',
        'source_policy':'complete-everyone FM union primary, Reep and typed IDs first; no CA/kind/gender pre-filter, no name-only auto-ranking',
        'players':ledger},indent=2,ensure_ascii=False)+'\n'
    # Safe to repeat identical runs; never silently mutate an existing as-of
    # snapshot when source data or code changes. Use a new version in that case.
    if outfile.exists() or review.exists():
        if (outfile.is_file() and review.is_file() and
            outfile.read_text(encoding='utf-8')==serial and
            review.read_text(encoding='utf-8')==review_serial):
            print('UNCHANGED identical canonical roster and identity audit',outfile,review)
            return
        raise ValueError('prior canonical snapshot differs; choose a new versioned output name')
    outfile.parent.mkdir(parents=True,exist_ok=True)
    review.parent.mkdir(parents=True,exist_ok=True)
    outfile.write_text(serial,encoding='utf-8')
    review.write_text(review_serial,encoding='utf-8')
    for team,rows in output['teams'].items():
        print(f'{team:20s} {rows["matched_count"]}/{rows["registered_count"]} FM candidate IDs; '
              f'unresolved {len(rows["unmatched"])}')
    print('WROTE',outfile,review)

if __name__=='__main__':run()
