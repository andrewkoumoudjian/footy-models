"""Provider bridge safety: redirect and provenance gates."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from audit_reep_crosswalk import classify_player_bridge, resolve_redirect, single


class ReepJoinTests(unittest.TestCase):
    def test_redirect_chain_and_cycles(self):
        self.assertEqual(resolve_redirect("old", {"old": "middle", "middle": "current"}), "current")
        with self.assertRaisesRegex(ValueError, "cycle"):
            resolve_redirect("a", {"a": "b", "b": "a"})

    def test_rejects_multiple_canonical_ids(self):
        self.assertIsNone(single({"11": {"reep_a", "reep_b"}}, "11"))
        self.assertEqual(single({"12": {"reep_c"}}, "12"), "reep_c")

    def test_rejects_mismatched_dob_and_nonplayer(self):
        item = {"fm_uids": ["10"], "sofascore_dob": "1997-01-02"}
        record = {"10": {"name": "A", "dob": "1998-02-03", "kind": "player", "attr_source": "db"}}
        self.assertEqual(classify_player_bridge(item, record)[0], "date_of_birth_conflict")
        record["10"]["dob"] = item["sofascore_dob"]
        record["10"]["kind"] = "staff"
        self.assertEqual(classify_player_bridge(item, record)[0], "fm_record_classified_nonplayer")

    def test_separates_database_and_reconstructed_ratings(self):
        item = {"fm_uids": ["10"], "sofascore_dob": "1997-01-02"}
        record = {"10": {"name": "A", "dob": "1997-01-02", "kind": "player", "attr_source": "db"}}
        self.assertEqual(classify_player_bridge(item, record)[0], "linked_player_with_db_attributes_and_matching_dob")
        record["10"]["attr_source"] = "derived"
        self.assertEqual(classify_player_bridge(item, record)[0], "linked_player_with_reconstructed_attributes_and_matching_dob")
        item["sofascore_dob"] = None
        self.assertEqual(classify_player_bridge(item, record)[0], "dob_not_independently_checked")


if __name__ == "__main__":
    unittest.main()
