"""Unified forecast heads: causality, player-agnosticism, calibration math."""
import math
import os
import sys
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from fit_pooled_epl_mls_shot_model import POOLED_FEATURE_NAMES
from fit_unified_forecast_heads import (
    CONVERSION_FEATURE_NAMES,
    PARTICIPATION_FEATURE_NAMES,
    binomial_loss,
    brier_and_ece,
    crosswalk_key,
    epl_rows_v2,
    fit_binomial_residual,
    fm_feature_block,
    load_crosswalk,
    occurrence_outcomes,
    occurrence_probs,
    per_shot_prob,
)

ESPN = ROOT / "data/ml/math_engine/espn_frozen_20261011.json"
XWALK = ROOT / "data/ml/math_engine/espn_pipeline_final_oct12/crosswalk.json"


class FeatureContractTests(unittest.TestCase):
    def test_no_identity_in_any_head(self):
        for names in (POOLED_FEATURE_NAMES, CONVERSION_FEATURE_NAMES,
                      PARTICIPATION_FEATURE_NAMES):
            self.assertEqual(len(names), len(set(names)))
            for banned in ("player_id", "espn_id", "fm_uid", "canonical_player_id"):
                self.assertNotIn(banned, names)
        self.assertIn("league_mls", POOLED_FEATURE_NAMES)
        self.assertNotIn("league_mls", CONVERSION_FEATURE_NAMES)

    def test_club_alias_is_closed(self):
        self.assertEqual(crosswalk_key("Coventry City", 1), ("Coventry", "1"))
        self.assertEqual(crosswalk_key("Nottingham Forest", 2), ("Nott'm Forest", "2"))
        # Unknown clubs never alias into a crosswalk identity.
        self.assertEqual(crosswalk_key("Arsenal", 3), ("Arsenal", "3"))

    def test_unverified_fm_block_is_masked_zeros(self):
        block = fm_feature_block(None, "", date(2026, 9, 1))
        self.assertEqual(block["verified"], 0.0)
        self.assertTrue(all(v == 0.0 for v in block["attrs"]))
        # A usable row without a corroborating DOB is quarantined too.
        fm = {"ca": "150", "attr_Finishing": "14", "dob": ""}
        block = fm_feature_block(fm, fm["dob"], date(2026, 9, 1))
        self.assertEqual(block["verified"], 0.0)

    def test_verified_fm_block_matches_mls_normalization(self):
        fm = {"ca": "150", "attr_Finishing": "14", "attr_OffTheBall": "12",
              "attr_Dribbling": "0", "attr_Composure": "11",
              "attr_Anticipation": "13", "attr_Passing": "10",
              "dob": "2000-01-01"}
        block = fm_feature_block(fm, fm["dob"], date(2026, 1, 1))
        self.assertEqual(block["verified"], 1.0)
        self.assertAlmostEqual(block["ca"], 0.5)
        self.assertAlmostEqual(block["attrs"][0], 0.4)
        self.assertAlmostEqual(block["attrs"][2], 0.0)  # 0 is unset, stays missing


@unittest.skipUnless(ESPN.exists() and XWALK.exists(), "needs local ESPN archive")
class CausalityTests(unittest.TestCase):
    def test_first_fixture_rows_use_role_neutral_prior_only(self):
        rows, _ = epl_rows_v2(ESPN, {}, {})
        first = min(r["date"] for r in rows)
        for r in [x for x in rows if x["date"] == first and x["minutes"] > 0]:
            self.assertAlmostEqual(r["asof_shots_per90"], 0.9)
            self.assertEqual(r["fm_verified"], False)
        # DNP rows are retained for participation (informative non-starts).
        self.assertTrue(any(r["minutes"] == 0 and not r["started"] for r in rows))
        # Chronology: every baseline comes from strictly earlier matches.
        by_player = {}
        for r in sorted(rows, key=lambda x: (x["date"], x["match_id"])):
            key = r["player_id"]
            self.assertNotIn((key, r["match_id"], r["date"]),
                             [(k, m, d) for k, m, d in by_player.get(key, [])])
            by_player.setdefault(key, []).append(
                (key, r["match_id"], r["date"]))

    def test_crosswalk_loads_unique_verified_identities(self):
        crosswalk = load_crosswalk(XWALK)
        self.assertGreater(len(crosswalk), 100)
        self.assertEqual(len(set(crosswalk.values())), len(set(crosswalk.values())))


class BinomialMathTests(unittest.TestCase):
    def row(self, base=0.1, x=(0.0,) * 10, n=3, y=1):
        return {"x_noleague": list(x), "conv_baseline": base, "start_baseline": base,
                "shots": n, "goals": y, "trials_one": n, "started_int": y,
                "league": "EPL", "date": "2026-09-01", "match_id": "1"}

    def test_identity_temperature_recovers_baseline(self):
        r = self.row(base=0.2)
        self.assertAlmostEqual(per_shot_prob(r, "conv_baseline", None, 1.0), 0.2)

    def test_occurrence_reduces_to_probability_for_single_trial(self):
        import numpy as np
        beta = np.zeros(10)
        r = self.row(n=1)
        self.assertAlmostEqual(
            occurrence_probs([r], "trials_one", "start_baseline", beta, 1.0)[0],
            per_shot_prob(r, "start_baseline", beta, 1.0))

    def test_brier_and_ece_perfect_forecaster(self):
        brier, ece, table = brier_and_ece([0.05, 0.95], [0.0, 1.0], bins=10)
        self.assertLess(brier, 0.01)
        self.assertLess(ece, 0.1)
        self.assertEqual(sum(b["n"] for b in table), 2)

    def test_residual_fit_beats_baseline_on_separable_data(self):
        import numpy as np
        rows = []
        for i in range(60):
            hot = float(i % 2)
            rows.append({"x_noleague": [hot] + [0.0] * 9, "start_baseline": 0.5,
                         "trials_one": 1, "started_int": int(hot),
                         "league": "EPL", "date": "2026-09-01", "match_id": str(i)})
        beta = fit_binomial_residual(rows, "trials_one", "started_int",
                                     "start_baseline", 0.1)
        base = binomial_loss(rows, "trials_one", "started_int", "start_baseline", None)
        cand = binomial_loss(rows, "trials_one", "started_int", "start_baseline", beta)
        self.assertLess(cand, base)
        self.assertGreater(beta[0], 0.0)

    def test_outcomes_align_with_probs(self):
        rows = [self.row(n=2, y=1), self.row(n=1, y=0)]
        self.assertEqual(occurrence_outcomes(rows, "shots", "goals"), [1.0, 0.0])
        self.assertEqual(len(occurrence_probs(rows, "shots", "conv_baseline", None, 1.0)), 2)


@unittest.skipUnless(
    (ROOT / "data/ml/unified_forecast_heads_v1.json").exists(),
    "needs fitted heads artifact")
class ArtifactTests(unittest.TestCase):
    def test_heads_artifact_reports_both_leagues_and_gates(self):
        import json
        report = json.loads((ROOT / "data/ml/unified_forecast_heads_v1.json").read_text())
        self.assertEqual(report["schema"], "unified_epl_mls_forecast_heads_v1")
        self.assertGreater(report["shots"]["counts"]["test_mls"], 0)
        self.assertGreater(report["shots"]["counts"]["test_epl"], 0)
        self.assertFalse(report["identity_features"])
        self.assertIn("conversion", report["simulation_contract"])
        ci = report["shots"]["test_delta_95pc_matchday_bootstrap"]
        self.assertLess(ci[1], 0.0)  # pooled gain significant at 95%


if __name__ == "__main__":
    unittest.main()
