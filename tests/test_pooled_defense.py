"""Defensive heads: label causality, feature contract, no-identity proofs."""
import json
import math
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from build_fm_mls_defense_dataset import (
    RECOVERY_ROLE_PRIORS,
    TACKLE_ROLE_PRIORS,
)
from fit_pooled_defense import HEADS

SAMPLES = ROOT / "data/ml/fm26-mls-defense.jsonl"


class DefenseContractTests(unittest.TestCase):
    def test_role_priors_are_positive_and_ordered(self):
        for priors in (TACKLE_ROLE_PRIORS, RECOVERY_ROLE_PRIORS):
            self.assertTrue(all(v > 0 for v in priors.values()))
            self.assertGreater(priors["M"], priors["F"])
            self.assertGreater(priors["D"], priors["G"])

    def test_head_feature_contracts_have_no_identity(self):
        for spec in HEADS.values():
            names = ["league_mls", "age_years_norm", "fm_ca_norm",
                     *(f"fm_{x}_norm" for x in spec["attrs"]),
                     "last_seen_log_days", "fm_verified"]
            self.assertEqual(len(names), len(set(names)))
            for banned in ("player_id", "espn_id", "fm_uid"):
                self.assertNotIn(banned, names)
        tackle_attrs = set(HEADS["tackles"]["attrs"])
        self.assertIn("Tackling", tackle_attrs)
        self.assertIn("Marking", tackle_attrs)

    @unittest.skipUnless(SAMPLES.exists(), "needs local defense labels")
    def test_labels_are_causal_and_split(self):
        splits = {"train": [], "validation": [], "test": []}
        for line in SAMPLES.read_text().splitlines():
            if not line.strip():
                continue
            s = json.loads(line)
            splits[s["split"]].append(s)
            self.assertGreater(s["minutes"], 0)
            self.assertLessEqual(s["minutes"], 130)
            for target, lab in s["labels"].items():
                self.assertGreaterEqual(lab["events"], 0)
                self.assertGreater(lab["prior_rate"], 0)
            self.assertIn(s["role"], ("G", "D", "M", "F"))
        for split, rows in splits.items():
            self.assertGreater(len(rows), 50, split)
        dates = {k: sorted(set(r["date"] for r in rows)) for k, rows in splits.items()}
        self.assertLess(max(dates["train"]), min(dates["validation"]))
        self.assertLess(max(dates["validation"]), min(dates["test"]))

    @unittest.skipUnless((ROOT / "data/ml/unified_forecast_heads_v2.json").exists(),
                         "needs fitted v2 artifact")
    def test_v2_artifact_keeps_v1_and_adds_tackles(self):
        report = json.loads((ROOT / "data/ml/unified_forecast_heads_v2.json").read_text())
        self.assertEqual(report["schema"], "unified_epl_mls_forecast_heads_v1")
        self.assertIn("shots", report)
        self.assertIn("tackles", report)
        self.assertIsNotNone(report["tackles"]["coefficients"])
        self.assertEqual(len(report["tackles"]["coefficients"]),
                         len(report["tackles"]["feature_names"]))
        ci = report["tackles"]["test_delta_95pc_matchday_bootstrap"]
        self.assertLess(ci[1], 0.0)


if __name__ == "__main__":
    unittest.main()
