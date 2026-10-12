"""Keep the 20-club official league roster distinct from a fixture shortlist."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from build_unified_prematch_bundle import build_bundle
from match_premier_league_fm26 import TEAM_ALIASES, fetch_official


class EPLFullLeagueRosterTests(unittest.TestCase):
    def test_current_season_maps_all_20_clubs(self):
        self.assertEqual(len(TEAM_ALIASES), 20)
        self.assertEqual(len(set(TEAM_ALIASES.values())), 20)
        for essential in ("Arsenal", "Chelsea", "Liverpool", "Manchester City", "Tottenham Hotspur"):
            self.assertIn(essential, TEAM_ALIASES.values())

    def test_full_official_article_contains_all_twenty_or_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            file = Path(tmp) / "pl.html"
            club_sections = "".join(
                f"<h5>{club}</h5><p>25 Squad players (*Home grown)"
                "<br/>Senior, Example</p><p>U21 players (Contract and Scholars; *Home grown)"
                "<br/>Youth, Example</p>"
                for club in TEAM_ALIASES.values()
            )
            file.write_text(f'<div class="article__content">{club_sections}</div>')
            roster = fetch_official(file, offline=True)
            self.assertEqual(set(roster), set(TEAM_ALIASES.values()))
            self.assertTrue(all(len(members) == 2 for members in roster.values()))
            self.assertTrue(all([member["registration"] for member in members] == ["registered", "u21"]
                                for members in roster.values()))

    def test_eight_club_fixture_sample_cannot_be_used_as_epl_league(self):
        incomplete = {"as_of": "2026-10-11", "roster_scope": "fixture_subset",
                      "teams": {name: {"players": []} for name in list(TEAM_ALIASES)[:8]}}
        with self.assertRaisesRegex(ValueError, "complete 20-club"):
            build_bundle("EPL", "2026-10-11", roster=incomplete, reep={},
                         sofa_root=None, fm_path=None)


if __name__ == "__main__":
    unittest.main()
