#!/usr/bin/env python3
"""Optional previous-completed-season shot-quality prior for current research.

Combine the last verified FBref player-season identity with the corresponding
Understat full-season xG and shots. The current Rust marked-shot engine keeps
its existing team-goal anchor; this only alters relative *player shot quality*.
FM26 remains the existing fallback for unidentifiable players, without using
2026 FM26 as a historical 2015/16 backtesting input.
"""
import argparse
from collections import Counter, defaultdict
from datetime import date, datetime, timezone
import hashlib
import json
import math
from pathlib import Path

from attach_espn import name_key, parse_utc, sha, get_roster_aliases
from hf_historical import season_end_year


def identity_match(person, historical_records):
    """Conservative full-name or legal given/family token match across clubs.

    Historic transfers are allowed, but *any* same-season ambiguity rejects
    the identity. FBref provider identity and historic club remain explicit.
    """
    legal = person.get('official_name') or ''
    aliases = {name_key(x) for x in (legal,person.get('fm_name') or '')
               if x}
    exact = [
        x for x in historical_records
        if x['normalized_player_name'] in aliases or name_key(x['understat_name']) in aliases
    ]
    if len(exact) == 1:
        return exact[0], 'unique_exact_full_name'
    if len(exact) > 1:
        return None, 'ambiguous_exact_full_name'
    if ',' not in legal:
        return None, 'no_legal_full_name_tokens'
    surname, given = legal.replace('*','').split(',', 1)
    g = {w for w in name_key(given).split() if len(w) >= 3}
    s = {w for w in name_key(surname).split() if len(w) >= 3}
    if not g or not s:
        return None, 'insufficient_legal_given_surname_tokens'
    possible = []
    for row in historical_records:
        for field in ('fbref_name','understat_name'):
            words = name_key(row[field]).split()
            if len(words) >= 2 and g.intersection(words[:-1]) and s.intersection(words[-2:]):
                possible.append(row)
                break
    if len(possible) == 1:
        return possible[0], 'unique_legal_given_surname_tokens'
    return None, 'ambiguous_or_unmatched'


