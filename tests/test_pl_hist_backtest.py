"""Synthetic and artifact regression checks for the historical same-engine backtest.

Covers:
- StatsBomb label construction (minutes, minute indices, own goals, SOT, xA)
- no-lookahead ordering in the prepared fixture manifest
- probability coherence and conservation in the committed same-engine report
- deterministic calibration application
"""
import json
import math
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from build_pl_hist_inputs import clock_seconds, minute_index, parse_events, parse_lineups  # noqa: E402
from score_pl_hist_backtest import apply_calibrator, discrete_crps, occurrence_from_mean, poisson_pmf, score_grid  # noqa: E402

REPORT = ROOT / "artifacts/validation/pl-hist-same-engine-oos.json"
PATHS = ROOT / "artifacts/validation/pl-hist-same-engine-paths.json"


class StatsBombLabelTests(unittest.TestCase):
    def test_minute_index_conserves_stoppage_and_window_boundaries(self):
        self.assertEqual(minute_index({"minute": 1}), 0)
        self.assertEqual(minute_index({"minute": 15}), 14)
        self.assertEqual(minute_index({"minute": 45}), 44)
        self.assertEqual(minute_index({"minute": 90}), 89)
        self.assertEqual(minute_index({"minute": 96}), 89)
        self.assertEqual(minute_index({}), 89)

    def test_clock_seconds_parses_provider_timestamps(self):
        self.assertEqual(clock_seconds("00:00"), 0)
        self.assertEqual(clock_seconds("45:00"), 2700)
        self.assertEqual(clock_seconds("79:08"), 4748)
        self.assertIsNone(clock_seconds(None))

    def test_lineups_mark_starters_and_sum_position_intervals(self):
        payload = {"lineup": [
            {"player_id": 1, "player_name": "Starter", "positions": [
                {"from": "00:00", "to": "60:00", "start_reason": "Starting XI"}]},
            {"player_id": 2, "player_name": "Sub", "positions": [
                {"from": "60:00", "to": None, "start_reason": "Substitution - On (Tactical)"}]},
        ]}
        rows = parse_lineups(payload, "A")
        self.assertTrue(rows[1]["started"])
        self.assertAlmostEqual(rows[1]["minutes"], 60.0)
        self.assertFalse(rows[2]["started"])
        self.assertAlmostEqual(rows[2]["minutes"], 30.0)

    def test_events_credit_goals_assists_sot_and_xa_distinctly(self):
        meta = {"home_team": {"home_team_name": "A"}, "away_team": {"away_team_name": "B"}}
        pass_id = "p1"
        events = [
            {"id": pass_id, "type": {"name": "Pass"}, "team": {"name": "A"},
             "player": {"id": 10, "name": "Creator"}, "pass": {"goal_assist": True}},
            {"type": {"name": "Shot"}, "team": {"name": "A"}, "player": {"id": 9, "name": "Scorer"},
             "minute": 30, "shot": {"statsbomb_xg": 0.4, "outcome": {"name": "Goal"}, "key_pass_id": pass_id}},
            {"type": {"name": "Shot"}, "team": {"name": "A"}, "player": {"id": 9, "name": "Scorer"},
             "minute": 55, "shot": {"statsbomb_xg": 0.1, "outcome": {"name": "Saved"}}},
            {"type": {"name": "Shot"}, "team": {"name": "B"}, "player": {"id": 21, "name": "Blocker"},
             "minute": 70, "shot": {"statsbomb_xg": 0.2, "outcome": {"name": "Blocked"}}},
            {"type": {"name": "Own Goal Against"}, "team": {"name": "B"}, "player": {"id": 22, "name": "Own"}},
        ]
        parsed = parse_events(meta, events)
        self.assertEqual(parsed["teams"]["A"]["goals"], 1)
        self.assertEqual(parsed["teams"]["A"]["shots"], 2)
        self.assertEqual(parsed["teams"]["A"]["sot"], 2)
        self.assertEqual(parsed["teams"]["A"]["own_goals_for"], 1)
        self.assertEqual(parsed["teams"]["B"]["own_goals_for"], 0)
        self.assertAlmostEqual(parsed["players"][10]["xa"], 0.4)
        self.assertEqual(parsed["players"][10]["assists"], 1)
        self.assertEqual(parsed["players"][9]["goals"], 1)
        self.assertEqual(parsed["players"][21]["sot"], 0)
        own = [goal for goal in parsed["goals"] if goal["own_goal"]]
        self.assertEqual(len(own), 1)
        self.assertEqual(own[0]["team"], "A")


