"""Historical OOS scoring uses independent event labels and strict time splits."""
import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from score_pl_hist_momentum_oos import (
    observed_windows, score_fixture, oos_split, fit_only_goal_clock_prior,
)
import json
import tempfile


def forecast():
    return {
        "match_id": 1, "date": "2016-02-13", "home": "Home", "away": "Away",
        "windows_15min": [
            {
                "minute_start": i * 15, "minute_end": (i + 1) * 15,
                "expected_home_shots": 1.0,
                "expected_away_shots": 2.0 if i == 4 else 1.0,
                "expected_home_goals": 0.2,
                "expected_away_goals": 0.3 if i == 4 else 0.2,
            }
            for i in range(6)
        ],
        "minutes_1min": [{} for _ in range(90)],
    }


def label():
    return {
        "match_id": 1, "date": "2016-02-13", "home": "Home", "away": "Away",
        "home_goals": 0, "away_goals": 1,
        "goals": [{"team": "Away", "minute_index": 65, "own_goal": False}],
        "teams": {"Home": {"shots": 1}, "Away": {"shots": 2}},
    }


def events():
    return [
        {"type": {"name": "Shot"}, "team": {"name": "Home"}, "minute": 4},
        {"type": {"name": "Shot"}, "team": {"name": "Away"}, "minute": 40},
        {"type": {"name": "Shot"}, "team": {"name": "Away"}, "minute": 66},
    ]


class HistMomentumTest(unittest.TestCase):
    def test_raw_statsbomb_windows_and_scored_outcomes(self):
        windows = observed_windows(forecast(), events(), label()["goals"])
        self.assertEqual(windows[0]["home_shots"], 1)
        self.assertEqual(windows[2]["away_shots"], 1)
        self.assertEqual(windows[4]["away_shots"], 1)
        self.assertEqual(windows[4]["away_goals"], 1)
        item = score_fixture(forecast(), label(), events())
        self.assertEqual(item["goal_count"], 1)
        # Both the 30–45 and 60–75 bins were 100% away pressure (no
        # home shot), so the normalized observed-pressure target is tied.
        self.assertEqual(item["away_pressure_peak_exact_window_hit"], 0)
        self.assertEqual(item["away_peak_match_fraction_tie_adjusted"], 0.5)
        self.assertEqual(item["pressure_windows_evaluated"], 3)

    def test_same_match_label_count_audited(self):
        bad = copy.deepcopy(label())
        bad["teams"]["Away"]["shots"] = 3
        with self.assertRaisesRegex(ValueError, "ground truth shot count"):
            score_fixture(forecast(), bad, events())

    def test_oos_never_splits_same_calendar_date(self):
        fixtures = [
            {"date": date, "match_id": i}
            for i, date in enumerate((
                "2016-01-01", "2016-01-02", "2016-01-03",
                "2016-01-03", "2016-01-05",
            ))
        ]
        train, test = oos_split(fixtures, 2)
        self.assertEqual(len(train), 2)
        self.assertEqual(len(test), 3)
        self.assertEqual(train[-1]["date"], "2016-01-02")

    def test_fit_only_goal_clock_uses_only_prior_labels(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "1.json").write_text(json.dumps({
                "date": "2016-01-01", "home": "Home", "away": "Away",
                "goals": [{"team": "Home", "minute_index": 75}]
            }))
            (root / "2.json").write_text(json.dumps({
                "date": "2016-02-13", "home": "Home", "away": "Away",
                "goals": [{"team": "Away", "minute_index": 1}] * 100
            }))
            prior, count = fit_only_goal_clock_prior(
                [{"match_id": 1, "date": "2016-01-01", "home": "Home", "away": "Away"}],
                root,
            )
            self.assertEqual(count, 1)
            self.assertAlmostEqual(prior[5], 4 / 19)
            self.assertAlmostEqual(prior[0], 3 / 19)
            # A later OOS fixture can exist on disk without entering the fit.
            (root / "2.json").write_text("{}")
            self.assertEqual(
                (prior, count),
                fit_only_goal_clock_prior(
                    [{"match_id": 1, "date": "2016-01-01", "home": "Home", "away": "Away"}],
                    root,
                ),
            )

    def test_fit_only_goal_clock_scored_without_future_refitting(self):
        prior = [0.1, 0.1, 0.1, 0.1, 0.1, 0.5]
        scored = score_fixture(forecast(), label(), events(), fit_only_clock=prior)
        self.assertAlmostEqual(scored["fit_only_goal_time_log_loss"], -__import__("math").log(0.1))
        self.assertEqual(scored["goal_count"], 1)


if __name__ == "__main__":
    unittest.main()
