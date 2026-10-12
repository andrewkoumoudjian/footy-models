#!/usr/bin/env python3
"""Dated, source-neutral FBref + Understat historical season-prior archive.

All input tables are **full-season aggregates**. A season's totals become
admissible only from July 1 after that season ended, irrespective of when the
individual match occurred. Never train a 2015/16 replay with 2015/16 totals.
This file deliberately does not touch the current Rust engine or FM data.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from datetime import date, datetime, timezone
import hashlib
import json
import math
from pathlib import Path

from attach_espn import name_key, club

FBREF_REQUIRED = {
    'player', 'player_id', 'pos', 'squad', 'mp', 'starts', 'min',
    'gls', 'season', 'league',
}
UNDERSTAT_REQUIRED = {
    'player_name', 'team', 'league', 'season', 'apps',
    'minutes', 'shots', 'goals', 'xG', 'npxG', 'xA',
}
ROY_REQUIRED = {
    'Player', 'Goals', 'xG', 'xG_Diff', 'xG_Overperformer',
}
FORBIDDEN_PREDICTOR_COLUMNS = {
    'xG_Diff', 'xG_Overperformer', 'npxG_Diff', 'rank_change',
    'Position_Encoded', 'Goals Per 90', 'Non-Penalty Goals Per 90',
}
CLUB_ALIASES = {
    'Manchester Utd': 'Man United', 'Tottenham': 'Tottenham',
    'Wolverhampton Wanderers': 'Wolves',
    'Ipswich': 'Ipswich', 'Ipswich Town': 'Ipswich',
    'Leicester': 'Leicester', 'Leicester City': 'Leicester',
    'West Ham': 'West Ham', 'West Ham United': 'West Ham',
}


def canonical_club(value):
    return CLUB_ALIASES.get(value, club(value))


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as inp:
        for chunk in iter(lambda: inp.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def number(value, field):
    try:
        x = float(value)
    except (ValueError, TypeError) as exc:
        raise ValueError(f'not numeric: {field}={value!r}') from exc
    if not math.isfinite(x) or x < 0:
        raise ValueError(f'negative / nonfinite: {field}={value!r}')
    return x


def data_rows(path, required):
    with path.open(encoding='utf-8-sig', newline='') as inp:
        reader = csv.DictReader(inp)
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f'{path}: missing required columns {sorted(missing)}')
        return list(reader)


def season_end_year(value):
    parts = value.split('-')
    if len(parts) != 2 or not all(part.isdigit() and len(part) == 4 for part in parts):
        raise ValueError(f'invalid full-season marker {value!r}')
    start, end = map(int, parts)
    if end != start + 1:
        raise ValueError(f'non-consecutive season marker {value!r}')
    return end


def archive(fbref_path, understat_path, roy_path, as_of):
    if not isinstance(as_of, date):
        raise TypeError('calendar as-of date required')
    f_rows = data_rows(fbref_path, FBREF_REQUIRED)
    u_rows = data_rows(understat_path, UNDERSTAT_REQUIRED)
    r_rows = data_rows(roy_path, ROY_REQUIRED)
    fbref = defaultdict(list)
    understat = defaultdict(list)
    stats = Counter()
    for row in f_rows:
        if row['league'] != 'EPL':
            continue
        end = season_end_year(row['season'])
        if date(end, 7, 1) > as_of:
            stats['full_season_fbref_rows_after_cutoff'] += 1
            continue
        team = canonical_club(row['squad'])
        if team.lower() in {'squad', '2 squads', '3 squads'} or ',' in team:
            stats['ambiguous_fbref_club'] += 1
            continue
        if not row['player_id'] or not row['player'] or len(row['player_id']) < 7:
            stats['fbref_missing_provider_id'] += 1
            continue
        # FBref provider IDs persist between seasons. Keep them distinct from
        # ESPN, Understat and FM UIDs; the joint link still needs a safe name.
        key = (row['season'], team, name_key(row['player']))
        fbref[key].append(row)
    for row in u_rows:
        if row['league'] != 'EPL':
            continue
        end = season_end_year(row['season'])
        if date(end, 7, 1) > as_of:
            stats['full_season_understat_rows_after_cutoff'] += 1
            continue
        team = canonical_club(row['team'])
        if ',' in team:
            stats['ambiguous_multi_club_understat'] += 1
            continue
        key = (row['season'], team, name_key(row['player_name']))
        understat[key].append(row)
    matches = []
    provider_seen = set()
    for key, f_values in sorted(fbref.items()):
        u_values = understat.get(key, [])
        if len(f_values) != 1 or len(u_values) != 1:
            stats['unresolved_or_ambiguous_provider_crosswalk'] += len(f_values)
            continue
        f, u = f_values[0], u_values[0]
        starts = number(f['starts'], 'FBref starts')
        appearances = number(f['mp'], 'FBref appearances')
        minutes = number(f['min'], 'FBref minutes')
        shots = number(u['shots'], 'Understat shots')
        xg = number(u['xG'], 'Understat xG')
        npxg = number(u['npxG'], 'Understat npxG')
        u_minutes = number(u['minutes'], 'Understat minutes')
        if (starts > appearances or minutes > appearances * 130
                or xg > shots + 1e-7 or npxg > xg + 1e-7
                or shots > max(u_minutes, 1) * 4):
            stats['provider_exposure_or_xg_inconsistency'] += 1
            continue
        ident = (f['season'], f['player_id'], key[1])
        if ident in provider_seen:
            raise ValueError(f'FBref provider ID duplicates same club/season: {ident}')
        provider_seen.add(ident)
        matches.append({
            'season': f['season'],
            'available_from': f"{season_end_year(f['season'])}-07-01",
            'league': 'EPL', 'club': key[1],
            'normalized_player_name': key[2],
            'fbref_player_id': f['player_id'],
            'fbref_name': f['player'],
            'understat_name': u['player_name'],
            'fbref_appearances': appearances,
            'fbref_starts': starts,
            'fbref_minutes': minutes,
            'understat_appearances': number(u['apps'], 'Understat apps'),
            'understat_minutes': u_minutes,
            'understat_shots': shots,
            'understat_xg': xg,
            'understat_npxg': npxg,
            'understat_xa': number(u['xA'], 'Understat xA'),
            'understat_goals': number(u['goals'], 'Understat goals'),
        })
    if not matches:
        raise ValueError('no usable dated FBref/Understat season joins')
    # The Roy file contains five-league season aggregates but no club, stable
    # provider ID, or dates; several columns are literal transforms of target
    # goals/xG. It can inform *QA*, never player matches or predictive priors.
    royalty = Counter()
    for row in r_rows:
        g = number(row['Goals'], 'Roy goals')
        x = number(row['xG'], 'Roy xG')
        d = float(row['xG_Diff'])
        if not math.isfinite(d) or abs((g - x) - d) > 1e-6:
            raise ValueError('Roy xG_Diff target column is inconsistent')
        royalty['verified_goal_minus_xg_label_rows'] += 1
    return {
        'schema': 'fbref_understat_season_v1',
        'retrieved_at_utc': datetime.now(timezone.utc).isoformat(),
        'as_of': as_of.isoformat(),
        'provider_schemas': {
            'fbref': 'aloobun/fbref_understat_combined:data/player_stats/master_player_stats.csv',
            'understat': 'aloobun/fbref_understat_combined:data/understat_xg/understat_xg_data.csv',
            'roy_reference': 'roirani80/fbref-xg-analysis-2024-2025:fbref_cleaned_with_features.csv',
        },
        'source_hashes': {str(p): digest(p) for p in (fbref_path, understat_path, roy_path)},
        'guardrails': [
            'Each entire season becomes observable from July 1 following its final match, never midseason.',
            'Historical 2024-25 full-season data cannot be used to forecast 2024-25 fixtures.',
            'No cross-provider person join on name alone without also matching unique season + league + club.',
            'Original FBref and Understat provider identity namespaces remain distinct.',
            'Roy 2024-25 file has no club/player ID, is multi-league, and contains direct target-derived columns; QA only.',
            'FM26 current attributes cannot be used to backtest pre-FM26 historical seasons.',
            'Stats do not imply player availability or current-season start probabilities.',
        ],
        'diagnostics': {
            **dict(stats),
            'fbref_total_rows': len(f_rows),
            'understat_total_rows': len(u_rows),
            'roy_total_rows': len(r_rows),
            'roy_target_label_rows_verified': royalty['verified_goal_minus_xg_label_rows'],
            'joined_epl_player_seasons': len(matches),
            'joined_by_season': dict(sorted(Counter(row['season'] for row in matches).items())),
        },
        'players': matches,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--fbref', type=Path, required=True)
    p.add_argument('--understat', type=Path, required=True)
    p.add_argument('--roy', type=Path, required=True)
    p.add_argument('--as-of', type=date.fromisoformat, required=True)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    if args.output.exists():
        p.error('refusing to overwrite immutable historical archive')
    result = archive(args.fbref, args.understat, args.roy, args.as_of)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, sort_keys=True, separators=(',', ':')) + '\n')
    print('Historical season archive:', result['diagnostics'])


if __name__ == '__main__':
    main()