def combine(base, historical, squads, cutoff_utc):
    if base.get('schema') != 'math_shot_goal_v1':
        raise ValueError('not a known Rust research feature contract')
    if historical.get('schema') != 'fbref_understat_season_v1':
        raise ValueError('unknown historical FBref/Understat archive')
    if parse_utc(historical['retrieved_at_utc']) > cutoff_utc:
        raise ValueError('historical FBref/Understat archive was obtained after forecast cutoff')
    if parse_utc(base['feature_cutoff_utc']) > cutoff_utc:
        raise ValueError('research features postdate requested forecast cutoff')
    if date.fromisoformat(squads['as_of']) > cutoff_utc.date():
        raise ValueError('official roster from future relative to cutoff')
    if not base.get('fixture_kickoff_utc') or parse_utc(base['fixture_kickoff_utc']) <= cutoff_utc:
        raise ValueError('cannot apply shot-quality prior after fixture kickoff')
    if len(base.get('fixtures',[])) != 1:
        raise ValueError('requires a uniquely identified upcoming fixture')
    cutoff_day = cutoff_utc.date()
    eligible = [
        x for x in historical['players']
        if date.fromisoformat(x['available_from']) < cutoff_day
    ]
    if not eligible:
        raise ValueError('no entire prior seasons completed before cutoff')
    # Only the most recent fully completed season: avoid training a moving
    # 2026 season-long coefficient from both stale and already-seen targets.
    latest = max(x['season'] for x in eligible)
    eligible = [x for x in eligible if x['season'] == latest]
    if latest != '2024-2025':
        raise ValueError('requires explicit review for new historical source vintage')
    total_shots = sum(x['understat_shots'] for x in eligible)
    total_xg = sum(x['understat_xg'] for x in eligible)
    if total_shots < 2000 or not (0 < total_xg < total_shots):
        raise ValueError('insufficient plausible xG/shots for historical population')
    league_probability = total_xg / total_shots
    current = json.loads(json.dumps(base))
    roster_people = {
        (str(x['fm_uid']), team): x
        for team, club in squads['teams'].items()
        for x in club['players'] if x.get('fm_uid')
    }
    # Provisional official roster IDs have no FM UID, but can have a documented
    # full legal name directly carried in their as-of research candidate row.
    counts = Counter()
    mapping = []
    used = set()
    for player in current['players']:
        if player.get('identity_namespace') == 'official_roster_provisional':
            person = {'official_name':player['official_name']}
        else:
            person = roster_people.get((str(player['id']),player['club']))
        if not person:
            counts['no_official_identity'] += 1
            continue
        matched, method = identity_match(person,eligible)
        if matched is None:
            counts[method] += 1
            continue
        # A historical FBref ID is uniquely linked to *one* contemporary
        # official registration. If duplicated, fail instead of fabricating.
        player_id = matched['fbref_player_id']
        if player_id in used:
            raise ValueError(f'historic FBref person ID matched multiple current players: {player_id}')
        used.add(player_id)
        current_shots = player['shots_per90']
        if not math.isfinite(current_shots) or current_shots < 0:
            raise ValueError('invalid current-season shot intensity')
        shots = matched['understat_shots']
        xg = matched['understat_xg']
        # Conjugate mean over an observed prior of forty pseudo-shots at the
        # EPL population probability: q=(xG+K*p)/(shots+K), K=40.
        # This is a fixed *research* regularizer, not an optimized or
        # historically validated football coefficient.
        posterior = (xg + 40.0 * league_probability) / (shots + 40.0)
        if not 0 < posterior < 1:
            raise ValueError('conditional historical shot quality not probabilistic')
        old_value = player['xg_per90']
        player['xg_per90'] = current_shots * posterior
        player['xg_rate_kind'] = '2024-25_fbref_understat_crossverified_shot_quality_prior'
        player['historical_fbref_id'] = player_id
        player['historical_prior_season'] = latest
        player['historical_previous_club'] = matched['club']
        player['historical_shots'] = shots
        player['historical_xg'] = xg
        player['historical_shot_quality_prior'] = posterior
        player['historical_source_identity_method'] = method
        counts[method] += 1
        mapping.append({
            'current_espn_id':player.get('espn_id'),
            'current_fm_uid':player['id'] if player.get('fm_kind')!='none' else None,
            'historical_fbref_id':player_id,
            'previous_club':matched['club'],'current_club':player['club'],
            'season':latest,'method':method,
            'old_fm_or_proxy_xg_per90':old_value,
            'shrunk_xg_per_shot':posterior,
        })
    current['historical_shot_quality_provenance'] = {
        'historical_source':'aloobun paired FBref/Understat EPL season aggregates',
        'reference_only':'roirani80 2024-25 no club/no ID; never applied',
        'historical_season':latest,'historical_archive_retrieved_at_utc':historical['retrieved_at_utc'],
        'prior_shots':40.0,'league_mean_xg_per_shot':league_probability,
        'matched_identity_counts':dict(counts),
        'current_players':len(current['players']),
        'matched_player_quality_priors':len(mapping),
        'feature_cutoff_utc':cutoff_utc.isoformat(),
        'not_a_validated_model_improvement':True,
    }
    return current, mapping


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--input',type=Path,required=True)
    p.add_argument('--historical',type=Path,required=True)
    p.add_argument('--squads',type=Path,required=True)
    p.add_argument('--cutoff-utc',required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--crosswalk',type=Path,required=True)
    args=p.parse_args()
    if args.output.exists() or args.crosswalk.exists():
        p.error('immutable source-augmented model input already exists')
    cutoff=parse_utc(args.cutoff_utc)
    if cutoff > datetime.now(timezone.utc):
        p.error('cannot invent a future retrieval/forecast cutoff')
    hist=json.loads(args.historical.read_text())
    for path,hash_expected in hist['source_hashes'].items():
        if sha(Path(path))!=hash_expected:
            raise ValueError(f'mutated historical source: {path}')
    incoming=json.loads(args.input.read_text())
    augmented,mapping=combine(
        incoming,hist,json.loads(args.squads.read_text()),cutoff)
    augmented['sources'][str(args.input)]=sha(args.input)
    augmented['sources'][str(args.historical)]=sha(args.historical)
    augmented['sources'][str(args.squads)]=sha(args.squads)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.crosswalk.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(augmented,sort_keys=True,separators=(',',':'))+'\n')
    args.crosswalk.write_text(json.dumps({
        'schema':'fbref_understat_fm2026_shot_quality_links_v1',
        'as_of':cutoff.isoformat(),
        'source_archive_sha256':sha(args.historical),
        'join_report':augmented['historical_shot_quality_provenance'],
        'matches':mapping,
    },sort_keys=True,separators=(',',':'))+'\n')
    print('Historical quality attached:',augmented['historical_shot_quality_provenance'])


if __name__=='__main__':
    main()
