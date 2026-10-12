#!/usr/bin/env python3
"""Attach genuinely observed, strictly pre-kickoff EPL participation to research inputs.

Identity is audited: exact normalized full names plus provider club/ID only.
Provider athlete ID is never silently equated with an FM UID. No fuzzy matching,
future event rows, guessed substitutions, or retrospectively fetched snapshots.
"""
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone, date
import hashlib
import json
import math
from pathlib import Path
import re
import unicodedata

from espn_feed import parse_utc, stable_json

CLUB_NAMES = {
    'AFC Bournemouth': 'Bournemouth', 'Brighton & Hove Albion': 'Brighton',
    'Coventry City': 'Coventry', 'Crystal Palace': 'Crystal Palace',
    'Hull City': 'Hull', 'Ipswich Town': 'Ipswich',
    'Manchester City': 'Man City', 'Manchester United': 'Man United',
    'Newcastle United': 'Newcastle', 'Nottingham Forest': "Nott'm Forest",
    'Tottenham Hotspur': 'Tottenham', 'West Ham United': 'West Ham',
    'Wolverhampton Wanderers': 'Wolves', 'Leeds United': 'Leeds',
    'Leicester City': 'Leicester', 'Sheffield United': 'Sheffield United',
    'AFC Sunderland': 'Sunderland',
}


def name_key(value):
    # Lower-case NFKD, remove combining accents and punctuation; never tokenize
    # to first+last alone (different people can share short names).
    value = ''.join(ch for ch in unicodedata.normalize('NFKD', value)
                    if not unicodedata.combining(ch)).casefold()
    return ' '.join(re.findall(r'[a-z0-9]+', value))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_raw_archive(espn):
    hashes = espn.get('raw_source_sha256')
    if not isinstance(hashes, dict) or not hashes:
        raise ValueError('ESPN snapshot has no independently retained raw summaries')
    for path, digest in hashes.items():
        if sha(Path(path)) != digest:
            raise ValueError(f'raw ESPN source mutated or missing: {path}')


def club(value):
    return CLUB_NAMES.get(value, value)


def get_roster_aliases(squad):
    by_uid = defaultdict(set)
    for team_name, team in squad['teams'].items():
        for person in team['players']:
            uid = person.get('fm_uid')
            if uid:
                for raw in (person.get('official_name'), person.get('fm_name')):
                    if raw:
                        by_uid[(str(uid), team_name)].add(name_key(raw))
    return by_uid


def resolve_roster_identity(official, fm_name, team, by_team_name, by_team_people):
    # Official PL registered names are often "Surname, Given Middle Name*",
    # whereas ESPN publishes "Given Surname". Plain string equality therefore
    # loses most legitimate athletes. Use only an unambiguous same-club first
    # given-name AND surname-token pair, never edit distance or FM CA.
    names = {name_key(value) for value in (official, fm_name) if value}
    direct = set().union(*(by_team_name.get((team, n), set()) for n in names))
    if len(direct) == 1:
        return next(iter(direct)), 'exact_full_name_and_club'
    if len(direct) > 1:
        return None, 'ambiguous_full_name'
    if ',' not in official:
        return None, 'unmatched_no_official_name_structure'
    last, first = official.replace('*','').split(',', 1)
    given = {t for t in name_key(first).split() if len(t) >= 3}
    family = {t for t in name_key(last).split() if len(t) >= 3}
    if not given or not family:
        return None, 'missing_legal_given_or_family_tokens'
    ids = set()
    for ident, provider_name in by_team_people[team].items():
        tokens = name_key(provider_name).split()
        if len(tokens) >= 2 and (given & set(tokens[:-1])) and (family & set(tokens[-2:])):
            ids.add(ident)
    if len(ids) == 1:
        return next(iter(ids)), 'unique_legal_given_family_token_pair_and_club'
    return None, 'ambiguous_or_unmatched_legal_tokens'


def provisional_official_roster_id(as_of, team, name):
    season = as_of.year if as_of.month >= 7 else as_of.year - 1
    key = f'pl-roster:{season}:{team}:{name.casefold()}'.encode('utf-8')
    return int.from_bytes(hashlib.sha256(key).digest()[:8], 'big') | (1 << 63)


