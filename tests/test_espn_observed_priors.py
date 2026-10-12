"""Dated ESPN observed shot-rate prior invariants; synthetic fixtures only."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "apps/matchforecast/math_engine"))

from build_espn_observed_priors import build, position_group, role_of  # noqa: E402


def summary(name, dob, espn_id, minutes, shots, started=True, appeared=True, position=""):
    return {"espn_id": espn_id, "name": name, "club": "Arsenal", "minutes": minutes,
            "shots": shots if appeared else None, "goals": 0.0, "started": started,
            "appeared": appeared, "position": position}


def write_archive(root, matches):
    for index, (day, players) in enumerate(matches):
        path = root / f"raw_{index}.json"
        path.write_text(json.dumps(players))
        match = {"date": day, "espn_match_id": str(index), "home": "Arsenal",
                 "away": "Chelsea", "kickoff_utc": f"{day}T14:00:00Z",
                 "players": players, "raw_summary_sha256": "unused",
                 "verified_completed_substitutions": 0, "verified_red_cards": 0}
        (root / f"summary_{index}.json").write_text(json.dumps(match))
    return matches


class ObservedPriorsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.squads = self.root / "squads.json"
        self.output = self.root / "priors.json"

    def make_archive(self, matches):
        raw_dir = self.root / "raw"
        raw_dir.mkdir()
        entries = {}
        for index, (day, players) in enumerate(matches):
            path = raw_dir / f"summary_{index}.json"
            path.write_text(json.dumps({"players": players}))
            entries[str(path)] = "pending"
        archive = {
            "schema": "espn_player_appearances_v1",
            "as_of_exclusive_calendar_date": "2026-10-10",
            "retrieved_completed_at_utc": "2026-10-09T00:00:00Z",
            "matches": [
                {"date": day, "espn_match_id": str(index), "home": "Arsenal",
                 "away": "Chelsea", "players": players}
                for index, (day, players) in enumerate(matches)
            ],
        }
        # Fingerprint the archive itself so verify_raw_archive sees stable raw sources.
        import hashlib
        digests = {}
        for index in range(len(matches)):
            path = raw_dir / f"summary_{index}.json"
            digests[str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
        archive["raw_source_sha256"] = digests
        self.archive = self.root / "espn.json"
        self.archive.write_text(json.dumps(archive))

    def make_squads(self, players):
        self.squads.write_text(json.dumps({
            "as_of": "2026-10-10",
            "teams": {"Arsenal": {"players": players}},
        }))

    def test_posterior_matches_validated_gamma_poisson_shape(self):
        # Three completed matches before the cutoff: 2 starts x 90 min, 1 sub x 30;
        # 5 observed shots total for a forward-role registration.
        self.make_archive([
            ("2026-09-20", [summary("Bukayo Saka", "2001-09-05", "77", 90.0, 2.0)]),
            ("2026-09-27", [summary("Bukayo Saka", "2001-09-05", "77", 90.0, 2.0)]),
            ("2026-10-04", [summary("Bukayo Saka", "2001-09-05", "77", 30.0, 1.0,
                                    started=False)]),
        ])
        self.make_squads([{"official_name": "Saka, Bukayo EO*", "fm_uid": "11523521",
                           "registration": "registered",
                           "historical_position": "Right Winger"}])
        result = build(self.archive, self.squads, self.output)
        person = result["people"][0]
        # (5 shots + 5 * 2.2 forward prior) / (210/90 + 5) exposures.
        expected = (5.0 + 5.0 * 2.2) / (210.0 / 90.0 + 5.0)
        self.assertAlmostEqual(person["gamma_poisson_shots_per90"], round(expected, 5))
        self.assertEqual(person["observed_appearances"], 3)
        self.assertEqual(person["observed_starts"], 2)
        # Official "Surname, Given Middle*" formats resolve via the unique
        # legal given+family token pair, not exact display-name equality.
        self.assertEqual(person["reconciliation_method"],
                         "unique_legal_given_family_token_pair_and_club")

    def test_matches_on_cutoff_day_are_excluded(self):
        self.make_archive([
            ("2026-10-10", [summary("Bukayo Saka", "2001-09-05", "77", 90.0, 4.0)]),
            ("2026-09-27", [summary("Bukayo Saka", "2001-09-05", "77", 90.0, 1.0)]),
            ("2026-09-20", [summary("Bukayo Saka", "2001-09-05", "77", 90.0, 0.0)]),
        ])
        self.make_squads([{"official_name": "Saka, Bukayo EO*", "fm_uid": "11523521",
                           "registration": "registered",
                           "historical_position": "Right Winger"}])
        result = build(self.archive, self.squads, self.output)
        person = result["people"][0]
        # The cutoff-day match (4 shots) never enters exposure; both earlier
        # completed matches do.
        self.assertEqual(person["observed_appearances"], 2)
        self.assertEqual(person["observed_shots"], 1.0)

    def test_unresolved_identity_stays_unresolved_and_no_output_overwrite(self):
        self.make_archive([
            ("2026-09-27", [summary("Different Person", "1990-01-01", "5", 90.0, 1.0)]),
            ("2026-09-20", [summary("Different Person", "1990-01-01", "5", 90.0, 1.0)]),
        ])
        self.make_squads([{"official_name": "Saka, Bukayo EO*", "fm_uid": "11523521",
                           "registration": "registered"}])
        result = build(self.archive, self.squads, self.output)
        self.assertEqual(result["people_count"], 0)
        self.assertIn("ambiguous_or_unmatched_legal_tokens",
                      result["identity_reconciliation_counts"])
        with self.assertRaises(ValueError):
            build(self.archive, self.squads, self.output)

    def test_gk_attribute_rule_and_unknown_role_prior(self):
        self.make_archive([
            ("2026-09-27", [summary("Jordan Pickford", "1994-03-07", "8", 90.0, 0.0)]),
            ("2026-09-20", [summary("Jordan Pickford", "1994-03-07", "8", 90.0, 0.0)]),
        ])
        self.make_squads([
            {"official_name": "Pickford, Jordan Lee*", "fm_uid": "20545",
             "registration": "registered", "historical_position": "",
             "fm_attributes_verified": True,
             "attributes": {"Reflexes": 17.0, "Handling": 14.0}},
        ])
        result = build(self.archive, self.squads, self.output)
        person = result["people"][0]
        self.assertEqual(person["role"], "GK")
        # (0 shots + 5 * 0.02 goalkeeper prior) / (180/90 + 5) exposures.
        expected = (0.0 + 5.0 * 0.02) / (180.0 / 90.0 + 5.0)
        self.assertAlmostEqual(person["gamma_poisson_shots_per90"], round(expected, 5))

    def test_position_group_rules(self):
        self.assertEqual(position_group("Goalkeeper"), "GK")
        self.assertEqual(position_group("Right Wing Back"), "DEF")
        self.assertEqual(position_group("Central Midfield"), "MID")
        self.assertEqual(position_group("Centre Forward"), "FWD")
        self.assertEqual(position_group(""), "UNKNOWN")


if __name__ == "__main__":
    unittest.main()