#!/usr/bin/env python3
"""Exhaustively resolve still-unmatched official EPL players using Reep IDs.

Reads ALL Reep FM-player aliases, typed FM-to-Transfermarkt bridges and dated
Transfermarkt player names/DOB/clubs. Candidate identity is separate from
whether a local FM export's same UID contains that person's attributes.
Wrong native name/DOB/staff classifications NEVER become observed traits.
No league roster registration is dropped and duplicate UID assignments are
quarantined. Outputs immutable roster plus human-auditable per-player ledger.
"""
from __future__ import annotations

import argparse
import csv
import difflib
import json
from collections import Counter, defaultdict
from pathlib import Path

from match_premier_league_fm26 import TEAM_ALIASES, norm, official_name
from promote_epl_fm_tm_evidence import club_matches
from reconcile_epl_fm_sources import match_quality, official_components
from resolve_epl_reep import read_local_fm, reep_index

ROOT=Path(__file__).resolve().parents[1]
NATIVE=Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')
IGNORE={'de','da','do','dos','das','del','la','le','el','y','van','von','der','den','di'}


def components(name):
    given,family=official_components(name)
    return tuple(t for t in given if t not in IGNORE), tuple(t for t in family if t not in IGNORE)


def shared_name(official, variant):
    """Evidence tier for full legal name, nicknames, transliterations.

    4 means all legal tokens agree (possibly reordered), 3 all specific
    official tokens occur, 2 all candidate label tokens occur, 1 family+given
    evidence; 0 is insufficient and cannot promote alone.
    """
    given,family=components(official)
    if not given or not family:return 0
    actual=tuple(t for t in norm(variant).split() if t not in IGNORE)
    if len(actual)<2:return 0
    legal=given+family
    if sorted(actual)==sorted(legal):return 4
    if set(legal).issubset(set(actual)):return 3
    if len(actual)>=2 and set(actual).issubset(set(legal)) and actual[0] == given[0] and set(actual)&set(family):
        return 2
    # An expanded official family name may appear in any order and with
    # different diacritics, e.g. Ryan Sessegnon / Kouassi Ryan Sessegnon.
    # Preserve given-name uncertainty for orthographic variants rather than
    # rejecting the whole entity (Djorde/Djordje, Pilip/Philip, patronymics).
    similar_given=any(difflib.SequenceMatcher(None,given[0],x).ratio()>=0.83
                      for x in actual[:2])
    if (given[0] in actual or similar_given) and set(family).intersection(actual):return 1
    return 0


def index_reep(db):
    by_given=defaultdict(set)
    _,identities=reep_index(db)
    tm_ids=defaultdict(set)
    for rid,tid in db.execute("""SELECT reep_id,external_id FROM bridges
       WHERE provider='transfermarkt' AND namespace='spieler'""").fetchall():
        if rid in identities:tm_ids[rid].add(str(tid))
    for rid,row in identities.items():
        for label in row['names']:
            words=norm(label).split()
            if len(words)>1:
                by_given[words[0]].add(rid)
                if len(words)>2:
                    by_given[words[1]].add(rid)
                # Surname index permits accented/transliterated given names,
                # name-order inversions and abbreviations. Final acceptance
                # still requires independent person evidence, not surname only.
                for family in set(words[1:]):
                    if len(family)>=4:by_given['surname:'+family].add(rid)
    return by_given,identities,tm_ids


def read_tm(path, wanted):
    rows={}
    with path.open(encoding='utf-8-sig',newline='') as h:
        for row in csv.DictReader(h):
            if row.get('player_id') in wanted:rows[row['player_id']]=row
    return rows


def augment_native_from_complete_export(path, uids, native):
    """Recover missing FM UIDs from the *full* v6 binary export.

    FM26's gender-partitioned men export omits some real male players. The
    complete binary v6 can contain those same FM UIDs; only missing UIDs are
    supplemented. Never overwrite a conflicting primary row or elevate
    incomplete/staff rows into observed psychological measurements.
    """
    if path is None or not path.exists():
        return {'fallback_available':False,'fallback_uid_matches':0}
    absent=set(uids)-set(native)
    if not absent:
        return {'fallback_available':True,'fallback_uid_matches':0}
    found=0
    with path.open(encoding='utf-8-sig',newline='') as h:
        for row in csv.DictReader(h):
            uid=row.get('uid')
            if uid not in absent:continue
            valid={k.removeprefix('attr_'):float(value)
                   for k,value in row.items() if k.startswith('attr_') and value
                   and value.replace('.', '', 1).isdigit() and 1<=float(value)<=20}
            if not row.get('name') or not row.get('dob'):
                continue
            native[uid].append({
                'uid':uid,'name':row['name'],'dob':row['dob'],
                'kind':row.get('kind',''),
                'attr_source':'db' if row.get('kind')=='player' and len(valid)>=45 else 'none',
                'ca':row.get('ca') or '', 'pa':row.get('pa') or '',
                'attributes':valid if row.get('kind')=='player' and len(valid)>=45 else {},
                'record_source':'complete_v6_binary_fallback',
            })
            found+=1
    return {'fallback_available':True,'fallback_uid_matches':found}


