#!/usr/bin/env python3
"""Immutable comparison of *previously inspected* participation ablations.

Identical fixture IDs, label dates, historical cutoff, path count and seeds
are required. Bootstrap resamples matchdays (not individual correlated players).
"""
import argparse
import hashlib
import json
from pathlib import Path

from historical_backtest import paired_bootstrap

TARGETS = (
    'start_engine', 'appearance_engine', 'minutes_engine',
    'shots_engine', 'goals_engine', 'wdl_mc_log_loss',
)


def load(path):
    with path.open() as stream:
        return json.load(stream)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare(base, variants):
    if base['status'] != 'exploratory_inspected_holdout_NOT_PROMOTABLE':
        raise ValueError('historical result is not marked retrospective')
    reference = [(x['match_id'], x['date']) for x in base['fixtures']]
    if len(reference) != len(set(reference)) or len(reference) == 0:
        raise ValueError('baseline fixtures missing or duplicated')
    result = {}
    for name, candidate in variants.items():
        if candidate['status'] != base['status']:
            raise ValueError(f'{name}: exploratory flag mismatch')
        for key in ('paths_per_match', 'seed', 'source_fixture_sha256'):
            if candidate[key] != base[key]:
                raise ValueError(f'{name}: mismatch {key}')
        matched = [(x['match_id'], x['date']) for x in candidate['fixtures']]
        if matched != reference:
            raise ValueError(f'{name}: unequal or reordered fixture sample')
        table = {}
        for metric in TARGETS:
            paired = [
                {'date': c['date'], 'difference': c[metric] - b[metric], 'null': 0.0}
                for b, c in zip(base['fixtures'], candidate['fixtures'])
            ]
            mean = sum(x['difference'] for x in paired) / len(paired)
            interval = paired_bootstrap(paired, 'difference', 'null')
            table[metric] = {
                'baseline_mean': sum(r[metric] for r in base['fixtures']) / len(reference),
                'challenger_mean': sum(r[metric] for r in candidate['fixtures']) / len(reference),
                'challenger_minus_baseline': mean,
                'matchday_bootstrap_95_interval': interval,
            }
        table['mean_substitutions_per_team'] = (
            candidate['mean_losses']['mean_simulated_home_substitutions']
            + candidate['mean_losses']['mean_simulated_away_substitutions']
        ) / 2
        result[name] = table
    return {
        'status': 'retrospective_only_not_an_oos_promotion_gate',
        'description': 'Three research configurations on already inspected StatsBomb 2015/16 fixture sample',
        'fixtures': len(reference),
        'model_binary_sha256': base['engine_binary_sha256'],
        'path_seed': base['seed'], 'paths_per_fixture': base['paths_per_match'],
        'caveats': [
            'The examined fixture set was reused and is heavily concentrated in two clubs; bootstrap intervals are descriptive, not independent hypothesis tests.',
            'Candidate lineups and substitutions alter random-number consumption, so same fixture seeds are not common random-number matched events.',
            'Output evaluation covers only previously known candidate players; future-only scorers remain missing.',
            'No current 2026 Premier League selection history exists for out-of-sample rollout of the new options.',
        ],
        'results': result,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--substitutions', type=Path, required=True)
    parser.add_argument('--joint', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('refusing to overwrite immutable ablation report')
    report = compare(load(args.baseline), {
        'substitutions_only': load(args.substitutions),
        'selection_marginals_and_substitutions': load(args.joint),
    })
    report['input_sha256'] = {
        name: sha256(path)
        for name, path in [('baseline', args.baseline),
                           ('substitutions', args.substitutions),
                           ('joint', args.joint)]
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(f'Wrote retrospective research comparison for {report["fixtures"]} inspected fixtures')


if __name__ == '__main__':
    main()
