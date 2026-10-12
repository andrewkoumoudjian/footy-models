"""Complete-everyone FM union: no CA/kind/gender pre-filter, UID-keyed, order-free."""
import csv
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from fm26_complete import load_union


def write_csv(path, rows):
    cols = sorted({k for row in rows for k in row})
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=cols)
        writer.writeheader()
        writer.writerows(rows)


class CompleteUnionTests(unittest.TestCase):
    def test_complete_only_people_are_discovered_not_dropped(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [
                dict(uid="1", name="Men Player", dob="2000-01-01", kind="player",
                     ca="120", attr_Finishing="12"),
                dict(uid="2", name="Complete Only", dob="2001-02-02", kind="player",
                     ca="90", attr_Finishing="11"),
            ])
            write_csv(provenance, [
                dict(uid="1", name="Men Player", dob="2000-01-01", kind="player",
                     attr_source="db", ca="120", attr_Finishing="12"),
            ])
            by_uid, report = load_union(primary, provenance)
            self.assertIn("1", by_uid)
            self.assertIn("2", by_uid)  # absent from men, still solved
            self.assertEqual(report["complete_only_uids"], 1)
            self.assertEqual(by_uid["2"]["attr_source"], "unknown_no_men_provenance")
            self.assertEqual(by_uid["1"]["attr_source"], "db")

    def test_no_ca_kind_gender_prefilter(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [
                dict(uid="10", name="Low CA Youth", dob="2009-01-01", kind="player",
                     ca="5", attr_Finishing="8"),
                dict(uid="11", name="Miscat Staff", dob="1994-10-03", kind="staff",
                     ca="150", attr_Finishing="9"),
                dict(uid="12", name="Woman Player", dob="2000-05-05", kind="player",
                     gender="female", ca="130", attr_Finishing="13"),
            ])
            write_csv(provenance, [
                dict(uid="10", name="Low CA Youth", dob="2009-01-01", kind="player",
                     attr_source="db", ca="5", attr_Finishing="8"),
                dict(uid="11", name="Miscat Staff", dob="1994-10-03", kind="staff",
                     attr_source="db", ca="150", attr_Finishing="9"),
                dict(uid="12", name="Woman Player", dob="2000-05-05", kind="player",
                     attr_source="db", ca="130", attr_Finishing="13"),
            ])
            by_uid, _ = load_union(primary, provenance)
            for uid in ("10", "11", "12"):
                self.assertIn(uid, by_uid)
            self.assertEqual(by_uid["11"]["kind"], "staff")  # preserved, rescued downstream

    def test_duplicate_uid_quarantined_never_last_row_wins(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [
                dict(uid="20", name="First Row", dob="2000-01-01", kind="player",
                     ca="100", attr_Finishing="10"),
                dict(uid="20", name="Second Row", dob="2001-01-01", kind="staff",
                     ca="180", attr_Finishing="18"),
            ])
            write_csv(provenance, [dict(uid="21", name="Solo", kind="player",
                                        attr_source="db", ca="100")])
            by_uid, report = load_union(primary, provenance)
            self.assertNotIn("20", by_uid)
            self.assertIn("21", by_uid)
            self.assertEqual(report["duplicate_uids_quarantined"], 1)

    def test_file_order_does_not_change_union(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            rows = [dict(uid=str(i), name=f"Player {i}", dob="2000-01-01",
                         kind="player", ca="100", attr_Finishing=str(10 + (i % 5)))
                    for i in range(50)]
            forward = Path(tmp) / "forward.csv"
            backward = Path(tmp) / "backward.csv"
            write_csv(forward, rows)
            write_csv(backward, list(reversed(rows)))
            empty = Path(tmp) / "empty.csv"
            write_csv(empty, [])
            first, _ = load_union(forward, empty)
            second, _ = load_union(backward, empty)
            self.assertEqual(set(first), set(second))
            for uid in first:
                self.assertEqual(first[uid]["attributes"], second[uid]["attributes"])

    def test_provenance_column_never_enters_trait_union(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [
                dict(uid="40", name="Provenance Case", dob="2000-01-01", kind="player",
                     ca="120", attr_Finishing="12"),
            ])
            write_csv(provenance, [
                dict(uid="40", name="Provenance Case", dob="2000-01-01", kind="player",
                     attr_source="db", ca="120", attr_Finishing="12"),
            ])
            by_uid, _ = load_union(primary, provenance)
            self.assertNotIn("source", by_uid["40"]["attributes"])
            self.assertEqual(by_uid["40"]["attr_source"], "db")
            self.assertEqual(by_uid["40"]["attributes"]["Finishing"], 12.0)

    def test_implausible_height_is_missing(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [dict(uid="30", name="Tall Tale", kind="player",
                                     ca="100", height="65354")])
            write_csv(provenance, [dict(uid="30", name="Tall Tale", kind="player",
                                        attr_source="db", ca="100", height="65354")])
            by_uid, _ = load_union(primary, provenance)
            self.assertIsNone(by_uid["30"]["height_cm"])


class IndexEndToEndTests(unittest.TestCase):
    def test_both_frames_index_row_carries_provenance(self):
        import tempfile
        from build_fm_mls_shot_dataset import fm_row_usable, load_fm_index, normalize
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [
                dict(uid="1", name="Luca Bombino", dob="2006-07-10", kind="player",
                     ca="98", attr_Finishing="12"),
            ])
            write_csv(provenance, [
                dict(uid="1", name="Luca Bombino", dob="2006-07-10", kind="player",
                     attr_source="db", ca="98", attr_Finishing="12"),
            ])
            idx = load_fm_index(provenance, complete_path=primary)
            rows = idx.get((normalize("Luca Bombino"), "2006-07-10"), [])
            self.assertEqual(len(rows), 1)
            self.assertEqual(rows[0]["attr_source"], "db")
            self.assertEqual(rows[0]["kind"], "player")
            usable, rescued = fm_row_usable(rows)
            self.assertIsNotNone(usable)
            self.assertFalse(rescued)

    def test_staff_db_row_rescued_complete_everyone(self):
        import tempfile
        from build_fm_mls_shot_dataset import fm_row_usable, load_fm_index, normalize
        with tempfile.TemporaryDirectory() as tmp:
            primary = Path(tmp) / "complete.csv"
            provenance = Path(tmp) / "men.csv"
            write_csv(primary, [
                dict(uid="2", name="Miscat Staff", dob="1994-10-03", kind="staff",
                     ca="150", attr_Finishing="9"),
            ])
            write_csv(provenance, [
                dict(uid="2", name="Miscat Staff", dob="1994-10-03", kind="staff",
                     attr_source="db", ca="150", attr_Finishing="9"),
            ])
            idx = load_fm_index(provenance, complete_path=primary)
            rows = idx.get((normalize("Miscat Staff"), "1994-10-03"), [])
            usable, rescued = fm_row_usable(rows)
            self.assertIsNotNone(usable)
            self.assertTrue(rescued)


if __name__ == "__main__":
    unittest.main()
