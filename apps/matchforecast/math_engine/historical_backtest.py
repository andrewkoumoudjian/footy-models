#!/usr/bin/env python3
"""Exploratory same-engine historical replay on *previously inspected* StatsBomb fixtures.

The engine is unchanged: only an explicitly different observed-stat-only input
contract replaces FM26, which did not exist in 2015. This is NOT fresh OOS
evidence and must never be used to select/publish new coefficients.
"""
import argparse
import collections
import csv
from datetime import date, timedelta
import hashlib
import json
import math
from pathlib import Path
import random
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
DEFAULT_DATA = ROOT / 'data/statsbomb/pl_hist_elapsed_v2'
DEFAULT_BIN = ROOT / 'target/release/footy-matchforecast-math'


def load(path):
    return json.loads(path.read_text())


def sha256(path):
    h = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def table(path):
    with path.open(newline='', encoding='utf-8-sig') as handle:
        return list(csv.DictReader(handle))


def nonnegative(value, *, missing=False):
    if value in (None, '', 'NaN'):
        if missing:
            return None
        raise ValueError('missing required numerical input')
    x = float(value)
    if not math.isfinite(x) or x < 0:
        raise ValueError(f'invalid numerical input: {value!r}')
    return x


def historical_goalkeepers(fixtures, lineup_dir, fixture_date):
    """Only observed earlier positions are evidence of 2015 goalkeeper roles."""
    observed = collections.defaultdict(set)
    sources = []
    for match in fixtures:
        if match['date'] >= fixture_date:
            continue
        filepath = lineup_dir / f"{match['match_id']}.json"
        if not filepath.exists():
            raise FileNotFoundError(f'prior lineup source unavailable: {filepath}')
        sources.append(filepath)
        for team in load(filepath):
            for player in team['lineup']:
                if any(position.get('position_id') == 1
                       and position.get('from_reason', position.get('start_reason')) is not None
                       for position in player.get('positions', [])):
                    observed[team['team_name']].add(int(player['player_id']))
    return observed, sources


def prepare_fixture(fixture, manifest, historical, source_dir, lineup_dir):
    cutoff = date.fromisoformat(fixture['date'])
    player_file = source_dir / 'players' / f"{fixture['match_id']}.csv"
    candidate_rows = table(player_file)
    observed, lineup_sources = historical_goalkeepers(manifest['fixtures'], lineup_dir, fixture['date'])
    pool = []
    raw_rows = {}
    for row in candidate_rows:
        if row['team'] not in (fixture['home'], fixture['away']):
            continue
        prior_list = row['last_listed_date']
        if prior_list and date.fromisoformat(prior_list) >= cutoff:
            raise ValueError(f'future/same-day player observation for match {fixture["match_id"]}')
        player_id = int(row['player_id'])
        if player_id in raw_rows:
            raise ValueError(f'duplicate historical player ID {player_id}')
        raw_rows[player_id] = row
        n = nonnegative(row['team_fixtures'])
        starts = nonnegative(row['selection_starts'])
        if starts > n:
            raise ValueError('invalid historical starts > earlier fixtures')
        # One common pseudo-start produces a conservative strictly earlier
        # playing-time prior. No target match minutes or XI can enter here.
        start_prior = (starts + 0.5) / (n + 1.0)
        pool.append({
            'id': player_id, 'club': row['team'], 'minutes_prior': 90 * start_prior,
            'shots_per90': nonnegative(row['shots_p90']),
            'xg_per90': nonnegative(row['xg_p90']),
            'is_goalkeeper': player_id in observed[row['team']],
            'fm_source': 'historical_observed_only', 'fm_kind': 'historical',
            'fm_finishing': None, 'fm_ca': None,
        })
    for club in (fixture['home'], fixture['away']):
        own = [p for p in pool if p['club'] == club]
        if sum(p['is_goalkeeper'] for p in own) < 1 or sum(not p['is_goalkeeper'] for p in own) < 10:
            raise ValueError(f'no as-of feasible 1+10 players for {club} in {fixture["match_id"]}')
    history = []
    for row in historical:
        when = date.fromisoformat(row['date'])
        if when >= cutoff:
            continue
        history.append({
            'day': when.toordinal(), 'home': row['home'], 'away': row['away'],
            'home_goals': nonnegative(row['home_goals']),
            'away_goals': nonnegative(row['away_goals']),
            'home_shots': nonnegative(row.get('home_shots'), missing=True),
            'away_shots': nonnegative(row.get('away_shots'), missing=True),
        })
    if len(history) < 30:
        raise ValueError('insufficient pre-fixture results for team-strength prior')
    return {
        'schema': 'math_shot_goal_v1', 'as_of': (cutoff - timedelta(days=1)).isoformat(),
        'fm_effective': 'not_applicable_historical_observations',
        'fm_source_counts': {'historical_observed_only': len(pool)},
        'sources': {
            str(path): sha256(path)
            for path in (source_dir / 'fixtures.json', source_dir / 'team_rows.csv',
                         player_file, *lineup_sources)
        },
        'feature_contract': 'strictly earlier StatsBomb 2015/16 player stats, no FM',
        'players': pool, 'history': history, 'championship': [],
        'fixtures': [{
            'date': fixture['date'], 'day': cutoff.toordinal(),
            'home': fixture['home'], 'away': fixture['away'],
        }],
    }, raw_rows


