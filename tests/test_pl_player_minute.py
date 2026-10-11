"""Player minute serialization and conservation contract: reject fake interpolation."""
import copy,json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from render_pl_player_minute import reconstruct,figure

class MinutePlayerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        p=ROOT/'artifacts/validation/pl-player-minute-smoke.json'
        cls.game=json.loads(p.read_text())['fixtures'][0]

    def test_one_minute_and_player_goal_conservation(self):
        rows,players=reconstruct(self.game)
        self.assertEqual(len(rows),90)
        goals=sum(p['goals'] for p in players.values())
        self.assertAlmostEqual(goals+self.game['unattributed_goals_per_match'],
            self.game['expected_home_goals']+self.game['expected_away_goals'])
        self.assertLessEqual(sum(p['assists_proxy'] for p in players.values()),goals+1e-6)
        self.assertTrue(all(v['team'] in (self.game['home'],self.game['away']) for v in players.values()))
        fig,items=figure(self.game)
        self.assertEqual(fig.layout.width,1450)

    def test_four_fixture_diagnostic_goal_shot_coupling(self):
        j=json.loads((ROOT/'artifacts/validation/pl-player-minute-fourfixture-8paths.json').read_text())
        self.assertEqual(len(j['fixtures']),4)
        for f in j['fixtures']:
            self.assertEqual(f['paths'],8)
            minutes,players=reconstruct(f)
            for w in minutes:
                self.assertGreaterEqual(w['expected_home_shots']+1e-8,w['expected_home_goals'])
                self.assertGreaterEqual(w['expected_away_shots']+1e-8,w['expected_away_goals'])
            for p in players.values():
                self.assertGreaterEqual(p['shots']+1e-8,p['goals'])
            self.assertAlmostEqual(sum(p['goals'] for p in players.values())+f['unattributed_goals_per_match'],
                f['expected_home_goals']+f['expected_away_goals'])

    def test_missing_native_minute_observations_must_fail(self):
        f=copy.deepcopy(self.game)
        f.pop('minutes_1min')
        with self.assertRaisesRegex(ValueError,'rerun updated Rust simulator'):reconstruct(f)

    def test_malformed_15min_conservation_rejected(self):
        f=copy.deepcopy(self.game)
        f['minutes_1min'][0]['expected_home_goals']+=1
        with self.assertRaisesRegex(ValueError,'reconciliation failed'):reconstruct(f)

if __name__=='__main__':unittest.main()