def rank_person(official,club,reep_id,record,linked_tm,tm_rows,native):
    alias_tier=max((shared_name(official,n) for n in record['names']),default=0)
    evidence=[]
    tms=[]
    for tid in linked_tm:
        tm=tm_rows.get(tid)
        if not tm:continue
        tm_tier=shared_name(official,tm['name'])
        # Names are supplementary; Reep itself is the identity authority
        # tying FM UIDs to typed Transfermarkt provider IDs.
        club_ok=club_matches(TEAM_ALIASES[club],tm.get('current_club_name',''))
        tms.append((tm_tier,int(club_ok),tid,tm))
    tms.sort(key=lambda t:(t[0],t[1],t[2]),reverse=True)
    best=tms[0] if tms else None
    tm_name_tier=best[0] if best else 0
    tm_club_ok=bool(best and best[1])
    uids=record['uids']
    if len(uids)!=1:return None
    uid=next(iter(uids))
    instances=native.get(uid,[])
    native_row=instances[0] if len(instances)==1 else None
    tm_dob=best[3]['date_of_birth'][:10] if best else None
    native_dob=native_row.get('dob') if native_row else None
    dob_matches=bool(tm_dob and native_dob and tm_dob==native_dob)
    native_quality=(shared_name(official,native_row['name']) if native_row else 0)
    # Direct legal name (often extended) + typed provider bridge is sufficient
    # as a **provisional FM identity**; a native extraction contradiction must
    # exclude individual attributes, not necessarily the Reep UID itself.
    score=4*alias_tier+2*tm_name_tier+3*int(tm_club_ok)+3*int(dob_matches)
    if alias_tier==0:return None
    if alias_tier>=3 and best:
        sufficient=True
    elif alias_tier==4:
        sufficient=True
    elif alias_tier==2 and (tm_club_ok or dob_matches) and best:
        sufficient=True
    elif alias_tier==1 and tm_name_tier>=1 and tm_club_ok and dob_matches:
        sufficient=True
    else:
        sufficient=False
    safe_native=bool(native_row and native_row['kind']=='player'
        and ((best and dob_matches and native_quality>=1) or
             (not best and native_quality>=3)))
    return {
        'reep_id':reep_id,'fm_uid':uid,
        'reep_name':sorted(record['names'],key=lambda n:(-shared_name(official,n),len(n)))[0],
        'alias_name_evidence_tier':alias_tier,'tm_name_evidence_tier':tm_name_tier,
        'score':score,'sufficient':sufficient,'tm_club_compatible':tm_club_ok,
        'tm_player_id':best[2] if best else None,
        'tm_name':best[3]['name'] if best else None,
        'tm_birth_date':tm_dob,'native_birth_date':native_dob,
        'native_name':native_row['name'] if native_row else None,
        'native_kind':native_row['kind'] if native_row else None,
        'native_attr_source':native_row['attr_source'] if native_row else None,
        'native_dob_agrees_with_tm':dob_matches,
        'native_name_evidence_tier':native_quality,
        'can_use_native_player_attributes':safe_native,
    }


def candidate_for_row(official,club,by_given,identities,tm_ids,tm_rows,native):
    given,family=components(official)
    rids=set()
    for first in given[:2]:rids.update(by_given.get(first,()))
    for token in family:
        if len(token)>=4:rids.update(by_given.get('surname:'+token,()))
    selections=[]
    for rid in rids:
        record=identities[rid]
        # Fast screen avoids scoring unrelated people with common first names.
        if not any(set(family).intersection(norm(n).split()) for n in record['names']):
            continue
        selection=rank_person(official,club,rid,record,tm_ids.get(rid,set()),tm_rows,native)
        if selection:selections.append(selection)
    selections.sort(key=lambda x:(-x['score'],-x['alias_name_evidence_tier'],x['reep_id']))
    accepted=[x for x in selections if x['sufficient']]
    # Unlike the old ranker, when competing identities are tied or close,
    # even a high-CA FM player is never chosen arbitrarily.
    if accepted and (len(accepted)==1 or
                     accepted[0]['score']>=accepted[1]['score']+5):
        return accepted[0],selections
    return None,selections


