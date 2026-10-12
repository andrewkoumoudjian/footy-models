#!/usr/bin/env python3
"""Freeze dated, provenance-checked research inputs using the existing PL/FM adapter.

The actual XI, target-match player events and target-match score are never inputs.
Runs entirely offline with stdlib. Preserves FM database-vs-derived-vs-staff status.
"""
import argparse
import csv
import hashlib
import json
from datetime import date, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_FM = Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv')
ALIASES = {'Nottingham Forest': "Nott'm Forest", 'Hull City': 'Hull',
           'Newcastle United': 'Newcastle', 'Manchester City': 'Man City',
           'Manchester United': 'Man United', 'Liverpool FC': 'Liverpool'}


def sha256(path):
    h = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def read_table(path, delimiter=','):
    with path.open(encoding='utf-8-sig', newline='') as handle:
        return list(csv.DictReader(handle, delimiter=delimiter))


def numeric(value, column, *, allow_missing=False):
    if (value is None or str(value).strip() == '') and allow_missing:
        return None
    try:
        number = float(value)
    except (ValueError, TypeError) as exc:
        raise ValueError(f'invalid numeric {column}: {value!r}') from exc
    if not __import__('math').isfinite(number) or number < 0:
        raise ValueError(f'invalid nonnegative {column}: {value!r}')
    return number