def independent_poisson(mu_h, mu_a, observed_h, observed_a):
    def pmf(mu, count):
        return math.exp(-mu + count * math.log(mu) - math.lgamma(count + 1))
    home = [pmf(mu_h, count) for count in range(30)]
    away = [pmf(mu_a, count) for count in range(30)]
    wdls = [
        sum(home[h] * away[a] for h in range(30) for a in range(h)),
        sum(home[h] * away[h] for h in range(30)),
        sum(home[h] * away[a] for h in range(30) for a in range(h + 1, 30)),
    ]
    exact = pmf(mu_h, observed_h) * pmf(mu_a, observed_a)
    return wdls, exact


def fixture_scores(fixture, forecast, labels, prior_players):
    home, away = fixture['home_goals'], fixture['away_goals']
    outcome = 0 if home > away else 1 if home == away else 2
    pred = forecast['fixtures'][0]
    baseline_probs, exact = independent_poisson(*pred['team_goal_anchor'], home, away)
    mc_probs = [pred[x] for x in ('home_win', 'draw', 'away_win')]
    actual = {int(p['player_id']): p for p in labels['players']}
    simulated = {int(p['id']): p for p in pred['player_forecasts']}
    player_metrics = collections.defaultdict(list)
    uncovered_goals = 0
    for player in actual.values():
        if player['goals'] > 0 and int(player['player_id']) not in simulated:
            uncovered_goals += int(player['goals'])
    for pid, row in prior_players.items():
        target = actual.get(pid, {})
        model = simulated.get(pid)
        if model is None:
            raise ValueError('candidate omitted from match-engine output')
        n = nonnegative(row['team_fixtures'])
        start_prior = (nonnegative(row['selection_starts']) + 0.5) / (n + 1.0)
        actual_started = float(bool(target.get('started', False)))
        minutes_per_fixture = nonnegative(row['selection_minutes']) / max(n, 1.0)
        shots_prior = nonnegative(row['shots_p90']) * minutes_per_fixture / 90.0
        goals_prior = nonnegative(row['goals_p90']) * minutes_per_fixture / 90.0
        actual_shots = float(target.get('shots', 0))
        actual_goals = float(target.get('goals', 0) > 0)
        player_metrics['start_engine'].append((model['p_start'] - actual_started) ** 2)
        player_metrics['start_prior'].append((start_prior - actual_started) ** 2)
        player_metrics['shots_engine'].append(abs(model['expected_shots'] - actual_shots))
        player_metrics['shots_prior'].append(abs(shots_prior - actual_shots))
        # Same-path occurrences: goals after a shot are drawn once per path,
        # so this is a true proper Brier target rather than a moment proxy.
        player_metrics['goals_engine'].append((model['p_goal'] - actual_goals) ** 2)
        player_metrics['goals_prior'].append(
            ((1.0 - math.exp(-goals_prior)) - actual_goals) ** 2)
    values = {key: sum(seq) / len(seq) for key, seq in player_metrics.items()}
    values.update({
        'wdl_mc_log_loss': -math.log(max(mc_probs[outcome], 1e-12)),
        'wdl_poisson_log_loss': -math.log(max(baseline_probs[outcome], 1e-12)),
        'scoreline_poisson_log_loss': -math.log(max(exact, 1e-12)),
        'start_coverable_players': len(prior_players),
        'actual_player_goals_not_in_prior_pool': uncovered_goals,
        'observed_player_goals': sum(int(p['goals']) for p in actual.values()),
    })
    return values


