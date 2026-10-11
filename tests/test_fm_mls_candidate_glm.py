"""Regularized candidate Poisson GLM must select with validation only."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from fit_fm_mls_candidate_glm import fit_validate, FEATURE_NAMES


def sample(split, date, number, shots, baseline=1.0):
    return {
        "split": split, "date": date, "match_id": number,
        "player_id": 100 + number, "shots": shots,
        "minutes": 90.0, "baseline": baseline,
        "x": [1.0] + [0.0] * (len(FEATURE_NAMES) - 1),
    }


class CandidateGlmTest(unittest.TestCase):
    def groups(self):
        return (
            [sample("train", "2026-04-10", i, 4) for i in range(12)]
            + [sample("validation", "2026-07-10", 20 + i, 0) for i in range(6)]
            + [sample("test", "2026-08-10", 40 + i, 7) for i in range(6)]
        )

    def test_validation_refuses_overfit_even_if_test_would_prefer_it(self):
        first, _ = fit_validate(self.groups())
        self.assertEqual(first["best"], "identity")
        alt = self.groups()
        for row in alt:
            if row["split"] == "test":
                row["shots"] = 100
        later, _ = fit_validate(alt)
        self.assertEqual(first["best"], later["best"])
        self.assertEqual(first["validation"], later["validation"])
        self.assertNotEqual(first["test"], later["test"])

    def test_nonchronological_split_is_rejected(self):
        rows = self.groups()
        for item in rows:
            if item["split"] == "test":
                item["date"] = "2026-05-10"
        with self.assertRaisesRegex(ValueError, "overlapping"):
            fit_validate(rows)


if __name__ == "__main__":
    unittest.main()
