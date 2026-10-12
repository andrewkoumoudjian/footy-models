#!/usr/bin/env python3
"""Freeze provider-ID-based PL match appearances from ESPN match summaries.

Output is immutable; no 2026 player history is retroactively attached to an
older forecast. All I/O and normalization live in this isolated research path.
The ESPN source is also supported by soccerdata. Use responsibly, respect
provider availability and caching, and never request future result labels.
"""
import argparse
import hashlib
import json
import math
import subprocess
import time
from datetime import date, datetime, timezone
from pathlib import Path

LEAGUE = 'eng.1'
BASE = f'https://site.api.espn.com/apis/site/v2/sports/soccer/{LEAGUE}'


def parse_utc(value):
    d = datetime.fromisoformat(value.replace('Z', '+00:00'))
    if d.tzinfo is None or d.utcoffset() is None:
        raise ValueError('time must be timezone-aware, in UTC')
    return d.astimezone(timezone.utc)


def stable_json(obj):
    return json.dumps(obj, sort_keys=True, ensure_ascii=False, separators=(',', ':')) + '\n'


def sha_bytes(body):
    return hashlib.sha256(body).hexdigest()


def stat_value(stats, name):
    for row in stats:
        if row.get('name') == name:
            value = row.get('value')
            if value is None:
                return None
            n = float(value)
            if not math.isfinite(n) or n < 0:
                raise ValueError(f'invalid measured ESPN stat {name}={value}')
            return n
    return None