def native_full_legal_fallback(person,review):
    """Native FM exact full legal name only; never a 2-token near-match."""
    eligible=[c for c in review if c.get('native_kind')=='player'
              and c.get('native_dob') and c.get('grade')==0]
    if len(eligible)!=1:return None
    official_words=norm(official_name(person)).split()
    if len(official_words)<3:return None
    candidate=eligible[0]
    if not candidate.get('uid'):return None
    return {'fm_uid':candidate['uid'],'native_name':candidate.get('fm_name'),
            'native_dob':candidate['native_dob'],'identity_source':'unique_native_full_legal_name',
            'can_use_native_player_attributes':False,
            'note':'no source-issued official UID; no independent DOB confirmation'}


def build_full_native_name_index(path, official_roster):
    """Scan every actual FM row for exact *whole legal name* evidence.

    Does not require a preexisting Reep bridge or SI mapping; this matters
    when a person is absent from Reep FM edges (e.g. Alisson Becker).
    """
    targets=defaultdict(list)
    for club,block in official_roster['teams'].items():
        for person in block['players']:
            legal=' '.join(sorted(norm(official_name(person['official_name'])).split()))
            targets[legal].append((club,person['official_name']))
    by_person=defaultdict(list)
    with path.open(encoding='utf-8-sig',newline='') as h:
        for row in csv.DictReader(h):
            name=row.get('name') or ''
            legal=' '.join(sorted(norm(name).split()))
            if legal not in targets or not row.get('uid'):
                continue
            for club,official in targets[legal]:
                by_person[(club,official)].append({
                    'uid':row['uid'],'fm_name':name,'native_kind':row.get('kind'),
                    'native_dob':row.get('dob'),'native_attr_source':row.get('attr_source'),
                    'grade':0,'source':'full_native_fm26_legal_name',
                })
    return by_person


def read_sortitoutsi_corrob(roster_path,map_path,official_roster,local_fm):
    """Only SI->binary UID explicit mappings; NEVER treat SI ID as FM UID.

    Exact full legal name, SI DOB, mapped native FM DOB and native name must
    agree. Unmapped SI UIDs remain provider-only leads, not engine FM IDs.
    """
    if not roster_path.exists() or not map_path.exists():return {}
    targets=defaultdict(list)
    for club,block in official_roster['teams'].items():
        for p in block['players']:
            key=' '.join(sorted(norm(official_name(p['official_name'])).split()))
            targets[key].append((club,p['official_name']))
    mapping={}
    with map_path.open(encoding='utf-8-sig',newline='') as h:
        for row in csv.DictReader(h):
            mapping[row.get('si_id')]=row
    found=defaultdict(list)
    with roster_path.open(encoding='utf-8-sig',newline='') as h:
        for row in csv.DictReader(h):
            legal=' '.join(sorted(norm(row.get('Name','')).split()))
            if legal not in targets:continue
            si_id=row.get('uid','')
            link=mapping.get(si_id) or {}
            fm_uid=str(link.get('binary_uid') or '')
            if not fm_uid:continue
            candidate=local_fm.get(fm_uid,[])
            if len(candidate)!=1:continue
            native=candidate[0]
            if not row.get('DOB') or row['DOB']!=native['dob']:continue
            if not link.get('binary_dob') or link['binary_dob']!=native['dob']:continue
            if norm(link.get('binary_name'))!=norm(native['name']):continue
            if norm(link.get('si_name'))!=norm(row.get('Name')):continue
            for club,official in targets[legal]:
                found[(club,official)].append({
                    'uid':fm_uid,'fm_name':native['name'],
                    'native_kind':native['kind'],'native_dob':native['dob'],
                    'native_attr_source':native['attr_source'],'grade':0,
                    'source':'sortitoutsi_explicit_binary_UID_name_DOB',
                    'sortitoutsi_uid':si_id,
                })
    return found


