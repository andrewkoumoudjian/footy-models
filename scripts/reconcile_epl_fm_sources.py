#!/usr/bin/env python3
"""Search *every remaining EPL registration* across FM26 and Sortitoutsi.

FM native UID and Sortitoutsi UID are distinct typed identifiers even if their
numeric strings happen to match. This audit compares every unmatched official
registration against multiple raw FM exports and the independent Sortitoutsi
roster + verified SI-to-binary-UID map. Ambiguous matches are never filled by
CA, transfer value or fame; outputs include per-person candidate evidence.

It can promote uniquely corroborated native **player** identities (official
name -> native FM name and independently agreeing Sortitoutsi name/DOB), but
the official Premier League registration has no DOB, so all promoted matches
remain marked official-identity-provisional. Native attribute values always
come from the exact linked FM UID, never a nearby name or reconstructed row.
"""
from __future__ import annotations

import argparse
import csv
import json
from collections import Counter, defaultdict
from pathlib import Path

from match_premier_league_fm26 import norm, official_name

ROOT = Path(__file__).resolve().parents[1]
FM = Path('/home/drewclaw/fm26-data')
EXPORTS = ('fm26_men_full_attrs.csv', 'fm26_final_men.csv',
           'fm26_correct.csv', 'fm26_men.csv')


def name_parts(name):
    return tuple(norm(name).split())


def official_components(name):
    stripped = name.replace('*', '').split('(Loan)')[0].strip()
    given, surname = (stripped.split(',', 1)[1], stripped.split(',', 1)[0]) if ',' in stripped else (
        stripped.split()[0], stripped.split()[-1])
    return name_parts(given), name_parts(surname)


def match_quality(official, name):
    """Pure string evidence. Don't classify a record as verified identity."""
    given, family = official_components(official)
    parts = name_parts(name)
    if not given or not family or len(parts) < 2:
        return None
    if parts == name_parts(official_name(official)):
        return (0, 'same_full_name')
    if sorted(parts) == sorted(name_parts(official_name(official))):
        return (1, 'all_legal_tokens_permuted')
    if parts[0] != given[0]:
        return None
    family_shared = set(parts[1:]) & set(family)
    if not family_shared:
        return None
    if parts[-1] == family[-1]:
        return (2, 'given_and_last_family_name')
    if any(len(word) >= 5 for word in family_shared):
        return (3, 'given_and_significant_family_name')
    return None


def source_candidates(official, source, record, out):
    grade = match_quality(official, record.get('name', ''))
    if grade is not None:
        out.append({
            'source': source, 'uid': record.get('uid', ''),
            'name': record.get('name', ''), 'dob': record.get('dob', ''),
            'kind': record.get('kind', ''), 'attr_source': record.get('attr_source', ''),
            'grade': grade[0], 'evidence': grade[1],
        })


def scan_all(unresolved, fm_dir, si_roster, si_map):
    """Full scans, not CA-filtered or already-matched-only subsets."""
    by_first = defaultdict(list)
    for item in unresolved:
        given, _ = official_components(item['official_name'])
        if given:
            by_first[given[0]].append(item)
    candidates = defaultdict(list)
    processed = Counter()
    for filename in EXPORTS:
        file = fm_dir / filename
        if not file.exists():
            continue
        with file.open(encoding='utf-8-sig', newline='') as handle:
            for row in csv.DictReader(handle):
                processed[filename] += 1
                name = row.get('name') or ''
                parts = name_parts(name)
                if not parts or parts[0] not in by_first:
                    continue
                # Restrict expensive exact-name analysis to plausible family
                # overlaps, but never filter on ability rating or type.
                family_tokens = set(parts[1:])
                for item in by_first[parts[0]]:
                    _, family = official_components(item['official_name'])
                    if not family_tokens.intersection(family):
                        continue
                    source_candidates(item['official_name'], filename, row,
                                      candidates[item['key']])
    si_rows = {}
    if si_roster.exists():
        with si_roster.open(encoding='utf-8-sig', newline='') as handle:
            for row in csv.DictReader(handle):
                processed['sortitoutsi_roster'] += 1
                sid = row.get('uid', '')
                si_rows[sid] = {'uid': sid, 'name': row.get('Name') or '',
                                'dob': row.get('DOB') or '',
                                'kind': 'player' if not row.get('Club Job') else 'uncertain_staff_or_player',
                                'attr_source': 'sortitoutsi_metadata_no_FM_attributes'}
    si_mappings = {}
    if si_map.exists():
        with si_map.open(encoding='utf-8-sig', newline='') as handle:
            for row in csv.DictReader(handle):
                processed['sortitoutsi_uid_map'] += 1
                si_mappings[row['si_id']] = row
    for sid, entry in si_rows.items():
        parts = name_parts(entry['name'])
        if not parts or parts[0] not in by_first:
            continue
        for item in by_first[parts[0]]:
            grade = match_quality(item['official_name'], entry['name'])
            if grade is None:
                continue
            mapping = si_mappings.get(sid, {})
            candidates[item['key']].append({
                'source': 'sortitoutsi_fm26_roster', 'sortitoutsi_uid': sid,
                'uid': mapping.get('binary_uid') or '',
                'name': entry['name'], 'dob': entry['dob'],
                'kind': entry['kind'], 'attr_source': entry['attr_source'],
                'grade': grade[0], 'evidence': grade[1],
                'si_map_name': mapping.get('binary_name'),
                'si_map_dob': mapping.get('binary_dob'),
                'si_map_type': mapping.get('match_type'),
                'si_name': mapping.get('si_name'),
            })
    return candidates, processed


