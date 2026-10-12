"""UCL bundle generalization: closed maps, homonym ids, preserved EPL defaults."""
import sys
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from build_universal_bundle import (
    UCL_ESPN_TO_CANONICAL,
    ESPN_TO_CANONICAL,
    canonical_club,
    provisional_id,
)


class UclBundleGeneralizationTest(unittest.TestCase):
    def test_epl_default_map_unchanged(self):
        self.assertEqual(canonical_club("Manchester City"), "Man City")
        self.assertEqual(canonical_club("Nottingham Forest"), "Nott'm Forest")

    def test_ucl_map_is_closed_identity(self):
        self.assertEqual(len(UCL_ESPN_TO_CANONICAL), 36)
        for name, canonical in UCL_ESPN_TO_CANONICAL.items():
            self.assertEqual(canonical_club(name, UCL_ESPN_TO_CANONICAL), canonical)

    def test_unknown_club_refuses_to_alias(self):
        with self.assertRaises(ValueError):
            canonical_club("FC Copenhagen", UCL_ESPN_TO_CANONICAL)
        with self.assertRaises(ValueError):
            canonical_club("Inter", UCL_ESPN_TO_CANONICAL)

    def test_provisional_ids_are_stable_and_scoped(self):
        first = provisional_id("Shakhtar Donetsk", "Pedrinho")
        self.assertEqual(first, provisional_id("Shakhtar Donetsk", "Pedrinho"))
        self.assertNotEqual(first, provisional_id("Shakhtar Donetsk", "Pedrinho#2"))
        self.assertNotEqual(first, provisional_id("AEK Athens", "Pedrinho"))
        self.assertTrue(first >> 63 == 1)

    def test_expected_clubs_default_is_epl(self):
        import inspect
        from build_universal_bundle import build_bundle
        self.assertEqual(
            inspect.signature(build_bundle).parameters["expected_clubs"].default, 20)


if __name__ == "__main__":
    unittest.main()