class ScoringPrimitiveTests(unittest.TestCase):
    def test_poisson_pmf_normalises_and_matches_occurrence(self):
        pmf = poisson_pmf(1.4, 20)
        self.assertAlmostEqual(sum(pmf), 1.0, places=9)
        self.assertAlmostEqual(1.0 - pmf[0], occurrence_from_mean(1.4), places=9)

    def test_score_grid_probabilities_sum_to_one(self):
        grid = score_grid(1.5, 1.1)
        self.assertAlmostEqual(sum(grid.values()), 1.0, places=6)
        self.assertTrue(all(value >= 0.0 for value in grid.values()))

    def test_calibrator_is_identity_at_default_and_normalises(self):
        raw = [0.5, 0.25, 0.25]
        identity = apply_calibrator(raw, 1.0, 0.0)
        for left, right in zip(identity, raw):
            self.assertAlmostEqual(left, right, places=12)
        calibrated = apply_calibrator(raw, 2.0, 0.4)
        self.assertAlmostEqual(sum(calibrated), 1.0, places=12)
        self.assertGreater(calibrated[1], raw[1])

    def test_discrete_crps_zero_only_for_degenerate_correct_forecast(self):
        certain = {2: 1.0}
        self.assertAlmostEqual(discrete_crps(certain, 2), 0.0)
        self.assertGreater(discrete_crps(certain, 1), 0.0)
        spread = {0: 0.5, 2: 0.5}
        self.assertGreater(discrete_crps(spread, 1), discrete_crps(certain, 2))


class NoLookaheadAuditTests(unittest.TestCase):
    @unittest.skipUnless((ROOT / "data/statsbomb/pl_hist/build_report.json").is_file(),
                         "historical inputs not built locally")
    def test_prepared_inputs_use_only_earlier_matches(self):
        report = json.loads((ROOT / "data/statsbomb/pl_hist/build_report.json").read_text())
        self.assertTrue(report["max_feature_date_before_fixture"],
                        "a fixture feature used a source match on or after its kickoff date")
        self.assertEqual(report["warnings"], [])
        fixtures = json.loads((ROOT / "data/statsbomb/pl_hist/fixtures.json").read_text())
        dates = [fixture["date"] for fixture in fixtures["fixtures"]]
        self.assertEqual(dates, sorted(dates), "fixtures must be chronologically ordered")


class CommittedArtifactTests(unittest.TestCase):
    @unittest.skipUnless(REPORT.is_file(), "same-engine OOS report not generated yet")
    def test_report_is_chronological_with_frozen_calibration(self):
        report = json.loads(REPORT.read_text())
        self.assertEqual(report["protocol"], "same_engine_walk_forward_frozen_pre_kickoff")
        self.assertLess(report["fit_end_date"], report["oos_start_date"])
        self.assertLess(report["oos_start_date"], report["oos_end_date"])
        self.assertTrue(report["tested_without_selection_or_tuning"])
        self.assertEqual(report["feature_contract"], "observed_stats_only_no_fm26_no_current_squads")
        self.assertGreaterEqual(report["oos_fixtures"], 12)
        for name, interval in report["matched_day_bootstrap_95"].items():
            self.assertEqual(len(interval), 2, name)
            self.assertTrue(all(isinstance(value, float) or isinstance(value, int) for value in interval))

    @unittest.skipUnless(PATHS.is_file(), "same-engine path export not generated yet")
    def test_path_export_coherence_and_conservation(self):
        document = json.loads(PATHS.read_text())
        self.assertEqual(document["feature_contract"], "observed_stats_only_no_fm26_no_current_squads")
        for fixture in document["fixtures"]:
            scoreline_sum = sum(cell[2] for cell in fixture["scoreline_distribution"])
            self.assertAlmostEqual(scoreline_sum, 1.0, places=6)
            raw_sum = sum(fixture["raw_probabilities"])
            self.assertAlmostEqual(raw_sum, 1.0, places=6)
            attributed = sum(player["expected_attributed_goals"] for player in fixture["players"])
            total = fixture["expected_home_goals"] + fixture["expected_away_goals"]
            self.assertAlmostEqual(attributed + fixture["unattributed_goals_per_match"], total, places=6)
            home_marginal = sum(probability for _, _, probability in
                                ((h, a, p) for h, a, p in fixture["scoreline_distribution"] if h > a))
            self.assertAlmostEqual(home_marginal, fixture["home_win"], places=6)


if __name__ == "__main__":
    unittest.main()