def normalize_match(summary, event, as_of_date):
    """Require completed fixtures; a new timestamped source is NOT a past snapshot."""
    header = summary['header']
    if str(header['id']) != str(event['id']):
        raise ValueError('ESPN scoreboard-to-summary fixture-ID mismatch')
    comp = header['competitions'][0]
    kickoff = parse_utc(comp['date'])
    if kickoff.date() >= as_of_date:
        raise ValueError('same-day game has no independently verified publish-time clock')
    if not comp['status']['type']['completed']:
        raise ValueError('incomplete result is not historical training data')
    sides = {}
    for roster in summary['rosters']:
        team = roster['team']
        team_id = str(team['id'])
        if team_id in sides:
            raise ValueError('duplicate ESPN team ID')
        players = {}
        for player in roster['roster']:
            athlete = player['athlete']
            identifier = str(athlete['id'])
            if identifier in players:
                raise ValueError('duplicate ESPN athlete in match roster')
            played = bool(player.get('starter')) or bool(player.get('subbedIn'))
            appearances = stat_value(player.get('stats', []), 'appearances')
            if appearances is not None:
                played = played or appearances > 0
            shots = stat_value(player.get('stats', []), 'totalShots')
            goals = stat_value(player.get('stats', []), 'totalGoals')
            players[identifier] = {
                'espn_id': identifier, 'name': athlete['fullName'],
                'club': team['displayName'], 'club_espn_id': team_id,
                'started': bool(player.get('starter')),
                'appeared': played,
                'shots': shots if played else None,
                'goals': goals if played else None,
                'position': athlete.get('position', {}).get('name', ''),
                'start_second': 0.0 if player.get('starter') else None,
                'end_second': 5400.0 if player.get('starter') else None,
            }
        if sum(p['started'] for p in players.values()) != 11:
            raise ValueError('no complete observed 11-starter roster for ESPN fixture')
        sides[team_id] = {'club': team['displayName'], 'players': players}
    if len(sides) != 2:
        raise ValueError('ESPN fixture is not a two-team match')
    verified_subs = 0
    verified_reds = 0
    for evt in summary.get('keyEvents', []):
        event_type = evt.get('type', {}).get('type', '')
        event_text = evt.get('type', {}).get('text', '').casefold()
        if 'red card' in event_text or event_type in {'red-card', 'red-card-2nd-yellow', 'second-yellow'}:
            team_id = str(evt.get('team', {}).get('id', ''))
            participants = evt.get('participants', [])
            if team_id not in sides or not participants:
                raise ValueError('unattributed red-card incident in event history')
            offender = str(participants[0]['athlete']['id'])
            participant = sides[team_id]['players'].get(offender)
            if participant is None or participant['start_second'] is None:
                raise ValueError('red card has no on-field athlete to dismiss')
            at = min(float(evt['clock']['value']), 5400.0)
            if not math.isfinite(at) or at < participant['start_second']:
                raise ValueError('invalid send-off clock or already dismissed player')
            participant['end_second'] = at
            participant['dismissed'] = True
            verified_reds += 1
            continue
        if evt.get('type', {}).get('type') != 'substitution':
            continue
        team_id = str(evt.get('team', {}).get('id', ''))
        participants = evt.get('participants', [])
        if team_id not in sides or len(participants) != 2:
            raise ValueError('substitution event lacks two attributable players')
        incoming = str(participants[0]['athlete']['id'])
        outgoing = str(participants[1]['athlete']['id'])
        players = sides[team_id]['players']
        if incoming not in players or outgoing not in players:
            raise ValueError('substitute identity missing from the declared roster')
        at = float(evt['clock']['value'])
        if not math.isfinite(at) or at < 0:
            raise ValueError('invalid substitution event clock')
        at = min(at, 5400.0)
        if (players[incoming]['started'] or players[incoming]['start_second'] is not None
                or players[outgoing]['end_second'] is None or players[outgoing].get('dismissed')):
            raise ValueError('substitution is not a valid in-out pair')
        players[outgoing]['end_second'] = at
        players[incoming]['start_second'] = at
        players[incoming]['end_second'] = 5400.0
        players[incoming]['appeared'] = True
        verified_subs += 1
    output = []
    for team in sides.values():
        for player in team['players'].values():
            if player['appeared'] and player['start_second'] is None:
                # No fabricated participation exposure when a summary omitted
                # a recorded sub-on event.
                raise ValueError('appearance event lacks a completed substitution clock')
            if not player['appeared'] and player['start_second'] is not None:
                raise ValueError('nonappearance has positive on-field interval')
            player['minutes'] = (player['end_second'] - player['start_second']) / 60.0 if player['appeared'] else 0.0
            if player['minutes'] < 0 or player['minutes'] > 90:
                raise ValueError('invalid observed capped match minutes')
            if player['appeared'] and (player['shots'] is None or player['goals'] is None):
                raise ValueError('played athlete lacks measured shots or goals')
            if player['goals'] is not None and player['shots'] is not None and player['goals'] > player['shots']:
                # Penalty shootout / event feeds with non-shot goals must be
                # handled separately, not forced into a shot-mark process.
                raise ValueError('event-goal counts exceed recorded shots')
            output.append(player)
    home_away = {c['homeAway']: c['team'] for c in comp['competitors']}
    return {
        'espn_match_id': str(event['id']), 'date': kickoff.date().isoformat(),
        'kickoff_utc': kickoff.isoformat(),
        'home': home_away['home']['displayName'],
        'away': home_away['away']['displayName'],
        'home_espn_id': str(home_away['home']['id']),
        'away_espn_id': str(home_away['away']['id']),
        'verified_completed_substitutions': verified_subs,
        'verified_red_cards': verified_reds,
        'players': output,
    }


def request_json(url):
    if not url.startswith(BASE + '/'):
        raise ValueError('unsupported untrusted provider URL')
    # Local provider access in this environment uses curl's CA/proxy settings;
    # urllib defaults currently receive an HTTP 403 from this endpoint.
    request = subprocess.run(['curl', '--fail', '--silent', '--show-error',
                              '--max-time', '20', url], check=True, capture_output=True)
    raw = request.stdout
    if len(raw) > 8_000_000:
        raise ValueError('unexpected oversized provider payload')
    return json.loads(raw)