def resolve_official_roster(official_roster,active_path,fm_path,tm_path,
                            si_roster_path=None,si_map_path=None,complete_fm_path=None):
    """Single source-neutral implementation used by the *normal* EPL command.

    This function starts from ZERO name-guessed UIDs. Full Reep resolution,
    native exact legal names, SI->binary UID and independent TM are all
    consulted in one deterministic execution, with no post-hoc UID patchlist.
    """
    import duckdb
    official=json.loads(json.dumps(official_roster))
    if len(official['teams'])!=20:
        raise ValueError('canonical identity resolver requires all 20 EPL clubs')
    if any(p.get('fm_uid') for block in official['teams'].values()
           for p in block['players']):
        raise ValueError('canonical Reep-first input must be raw official registrations, not old FM guesses')
    active=json.loads(active_path.read_text())
    if str(active.get('stamp',''))[:8]>official['as_of'].replace('-',''):
        raise ValueError('Reep identity release is newer than forecast cutoff')
    with duckdb.connect(active['path'],read_only=True) as db:
        by_given,identities,tm_ids=index_reep(db)
    tm_rows=read_tm(tm_path,{tid for ids in tm_ids.values() for tid in ids})
    wanted_fm={uid for r in identities.values() for uid in r['uids']}
    if si_roster_path and si_map_path and si_roster_path.exists() and si_map_path.exists():
        # Discover SI IDs from all source names before following the EXPLICIT
        # si_id->binary_uid table. Numeric SI IDs must never be used as FM.
        target={' '.join(sorted(norm(official_name(p['official_name'])).split()))
                for block in official['teams'].values() for p in block['players']}
        si_names=set()
        with si_roster_path.open(encoding='utf-8-sig',newline='') as h:
            for row in csv.DictReader(h):
                if ' '.join(sorted(norm(row.get('Name','')).split())) in target:
                    si_names.add(str(row.get('uid') or ''))
        with si_map_path.open(encoding='utf-8-sig',newline='') as h:
            for row in csv.DictReader(h):
                if row.get('si_id') in si_names and row.get('binary_uid'):
                    wanted_fm.add(str(row['binary_uid']))
    native=read_local_fm(fm_path,wanted_fm)
    supplement=augment_native_from_complete_export(complete_fm_path,wanted_fm,native)
    lookup=build_full_native_name_index(fm_path,official)
    if si_roster_path and si_map_path:
        for key,entries in read_sortitoutsi_corrob(si_roster_path,si_map_path,
                                                    official,native).items():
            lookup[key].extend(entries)
    resolved,ledger=recover(official,lookup,by_given,identities,tm_ids,tm_rows,native)
    resolved['identity_rebuild']='canonical_reep_first_v2'
    resolved['identity_pipeline']='single_pass_reep_native_FM_SI_binary_UID_TM_DOB'
    resolved['reep_bridge_release_stamp']=active.get('stamp')
    resolved['reep_bridge_sha256']=active.get('sha256')
    resolved['full_binary_fm_fallback']=supplement
    return resolved,ledger


