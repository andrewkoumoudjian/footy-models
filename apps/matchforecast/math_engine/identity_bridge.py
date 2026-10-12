#!/usr/bin/env python3
"""ID-first FM26 identity bridge; never confuse provider-specific ID namespaces.

The updated 2026-08-09 FM26 extraction establishes that native FM26 UIDs can
share values with Sortitoutsi IDs, but this does NOT prove that all values are
interchangeable. The supplied sortitoutsi_uid_map.csv supplies the actual
observed mapping. The older fm_tm_joined.csv is a heuristic TM join, not a
provider-issued identity assertion, and is consequently audit-only.

Official Premier League lists contain *names*, not person IDs. Their 600
registrations therefore cannot be linked automatically by IDs alone. This
contract deliberately leaves any row without a source-backed cross-provider ID
unresolved; unique-name matches never become an ID assertion.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FM = Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')
SI = Path('/home/drewclaw/fm26-data/sortitoutsi_uid_map.csv')
TM = ROOT / 'data/fm_tm_joined.csv'
SQUAD = ROOT / 'data/pl/squads_2026-10-10.json'

NAMESPACES = {'fm26_uid', 'sortitoutsi_id', 'transfermarkt_id'}
VERIFIED_SI_TYPES = {'full', 'surname'}


def file_sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def rows(path, required):
    with path.open(newline='', encoding='utf-8-sig') as handle:
        reader = csv.DictReader(handle)
        absent = required - set(reader.fieldnames or [])
        if absent:
            raise ValueError(f'{path}: missing fields {sorted(absent)}')
        yield from reader


def fm_identity_index(fm_path):
    """Scan all native UIDs without restricting current ability or first name."""
    identities = {}
    scanned = 0
    conflicting = set()
    for r in rows(fm_path, {'uid', 'kind', 'attr_source'}):
        scanned += 1
        uid = r['uid'].strip()
        if not uid:
            continue
        if uid in identities:
            # The published FM26 men export contains a small number of bad
            # repeated UID values (often staff records for different humans).
            # Quarantine *all* such UIDs rather than rejecting the other
            # ~563,000 usable source records or choosing the first name.
            conflicting.add(uid)
            continue
        identities[uid] = {
            'kind': r['kind'].strip().lower(),
            'attr_source': r['attr_source'].strip().lower(),
        }
    for uid in conflicting:
        identities.pop(uid, None)
    return identities, scanned, len(conflicting)


def build_si_edges(mapping_rows, known_uids):
    """Create *bidirectional, one-to-one* verified native-UID/SI-ID aliases."""
    candidates = defaultdict(set)
    inverses = defaultdict(set)
    descriptions = {}
    counts = Counter()
    for r in mapping_rows:
        si_id = (r.get('si_id') or '').strip()
        uid = (r.get('binary_uid') or '').strip()
        quality = (r.get('match_type') or '').strip().lower()
        if not si_id or not uid or quality not in VERIFIED_SI_TYPES:
            counts['not_verified_or_missing'] += 1
            continue
        if uid not in known_uids:
            counts['native_fm_uid_not_in_local_export'] += 1
            continue
        # Sortitoutsi is a separate namespace even when numbers happen to
        # coincide. Multiple edges for either ID are quarantined.
        candidates[si_id].add(uid)
        inverses[uid].add(si_id)
        descriptions[(si_id, uid)] = quality
    accepted = {}
    rejected_si = set()
    for si_id, uid_set in candidates.items():
        if len(uid_set) != 1:
            rejected_si.add(si_id)
            continue
        uid = next(iter(uid_set))
        if len(inverses[uid]) != 1:
            rejected_si.add(si_id)
            continue
        accepted[si_id] = {
            'fm26_uid': uid,
            'verification': 'sortitoutsi_uid_map:' + descriptions[(si_id, uid)],
        }
    counts['verified_one_to_one_edges'] = len(accepted)
    counts['ambiguous_si_ids'] = len(rejected_si)
    counts['ambiguous_fm_uids'] = sum(len(v) != 1 for v in inverses.values())
    return accepted, dict(counts)


def tm_audit_edges(tm_rows, known_uids):
    """Transfermarkt join has no direct provider assertion; never auto-link."""
    proposed = defaultdict(set)
    counts = Counter()
    for r in tm_rows:
        uid = (r.get('uid') or '').strip()
        tm_id = (r.get('player_id') or '').strip()
        if not tm_id or not uid:
            continue
        if uid not in known_uids:
            counts['tm_row_uid_missing_in_fm26'] += 1
            continue
        proposed[tm_id].add(uid)
    counts['unique_but_unverified_tm_id_links'] = sum(len(v) == 1 for v in proposed.values())
    counts['ambiguous_tm_id_links'] = sum(len(v) != 1 for v in proposed.values())
    return proposed, dict(counts)


def resolve_official_ids(person, known_uids, si_edges, tm_proposals):
    """Only source-owned explicit IDs authorize an FM UID assignment.

    A Transfermarkt-derived proposal is NOT an accepted ID edge. An official
    source may include existing fm_uid, or a Sortitoutsi ID from a verified
    mapping; conflicting evidence fails rather than overwriting.
    """
    asserted = {}
    fm_id = str(person.get('fm_uid') or '').strip()
    if fm_id:
        if fm_id not in known_uids:
            return None, 'native_uid_missing'
        asserted['fm26_existing'] = fm_id
    si_id = str(person.get('sortitoutsi_id') or '').strip()
    if si_id:
        if si_id not in si_edges:
            return None, 'sortitoutsi_uid_not_verified'
        asserted['sortitoutsi_verified_map'] = si_edges[si_id]['fm26_uid']
    tm_id = str(person.get('transfermarkt_id') or '').strip()
    if tm_id:
        # A TM ID is useful only as an *external key* for further confirmation.
        # Our existing name/DOB-joined TM file does not independently prove an
        # FM identity, even if its TM ID happens to be unique in that file.
        if len(tm_proposals.get(tm_id, set())) != 1:
            return None, 'transfermarkt_id_without_unique_local_candidate'
        if not asserted:
            return None, 'transfermarkt_unverified_candidate_only'
        candidate = next(iter(tm_proposals[tm_id]))
        if candidate not in asserted.values():
            return None, 'transfermarkt_join_disagrees_with_verified_uid'
    if not asserted:
        return None, 'official_registration_has_no_provider_id'
    if len(set(asserted.values())) != 1:
        return None, 'conflicting_verified_fm_uid_evidence'
    uid = next(iter(asserted.values()))
    return uid, '+'.join(sorted(asserted))


def build_report(squad_path, fm_path, si_path, tm_path):
    native, fm_rows, conflicting_native_uids = fm_identity_index(fm_path)
    si_edges, si_counts = build_si_edges(
        rows(si_path, {'si_id', 'binary_uid', 'match_type'}), native)
    tm_proposals, tm_counts = tm_audit_edges(
        rows(tm_path, {'uid', 'player_id'}), native)
    squad = json.loads(squad_path.read_text(encoding='utf-8'))
    audited = []
    counts = Counter()
    verified_si_by_uid = {v['fm26_uid']: key for key, v in si_edges.items()}
    for club, members in squad['teams'].items():
        for person in members['players']:
            uid, reason = resolve_official_ids(person, native, si_edges, tm_proposals)
            counts[reason] += 1
            audited.append({
                'club': club,
                'official_name': person['official_name'],
                'registration': person.get('registration'),
                'native_fm_uid': uid,
                'verified_identity_method': reason,
                'sortitoutsi_id': verified_si_by_uid.get(uid) if uid else None,
                'fm_kind': native[uid]['kind'] if uid else None,
                'fm_attribute_source': native[uid]['attr_source'] if uid else None,
                'observed_fm_player_attributes': bool(
                    uid and native[uid]['kind'] == 'player'
                    and native[uid]['attr_source'] == 'db'),
                'source_supplied_fm_id': bool(person.get('fm_uid')),
            })
    registered = sum(m['registered_count'] for m in squad['teams'].values())
    if registered != len(audited):
        raise ValueError('registered squad totals differ from enumerated people')
    return {
        'schema': 'fm26_identity_namespace_bridge_v1',
        'research_only': True,
        'created_utc': datetime.now(timezone.utc).isoformat(),
        'squad_as_of': squad['as_of'],
        'source_sha256': {str(p): file_sha(p)
                          for p in (squad_path,fm_path,si_path,tm_path)},
        'raw_native_fm_rows': fm_rows,
        'raw_unique_native_fm_uids': len(native),
        'quarantined_duplicate_native_uids': conflicting_native_uids,
        'verified_si_edges': si_counts,
        'tm_proposals_not_accepted_as_ids': tm_counts,
        'registered_total': registered,
        'native_id_resolved': sum(bool(x['native_fm_uid']) for x in audited),
        'fm_attributes_directly_observed': sum(x['observed_fm_player_attributes'] for x in audited),
        'resolution_counts': dict(sorted(counts.items())),
        'rules': [
            'Only documented FM26 UID and one-to-one mapped Sortitoutsi ID are trusted identity edges.',
            'The old Transfermarkt join was inferred by name/DOB; retain IDs as candidate metadata, never independently authoritative.',
            'Official league registration has names but no native FM/SI/provider IDs; no new ID mapping can be inferred from numbers without a source.',
            'SofaScore, ESPN, FBref, Understat, Transfermarkt and FM26 all keep disjoint namespaces until verified bridge edges exist.',
            'FM26 attributes with attr_source=derived are distributional, not actual per-player 1-20 values; staff rows have no observed player mental traits.',
            'Registered does not mean fit, available, matchday squad member or starting.',
        ],
        'people': audited,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--squad', type=Path, default=SQUAD)
    p.add_argument('--fm', type=Path, default=FM)
    p.add_argument('--si', type=Path, default=SI)
    p.add_argument('--tm', type=Path, default=TM)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    if a.output.exists():
        p.error('refusing to overwrite an immutable audit')
    report = build_report(a.squad, a.fm, a.si, a.tm)
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(report, sort_keys=True, ensure_ascii=False,
                                   separators=(',', ':')) + '\n', encoding='utf-8')
    print(json.dumps({
        'registered': report['registered_total'],
        'id_resolved': report['native_id_resolved'],
        'fm_observed_attributes': report['fm_attributes_directly_observed'],
        'sortitoutsi_edges': report['verified_si_edges'],
        'resolution_counts': report['resolution_counts'],
    }, sort_keys=True, indent=2))


if __name__ == '__main__':
    main()
