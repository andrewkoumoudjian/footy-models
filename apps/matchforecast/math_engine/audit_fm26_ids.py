#!/usr/bin/env python3
"""Audit the whole FM26 UID namespace against an official dated EPL roster.

Unlike the original match_premier_league_fm26.py, this scan DOES NOT discard
players by CA >= 50, two shared name tokens, or player/staff classification.

Identity evidence is distinct from verified player attributes. All new matches
are CANDIDATES for review; they do not overwrite the existing roster, FM UID,
or existing forecast data. IDs from different providers are never equated.
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

DEFAULT_FM = Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')
ROOT = Path(__file__).resolve().parents[3]


def normalize(value: str) -> str:
    value = unicodedata.normalize('NFKD', value or '').casefold()
    return ' '.join(re.findall(r'[a-z0-9]+',
        ''.join(x for x in value if not unicodedata.combining(x))))


def legal_name_parts(official: str) -> tuple[list[str], list[str]]:
    """Return given and surname tokens; retain all official legal names."""
    clean = re.sub(r'\s*\(loan\).*', '', official, flags=re.I).replace('*', '')
    if ',' not in clean:
        tokens = normalize(clean).split()
        return tokens[:1], tokens[1:]
    family, given = clean.split(',', 1)
    return normalize(given).split(), normalize(family).split()


def matching_evidence(official: str, fm_name: str) -> str | None:
    given, family = legal_name_parts(official)
    candidate = normalize(fm_name).split()
    if len(candidate) < 2 or not given or not family:
        return None
    full = given + family
    if candidate == full:
        return 'full_official_name'
    if candidate[0] != given[0]:
        return None
    if not any(token in family for token in candidate[1:]):
        return None
    if not any(token in candidate[1:] for token in family):
        return None
    # Proposals may have abbreviations or incomplete legal middle names.
    # They are not automatically accepted without an independent ID/DOB link.
    return 'legal_given_and_family_tokens'


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as file:
        for piece in iter(lambda: file.read(1 << 20), b''):
            digest.update(piece)
    return digest.hexdigest()


def load_csv(path: Path):
    with path.open(encoding='utf-8-sig', newline='') as file:
        yield from csv.DictReader(file)


def collect_transfermarkt(tm_path: Path):
    """Return pre-existing FM UID => TM ID crosswalk with ambiguity controls."""
    external = defaultdict(set)
    rows_by_uid = defaultdict(list)
    for row in load_csv(tm_path):
        uid = str(row.get('uid') or '').strip()
        tm_id = str(row.get('player_id') or '').strip()
        if not uid or not tm_id:
            continue
        external[uid].add(tm_id)
        rows_by_uid[uid].append({
            'transfermarkt_id': tm_id,
            'name_tm': row.get('name_tm') or '',
            'tm_dob': row.get('date_of_birth') or '',
            'source': str(tm_path),
        })
    trusted = {}
    contradictory = set()
    for uid, ids in external.items():
        if len(ids) != 1:
            contradictory.add(uid)
        else:
            trusted[uid] = rows_by_uid[uid][0]
    return trusted, contradictory


def run(squad_path: Path, fm_path: Path, tm_path: Path):
    squad = json.loads(squad_path.read_text(encoding='utf-8'))
    requests = []
    for club, block in squad['teams'].items():
        for person in block['players']:
            given, family = legal_name_parts(person['official_name'])
            requests.append({
                'club': club,
                'name': person['official_name'],
                'registration': person.get('registration'),
                'old_uid': str(person.get('fm_uid') or ''),
                'old_method': person.get('match_method') or '',
                'given': given, 'family': family,
            })
    if not requests:
        raise ValueError('no officially registered players')
    if len({(p['club'], p['name']) for p in requests}) != len(requests):
        raise ValueError('duplicate official name and club; disambiguation required')

    wanted_first = {person['given'][0] for person in requests if person['given']}
    existing_ids = {person['old_uid'] for person in requests if person['old_uid']}
    # Do not load ~500k full FM attribute vectors into memory. Scan the complete
    # native UID namespace and retain ONLY name/identity/provenance fields.
    raw_by_uid = {}
    by_given = defaultdict(list)
    raw_uids = set()
    kinds = Counter()
    scanned = 0
    for row in load_csv(fm_path):
        scanned += 1
        uid = str(row.get('uid') or '').strip()
        if not uid:
            continue
        if uid in raw_uids:
            raise ValueError(f'duplicate raw FM UID {uid}: identity not safely resolvable')
        raw_uids.add(uid)
        kind = str(row.get('kind') or '').strip().lower()
        attr_source = str(row.get('attr_source') or '').strip().lower()
        kinds[f'{kind}/{attr_source}'] += 1
        name = str(row.get('name') or '')
        norm = normalize(name).split()
        if uid not in existing_ids and (not norm or norm[0] not in wanted_first):
            continue
        record = {
            'fm26_uid': uid,
            'fm_name': name,
            'kind': kind,
            'attr_source': attr_source,
            'fm_birth_date': str(row.get('dob_d') or row.get('date_of_birth') or ''),
            'ca': row.get('ca') or '',
        }
        if uid in existing_ids:
            raw_by_uid[uid] = record
        if norm and norm[0] in wanted_first:
            by_given[norm[0]].append(record)

    tm, tm_conflicts = collect_transfermarkt(tm_path)
    summary = Counter()
    result = []
    for person in requests:
        old_uid = person['old_uid']
        existing = raw_by_uid.get(old_uid) if old_uid else None
        candidates = []
        if person['given']:
            for record in by_given[person['given'][0]]:
                strength = matching_evidence(person['name'], record['fm_name'])
                if strength is None:
                    continue
                entry = dict(record)
                entry['match_evidence'] = strength
                if record['fm26_uid'] in tm_conflicts:
                    entry['transfermarkt_identity'] = 'conflicting_ids'
                elif record['fm26_uid'] in tm:
                    tmrow = tm[record['fm26_uid']]
                    entry['transfermarkt_id'] = tmrow['transfermarkt_id']
                    entry['transfermarkt_name'] = tmrow['name_tm']
                    entry['transfermarkt_dob'] = tmrow['tm_dob']
                    entry['transfermarkt_identity'] = 'preexisting_uid_link_requires_source_verification'
                # The FM extractor often has 'staff' rows with good-looking
                # ratings; the player kind must not be inferred from CA or age.
                entry['has_verified_fm_player_attributes'] = (
                    record['kind'] == 'player' and record['attr_source'] == 'db')
                candidates.append(entry)
        candidates.sort(key=lambda x: (
            x['match_evidence'] != 'full_official_name',
            x['kind'] != 'player',
            x['attr_source'] != 'db',
            x['fm26_uid'],
        ))
        if old_uid:
            if existing is None:
                summary['old_fm_uid_missing_in_raw'] += 1
                status = 'old_uid_unverified'
            else:
                evidence = matching_evidence(person['name'], existing['fm_name'])
                status = 'existing_uid_present_name_consistent' if evidence else 'existing_uid_present_name_not_reconciled'
                summary[status] += 1
        else:
            if not candidates:
                status = 'unresolved_no_candidate'
            elif len(candidates) == 1:
                status = 'single_fm_name_candidate_needs_external_id_confirmation'
            else:
                status = 'multiple_fm_candidates_needs_external_id_confirmation'
            summary[status] += 1
        result.append({
            'official_name': person['name'],
            'club': person['club'],
            'registration': person['registration'],
            'original_fm26_uid': old_uid or None,
            'original_match_method': person['old_method'],
            'audit_status': status,
            'candidate_count': len(candidates),
            'candidates': candidates[:10],
            'truncated_candidate_count': max(0, len(candidates) - 10),
        })

    counts = {
        'registered_total': len(requests),
        'old_uid_linked': sum(bool(p['old_uid']) for p in requests),
        'old_uid_unlinked': sum(not p['old_uid'] for p in requests),
        'raw_fm_rows_scanned': scanned,
        'raw_unique_uids': len(raw_uids),
        'raw_record_kind_counts': dict(kinds),
        'tm_uid_links_without_conflict': len(tm),
        'tm_uid_conflicts': len(tm_conflicts),
        **dict(summary),
    }
    return {
        'schema': 'fm26_full_id_audit_v1',
        'created_utc': datetime.now(timezone.utc).isoformat(),
        'official_squad_as_of': squad['as_of'],
        'full_fm_database_not_CA_filtered': True,
        'identity_authority': {
            'native_fm': 'fm26_uid',
            'external_bridge': 'Transfermarkt player_id from pre-existing FM/TM join',
            'other_providers': ['fbref_id','sofascore_id','espn_id','understat_id'],
            'cross_provider_ids_are_not_equivalent': True,
        },
        'source_sha256': {str(p): sha256(p) for p in (squad_path,fm_path,tm_path)},
        'rules': [
            'Original UID exists only if verified against complete unfiltered raw FM26.',
            'Unmatched candidate UIDs are suggestions, not accepted IDs.',
            'An external ID or source-verified date of birth is required to promote proposals.',
            'kind=staff is not player; attr_source=derived/staff is not observed FM mental attributes.',
            'CA>=50 filters are disallowed for identity discovery.',
            'The existing ESPN/FBref/SofaScore/Understat numerical IDs are not FM26 UIDs.',
            'Official junior registrations may genuinely not exist in the FM26 edition.',
        ],
        'counts': counts,
        'roster': result,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--squad', type=Path,
                        default=ROOT/'data/pl/squads_2026-10-10.json')
    parser.add_argument('--fm', type=Path, default=DEFAULT_FM)
    parser.add_argument('--fm-transfermarkt', type=Path,
                        default=ROOT/'data/fm_tm_joined.csv')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('refusing to overwrite the identity audit snapshot')
    report = run(args.squad,args.fm,args.fm_transfermarkt)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,sort_keys=True,ensure_ascii=False,
                                       separators=(',', ':'))+'\n',encoding='utf-8')
    print(json.dumps(report['counts'],sort_keys=True,indent=2))


if __name__ == '__main__':
    main()

