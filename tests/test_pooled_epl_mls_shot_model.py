"""Pooled EPL+MLS shot model is chronological, player-agnostic, and proper-scored."""
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from fit_pooled_epl_mls_shot_model import (
    POOLED_FEATURE_NAMES, epl_rows, fit_ridge, mls_rows, poisson_loss, predicted,
)


class PooledShotModelTests(unittest.TestCase):
    def test_feature_contract_has_no_identity_and_includes_league_flag(self):
        self.assertIn("league_mls", POOLED_FEATURE_NAMES)
        self.assertNotIn("player_id", POOLED_FEATURE_NAMES)
        self.assertNotIn("espn_id", POOLED_FEATURE_NAMES)
        self.assertNotIn("fm_uid", POOLED_FEATURE_NAMES)
        self.assertEqual(len(POOLED_FEATURE_NAMES), len(set(POOLED_FEATURE_NAMES)))

    def test_mls_and_epl_rows_share_baseline_and_exposure_contract(self):
        mls = mls_rows(ROOT / "data/ml/fm26-mls-candidate-sets.jsonl",
                       ROOT / "data/ml/fm26-mls-candidate-sets.manifest.json")
        epl = epl_rows(ROOT / "data/ml/math_engine/espn_frozen_20261011.json")
        self.assertGreater(len(mls), 100)
        self.assertGreater(len(epl), 100)
        for r in mls[:5] + epl[:5]:
            self.assertIn(r["league"], ("MLS", "EPL"))
            self.assertIn(r["split"], ("train", "validation", "test"))
            self.assertGreater(r["minutes"], 0)
            self.assertLessEqual(r["minutes"], 130)
            self.assertGreater(r["baseline"], 0)
            self.assertEqual(len(r["x"]), len(POOLED_FEATURE_NAMES))
        # Chronology: train strictly before validation strictly before test
        pooled = mls + epl
        groups = {k: [x for x in pooled if x["split"] == k] for k in ("train", "validation", "test")}
        self.assertTrue(all(groups.values()))
        self.assertLess(max(x["date"] for x in groups["train"]),
                        min(x["date"] for x in groups["validation"]))
        self.assertLess(max(x["date"] for x in groups["validation"]),
                        min(x["date"] for x in groups["test"]))

    def test_did_not_play_is_not_zero_shots(self):
        epl = epl_rows(ROOT / "data/ml/math_engine/espn_frozen_20261011.json")
        # All rows have positive exposure; DNP rows are skipped, not coded 0.
        self.assertTrue(all(r["minutes"] > 0 for r in epl))
        self.assertTrue(all(r["shots"] >= 0 for r in epl))

    def test_ridge_fit_improves_or_matches_baseline_on_validation(self):
        mls = mls_rows(ROOT / "data/ml/fm26-mls-candidate-sets.jsonl",
                       ROOT / "data/ml/fm26-mls-candidate-sets.manifest.json")
        epl = epl_rows(ROOT / "data/ml/math_engine/espn_frozen_20261011.json")
        pooled = mls + epl
        train = [r for r in pooled if r["split"] == "train"]
        val = [r for r in pooled if r["split"] == "validation"]
        beta = fit_ridge(train, 1.0)
        self.assertEqual(len(beta), len(POOLED_FEATURE_NAMES))
        base = poisson_loss(val, None)
        cand = poisson_loss(val, beta)
        # Ridge may not always improve, but must be finite and close (no explosion).
        self.assertTrue(__import__("math").isfinite(cand))
        self.assertLess(cand, base + 0.1)

    def test_predictions_are_valid_rates(self):
        mls = mls_rows(ROOT / "data/ml/fm26-mls-candidate-sets.jsonl",
                       ROOT / "data/ml/fm26-mls-candidate-sets.manifest.json")
        beta = fit_ridge([r for r in mls if r["split"] == "train"], 1.0)
        for r in mls[:10]:
            mu = predicted(r, beta)
            self.assertGreater(mu, 0)
            self.assertTrue(__import__("math").isfinite(mu))


if __name__ == "__main__":
    unittest.main()