def make_inputs(*, fm_file, player_file, history_file, fixtures_file,
                results_file, raw_dir, as_of, fm_effective, squads_file=None):
    as_of_day = date.fromisoformat(as_of)
    if date.fromisoformat(fm_effective) > as_of_day:
        raise ValueError('FM vintage is after pre-match cutoff')
    players = read_table(player_file)
    if not players:
        raise ValueError('empty player table')
    required = {'player_id', 'name', 'team', 'minutes', 'shots_p90', 'xg_p90', 'last_listed_date'}
    if not required.issubset(players[0]):
        raise ValueError(f'player adapter lacks {sorted(required - set(players[0]))}')
    ids = [row['player_id'] for row in players]
    if len(set(ids)) != len(ids):
        raise ValueError('ambiguous duplicate player IDs in joined adapter')
    wanted = set(ids)
    fm = {}
    with fm_file.open(encoding='utf-8-sig', newline='') as handle:
        reader = csv.DictReader(handle)
        needed = {'uid', 'attr_source', 'attr_Reflexes', 'attr_Handling', 'attr_Finishing'}
        if not needed.issubset(reader.fieldnames or []):
            raise ValueError('FM raw export lacks essential identity and source fields')
        for row in reader:
            uid = row['uid']
            if uid in wanted:
                if uid in fm:
                    raise ValueError(f'duplicate raw FM UID {uid}')
                fm[uid] = row
    if set(fm) != wanted:
        raise ValueError(f'FM raw join coverage failed: missing {len(wanted - set(fm))} IDs')
    if squads_file is None:
        squads_file = ROOT / 'data/pl' / f'squads_{as_of}.json'
    roster_snapshot = json.loads(squads_file.read_text())
    snapshot_day = date.fromisoformat(roster_snapshot.get('as_of', '1970-01-01'))
    if snapshot_day > as_of_day or (as_of_day - snapshot_day).days > 7:
        raise ValueError('registered-squad snapshot is future-dated or older than seven days')
    registered = {}
    for club, squad in roster_snapshot.get('teams', {}).items():
        for entry in squad.get('players', []):
            uid = entry.get('fm_uid')
            if not uid:
                continue
            key = (str(uid), club)
            if key in registered:
                raise ValueError(f'duplicate registered FM identity within club: {key}')
            registered[key] = entry
    source_counts = {}
    fm_record_counts = {}
    trusted_attributes = 0
    candidate_rows = []
    for player in players:
        uid = player['player_id']
        record = fm[uid]
        source = record['attr_source']
        kind = record.get('kind', '').strip().lower()
        if kind not in {'player', 'staff'}:
            raise ValueError(f'unknown FM record kind {kind!r} for {uid}')
        if source not in {'db', 'derived', 'staff', 'none'}:
            raise ValueError(f'unknown FM attribute provenance {source!r}')
        source_counts[source] = source_counts.get(source, 0) + 1
        pair = f'{kind}/{source}'
        fm_record_counts[pair] = fm_record_counts.get(pair, 0) + 1
        trusted = kind == 'player' and source == 'db'
        trusted_attributes += int(trusted)
        roster_entry = registered.get((uid, player['team']))
        if roster_entry is None:
            raise ValueError(f'player {uid} is missing from as-of registered club {player["team"]}')
        listed = player['last_listed_date'].strip()
        if listed and date.fromisoformat(listed) > as_of_day:
            raise ValueError(f'future roster observation for {uid}')
        # Verified historical position (from the dated registered-squad record)
        # takes precedence over attribute inference. E.g., an outfield attacker
        # may have 8+ Handling and Reflexes in this FM extraction.
        position = (roster_entry.get('historical_position') or '').strip().lower()
        if position and position not in {'goalkeeper', 'defender', 'midfield', 'attack'}:
            raise ValueError(f'unsupported recorded position {position!r} for {uid}')
        inferred_keeper = (
            trusted and not position
            and all(numeric(record.get('attr_' + field), field, allow_missing=True) is not None
                    and numeric(record.get('attr_' + field), field) >= 11
                    for field in ('Handling', 'Reflexes', 'AerialReach', 'OneOnOnes'))
        )
        keeper = position == 'goalkeeper' or inferred_keeper
        candidate_rows.append({
            'id': int(uid), 'club': player['team'],
            'minutes_prior': numeric(player['minutes'], 'minutes'),
            'shots_per90': numeric(player['shots_p90'], 'shots_p90'),
            'xg_per90': numeric(player['xg_p90'], 'xg_p90'),
            'is_goalkeeper': bool(keeper),
            'position_source': 'prior_position' if position else (
                'verified_fm_gk_profile' if inferred_keeper else 'unknown_outfield'),
            'fm_kind': kind, 'raw_attr_source': source,
            'fm_attribute_verified': trusted,
            # The Rust engine only applies optional FM finishing adjustments to
            # source=db. Never promote a staff/db row to a trusted player rating.
            'fm_source': 'db' if trusted else f'unverified_{kind}_{source}',
            'fm_finishing': numeric(record.get('attr_Finishing'), 'FM finishing',
                                    allow_missing=True) if trusted else None,
            'fm_ca': numeric(record.get('ca'), 'FM current ability',
                             allow_missing=True) if trusted else None,
            # The current Premier League adapter has no observed starts or
            # bench appearances. Never infer this from FM current ability.
            'start_probability': None,
            'expected_minutes_if_start': None,
            'bench_appearance_probability': None,
        })
    fixtures = []
    for row in read_table(fixtures_file, '\t'):
        kickoff = date.fromisoformat(row['date'])
        if kickoff <= as_of_day:
            raise ValueError('fixtures must follow cutoff; never replay completed results as predictions')
        if row['home'] == row['away']:
            raise ValueError('same home and away club')
        fixtures.append({'date': row['date'], 'day': kickoff.toordinal(),
                         'home': row['home'], 'away': row['away']})
    if not fixtures:
        raise ValueError('no future fixtures')
    clubs = {p['club'] for p in candidate_rows}
    for fixture in fixtures:
        for name in (fixture['home'], fixture['away']):
            if name not in clubs:
                raise ValueError(f'fixture club has no dated player pool: {name}')
            team = [p for p in candidate_rows if p['club'] == name]
            if sum(p['is_goalkeeper'] for p in team) < 1 or sum(not p['is_goalkeeper'] for p in team) < 10:
                raise ValueError(f'cannot form one verified goalkeeper and ten outfield players: {name}')
    # The older adapter legitimately inserts proxy 12/11 shots for manually
    # supplied result-only fixtures. For modeling, shots are observations only
    # when matched to an actual football-data.co.uk row with HS and AS fields.
    raw_paths = sorted(raw_dir.glob('*_E0.csv'))
    if not raw_paths:
        raise ValueError('no underlying observed-shots CSV sources')
    observed_shots = {}
    for source in raw_paths:
        for raw in read_table(source):
            try:
                when = datetime.strptime(raw['Date'], '%d/%m/%Y').date()
            except (ValueError, KeyError):
                continue
            # Calendar-day snapshots lack a result-publication clock:
            # never infer that a same-day final result was already known.
            if when >= as_of_day:
                continue
            key = (when, ALIASES.get(raw['HomeTeam'], raw['HomeTeam']),
                   ALIASES.get(raw['AwayTeam'], raw['AwayTeam']))
            if key in observed_shots:
                raise ValueError(f'duplicate raw fixture identity {key}')
            observed_shots[key] = (
                numeric(raw.get('HS'), 'raw home shots', allow_missing=True),
                numeric(raw.get('AS'), 'raw away shots', allow_missing=True),
                numeric(raw.get('FTHG'), 'raw home goals', allow_missing=True),
                numeric(raw.get('FTAG'), 'raw away goals', allow_missing=True),
            )
    history = []
    missing_shot_matches = 0
    for row in read_table(history_file):
        when = date.fromisoformat(row['date'])
        if when >= as_of_day:
            # No verified publication timestamp means same-day scores are not
            # guaranteed known to a pre-match run; exclude them entirely.
            continue
        if row['home'] == row['away']:
            raise ValueError('invalid historical match identity')
        raw_shots = observed_shots.get((when, row['home'], row['away']))
        home_goals = numeric(row['home_goals'], 'home goals')
        away_goals = numeric(row['away_goals'], 'away goals')
        if raw_shots is not None and raw_shots[2] is not None and raw_shots[3] is not None:
            if home_goals != raw_shots[2] or away_goals != raw_shots[3]:
                raise ValueError(f'underlying raw fixture results disagree with adapter: {when}')
        shots = raw_shots[:2] if raw_shots else (None, None)
        if None in shots:
            missing_shot_matches += 1
        history.append({
            'day': when.toordinal(), 'home': row['home'], 'away': row['away'],
            'home_goals': home_goals, 'away_goals': away_goals,
            'home_shots': shots[0], 'away_shots': shots[1],
        })
    if len(history) < 30:
        raise ValueError('insufficient dated historical fixtures for fitted team prior')
    if len({(x['day'], x['home'], x['away']) for x in history}) != len(history):
        raise ValueError('duplicate historical fixtures')
    # Preserve the *existing* team's Championship promotion priors. The prepared
    # results.tsv has exact historical Unix-day dates and canonical club names.
    # Its E1 rows contain goals, not fabricated shot statistics.
    unix_epoch_ordinal = date(1970, 1, 1).toordinal()
    championship = []
    for row in read_table(results_file, '\t'):
        if row['division'] != 'E1':
            continue
        day = int(row['day']) + unix_epoch_ordinal
        if day >= as_of_day.toordinal():
            continue
        if row['home'] == row['away']:
            raise ValueError('invalid Championship match identity')
        championship.append({
            'day': day, 'home': row['home'], 'away': row['away'],
            'home_goals': numeric(row['home_goals'], 'E1 home goals'),
            'away_goals': numeric(row['away_goals'], 'E1 away goals'),
            'home_shots': None, 'away_shots': None,
        })
    if len({(x['day'], x['home'], x['away']) for x in championship}) != len(championship):
        raise ValueError('duplicate Championship fixtures')
    return {
        'schema': 'math_shot_goal_v1', 'as_of': as_of,
        'fm_effective': fm_effective,
        'sources': {str(p): sha256(p) for p in (fm_file, player_file, history_file,
                                              results_file, fixtures_file, squads_file, *raw_paths)},
        'contracts': {
            'player_rates': 'FM26-derived research priors, NOT observed 2026-27 shots or xG',
            'roles': 'verified db goalkeeper heuristics, otherwise role-neutral outfield',
            'history': 'only observations strictly before calendar as_of, per-fixture strictly earlier; no same-day result assumption',
            'shots': 'only raw HS/AS with verified date/team match, never result-only 12/11 placeholders',
            'promoted_clubs': 'Championship E1 match results supplied to existing TeamGoalModel',
            'fm_derived': 'only raw kind=player AND attr_source=db qualify as measured FM traits',
            'registered_squad_as_of': roster_snapshot['as_of'],
            'positions': 'prior registered-squad historical_position first; strict observed-player GK profile fallback',
        },
        'fm_source_counts': source_counts, 'fm_record_kind_counts': fm_record_counts,
        'fm_trusted_attributes': trusted_attributes,
        'missing_shot_matches': missing_shot_matches,
        'history': history, 'championship': championship,
        'players': candidate_rows, 'fixtures': fixtures,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--fm', type=Path, default=DEFAULT_FM)
    parser.add_argument('--players', type=Path, default=ROOT / 'data/pl/player_fm_join.csv')
    parser.add_argument('--history', type=Path, default=ROOT / 'data/pl/asof_team_stats.csv')
    parser.add_argument('--results', type=Path, default=ROOT / 'data/pl/results.tsv')
    parser.add_argument('--fixtures', type=Path, default=ROOT / 'data/pl/fixtures.tsv')
    parser.add_argument('--raw-dir', type=Path, default=ROOT / 'data/pl/raw')
    parser.add_argument('--as-of', default='2026-10-10')
    parser.add_argument('--fm-effective', default='2026-03-01')
    parser.add_argument('--squads', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT / 'data/ml/math_engine/input.json')
    args = parser.parse_args()
    data = make_inputs(fm_file=args.fm, player_file=args.players, history_file=args.history,
                       results_file=args.results, fixtures_file=args.fixtures, raw_dir=args.raw_dir,
                       squads_file=args.squads,
                       as_of=args.as_of, fm_effective=args.fm_effective)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.output.exists():
        raise FileExistsError(f'refusing to overwrite frozen pre-match inputs: {args.output}')
    args.output.write_text(json.dumps(data, sort_keys=True, separators=(',', ':')) + '\n')
    print(f"{args.output}: {len(data['players'])} players, {len(data['history'])} prior matches, "
          f"{len(data['championship'])} Championship games, "
          f"{len(data['fixtures'])} fixtures, sources {data['fm_record_kind_counts']}, "
          f"missing observed shot counts for {data['missing_shot_matches']} history matches")


if __name__ == '__main__':
    main()
