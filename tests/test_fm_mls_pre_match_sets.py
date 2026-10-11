"""Fixture candidate membership must never depend on actual current match players."""
import json
import sys
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from build_fm_mls_pre_match_sets import build_sets, SET_FEATURE_NAMES


def epoch(iso):
    return int(datetime.fromisoformat(iso).replace(tzinfo=timezone.utc).timestamp())


def entry(pid, name, shots):
    return {
        "player": {"id": pid, "name": name, "position": "F",
                   "dateOfBirthTimestamp": epoch("2000-01-01")},
        "position": "F",
        "statistics": {"minutesPlayed": 90, "totalShots": shots},
    }


class PreMatchSetTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "player_matches").mkdir()
        events = []
        for mid, date, home_players in (
            (1, "2026-03-15", [entry(1, "Existing Striker", 2)]),
            (2, "2026-04-10", [entry(1, "Existing Striker", 4),
                              entry(3, "Future Debutant", 5)]),
            (3, "2026-07-10", [entry(1, "Existing Striker", 1),
                              entry(3, "Future Debutant", 3)]),
            (4, "2026-08-10", [entry(1, "Existing Striker", 2)]),
        ):
            events.append({
                "id": mid, "startTimestamp": epoch(date),
                "homeTeam": {"id": 10}, "awayTeam": {"id": 20},
            })
            (self.root / "player_matches" / f"{mid}.json").write_text(json.dumps({
                "home": {"players": home_players},
                "away": {"players": [entry(2, "Away Striker", 1)]},
            }))
        (self.root / "history_manifest.json").write_text(json.dumps({"events": events}))
        (self.root / "charlotte_history_manifest.json").write_text("[]")
        (self.root / "opponent_history_manifest.json").write_text("{}")
        self.fm = self.root / "fm.csv"
        self.fm.write_text(
            "uid,name,dob,kind,attr_source,ca,attr_Finishing\n"
            "1,Existing Striker,2000-01-01,player,db,140,15\n"
            "2,Away Striker,2000-01-01,player,db,140,15\n"
            "3,Future Debutant,2000-01-01,player,db,140,15\n"
        )

    def test_asof_candidate_pool_excludes_first_appearance(self):
        samples, manifest = build_sets(self.fm, self.root)
        self.assertEqual(len(samples), 3)
        first = samples[0]
        self.assertEqual(first["date"], "2026-04-10")
        self.assertEqual({c["player_id"] for c in first["candidates"]}, {1, 2})
        existing = next(c for c in first["candidates"] if c["player_id"] == 1)
        self.assertEqual(existing["label"]["shots"], 4)
        # First appearance itself is forbidden as pre-match candidate information.
        self.assertNotIn(3, {c["player_id"] for c in first["candidates"]})
        # Later games can now include the player, based on the *earlier* debut.
        self.assertIn(3, {c["player_id"] for c in samples[1]["candidates"]})
        self.assertEqual(samples[0]["candidate_count"], 2)
        self.assertEqual(len(existing["features"]), len(SET_FEATURE_NAMES))
        self.assertEqual(manifest["counts"]["test_fixtures"], 1)

    def test_mutating_future_outcomes_never_changes_past_features(self):
        original, _ = build_sets(self.fm, self.root)
        future_path = self.root / "player_matches" / "3.json"
        future = json.loads(future_path.read_text())
        future["home"]["players"][0]["statistics"]["totalShots"] = 100
        future_path.write_text(json.dumps(future))
        changed, _ = build_sets(self.fm, self.root)
        self.assertEqual(original[0], changed[0])
        # July samples may change their labels, but nothing from April can.
        self.assertNotEqual(original[1], changed[1])

    def test_all_samples_have_no_realized_minutes_in_features(self):
        samples, _ = build_sets(self.fm, self.root)
        self.assertTrue(samples)
        self.assertNotIn("shots", SET_FEATURE_NAMES)
        self.assertNotIn("minutes", SET_FEATURE_NAMES)
        for sample in samples:
            self.assertGreater(sample["candidate_count"], 0)
            for candidate in sample["candidates"]:
                self.assertEqual(len(candidate["features"]), len(SET_FEATURE_NAMES))
                self.assertEqual(candidate["features"][-3], float(candidate["side"] == "home"))


if __name__ == "__main__":
    unittest.main()
