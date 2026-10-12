"""Negative-control checks for source-clock and chronological replay contracts."""
import importlib.util
import json
import tempfile
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location(
    'math_historical_backtest', Path(__file__).with_name('historical_backtest.py'))
history = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(history)


class HistoricalContracts(unittest.TestCase):
    def test_only_earlier_lineup_positions_classify_goalkeepers(self):
        with tempfile.TemporaryDirectory() as root:
            folder = Path(root)
            def make(match_id, name, keeper_id):
                (folder / f'{match_id}.json').write_text(json.dumps([
                    {'team_name': name, 'lineup': [
                        {'player_id': keeper_id, 'positions': [
                            {'position_id': 1, 'start_reason': 'Starting XI'}]}]}]))
            make(1, 'Club A', 11)
            make(2, 'Club A', 22)
            make(3, 'Club A', 33)
            fixtures = [
                {'match_id': 1, 'date': '2015-08-01'},
                {'match_id': 2, 'date': '2015-08-02'},
                {'match_id': 3, 'date': '2015-08-03'},
            ]
            keepers, _ = history.historical_goalkeepers(fixtures, folder, '2015-08-02')
            self.assertEqual(keepers['Club A'], {11})
            keepers, _ = history.historical_goalkeepers(fixtures, folder, '2015-08-03')
            self.assertEqual(keepers['Club A'], {11, 22})
            self.assertNotIn(33, keepers['Club A'])

    def test_missing_is_not_zero_and_nan_rejected_for_counts(self):
        self.assertIsNone(history.nonnegative('NaN', missing=True))
        with self.assertRaises(ValueError):
            history.nonnegative('NaN')
        with self.assertRaises(ValueError):
            history.nonnegative('inf', missing=True)

    def test_analytical_goal_law_normalizes_and_has_positive_exact_score(self):
        probabilities, exact = history.independent_poisson(1.4, 1.1, 2, 1)
        self.assertAlmostEqual(sum(probabilities), 1.0, places=9)
        self.assertGreater(exact, 0)
        self.assertLess(exact, 1)

    def test_fixture_score_uses_direct_path_goal_occurrence(self):
        fixture = {'home_goals': 1, 'away_goals': 0}
        forecast = {'fixtures': [{
            'team_goal_anchor': [1.4, 1.1],
            'home_win': 0.44, 'draw': 0.27, 'away_win': 0.29,
            'player_forecasts': [{
                'id': 101, 'p_start': 1.0, 'expected_shots': 1.0,
                'expected_goals': 0.4, 'p_goal': 0.4,
            }],
        }]}
        labels = {'players': [{
            'player_id': 101, 'started': True, 'shots': 1,
            'goals': 1,
        }]}
        priors = {101: {
            'team_fixtures': '1', 'selection_starts': '1',
            'selection_minutes': '90', 'shots_p90': '1',
            'goals_p90': '0.2',
        }}
        result = history.fixture_scores(fixture, forecast, labels, priors)
        self.assertAlmostEqual(result['goals_engine'], 0.36, places=9)
        self.assertNotIn('goals_engine_proxy', result)


if __name__ == '__main__':
    unittest.main()