def adjudicate(records):
    """Promote only a native FM person corroborated by verified SI mapping."""
    by_uid = defaultdict(list)
    for record in records:
        if record.get('uid'):
            by_uid[record['uid']].append(record)
    options = []
    for uid, entries in by_uid.items():
        fm = [e for e in entries if e['source'] in EXPORTS]
        si = [e for e in entries if e['source'] == 'sortitoutsi_fm26_roster']
        fm_latest = next((e for source in EXPORTS for e in fm if e['source'] == source), None)
        verified_si = [s for s in si if s.get('si_map_name') and s.get('si_map_dob')
                       and norm(s.get('si_name', '')) == norm(s.get('name', ''))
                       and s['si_map_dob'] == s['dob']
                       and s['si_map_type'] in ('full', 'name-dob', 'exact', 'direct')]
        consistent = [s for s in verified_si if fm_latest and s['dob'] == fm_latest['dob']
                      and norm(s['si_map_name']) == norm(fm_latest['name'])]
        options.append({
            'uid': uid, 'fm_name': fm_latest['name'] if fm_latest else None,
            'native_kind': fm_latest['kind'] if fm_latest else None,
            'native_attr_source': fm_latest['attr_source'] if fm_latest else None,
            'native_dob': fm_latest['dob'] if fm_latest else None,
            'native_file': fm_latest['source'] if fm_latest else None,
            'grade': min(e['grade'] for e in entries),
            'sortitoutsi_ids': sorted({e['sortitoutsi_uid'] for e in si}),
            'verified_si_dob_native_agreement': bool(consistent),
            'independent_sources': sorted({e['source'] for e in entries}),
            'si_map_evidence': [{k:s.get(k) for k in ('sortitoutsi_uid', 'si_map_type','dob')}
                                for s in consistent],
        })
    options.sort(key=lambda o:(o['grade'], not o['verified_si_dob_native_agreement'], o['uid']))
    verified = [o for o in options if o['verified_si_dob_native_agreement']
                and o['native_kind'] == 'player']
    # Distinguish a candidate SI UID from native FM UID: promote only the
    # explicit binary_uid path corroborated by independent same-name/DOB.
    if len(verified) == 1 and all(other['grade'] > verified[0]['grade']
                                  for other in options if other['uid'] != verified[0]['uid']):
        return verified[0], options
    return None, options


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--roster', type=Path, default=ROOT/'data/identity/reep/epl_full_reep_first_v3.json')
    parser.add_argument('--fm-directory', type=Path, default=FM)
    parser.add_argument('--sortitoutsi-roster', type=Path, default=FM/'sortitoutsi_fm26_roster.csv')
    parser.add_argument('--sortitoutsi-map', type=Path, default=FM/'sortitoutsi_uid_map.csv')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists() or args.output.exists():
        parser.error('refusing to overwrite prior reconciliation outputs')
    roster = json.loads(args.roster.read_text())
    unresolved = []
    for club, block in roster['teams'].items():
        for player in block['players']:
            if not player.get('fm_uid'):
                unresolved.append({'club': club, 'official_name': player['official_name'],
                                   'registration': player['registration'],
                                   'key': (club, player['official_name'])})
    candidates, processed = scan_all(unresolved, args.fm_directory,
                                     args.sortitoutsi_roster, args.sortitoutsi_map)
    evidence = []
    promoted = 0
    for item in unresolved:
        key = item['key']
        chosen, options = adjudicate(candidates.get(key, []))
        status = ('uniquely_si_dob_corroborated_native_player' if chosen else
                  'ambiguous_or_unverified_candidates' if options else
                  'not_found_in_any_scanned_export')
        evidence.append({**{k:v for k,v in item.items() if k!='key'},
                         'review_status':status, 'candidate_count':len(options),
                         'evidence':options[:20]})
        # Do not automatically attach here: a second-stage per-team collision
        # check and the official list's missing DOB are independent issues.
        # Candidate reports are actionable, but an identity must not be
        # promoted without external official-to-provider corroboration.
        if chosen: promoted+=1
    summary = dict(Counter(record['review_status'] for record in evidence))
    report = {'schema':'epl_unmatched_all_local_fm_and_sortitoutsi_v1',
              'as_of':roster['as_of'], 'fm_exports_scanned':dict(processed),
              'unresolved_at_start':len(unresolved),
              'uniquely_si_dob_corroborated_candidates_not_auto_promoted':promoted,
              'status_counts':summary, 'players':evidence,
              'warning':'native FM and Sortitoutsi IDs are namespaced; official league names have no DOB; candidate review not auto-identity verification'}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
    # The roster is intentionally unchanged: adding a candidate without
    # official-to-provider ID corroboration would inject false FM traits.
    roster['fm_multi_export_reconciliation_report'] = str(args.report)
    roster['fm_multi_export_reconciliation_summary'] = summary
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(roster,indent=2,ensure_ascii=False)+'\n')
    print(json.dumps({'scanned':dict(processed), 'unresolved':len(unresolved),
                      'candidate_status':summary,
                      'unique_native_plus_SI_DOB_candidates':promoted},indent=2))


if __name__=='__main__':
    main()