def validate_snapshot(base, espn, roster, cutoff_utc, kickoff_utc):
    if base.get('schema') != 'math_shot_goal_v1' or espn.get('schema') != 'espn_player_appearances_v1':
        raise ValueError('incompatible feature or source schema')
    retrieval = parse_utc(espn['retrieved_completed_at_utc'])
    if retrieval > cutoff_utc:
        raise ValueError('ESPN raw data were retrieved AFTER the forecast cutoff; no historical backdating')
    if kickoff_utc <= cutoff_utc:
        raise ValueError('fixture kickoff must strictly follow the documented data cutoff')
    if len(base['fixtures']) != 1:
        raise ValueError('match identity/kickoff gate requires a single fixture')
    fixture = base['fixtures'][0]
    if kickoff_utc.date() != date.fromisoformat(fixture['date']):
        raise ValueError('forecast kickoff date disagrees with fixture file')
    if date.fromisoformat(base['as_of']) > cutoff_utc.date():
        raise ValueError('base model inputs are later than the actual cutoff')
    if date.fromisoformat(roster['as_of']) > cutoff_utc.date():
        raise ValueError('official roster is from after the forecast cutoff')
    if date.fromisoformat(espn['as_of_exclusive_calendar_date']) > cutoff_utc.date():
        raise ValueError('future season source partition')
    for match in espn['matches']:
        event_date = parse_utc(match['kickoff_utc'])
        if event_date >= cutoff_utc:
            raise ValueError('ESPN historical fixture starts on or after prediction cutoff')
        if match['date'] >= fixture['date']:
            raise ValueError('ESPN source accidentally includes the prediction fixture')


def verify_future_schedule(board, fixture, kickoff_utc):
    if board.get('leagues', [{}])[0].get('slug') != 'eng.1':
        raise ValueError('future fixture schedule must come from the ESPN EPL endpoint')
    matches = []
    for event in board.get('events', []):
        comps = event.get('competitions', [])
        if len(comps) != 1:
            continue
        teams = {team.get('homeAway'): club(team['team']['displayName'])
                 for team in comps[0].get('competitors', [])}
        if teams.get('home') == fixture['home'] and teams.get('away') == fixture['away']:
            if parse_utc(event['date']) != kickoff_utc:
                raise ValueError('ESPN kickoff timestamp differs from requested event')
            if event.get('status', {}).get('type', {}).get('completed'):
                raise ValueError('ESPN game already completed; this is not a forecast')
            matches.append(str(event['id']))
    if len(matches) != 1:
        raise ValueError('future fixture is not uniquely verified by the ESPN scoreboard')
    return matches[0]


