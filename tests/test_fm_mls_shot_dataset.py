"""Chronological as-of dataset invariants; synthetic fixtures only, no external downloads."""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from datetime import datetime, timezone

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from build_fm_mls_shot_dataset import make_dataset, FEATURE_NAMES


def kickoff(date):
    return int(datetime.fromisoformat(date).replace(tzinfo=timezone.utc).timestamp())


def player(shots, name="Goal Scorer", dob="2000-01-01"):
    return {"player": {
        "id": 123, "name": name,
        "dateOfBirthTimestamp": kickoff(dob), "position": "F",
    }, "position": "F", "statistics": {
        "minutesPlayed": 90, "totalShots": shots,
    }}


class FmMlsAsOfTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "player_matches").mkdir()
        events = []
        for id, date, shots in [
            (1, "2026-03-20", 4),
            (2, "2026-04-10", 2),
            (3, "2026-05-10", 3),
            (4, "2026-07-10", 1),
            (5, "2026-08-10", 5),
        ]:
            events.append({"id": id, "startTimestamp": kickoff(date)})
            (self.root / "player_matches" / f"{id}.json").write_text(
                json.dumps({"home": {"players": [player(shots)]}, "away": {"players": []}})
            )
        (self.root / "history_manifest.json").write_text(json.dumps({"events": events}))
        (self.root / "charlotte_history_manifest.json").write_text("[]")
        (self.root / "opponent_history_manifest.json").write_text("{}")
        self.fm = self.root / "fm.csv"
        self.fm.write_text(
            "uid,name,dob,kind,attr_source,ca,attr_Finishing,attr_Pace\n"
            "999,Goal Scorer,2000-01-01,player,db,140,15,18\n"
            "1000,Derived Defender,2000-01-01,player,derived,120,15,14\n"
        )

    def build(self):
        return make_dataset(
            self.fm, self.root, "2026-04-01", "2026-06-01",
            "2026-08-01", "2026-09-01"
        )

    def test_chronology_and_same_match_label_not_used_in_prior(self):
        samples, manifest = self.build()
        self.assertEqual([row["split"] for row in samples],
                         ["train", "train", "validation", "test"])
        self.assertEqual(manifest["split_counts"], {
            "train": 2, "validation": 1, "test": 1
        })
        self.assertAlmostEqual(samples[0]["prior_rate"], (4 + 5 * 2.2) / 6)
        self.assertAlmostEqual(samples[1]["prior_rate"], (4 + 2 + 5 * 2.2) / 7)
        self.assertAlmostEqual(samples[2]["prior_rate"], (4 + 2 + 3 + 5 * 2.2) / 8)
        self.assertEqual([row["shots"] for row in samples], [2, 3, 1, 5])
        self.assertTrue(all(len(row["features"]) == len(FEATURE_NAMES) for row in samples))
        self.assertTrue(all(row["fm_uid"] == "999" for row in samples))
        self.assertNotIn("minutes", FEATURE_NAMES)
        self.assertNotIn("shots", FEATURE_NAMES)

    def test_ambiguous_name_dob_is_not_resolved_by_highest_ability(self):
        with self.fm.open("a") as stream:
            stream.write("998,Goal Scorer,2000-01-01,player,db,199,20,20\n")
        with self.assertRaisesRegex(ValueError, "no verified post-vintage"):
            self.build()

    def test_reconstructed_attributes_are_excluded(self):
        self.fm.write_text(
            self.fm.read_text().replace("player,db", "player,derived")
        )
        with self.assertRaisesRegex(ValueError, "no verified post-vintage"):
            self.build()


if __name__ == "__main__":
    unittest.main()
