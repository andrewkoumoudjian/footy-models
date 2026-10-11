import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from score_fm_mls_candle import per_player_delta, summarize_split


class PairedShotScoreTest(unittest.TestCase):
    def test_exact_identical_models_have_zero_delta_and_ci(self):
        rows = [
            {"match_id": i, "shots": i % 3,
             "baseline_expected_shots": 1.5, "candle_expected_shots": 1.5}
            for i in range(10)
        ]
        report = summarize_split(rows, draws=500)
        self.assertEqual(report["mean_poisson_log_loss_delta_candidate_minus_baseline"], 0.0)
        self.assertEqual(report["paired_fixture_bootstrap_95pct"], [0.0, 0.0])

    def test_delta_is_poisson_proper_score_difference(self):
        self.assertAlmostEqual(
            per_player_delta({
                "shots": 2, "baseline_expected_shots": 1, "candle_expected_shots": 2
            }),
            1 - 2 * math.log(2)
        )

    def test_single_fixture_cannot_claim_uncertainty(self):
        with self.assertRaisesRegex(ValueError, "fewer than two"):
            summarize_split([{
                "match_id": 1, "shots": 1,
                "baseline_expected_shots": 1.5, "candle_expected_shots": 1.4
            }])


if __name__ == "__main__":
    unittest.main()
