#!/usr/bin/env python3
"""Resolve official EPL registrations via Reep aliases -> native FM26 UIDs.

Unlike the old matcher, discovery starts from the entire Reep player identity
register (label + aliases), not from FM display-name similarity. Only unique,
first-name-and-family compatible Reep identities are promoted; no CA/PA,
reputation, popularity, or model score decides an identity. The existing
20-team roster remains intact and prior name-only assignments can be corrected.

Reep v1 publishes aliases but NOT date of birth or career/squad evidence.
Name-compatible Reep matches are therefore explicitly provisional official-
roster joins even when the FM<->Reep provider bridge is verified. Later
external DOB or club evidence must be added before calling them fully verified.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from collections import Counter, defaultdict
from pathlib import Path

from match_premier_league_fm26 import norm, official_name

ROOT = Path(__file__).resolve().parents[1]
FM_FILE = Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')


def tokens(name):
    return tuple(norm(name).split())


def candidate_key(name):
    t = tokens(name)
    return (t[0], t[-1]) if len(t) >= 2 else None


def family_tokens(official):
    # Premier League registration uses "Surname, Given", often with extended
    # Portuguese/Spanish legal family names.
    family = official.split(',', 1)[0] if ',' in official else official_name(official).split()[-1]
    return set(tokens(family))


def label_match(off_name, names):
    """Name evidence: exact, expanded middle names, or explicit aliases.

    Never skip the family/first name anchors just because a Reep entity is
    well-known. An expanded legal name may contain or omit middle tokens.
    """
    name = tokens(official_name(off_name))
    if len(name) < 2:
        return None
    for variant in names:
        t = tokens(variant)
        if t == name:
            return 'exact_reep_name_or_alias'
    for variant in names:
        t = tokens(variant)
        if len(t) >= 2 and t[0] == name[0] and t[-1] == name[-1]:
            return 'compatible_reep_given_and_family_with_middle_variation'
    # Legal family name may contain multiple parts, whereas a commonly used
    # Reep alias may contain only one, e.g. Bruno Guimarães vs the long official
    # Guimaraes Rodriguez Moura. Require a substantial surname component.
    # This is weaker than a full name match and still needs native corroboration.
    significant_family = {v for v in family_tokens(off_name) if len(v) >= 6}
    if significant_family:
        for variant in names:
            t = tokens(variant)
            if len(t) >= 2 and t[0] == name[0] and significant_family.intersection(t[1:]):
                return 'partial_legal_family_with_matching_given_name'
    return None


def native_name_consistent(official, native_name):
    """Reject Reep FM edges contradicted by the actual on-disk person."""
    q = tokens(official_name(official))
    native = tokens(native_name)
    if len(q) < 2 or len(native) < 2 or native[0] != q[0]:
        return False
    return bool((set(q[1:]) | family_tokens(official)).intersection(native[1:]))


def read_local_fm(path, wanted):
    """Read all native IDs (including low CA and staff-kind) without filtering."""
    found = defaultdict(list)
    with path.open(encoding='utf-8-sig', newline='') as file:
        for row in csv.DictReader(file):
            uid = row.get('uid', '')
            if uid not in wanted:
                continue
            found[uid].append({
                'uid': uid, 'name': row.get('name', ''), 'dob': row.get('dob', ''),
                'kind': row.get('kind', ''), 'attr_source': row.get('attr_source', ''),
                'ca': row.get('ca', ''), 'pa': row.get('pa', ''),
                'attributes': {key.removeprefix('attr_'): float(value)
                               for key,value in row.items() if key.startswith('attr_')
                               and value and value.replace('.', '', 1).isdigit()},
            })
    return found


def reep_index(db):
    """Index every name for entities with an FM player bridge, once."""
    index = defaultdict(set)
    by_reep = defaultdict(lambda: {'uids': set(), 'names': set(), 'rungs': set()})
    rows = db.execute('''
        SELECT b.reep_id, b.external_id, b.rung, e.label
          FROM bridges b
          JOIN entities e ON e.reep_id = b.reep_id
         WHERE b.provider = 'fm' AND b.namespace = 'player'
           AND e.entity_type = 'player'
    ''').fetchall()
    for rid, uid, rung, label in rows:
        item = by_reep[rid]
        item['uids'].add(str(uid))
        item['rungs'].add(str(rung))
        if label:
            item['names'].add(label)
            key = candidate_key(label)
            if key:
                index[key].add(rid)
    # All aliases for registered Reep entities are eligible, including
    # normalized/transliterated provider spellings. Single initials cannot
    # replace the official player's full given name by themselves.
    aliases = db.execute('''
        SELECT a.reep_id, a.alias
        FROM aliases a
        JOIN (SELECT DISTINCT reep_id FROM bridges
              WHERE provider='fm' AND namespace='player') b USING (reep_id)
    ''').fetchall()
    for rid, alias in aliases:
        if rid not in by_reep or not alias:
            continue
        by_reep[rid]['names'].add(alias)
        key = candidate_key(alias)
        if key:
            index[key].add(rid)
    return index, by_reep


def resolve_one(official, index, entries, fm):
    name = official_name(official)
    key = candidate_key(name)
    rid_set = set(index.get(key, set()))
    for family in family_tokens(official):
        if len(family) >= 6:
            rid_set.update(index.get((tokens(name)[0], family), set()))
    candidates = []
    for rid in rid_set:
        item = entries[rid]
        kind = label_match(official, item['names'])
        if not kind:
            continue
        if len(item['uids']) != 1:
            candidates.append({'reep_id': rid, 'name_evidence': kind,
                               'status': 'multiple_fm_uids_on_reep_entity',
                               'fm_uids': sorted(item['uids'])})
            continue
        uid = next(iter(item['uids']))
        native = fm.get(uid, [])
        if len(native) > 1:
            candidates.append({'reep_id': rid, 'fm_uid': uid,
                               'name_evidence': kind,
                               'status': 'duplicate_native_fm_uid'})
            continue
        native_row = native[0] if native else None
        if native_row and not native_name_consistent(official, native_row['name']):
            candidates.append({'reep_id': rid, 'fm_uid': uid,
                               'name_evidence': kind,
                               'status': 'native_fm_name_contradicts_reep_alias',
                               'fm_name': native_row['name']})
            continue
        # The Reep alias ties the official person's name to this Reep entity;
        # the typed bridge ties that entity to the FM UID. If the native FM
        # name also agrees, this is extra corroboration, not a prerequisite.
        candidates.append({
            'reep_id': rid, 'fm_uid': uid, 'name_evidence': kind,
            'status': 'native_record_present' if native_row else 'reep_uid_not_in_local_fm',
            'reep_bridge_rungs': sorted(item['rungs']),
            'fm_name': native_row['name'] if native_row else None,
            'fm_dob': native_row['dob'] if native_row else None,
            'fm_kind': native_row['kind'] if native_row else None,
            'fm_attr_source': native_row['attr_source'] if native_row else None,
            'native_row': native_row,
        })
    candidates.sort(key=lambda x: (x.get('fm_uid') or '', x['reep_id']))
    # Any same-name Reep collision is an ambiguity. Do not pick the higher
    # rated/well-known FM record or ignore a conflict with staff-kind records.
    if len(candidates) == 1 and candidates[0]['status'] in ('native_record_present', 'reep_uid_not_in_local_fm'):
        return candidates[0], candidates
    return None, candidates


def resolve_roster(roster, index, entries, fm):
    if roster.get('roster_scope') != 'full_league' or len(roster['teams']) != 20:
        raise ValueError('Reep EPL resolver requires all 20 official EPL clubs')
    result = json.loads(json.dumps(roster))
    counts = Counter()
    global_id_owners = defaultdict(list)
    for club, team in result['teams'].items():
        for p in team['players']:
            original = p.get('fm_uid')
            selected, options = resolve_one(p['official_name'], index, entries, fm)
            if not selected:
                p['reep_resolution_status'] = ('ambiguous_reep_alias' if options
                                               else 'not_found_by_reep_given_and_family')
                p['reep_candidates'] = options[:10]
                counts[p['reep_resolution_status']] += 1
                if original:
                    p['legacy_name_only_fm_uid'] = str(original)
                    native = fm.get(str(original), [])
                    # Even without a Reep resolution, an old FM assignment
                    # must agree with the on-disk person's first/family name.
                    if len(native) == 1 and not native_name_consistent(p['official_name'], native[0]['name']):
                        p.pop('fm_uid', None)
                        p.pop('attributes', None)
                        p.pop('ca', None)
                        p.pop('pa', None)
                        p['fm_attributes_verified'] = False
                        p['fm_player_record_verified'] = False
                        p['identity_verification'] = 'legacy_uid_contradicted_by_native_name'
                        counts['quarantined_inconsistent_old_fm_uid'] += 1
                    else:
                        p['identity_verification'] = 'legacy_name_only_unverified'
                continue
            rid, uid = selected['reep_id'], selected['fm_uid']
            if original and str(original) != uid:
                p['legacy_name_only_fm_uid'] = str(original)
                counts['corrected_old_name_only_uid'] += 1
            p['fm_uid'] = uid
            p['fm_name'] = selected.get('fm_name')
            p['fm_kind'] = selected.get('fm_kind')
            p['attr_source'] = selected.get('fm_attr_source')
            p['reep_id'] = rid
            p['reep_resolution_status'] = selected['status']
            p['identity_verification'] = 'unique_reep_alias_with_fm_provider_bridge_roster_name_not_dob_verified'
            p['match_method'] = 'reep_'+selected['name_evidence']
            p['fm_player_record_verified'] = selected.get('fm_kind') == 'player'
            p['fm_attributes_verified'] = selected.get('fm_kind') == 'player' and selected.get('fm_attr_source') == 'db'
            p['source_quality'] = ('db_observed_but_official_identity_provisional'
                                   if p['fm_attributes_verified'] else 'no_verified_individual_attributes')
            # The individual attribute block belongs to the resolved native
            # FM UID. A staff-kind or synthesized block cannot be promoted.
            native = selected.get('native_row')
            p['attributes'] = native['attributes'] if native and p['fm_attributes_verified'] else {}
            p['ca'] = float(native['ca']) if native and p['fm_attributes_verified'] and native['ca'] else None
            p['pa'] = float(native['pa']) if native and p['fm_attributes_verified'] and native['pa'] else None
            global_id_owners[uid].append((club,p))
            counts['reep_fm_resolved'] += 1
            if original is None:
                counts['recovered_from_old_unmatched'] += 1
    # Name matching can produce identity collisions across different clubs;
    # keep the old ID evidence but drop the unsafe new promoted assignment.
    for uid, owners in global_id_owners.items():
        if len(owners) <= 1:
            continue
        counts['collision_fm_uid_count'] += 1
        for club, person in owners:
            person['reep_resolution_status'] = 'duplicate_fm_person_multiple_official_clubs'
            person['reep_conflicted_uid'] = person.pop('fm_uid')
            person.pop('attributes', None)
            person.pop('ca', None)
            person.pop('pa', None)
            person['fm_attributes_verified'] = False
            person['fm_player_record_verified'] = False
            person['identity_verification'] = 'collision_unverified'
            counts['colliding_registration_rows_quarantined'] += 1
    for club, block in result['teams'].items():
        members = block['players']
        block['matched_count'] = sum(bool(p.get('fm_uid')) for p in members)
        block['matched_25'] = sum(bool(p.get('fm_uid')) and p['registration'] == 'registered' for p in members)
        block['matched_u21'] = sum(bool(p.get('fm_uid')) and p['registration'] == 'u21' for p in members)
        block['unmatched'] = [p['official_name'] for p in members if not p.get('fm_uid')]
    result['resolution_summary'] = dict(sorted(counts.items()))
    result['roster_matching_pipeline'] = 'Reep-first verified provider bridge and aliases, legacy FM fallback provisional'
    result['reep_bridge_release_stamp'] = None
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--roster', type=Path, default=ROOT/'data/pl/squads_2026-10-11_full.json')
    parser.add_argument('--fm', type=Path, default=FM_FILE)
    parser.add_argument('--active', type=Path, default=ROOT/'data/identity/reep/active.json')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('refusing to overwrite earlier research identity roster')
    import duckdb
    info = json.loads(args.active.read_text())
    with duckdb.connect(info['path'], read_only=True) as db:
        index, entries = reep_index(db)
    roster = json.loads(args.roster.read_text())
    wanted = {uid for item in entries.values() for uid in item['uids']}
    wanted.update(str(p['fm_uid']) for team in roster['teams'].values()
                  for p in team['players'] if p.get('fm_uid'))
    native = read_local_fm(args.fm, wanted)
    result = resolve_roster(roster, index, entries, native)
    result['reep_bridge_release_stamp'] = info['stamp']
    result['reep_bridge_sha256'] = info['sha256']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({'clubs':len(result['teams']),
                      'registered':sum(v['registered_count'] for v in result['teams'].values()),
                      'fm_uid_attached':sum(v['matched_count'] for v in result['teams'].values()),
                      'stats':result['resolution_summary']}, indent=2))


if __name__ == '__main__':
    main()
