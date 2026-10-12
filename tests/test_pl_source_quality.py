"""Source-boundary invariants: no fabricated observations or CA identity tie-breaks."""
import csv
import tempfile
import unittest
from datetime import date
from pathlib import Path

from scripts import match_premier_league_fm26 as matching
from scripts import prepare_pl_player_inputs as prepare


class PremierLeagueSourceQualityTests(unittest.TestCase):
    def test_identical_names_with_distinct_uids_are_ambiguous(self):
        records = [
            {"uid": "1", "name": "John Smith", "ca": 150., "kind": "staff", "attr_source": "db"},
            {"uid": "2", "name": "John Smith", "ca": 80., "kind": "player", "attr_source": "db"},
        ]
        candidate, method, suggestions = matching.best_match(
            "John Smith", {"john smith": records}, {}, records,
        )
        self.assertIsNone(candidate)
        self.assertEqual(method, "ambiguous exact")
        self.assertEqual(len(suggestions), 2)

    def test_missing_and_zero_are_distinct(self):
        self.assertIsNone(prepare.observed_number({"HxG": ""}, "HxG"))
        self.assertIsNone(prepare.observed_number({}, "HxG"))
        self.assertEqual(prepare.observed_number({"HxG": "0"}, "HxG"), 0.)
        with self.assertRaises(ValueError):
            prepare.observed_number({"HxG": "not-a-number"}, "HxG")

    def test_team_results_preserve_source_missingness_and_cutoff(self):
        previous = prepare.OUT, prepare.CUTOFF, prepare.CONFIG
        try:
            with tempfile.TemporaryDirectory() as directory:
                prepare.OUT = Path(directory)
                prepare.CUTOFF = date(2026, 10, 10)
                prepare.CONFIG = {"manual_results": []}
                raw = prepare.OUT / "raw"
                raw.mkdir()
                with (raw / "sample_E0.csv").open("w", newline="") as file:
                    writer = csv.DictWriter(file, fieldnames=[
                        "Date", "HomeTeam", "AwayTeam", "FTHG", "FTAG",
                        "HxG", "AxG", "HS", "AS", "HST", "AST",
                    ])
                    writer.writeheader()
                    writer.writerow({"Date": "08/10/2026", "HomeTeam": "Liverpool", "AwayTeam": "Everton",
                                     "FTHG": 2, "FTAG": 0, "HS": 12})
                    writer.writerow({"Date": "09/10/2026", "HomeTeam": "Man City", "AwayTeam": "Newcastle",
                                     "FTHG": 0, "FTAG": 1, "HxG": 0, "AxG": 0.8})
                    writer.writerow({"Date": "10/10/2026", "HomeTeam": "Everton", "AwayTeam": "Man City",
                                     "FTHG": 1, "FTAG": 0, "HxG": 1.5})
                self.assertEqual(prepare.write_results(), 2)
                with (prepare.OUT / "asof_team_stats.csv").open() as file:
                    rows = list(csv.DictReader(file))
                self.assertEqual(rows[0]["home_xg"], "")
                self.assertEqual(rows[0]["away_xg_kind"], "missing")
                self.assertEqual(rows[0]["home_shots_kind"], "measured")
                self.assertEqual(rows[0]["away_shots_kind"], "missing")
                self.assertEqual(rows[1]["home_xg"], "0.0")
                self.assertEqual(rows[1]["home_xg_kind"], "measured")
        finally:
            prepare.OUT, prepare.CUTOFF, prepare.CONFIG = previous

    def test_position_and_provisional_identity(self):
        self.assertEqual(prepare.position_group("Attack"), "FWD")
        self.assertEqual(prepare.position_group("Goalkeeper"), "GK")
        uid = prepare.official_candidate_id("Liverpool", "Unknown, Player")
        self.assertEqual(uid, prepare.official_candidate_id("Liverpool", "Unknown, Player"))
        self.assertGreater(int(uid), 2**63)


if __name__ == "__main__":
    unittest.main()