def merge(base, espn, roster, cutoff_utc, kickoff_utc, observed_only=False):
    validate_snapshot(base, espn, roster, cutoff_utc, kickoff_utc)
    known = get_roster_aliases(roster)
    by_team_name = defaultdict(set)
    by_team_people = defaultdict(dict)
    by_name = defaultdict(set)
    stats = defaultdict(lambda: {'starts': 0, 'appearances': 0, 'minutes': 0.0,
                                 'starts_minutes': 0.0, 'shots': 0.0, 'goals': 0.0})
    matches_by_club = Counter()
    population_starts = population_start_minutes = 0.0
    population_shots = population_minutes = 0.0
    for match in espn['matches']:
        clubs = [club(match['home']), club(match['away'])]
        if len(set(clubs)) != 2:
            raise ValueError('duplicate provider fixture clubs')
        matches_by_club.update(clubs)
        used = set()
        for p in match['players']:
            ident = str(p['espn_id'])
            team = club(p['club'])
            if team not in clubs:
                raise ValueError('provider player club outside fixture')
            if (team, ident) in used:
                raise ValueError('duplicate provider person within a match')
            used.add((team, ident))
            key = name_key(p['name'])
            by_team_name[(team, key)].add(ident)
            if ident in by_team_people[team] and by_team_people[team][ident] != p['name']:
                raise ValueError('same provider ID has conflicting names')
            by_team_people[team][ident] = p['name']
            by_name[key].add(ident)
            s = stats[(team, ident)]
            if p['started']:
                s['starts'] += 1
                s['starts_minutes'] += float(p['minutes'])
                population_starts += 1
                population_start_minutes += float(p['minutes'])
            if p['appeared']:
                if p['shots'] is None:
                    raise ValueError('played player has no measured shots')
                s['appearances'] += 1
                s['shots'] += float(p['shots'])
                s['goals'] += float(p['goals'])
                s['minutes'] += float(p['minutes'])
                population_shots += float(p['shots'])
                population_minutes += float(p['minutes'])
    if population_starts < 100 or population_minutes <= 0:
        raise ValueError('insufficient completed observed ESPN player matches')
    typical_start_minutes = population_start_minutes / population_starts
    mean_shots_per90 = 90.0 * population_shots / population_minutes
    output = json.loads(json.dumps(base))
    lookup = []
    mapping_counts = Counter()
    occupied = set()
    def add_observed(p, espn_id, method):
        team = p['club']
        if (team, espn_id) in occupied:
            raise ValueError(f'ESPN athlete {espn_id} mapped to multiple roster identities for {team}')
        occupied.add((team, espn_id))
        data = stats[(team, espn_id)]
        n = matches_by_club[team]
        starts = data['starts']
        appearances = data['appearances']
        if n < 2 or starts > appearances or appearances > n:
            raise ValueError(f'invalid/insufficient ESPN match exposure for {team}')
        mapping_counts[method] += 1
        p['espn_id'] = espn_id
        p['espn_identity_status'] = method
        p['start_probability'] = (starts + 0.5) / (n + 1.0)
        p['bench_appearance_probability'] = (appearances - starts + 0.5) / (n - starts + 1.0)
        p['expected_minutes_if_start'] = min(
            90.0, (data['starts_minutes'] + 4.0 * typical_start_minutes) / (starts + 4.0))
        exposure = data['minutes'] / 90.0
        p['shots_per90'] = (data['shots'] + 2.0 * mean_shots_per90) / (exposure + 2.0)
        p['shot_rate_kind'] = 'ESPN_measured_exposure_empirical_Bayes_2_matches'
        p['xg_rate_kind'] = 'FM_proxy_not_observed_xG' if p.get('fm_kind') != 'none' else (
            'observed_league_goal_per_shot_proxy_not_xG')
        p['espn_previous_team_fixtures'] = n
        p['espn_previous_starts'] = starts
        p['espn_previous_appearances'] = appearances
        p['espn_previous_minutes'] = data['minutes']
        p['espn_previous_shots'] = data['shots']
        return data
    roster_index = {
        (str(person['fm_uid']), team): person
        for team, group in roster['teams'].items()
        for person in group['players'] if person.get('fm_uid')
    }
    for p in output['players']:
        uid = str(p['id'])
        team = p['club']
        names = known.get((uid, team), set())
        if not names:
            raise ValueError('joined input player lacks a registered FM identity')
        roster_person = roster_index[(uid, team)]
        espn_id, method = resolve_roster_identity(
            roster_person['official_name'], roster_person.get('fm_name'),
            team, by_team_name, by_team_people)
        if espn_id is None:
            mapping_counts['unresolved_no_unique_provider_identity'] += 1
            p['espn_identity_status'] = 'unresolved'
            lookup.append({'fm_uid': uid, 'club': team, 'status': 'unresolved',
                           'reason': method})
            continue
        data = add_observed(p, espn_id, method)
        lookup.append({'fm_uid': uid, 'espn_id': espn_id, 'club': team,
                       'status':p['espn_identity_status'], 'n_starts':data['starts'],
                       'n_appearances':data['appearances']})
    # The official registered roster includes genuine players unmatched by FM.
    # Admit only those with one uniquely matched observed ESPN identity, keeping
    # a stable *provisional* namespace and no invented FM attributes.
    for team, group in roster['teams'].items():
        for person in group['players']:
            if person.get('fm_uid') or person.get('loaned_out') or '(Loan)' in person['official_name']:
                continue
            espn_id, method = resolve_roster_identity(
                person['official_name'], person.get('fm_name'), team,
                by_team_name, by_team_people)
            if espn_id is None or (team, espn_id) in occupied:
                continue
            if stats[(team, espn_id)]['appearances'] == 0:
                continue
            uid = provisional_official_roster_id(date.fromisoformat(roster['as_of']),
                                                  team, person['official_name'])
            position = (person.get('historical_position') or '').casefold()
            provider_position = next((p['position'] for m in espn['matches']
                                      for p in m['players'] if club(p['club']) == team
                                      and str(p['espn_id']) == espn_id), '')
            keeper = 'goalkeeper' in position or (
                not position and 'goalkeeper' in provider_position.casefold())
            candidate = {
                'id':uid,'club':team,'minutes_prior':500.0,
                'shots_per90':mean_shots_per90,
                'xg_per90':max(0.01,mean_shots_per90 * 0.10),
                'is_goalkeeper':keeper, 'fm_source':'unverified_official_roster',
                'fm_kind':'none','fm_finishing':None,'fm_ca':None,
                'identity_namespace':'official_roster_provisional',
                'official_name':person['official_name'],
                'position_source':'prior_official_position_or_ESPN',
                'fm_attribute_verified':False,
            }
            add_observed(candidate,espn_id,method)
            output['players'].append(candidate)
            lookup.append({'official_provisional_id':str(uid), 'espn_id':espn_id,
                           'club':team,'status':method,'fm_uid':None})
            mapping_counts['observed_roster_unmatched_by_FM_admitted'] += 1
    by_club = Counter(p['club'] for p in output['players'])
    mapped_clubs = Counter(p['club'] for p in output['players'] if p.get('espn_id'))
    output['feature_cutoff_utc'] = cutoff_utc.isoformat()
    output['fixture_kickoff_utc'] = kickoff_utc.isoformat()
    output['feature_source_provenance'] = {
        'provider':espn['provider'],
        'retrieved_started_at_utc':espn['retrieved_started_at_utc'],
        'retrieved_completed_at_utc':espn['retrieved_completed_at_utc'],
        'historical_completed_matches':len(espn['matches']),
        'identity_reconciliation':dict(mapping_counts),
        'per_club_matched':dict(mapped_clubs), 'per_club_candidates':dict(by_club),
        'source_player_shots_mean_per90':mean_shots_per90,
        'source_start_minutes_mean':typical_start_minutes,
        'full_selection_coverage_for_forecast_clubs':all(
            mapped_clubs[team] == by_club[team]
            for team in (output['fixtures'][0]['home'], output['fixtures'][0]['away'])),
        'not_observed': ['player_xG','injuries','individual_FM_to_ESPN_provider_ID_crosswalk_for_unmatched',
                         'same-day result release timestamps'],
    }
    if observed_only:
        # This is an explicitly conditional mathematical *scenario*, not a
        # claim that never-observed/newly transferred players are unavailable.
        output['players'] = [player for player in output['players']
                             if player.get('espn_id') is not None]
        for team in (output['fixtures'][0]['home'], output['fixtures'][0]['away']):
            club_players = [person for person in output['players'] if person['club'] == team]
            if sum(person['is_goalkeeper'] for person in club_players) < 1 or sum(
                    not person['is_goalkeeper'] for person in club_players) < 10:
                raise ValueError(f'cannot run observed-only scenario: incomplete keeper/XI for {team}')
        output['feature_source_provenance']['observed_only_scenario'] = True
        output['feature_source_provenance']['not_a_full_eligible_roster'] = True
        output['feature_source_provenance']['scenario_caution'] = (
            'Players unseen in this partial ESPN archive have been excluded solely '
            'for a conditional computational validation. Never publish as lineup probabilities.')
    else:
        output['feature_source_provenance']['observed_only_scenario'] = False
    return output, lookup


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-input', type=Path, required=True)
    parser.add_argument('--espn', type=Path, required=True)
    parser.add_argument('--squads', type=Path, required=True)
    parser.add_argument('--forecast-cutoff-utc', required=True)
    parser.add_argument('--kickoff-utc', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--crosswalk-report', type=Path, required=True)
    parser.add_argument('--schedule-board', type=Path, required=True,
                        help='ESPN future schedule response freezing home/away and kickoff')
    parser.add_argument('--research-observed-only', action='store_true',
                        help='explicit conditional integration smoke: exclude unverified candidates')
    args = parser.parse_args()
    if args.output.exists() or args.crosswalk_report.exists():
        parser.error('refusing to overwrite frozen research inputs or crosswalk report')
    base = json.loads(args.base_input.read_text())
    espn = json.loads(args.espn.read_text())
    verify_raw_archive(espn)
    squads = json.loads(args.squads.read_text())
    cutoff = parse_utc(args.forecast_cutoff_utc)
    kickoff = parse_utc(args.kickoff_utc)
    if cutoff > datetime.now(timezone.utc):
        parser.error('cannot fabricate a future feature cutoff')
    board = json.loads(args.schedule_board.read_text())
    if not args.base_input.is_file():
        parser.error('baseline as-of input does not exist')
    event_id = verify_future_schedule(board, base['fixtures'][0], kickoff)
    output, mapping = merge(base, espn, squads, cutoff, kickoff,
                            observed_only=args.research_observed_only)
    output['sources'][str(args.espn)] = sha(args.espn)
    output['sources'][str(args.squads)] = sha(args.squads)
    output['sources'][str(args.base_input)] = sha(args.base_input)
    output['sources'][str(args.schedule_board)] = sha(args.schedule_board)
    output['feature_source_provenance']['verified_future_espn_event_id'] = event_id
    output['feature_source_provenance']['verified_future_schedule_sha256'] = sha(args.schedule_board)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.crosswalk_report.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(stable_json(output))
    args.crosswalk_report.write_text(stable_json({'schema':'fm_espn_crosswalk_v1',
        'created_at_utc':datetime.now(timezone.utc).isoformat(),
        'feature_cutoff_utc':cutoff.isoformat(),
        'matches_sha256':sha(args.espn),
        'coverage':output['feature_source_provenance'],'people':mapping}))
    print('Attached ESPN observed participation:',json.dumps(output['feature_source_provenance'],
        indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
