"""Offline contract tests for the isolated FM26 / historical input materializer."""
import csv
import importlib.util
import json
import tempfile
import unittest
from datetime import date, timedelta
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('math_build_inputs', Path(__file__).with_name('build_inputs.py'))
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


def write_csv(path, fields, rows, delimiter=','):
    with path.open('w', newline='') as handle:
        writer = csv.DictWriter(handle, fields, delimiter=delimiter)
        writer.writeheader()
        writer.writerows(rows)


class InputContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.fm = root / 'fm.csv'
        self.players = root / 'players.csv'
        self.history = root / 'history.csv'
        self.fixtures = root / 'fixtures.tsv'
        self.results = root / 'results.tsv'
        self.squads = root / 'squads.json'
        self.raw = root / 'raw'
        self.raw.mkdir()
        self.fm_rows = []
        self.player_rows = []
        for club, offset in [('Home', 100), ('Away', 200)]:
            for i in range(11):
                uid = offset + i
                source = 'derived' if club == 'Home' and i == 2 else 'db'
                self.fm_rows.append({
                    'uid': uid, 'kind': 'player', 'attr_source': source,
                    'attr_Reflexes': 16 if i == 0 else 0,
                    'attr_Handling': 15 if i == 0 else 0, 'attr_Finishing': 0 if i == 0 else 14,
                    'attr_OneOnOnes': 14 if i == 0 else 0,
                    'attr_AerialReach': 14 if i == 0 else 0, 'ca': 100,
                })
                self.player_rows.append({
                    'player_id': uid, 'name': str(uid), 'team': club,
                    'minutes': 1000, 'shots_p90': 0.01 if i == 0 else 1,
                    'xg_p90': 0.001 if i == 0 else 0.12,
                    'last_listed_date': '2026-10-10',
                })
        self.history_rows = []
        self.raw_rows = []
        for n in range(35):
            when = date(2026, 8, 1) + timedelta(days=n)
            self.history_rows.append({
                'date': when.isoformat(), 'home': 'Home', 'away': 'Away',
                'home_goals': 2, 'away_goals': 1,
                'home_shots': 12, 'away_shots': 11,
            })
            if n:
                self.raw_rows.append({
                    'Date': when.strftime('%d/%m/%Y'), 'HomeTeam': 'Home', 'AwayTeam': 'Away',
                    'HS': 14, 'AS': 9, 'FTHG': 2, 'FTAG': 1,
                })
        self.write()

    def write(self):
        write_csv(self.fm, list(self.fm_rows[0]), self.fm_rows)
        write_csv(self.players, list(self.player_rows[0]), self.player_rows)
        write_csv(self.history, list(self.history_rows[0]), self.history_rows)
        write_csv(self.raw / '2627_E0.csv', list(self.raw_rows[0]), self.raw_rows)
        write_csv(self.fixtures, ['date', 'home', 'away'],
                  [{'date': '2026-10-11', 'home': 'Home', 'away': 'Away'}], '\t')
        write_csv(self.results, ['day', 'division', 'home', 'away', 'home_goals', 'away_goals'],
                  [{'day': 20000, 'division': 'E1', 'home': 'Home', 'away': 'Away',
                    'home_goals': 1, 'away_goals': 0}], '\t')
        self.squads.write_text(json.dumps({
            'as_of': '2026-10-10',
            'teams': {team: {'players': [
                {'fm_uid': str(row['player_id']),
                 'historical_position': 'Goalkeeper' if row['player_id'] % 100 == 0
                 else 'Attack'}
                for row in self.player_rows if row['team'] == team]}
                for team in ('Home', 'Away')}
        }))

    def build(self):
        return module.make_inputs(
            fm_file=self.fm, player_file=self.players, history_file=self.history,
            results_file=self.results, fixtures_file=self.fixtures, raw_dir=self.raw,
            squads_file=self.squads,
            as_of='2026-10-10', fm_effective='2026-03-01')

    def test_exact_raw_fm_join_and_no_synthetic_features(self):
        result = self.build()
        self.assertEqual(len(result['players']), 22)
        self.assertEqual(result['fm_source_counts'], {'db': 21, 'derived': 1})
        self.assertEqual(result['fm_record_kind_counts'],
                         {'player/db': 21, 'player/derived': 1})
        self.assertEqual(result['fm_trusted_attributes'], 21)
        self.assertIsNone(next(x for x in result['players'] if x['id'] == 102)['fm_finishing'])
        self.assertEqual(result['missing_shot_matches'], 1)
        self.assertIsNone(result['history'][0]['home_shots'])
        self.assertEqual(result['history'][1]['home_shots'], 14)
        self.assertEqual(result['history'][1]['away_shots'], 9)
        self.assertEqual(sum(p['is_goalkeeper'] for p in result['players']), 2)
        self.assertEqual(len(result['sources']), 7)
        self.assertEqual(len(result['championship']), 1)

    def test_lookahead_prevented(self):
        self.player_rows[0]['last_listed_date'] = '2026-10-11'
        self.write()
        with self.assertRaisesRegex(ValueError, 'future roster'):
            self.build()

    def test_identity_ambiguity_prevented(self):
        self.player_rows[1]['player_id'] = self.player_rows[0]['player_id']
        self.write()
        with self.assertRaisesRegex(ValueError, 'duplicate player'):
            self.build()

    def test_raw_goals_reconciled(self):
        self.raw_rows[1]['FTHG'] = 3
        self.write()
        with self.assertRaisesRegex(ValueError, 'disagree'):
            self.build()

    def test_staff_db_is_never_trusted_or_given_fm_finishing(self):
        self.fm_rows[2]['kind'] = 'staff'
        self.fm_rows[2]['attr_source'] = 'db'
        self.write()
        row = next(x for x in self.build()['players'] if x['id'] == 102)
        self.assertFalse(row['fm_attribute_verified'])
        self.assertIsNone(row['fm_finishing'])
        self.assertIsNone(row['fm_ca'])
        self.assertEqual(row['fm_source'], 'unverified_staff_db')

    def test_explicit_attack_position_overrides_goalkeeper_attributes(self):
        self.fm_rows[2].update({
            'attr_Reflexes': 17, 'attr_Handling': 17,
            'attr_OneOnOnes': 17, 'attr_AerialReach': 17,
        })
        self.write()
        row = next(x for x in self.build()['players'] if x['id'] == 102)
        self.assertFalse(row['is_goalkeeper'])
        self.assertEqual(row['position_source'], 'prior_position')

    def test_registered_identity_is_required(self):
        entry = json.loads(self.squads.read_text())
        entry['teams']['Home']['players'][2]['fm_uid'] = '99999'
        self.squads.write_text(json.dumps(entry))
        with self.assertRaisesRegex(ValueError, 'registered club'):
            self.build()

    def test_fm_vintage_must_precede_forecast(self):
        with self.assertRaisesRegex(ValueError, 'FM vintage'):
            module.make_inputs(
                fm_file=self.fm, player_file=self.players, history_file=self.history,
                results_file=self.results, fixtures_file=self.fixtures,
                raw_dir=self.raw, squads_file=self.squads, as_of='2026-10-10',
                fm_effective='2026-11-01')


if __name__ == '__main__':
    unittest.main()
