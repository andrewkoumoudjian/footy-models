"""Offline synthetic coverage for soccerdata FBref output contracts."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from import_fbref_soccerdata import convert, export_df_rows, normalize_row


SCHEDULE = [{
    "league": "ENG-Premier League", "season": "2020-2021",
    "game": "2020-09-12 Arsenal Fulham",
    "game_id": "ab12cd34", "date": "2020-09-12",
    "home_team": "Fulham", "away_team": "Arsenal",
}, {
    "league": "ENG-Premier League", "season": "2020-2021",
    "game": "2020-09-19 Arsenal West Ham",
    "game_id": "ab12cd35", "date": "2020-09-19",
    "home_team": "Arsenal", "away_team": "West Ham",
}]
PLAYERS = [{
    "league": "ENG-Premier League", "season": "2020-2021",
    "game": "2020-09-12 Arsenal Fulham",
    "team": "Arsenal", "player": "Pierre-Emerick Aubameyang",
    "Pos": "FW", "Min": "90", "Performance_Sh": "3",
}, {
    "league": "ENG-Premier League", "season": "2020-2021",
    "game": "2020-09-19 Arsenal West Ham",
    "team": "Arsenal", "player": "Pierre-Emerick Aubameyang",
    "pos": "FW", "min": 85, "performance_sh": 1,
}]


class FbrefContractTest(unittest.TestCase):
    def test_fixture_stable_id_date_and_labels(self):
        rows, manifest = convert(SCHEDULE, PLAYERS, league="ENG-Premier League", season="2020-2021")
        self.assertEqual(len(rows), 2)
        self.assertEqual([r["shots"] for r in rows], [3, 1])
        self.assertEqual([r["minutes"] for r in rows], [90, 85])
        self.assertEqual([r["date"] for r in rows], ["2020-09-12", "2020-09-19"])
        self.assertEqual(rows[0]["source_match_id"], "ab12cd34")
        self.assertEqual(rows[0]["player_identity_status"], "NAME_ONLY_NOT_VETTED_FOR_FM_JOIN")
        self.assertIn("source_sha256", manifest)

    def test_cutoff_rejects_future_matches(self):
        rows, manifest = convert(SCHEDULE, PLAYERS, end="2020-09-19")
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["shots"], 3)
        self.assertEqual(manifest["warnings"]["player_unmatched_schedule"], 1)

    def test_missing_shots_skipped_not_zero_imputed(self):
        p = [{**PLAYERS[0], "Performance_Sh": ""}]
        rows, manifest = convert(SCHEDULE, p)
        self.assertEqual(rows, [])
        self.assertEqual(manifest["warnings"]["missing_player_minutes_or_shots"], 1)

    def test_source_only_name_cannot_be_treated_as_fm_identity(self):
        player = {**PLAYERS[0], "player": "Ambiguous Name"}
        rows, _ = convert(SCHEDULE, [player])
        self.assertEqual(rows[0]["player"], "Ambiguous Name")
        self.assertEqual(rows[0]["player_identity_status"], "NAME_ONLY_NOT_VETTED_FOR_FM_JOIN")

    def test_invalid_duplicate_or_bad_team_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate player"):
            convert(SCHEDULE, [PLAYERS[0], PLAYERS[0]])
        with self.assertRaisesRegex(ValueError, "not in fixture"):
            convert(SCHEDULE, [{**PLAYERS[0], "team": "Chelsea"}])

    def test_negative_missing_and_inconsistent_minutes(self):
        with self.assertRaisesRegex(ValueError, "bad shots"):
            convert(SCHEDULE, [{**PLAYERS[0], "Performance_Sh": -2}])
        with self.assertRaisesRegex(ValueError, "positive shots"):
            convert(SCHEDULE, [{**PLAYERS[0], "Min": "0"}])

    def test_multilevel_soccerdata_column_support(self):
        import pandas as pd
        df = pd.DataFrame({
            ("league", ""): ["ENG-Premier League"],
            ("season", ""): ["2020-2021"],
            ("game", ""): ["match"],
            ("team", ""): ["Arsenal"],
            ("player", ""): ["Player A"],
            ("Performance", "Sh"): [2],
            ("Playing Time", "Min"): [90],
        })
        rows = export_df_rows(df)
        self.assertEqual(rows[0]["performance_sh"], "2")
        self.assertEqual(rows[0]["playing_time_min"], "90")
        self.assertEqual(rows[0]["league"], "ENG-Premier League")

    def test_different_source_column_names_same_semantics(self):
        row = normalize_row({"Performance|Sh": "2", "Min": 73})
        self.assertEqual(row["performance_sh"], "2")
        self.assertEqual(row["min"], "73")


if __name__ == "__main__":
    unittest.main()
