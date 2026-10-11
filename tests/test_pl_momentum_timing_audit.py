"""Timing audit must be a scoring-side operation: no future-label features."""
import copy
import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from audit_pl_momentum_timing import (
    pressure_index, score_timing, probability_at_least_two_of_three, rerun_drift,
)


def sample():
    windows = []
    for i in range(6):
        windows.append({
            "minute_start": 15 * i, "minute_end": 15 * (i + 1),
            "expected_home_shots": 2.0,
            "expected_away_shots": 4.0 if i == 4 else 2.0,
            "expected_home_goals": 0.1,
            "expected_away_goals": 0.2 if i == 4 else 0.1,
            "p_red_card": 0.05,
            "p_substitution": 0.8 if i >= 4 else 0.1,
        })
    return {
        "as_of": "2026-10-10",
        "fixtures": [{
            "date": "2026-10-11", "home": "Liverpool", "away": "Man City",
            "seed": 44, "paths": 72, "expected_away_goals": 0.7,
            "15_minute_windows": windows,
        }]
    }, {
        "date": "2026-10-11", "home": "Liverpool", "away": "Man City",
        "home_goals": 0, "away_goals": 3,
        "away_goal_minutes": [65, 68, 78],
        "home_red_card_minutes": [62],
        "provenance": {"actual_result_observed_after_match": True},
    }


class GoalTimingAuditTest(unittest.TestCase):
    def test_pressure_peak_and_distinct_absolute_goal_peak(self):
        forecast, actual = sample()
        report = score_timing(forecast, actual)
        self.assertEqual(report["pressure_peak_minutes"], [60, 75])
        self.assertEqual(report["actual_goals_in_pressure_peak"], 2)
        self.assertEqual(report["actual_goal_windows"], [4, 4, 5])
        self.assertEqual(report["actual_red_card_windows"], [4])
        self.assertEqual(report["pressure_peak_window"], 4)
        self.assertTrue(math.isfinite(
            report["conditional_away_goal_time_negative_log_loss"]["model"]
        ))

    def test_asof_violation_and_wrong_result_fail_closed(self):
        forecast, actual = sample()
        forecast["as_of"] = "2026-10-11"
        with self.assertRaisesRegex(ValueError, "pre-match"):
            score_timing(forecast, actual)
        forecast["as_of"] = "2026-10-10"
        actual["away_goals"] = 2
        with self.assertRaisesRegex(ValueError, "inconsistent"):
            score_timing(forecast, actual)

    def test_timing_peak_is_not_selected_from_actual_goals(self):
        forecast, actual = sample()
        first = score_timing(forecast, actual)
        changed = copy.deepcopy(actual)
        changed["away_goal_minutes"] = [5, 21, 39]
        after = score_timing(forecast, changed)
        self.assertEqual(first["pressure_peak_window"], after["pressure_peak_window"])
        self.assertNotEqual(
            first["conditional_away_goal_time_negative_log_loss"]["model"],
            after["conditional_away_goal_time_negative_log_loss"]["model"]
        )

    def test_binomial_comparison_and_finite_pressure(self):
        self.assertAlmostEqual(probability_at_least_two_of_three(1 / 6), 2 / 27)
        self.assertEqual(pressure_index({
            "expected_home_shots": 0, "expected_away_shots": 0,
            "expected_home_goals": 0, "expected_away_goals": 0,
        }), 0)
        with self.assertRaisesRegex(ValueError, "invalid expected"):
            pressure_index({
                "expected_home_shots": -1, "expected_away_shots": 0,
                "expected_home_goals": 0, "expected_away_goals": 0,
            })

    def test_identical_rerun_preserves_all_timeline_cells(self):
        forecast, _ = sample()
        reproduction = rerun_drift(forecast, copy.deepcopy(forecast))
        self.assertTrue(reproduction["exact_equal_full_15_min_windows"])
        self.assertEqual(reproduction["maximum_absolute_pressure_index_difference"], 0)
        self.assertEqual(reproduction["original_peak_window"], reproduction["rerun_peak_window"])
        other = copy.deepcopy(forecast)
        other["fixtures"][0]["seed"] = 99
        with self.assertRaisesRegex(ValueError, "different match dates, seeds or path counts"):
            rerun_drift(forecast, other)


if __name__ == "__main__":
    unittest.main()
