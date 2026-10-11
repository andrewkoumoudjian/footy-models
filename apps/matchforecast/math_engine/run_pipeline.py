#!/usr/bin/env python3
"""One-command, offline and immutable FM26 -> mathematical match forecast run."""
import argparse
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
MODEL = ROOT / 'target/release/footy-matchforecast-math'
MODEL_SOURCES = (
    'apps/matchforecast/math_engine/build_inputs.py',
    'apps/matchforecast/math_engine/run_pipeline.py',
    'apps/matchforecast/Cargo.toml',
    'apps/matchforecast/src/lib.rs',
    'apps/matchforecast/src/math_engine/mod.rs',
    'apps/matchforecast/src/math_engine_main.rs',
    'crates/match/src/lineup_entropy.rs',
    'crates/match/src/team_goals.rs',
    'crates/forecast/src/lib.rs',
    'Cargo.lock',
)


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as handle:
        for part in iter(lambda: handle.read(1 << 20), b''):
            h.update(part)
    return h.hexdigest()


def poisson(mu, cap=22):
    p = [math.exp(-mu)]
    for k in range(1, cap + 1):
        p.append(p[-1] * mu / k)
    return p


def analytic_wdl(goals):
    home, away = (poisson(mu) for mu in goals)
    result = [0.0, 0.0, 0.0]
    for h, ph in enumerate(home):
        for a, pa in enumerate(away):
            result[0 if h > a else 1 if h == a else 2] += ph * pa
    return result


def validate(input_path, output_path, *, paths, default_effects):
    data = json.loads(input_path.read_text())
    report = json.loads(output_path.read_text())
    assert data['schema'] == report['schema'] == 'math_shot_goal_v1'
    assert report['status'] == 'research_only_unvalidated'
    assert report['data_sources'] == data['sources']
    assert report['source_counts'] == data['fm_source_counts']
    assert report['paths'] == paths and len(report['fixtures']) == len(data['fixtures'])
    for filepath, expected_hash in data['sources'].items():
        if sha(filepath) != expected_hash:
            raise ValueError(f'input was modified during simulation: {filepath}')
    checks = 0
    for fixture in report['fixtures']:
        base = fixture['team_goal_anchor']
        assert fixture['fit_matches'] >= 30
        assert abs(sum(fixture[k] for k in ('home_win', 'draw', 'away_win')) - 1) <= 1e-10
        assert abs(sum(s['probability'] for s in fixture['scorelines']) - 1) <= 1e-10
        assert len(fixture['player_forecasts']) >= 22
        for key in ('home', 'away'):
            assert abs(sum(fixture[f'{key}_window_goals']) / paths - fixture[f'expected_{key}_goals']) < 1e-10
            assert abs(sum(fixture[f'{key}_window_shots']) / paths - fixture[f'expected_{key}_shots']) < 1e-10
            assert fixture[f'expected_{key}_goals'] <= fixture[f'expected_{key}_shots']
        assert abs(sum(p['expected_goals'] for p in fixture['player_forecasts'])
                   - fixture['expected_home_goals'] - fixture['expected_away_goals']) <= 1e-8
        assert abs(sum(p['expected_shots'] for p in fixture['player_forecasts'])
                   - fixture['expected_home_shots'] - fixture['expected_away_shots']) <= 1e-8
        assert all(p['expected_goals'] <= p['expected_shots'] for p in fixture['player_forecasts'])
        if default_effects and paths >= 256:
            reference = analytic_wdl(base)
            for estimate, expected in zip(
                (fixture['home_win'], fixture['draw'], fixture['away_win']), reference
            ):
                mc_se = math.sqrt(max(expected * (1 - expected), 0) / paths)
                if abs(estimate - expected) > 6 * mc_se + 0.001:
                    raise ValueError('constant-hazard engine diverged from analytic Poisson')
            for key, anchor in zip(('home', 'away'), base):
                sample = fixture[f'expected_{key}_goals']
                if abs(sample - anchor) > 6 * math.sqrt(anchor / paths) + 0.001:
                    raise ValueError('sampled goals diverged from independent-Poisson anchor')
        checks += 1
    return {'checked_fixtures': checks, 'analytical_poisson_consistency_checked': default_effects and paths >= 256,
            'passed': True, 'missing_historical_shots': data['missing_shot_matches']}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--output-dir', type=Path, required=True)
    ap.add_argument('--fm', type=Path, default=Path('/home/drewclaw/fm26-data/fm26_men_full_attrs.csv'))
    ap.add_argument('--as-of', default='2026-10-10')
    ap.add_argument('--fm-effective', default='2026-03-01')
    ap.add_argument('--paths', type=int, default=2500)
    ap.add_argument('--seed', type=int, default=42)
    ap.add_argument('--red-per90', type=float, default=0)
    ap.add_argument('--score-effect', type=float, default=0)
    ap.add_argument('--man-advantage', type=float, default=0)
    ap.add_argument('--fm-effect', type=float, default=0)
    args = ap.parse_args()
    out = args.output_dir.resolve()
    if out.exists():
        ap.error(f'refusing to overwrite an existing run: {out}')
    out.mkdir(parents=True)
    inputs = out / 'inputs.json'
    predictions = out / 'predictions.json'
    cmd = [sys.executable, str(HERE/'build_inputs.py'), '--fm', str(args.fm),
           '--as-of', args.as_of, '--fm-effective', args.fm_effective,
           '--output', str(inputs)]
    subprocess.run(cmd, cwd=ROOT, check=True)
    subprocess.run(['cargo', 'build', '--release', '-p', 'footy-matchforecast',
                    '--bin', 'footy-matchforecast-math'], cwd=ROOT, check=True)
    sim = [str(MODEL), str(inputs), str(predictions), '--paths', str(args.paths),
           '--seed', str(args.seed), '--red-per90', str(args.red_per90),
           '--score-effect', str(args.score_effect), '--man-advantage', str(args.man_advantage),
           '--fm-effect', str(args.fm_effect)]
    subprocess.run(sim, cwd=ROOT, check=True)
    effects_zero = args.red_per90 == args.score_effect == args.man_advantage == 0
    validation = validate(inputs, predictions, paths=args.paths, default_effects=effects_zero)
    manifest = {
        'status': 'research_only', 'input_sha256': sha(inputs),
        'binary_sha256': sha(MODEL), 'predictions_sha256': sha(predictions),
        'model_source_sha256': {filename: sha(ROOT/filename) for filename in MODEL_SOURCES},
        'simulation_arguments': sim[3:], 'validation': validation,
    }
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    print(f'PASS: {validation}; immutable run saved to {out}')


if __name__ == '__main__':
    main()