def recover(roster,lookup,by_given,identities,tm_ids,tm_rows,native):
    result=json.loads(json.dumps(roster))
    owners=defaultdict(list)
    for club,block in result['teams'].items():
        for p in block['players']:
            if p.get('fm_uid'):owners[str(p['fm_uid'])].append((club,p['official_name']))
    pending=[]
    review=[]
    for club,block in result['teams'].items():
        for p in block['players']:
            if p.get('fm_uid'):continue
            name=p['official_name']
            candidate,choices=candidate_for_row(name,club,by_given,identities,tm_ids,tm_rows,native)
            fallback=False
            if candidate is None and not any(x['sufficient'] for x in choices):
                fallback=native_full_legal_fallback(name,lookup.get((club,name),[]))
                if fallback:candidate=fallback
            row={'club':club,'official_name':name,'registration':p['registration'],
                 'accepted_candidate':candidate,
                 'candidate_count':len(choices),'alternatives':choices[:12]}
            review.append(row)
            if candidate:pending.append((club,p,candidate,row))
    pending_owners=defaultdict(list)
    for club,p,candidate,row in pending:
        pending_owners[candidate['fm_uid']].append((club,p['official_name']))
    counts=Counter()
    for club,p,candidate,row in pending:
        uid=candidate['fm_uid']
        if owners[uid] or len(pending_owners[uid])!=1:
            row['status']='fm_uid_collides_across_registrations'
            counts[row['status']]+=1
            continue
        # Keep source provider UID separate from measured FM attributes.
        p['fm_uid']=uid
        p['reep_id']=candidate.get('reep_id')
        p['fm_name']=candidate.get('native_name')
        p['fm_kind']=candidate.get('native_kind')
        p['attr_source']=candidate.get('native_attr_source')
        p['fm_native_record_trusted_for_this_player']=candidate.get('can_use_native_player_attributes',False)
        p['fm_player_record_verified']=bool(candidate.get('can_use_native_player_attributes'))
        native_rows=native.get(uid,[])
        direct=(native_rows[0] if len(native_rows)==1 and
                candidate.get('can_use_native_player_attributes') and
                native_rows[0]['kind']=='player' and
                native_rows[0]['attr_source']=='db' and
                native_rows[0]['name']==candidate.get('native_name') and
                native_rows[0]['dob']==candidate.get('native_birth_date') else None)
        p['fm_attributes_verified']=direct is not None
        p['identity_verification']='reep_tm_provisional' if candidate.get('reep_id') else 'native_exact_legal_provisional'
        p['match_method']='reep_fm_tm_identity_first' if candidate.get('reep_id') else 'native_full_legal_fallback'
        p['identity_evidence']=candidate
        if direct:
            p['attributes']=dict(direct['attributes'])
            p['ca']=float(direct['ca']) if direct.get('ca') else None
            p['pa']=float(direct['pa']) if direct.get('pa') else None
        else:
            p.pop('attributes',None);p.pop('ca',None);p.pop('pa',None)
        row['status']='candidate_uid_attached_attributes_quarantined'
        counts[row['status']]+=1
        if direct:
            counts['with_name_and_dob_consistent_native_player']+=1
        owners[uid].append((club,p['official_name']))
    for row in review:
        if 'status' not in row:
            row['status']='not_confident_enough_for_automatic_UID_attachment'
            counts[row['status']]+=1
    for club,block in result['teams'].items():
        players=block['players']
        for p in players:
            if not p.get('fm_uid'):
                p['match_method']='unresolved_after_canonical_multi_source_search'
                p['identity_verification']='unresolved_no_provider_id'
                p['fm_player_record_verified']=False
                p['fm_attributes_verified']=False
            elif not p.get('fm_name'):
                p['fm_name']=None
        block['matched_count']=sum(bool(p.get('fm_uid')) for p in players)
        block['matched_25']=sum(bool(p.get('fm_uid')) and p['registration']=='registered' for p in players)
        block['matched_u21']=sum(bool(p.get('fm_uid')) and p['registration']=='u21' for p in players)
        block['unmatched']=[p['official_name'] for p in players if not p.get('fm_uid')]
    result['remaining_identity_recovery_summary']=dict(counts)
    return result,review


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--roster',type=Path,default=ROOT/'data/identity/reep/epl_full_all_sources_v1.json')
    ap.add_argument('--prior-review',type=Path,default=ROOT/'data/identity/reep/epl_unmatched_all_sources_v1.json')
    ap.add_argument('--active',type=Path,default=ROOT/'data/identity/reep/active.json')
    ap.add_argument('--fm',type=Path,default=NATIVE)
    ap.add_argument('--tm-players',type=Path,default=ROOT/'data/tm_players.csv')
    ap.add_argument('--output',type=Path,required=True)
    ap.add_argument('--review',type=Path,required=True)
    a=ap.parse_args()
    if a.output.exists() or a.review.exists():ap.error('immutable output already exists')
    import duckdb
    active=json.loads(a.active.read_text())
    with duckdb.connect(active['path'],read_only=True) as db:
        by_given,identities,tm_ids=index_reep(db)
    wanted_tm={tid for ids in tm_ids.values() for tid in ids}
    tm_rows=read_tm(a.tm_players,wanted_tm)
    wanted_uid={uid for r in identities.values() for uid in r['uids']}
    native=read_local_fm(a.fm,wanted_uid)
    lookup={(p['club'],p['official_name']):p['evidence']
            for p in json.loads(a.prior_review.read_text())['players']}
    result,review=recover(json.loads(a.roster.read_text()),lookup,by_given,identities,tm_ids,tm_rows,native)
    summary={'initial_unmatched':len(review),'recovery':result['remaining_identity_recovery_summary'],
             'final_unmatched':sum(len(b['unmatched']) for b in result['teams'].values()),
             'final_matches':sum(b['matched_count'] for b in result['teams'].values()),
             'remaining_senior':sum(sum(p['registration']=='registered' and not p.get('fm_uid')
                                        for p in b['players']) for b in result['teams'].values()),
             'native_trait_rule':'recover identity separately; never use attributes without native name+DOB corroboration'}
    a.output.parent.mkdir(parents=True,exist_ok=True)
    a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    a.review.parent.mkdir(parents=True,exist_ok=True)
    a.review.write_text(json.dumps({'summary':summary,'players':review},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':main()