def fetch(as_of, since, delay, raw_dir):
    # Calendar is sourced live, but a completed fixture is only valid when the
    # *entire calendar date* precedes the declared as_of calendar date.
    calendar = request_json(BASE + '/scoreboard?dates=' + as_of.strftime('%Y%m%d'))
    days = sorted({
        date.fromisoformat(value[:10]) for value in calendar['leagues'][0]['calendar']
        if since <= date.fromisoformat(value[:10]) < as_of
    })
    collected = {}
    incomplete = []
    source_hashes = {}
    for day in days:
        scoreboard = request_json(BASE + '/scoreboard?dates=' + day.strftime('%Y%m%d'))
        board_path = raw_dir / ('scoreboard_' + day.strftime('%Y%m%d') + '.json')
        body = stable_json(scoreboard)
        board_path.write_text(body)
        source_hashes[str(board_path)] = sha_bytes(body.encode())
        for event in scoreboard['events']:
            eid = str(event['id'])
            status = event.get('status', {}).get('type', {})
            if not status.get('completed'):
                incomplete.append({'event_id': eid, 'date': event.get('date'), 'reason': 'not completed'})
                continue
            if parse_utc(event['date']).date() >= as_of:
                continue
            if eid in collected:
                raise ValueError('duplicate ESPN fixture ID across calendar days')
            summary = request_json(BASE + f'/summary?event={eid}')
            raw_path = raw_dir / f'summary_{eid}.json'
            body = stable_json(summary)
            if raw_path.exists():
                raise ValueError('duplicate provider summary ID in source snapshots')
            raw_path.write_text(body)
            source_hashes[str(raw_path)] = sha_bytes(body.encode())
            try:
                collected[eid] = normalize_match(summary, event, as_of)
                collected[eid]['raw_summary_sha256'] = source_hashes[str(raw_path)]
            except (ValueError, KeyError, TypeError) as exc:
                incomplete.append({'event_id': eid, 'date': event.get('date'), 'reason': str(exc)})
            time.sleep(delay)
        time.sleep(delay)
    return list(collected.values()), incomplete, len(days), source_hashes


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--as-of', type=date.fromisoformat, required=True,
                   help='exclusive source date YYYY-MM-DD: no events on or after this day')
    p.add_argument('--since', type=date.fromisoformat, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--raw-dir', type=Path,
                   help='immutable raw ESPN responses; defaults to output path with .raw extension')
    p.add_argument('--delay', type=float, default=0.15,
                   help='sleep between provider calls; API policies remain authoritative')
    args = p.parse_args()
    raw_dir = args.raw_dir or args.output.with_suffix('.raw')
    if args.output.exists() or raw_dir.exists():
        p.error('immutable ESPN research archive already exists')
    if args.since >= args.as_of or args.as_of > datetime.now(timezone.utc).date():
        p.error('invalid interval: neither future nor same-day result fetching is allowed')
    if not math.isfinite(args.delay) or args.delay < 0:
        p.error('invalid request delay')
    started = datetime.now(timezone.utc).isoformat()
    raw_dir.mkdir(parents=True)
    matches, rejects, visited_days, hashes = fetch(args.as_of, args.since, args.delay, raw_dir)
    if not matches:
        raise RuntimeError('no complete historical ESPN match summaries were found')
    frozen = {
        'schema':'espn_player_appearances_v1',
        'provider':'ESPN site.api.espn.com summary via soccerdata-compatible contract',
        'source_url':BASE, 'source_league':LEAGUE,
        'retrieved_started_at_utc':started,
        'retrieved_completed_at_utc':datetime.now(timezone.utc).isoformat(),
        'as_of_exclusive_calendar_date':args.as_of.isoformat(),
        'since_inclusive_calendar_date':args.since.isoformat(),
        'calendar_dates_checked':visited_days,
        'raw_source_sha256':hashes,
        'matches':sorted(matches,key=lambda m:(m['kickoff_utc'],m['espn_match_id'])),
        'rejected_matches':rejects,
        'limitations':[
            'Fetched now; not a historical source-time snapshot. Cannot be used in forecasts cut off before retrieval.',
            'Provider ESPN identities are not FM26 IDs, and must not be equated without an audited identity mapping.',
            'Stoppage-time playing exposure is clipped to 90 minutes for the regulation-time hazard model.',
            'No xG, official injuries, or reliable result publication timestamp is inferred.',
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(stable_json(frozen))
    print(f"ESPN: froze {len(matches)} verified completed matches, rejected {len(rejects)} / "
          f"{visited_days} dates into {args.output}; sha256={sha_bytes(args.output.read_bytes())}")


if __name__ == '__main__':
    main()