def paired_bootstrap(rows, a, b, *, draws=2500, seed=42):
    # Matchday is the resampling unit: same-day matches are not independent.
    days = collections.defaultdict(list)
    for row in rows:
        days[row['date']].append(row[a] - row[b])
    groups = list(days.values())
    rng = random.Random(seed)
    estimates = []
    for _ in range(draws):
        sample = [rng.choice(groups) for _ in groups]
        values = [v for group in sample for v in group]
        estimates.append(sum(values) / len(values))
    estimates.sort()
    return [estimates[int(0.025 * draws)], estimates[int(0.975 * draws)]]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--source', type=Path, default=DEFAULT_DATA)
    ap.add_argument('--lineups', type=Path, default=ROOT / 'data/statsbomb/lineups')
    ap.add_argument('--bin', type=Path, default=DEFAULT_BIN)
    ap.add_argument('--output-dir', type=Path, required=True)
    ap.add_argument('--paths', type=int, default=2500)
    ap.add_argument('--seed', type=int, default=42)
    ap.add_argument('--max-fixtures', type=int, default=25)
    ap.add_argument('--build', action='store_true')
    args = ap.parse_args()
    if args.max_fixtures < 1 or args.paths < 1:
        ap.error('positive path and fixture counts required')
    out = args.output_dir.resolve()
    if out.exists():
        ap.error(f'immutable output already exists: {out}')
    if args.build:
        subprocess.run(['cargo','build','--release','-p','footy-matchforecast',
                        '--bin','footy-matchforecast-math'],cwd=ROOT,check=True)
    if not args.bin.exists():
        ap.error(f'compile --build first: {args.bin}')
    out.mkdir(parents=True)
    (out / 'inputs').mkdir()
    (out / 'forecasts').mkdir()
    manifest = load(args.source / 'fixtures.json')
    fixtures = [f for f in manifest['fixtures'] if f['evaluable']]
    # The final 25 here are the *already examined* 2015/16 fixtures.
    # Any comparison is retrospective diagnosis, never an unbiased holdout.
    fixtures = fixtures[-args.max_fixtures:]
    historical = table(args.source / 'team_rows.csv')
    details = []
    skipped = []
    for i, fixture in enumerate(fixtures):
        ident = str(fixture['match_id'])
        try:
            bundle, player_priors = prepare_fixture(
                fixture, manifest, historical, args.source, args.lineups)
        except (ValueError, FileNotFoundError) as exc:
            skipped.append({'match_id': ident, 'reason': str(exc)})
            continue
        input_path = out / 'inputs' / f'{ident}.json'
        forecast_path = out / 'forecasts' / f'{ident}.json'
        input_path.write_text(json.dumps(bundle, sort_keys=True) + '\n')
        subprocess.run([str(args.bin), str(input_path), str(forecast_path),
                        '--paths', str(args.paths), '--seed', str(args.seed)],
                       cwd=ROOT, check=True, capture_output=True, text=True)
        forecast = load(forecast_path)
        labels = load(args.source / 'labels' / f'{ident}.json')
        if labels['date'] != fixture['date'] or labels['home'] != fixture['home']:
            raise ValueError(f'wrong held-out event labels for {ident}')
        scores = fixture_scores(fixture, forecast, labels, player_priors)
        details.append({'match_id': ident, 'date': fixture['date'],
                        'home': fixture['home'], 'away': fixture['away'],
                        'input_sha256': sha256(input_path),
                        'prediction_sha256': sha256(forecast_path), **scores})
        print(f'[{i + 1}/{len(fixtures)}] {fixture["date"]}: {fixture["home"]} v {fixture["away"]}', flush=True)
    if not details:
        raise ValueError('no fixtures were evaluable with historical GK coverage')
    keys = ['wdl_mc_log_loss', 'wdl_poisson_log_loss', 'scoreline_poisson_log_loss',
            'start_engine', 'start_prior', 'shots_engine', 'shots_prior',
            'goals_engine', 'goals_prior']
    means = {key: sum(row[key] for row in details) / len(details) for key in keys}
    ci = {}
    for a,b in [('wdl_mc_log_loss','wdl_poisson_log_loss'),
                ('start_engine','start_prior'),('shots_engine','shots_prior'),
                ('goals_engine','goals_prior')]:
        ci[f'{a}_minus_{b}'] = paired_bootstrap(details,a,b)
    report = {
        'status': 'exploratory_inspected_holdout_NOT_PROMOTABLE',
        'method': 'historically as-of player event inputs / current math_engine binary; NO 2026 FM',
        'engine_binary_sha256': sha256(args.bin),
        'source_fixture_sha256': sha256(args.source / 'fixtures.json'),
        'source_labels_are_scoring_only': True,
        'paths_per_match':args.paths,'seed':args.seed,
        'fixtures_scored':len(details), 'skipped':skipped,
        'mean_losses':means, 'matchday_cluster_bootstrap_95_percent':ci,
        'caveats':[
            'The 2015/16 holdout was previously inspected; these results cannot promote new parameters.',
            'Goal marks are normalized to the independent Poisson anchor: WDL and exact score cannot be interpreted as a new structural model improvement.',
            'Current challenger does not simulate substitutions or goals scored by substitute-only players; report pool coverage.',
            'Player goal-occurrence Brier uses actual path-level scorer indicators; substitute-only scorers remain uncovered.',
            'Only two clubs have extensive event coverage in this 74-match subset; wider data are necessary.'
        ],
        'fixtures':details,
    }
    (out / 'report.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps({'fixture_count':len(details),'skipped':len(skipped),
                      'mean_losses':means,'bootstrap_ci':ci},indent=2))


if __name__ == '__main__':
    main()
