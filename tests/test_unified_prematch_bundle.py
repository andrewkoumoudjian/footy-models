"""The common EPL/MLS match+valuation primitive contract is strictly as-of."""
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from build_unified_prematch_bundle import (
    MENTAL, PSYCHOLOGY_TRAITS, SURGE_FEATURE_NAMES, build_bundle, chemistry,
    flatten_model_features, fm_features, form_window, load_provider_observations,
    number, observed_roles, player_bundle_row, player_usage, psychology_vector,
    rolling, value_primitive_features,
)


class UnifiedPrematchTests(unittest.TestCase):
    def setUp(self):
        self.cutoff = "2026-08-24"

    def test_value_feature_order_matches_rust_primitive(self):
        rust = (ROOT / "crates/recruitment/src/surge.rs").read_text()
        source = re.search(r'pub const SURGE_FEATURE_NAMES:\s*\[&str;\s*47\]\s*=\s*\[([^]]*)\]', rust, re.S)
        self.assertIsNotNone(source)
        original = re.findall(r'"([a-z0-9_]+)"', source.group(1))
        self.assertEqual(tuple(original), SURGE_FEATURE_NAMES)
        self.assertEqual(len(SURGE_FEATURE_NAMES), 47)

    def test_rolling_strictly_excludes_target_day_and_future(self):
        observations = [
            {"date": "2026-08-23", "match_id": "1", "stats": {"totalShots": 0}},
            {"date": "2026-08-24", "match_id": "2", "stats": {"totalShots": 100}},
            {"date": "2026-08-25", "match_id": "3", "stats": {"totalShots": 999}},
        ]
        out = rolling(observations, self.cutoff, ("totalShots", "rating"))
        self.assertEqual(out["features"]["totalShots"], 0.0)
        self.assertIsNone(out["features"]["rating"])
        self.assertFalse(out["mask"]["rating"])

    def test_pub_date_and_namespace_block_leakage(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fbref.jsonl"
            row = {"provider": "fbref", "fbref_person_id": "abcd", "date": "2026-08-22",
                   "published_at": "2026-08-25T09:00:00Z", "features": {"xG": 3.0}}
            path.write_text(json.dumps(row) + "\n")
            self.assertEqual(dict(load_provider_observations(path, self.cutoff, "fbref", "fbref_person_id")), {})
            row["published_at"] = "2026-08-23"
            path.write_text(json.dumps(row) + "\n")
            out = load_provider_observations(path, self.cutoff, "fbref", "fbref_person_id")
            self.assertEqual(out["abcd"][0]["stats"]["xG"], 3.0)
            row["provider"] = "transfermarkt"
            path.write_text(json.dumps(row) + "\n")
            with self.assertRaisesRegex(ValueError, "explicit fbref"):
                load_provider_observations(path, self.cutoff, "fbref", "fbref_person_id")

    def test_hidden_psychology_is_missing_not_invented(self):
        record = {"uid": "42", "dob": "2000-01-01", "name": "Example",
                  "kind": "player", "attr_source": "db", "ca": 130.0, "pa": 160.0,
                  "height_cm": 180.0,
                  "attributes": {"Leadership": 14.0, "Teamwork": 16.0,
                                 "Consistency": 12.0, "Composure": 15.0}}
        fm = fm_features(record, "reep_dob_verified")
        mind = psychology_vector(fm)
        self.assertTrue(fm["observed_mental_mask"]["Leadership"])
        self.assertIsNone(mind["stable_traits"]["professionalism"]["value"])
        self.assertIsNone(mind["stable_traits"]["pressure"]["value"])
        self.assertIsNone(mind["dynamic_morale"])
        self.assertAlmostEqual(mind["stable_traits"]["teamwork"]["value"], 5.5 / 9.5)
        record["attr_source"] = "derived"
        derived = fm_features(record, "reep_dob_verified")
        self.assertFalse(derived["observed_mental_mask"]["Teamwork"])
        self.assertTrue(derived["reconstructed_mental_mask"]["Teamwork"])
        record["kind"] = "staff"
        staff = fm_features(record, "reep_dob_verified")
        self.assertFalse(staff["available"])

    def test_exact_future_value_is_not_used_as_input(self):
        fm = fm_features(None, "none")
        state = player_bundle_row("sofascore:9", "Any Club", "Example", "MLS", self.cutoff,
                                  fm, {"sofascore_id": "9"}, None, [], [], [], [],
                                  identity_state="unresolved")
        self.assertIsNone(state["value_forecast_12m"])
        self.assertIsNone(state["transfer_hazard"]["value"])
        self.assertEqual(len(state["market_and_value_features"]["features"]), 47)
        self.assertTrue(all(value is None for value in state["market_and_value_features"]["features"].values()))

    def test_chemistry_is_from_actual_previous_co_starts(self):
        candidates = [{"canonical_player_id": "reep:p1"}, {"canonical_player_id": "reep:p2"}]
        lineups = [("2026-08-22", "1", {"sofascore:1", "sofascore:2"}),
                   ("2026-08-23", "2", {"sofascore:1"})]
        out = chemistry(candidates, lineups, {"sofascore:1": "reep:p1", "sofascore:2": "reep:p2"})
        self.assertEqual(len(out["pair_edges"]), 1)
        self.assertEqual(out["pair_edges"][0]["shared_starts"], 1)

    def test_form_and_roles_never_read_future_labels(self):
        earlier = {"date": "2026-08-23", "match_id": "1", "minutes": 90.0,
                   "started": True, "role": "M", "stats": {"totalShots": 1}}
        future = {"date": "2026-08-24", "match_id": "2", "minutes": 90.0,
                  "started": True, "role": "F", "stats": {"totalShots": 99}}
        form = form_window([earlier, future], self.cutoff)
        self.assertAlmostEqual(form["metrics"]["totalShots"]["per90"], 1.0)
        self.assertEqual(observed_roles([earlier, future], self.cutoff)["roles"]["M"], 1.0)

    def test_feature_contract_is_numeric_null_and_masked(self):
        fm = fm_features(None, "none")
        p = player_bundle_row("sofascore:9", "Any Club", "Example", "MLS", self.cutoff,
                              fm, {"sofascore_id": "9"}, None, [], [], [], [],
                              identity_state="unresolved")
        from build_unified_prematch_bundle import team_sources
        team = team_sources("Any Club", [], [p], [], self.cutoff, {})
        row = flatten_model_features(p, team)
        self.assertEqual(set(row["features"]), set(row["observed_mask"]))
        self.assertGreater(len(row["features"]), 120)
        self.assertIsNone(row["features"]["psych_professionalism"])
        self.assertFalse(row["observed_mask"]["fbref_xG_recent10_mean"])
        self.assertFalse(row["observed_mask"]["value_primitive_log_mv"])

    def test_duplicate_native_uid_is_quarantined_not_last_record_wins(self):
        import csv
        from build_unified_prematch_bundle import load_fm
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/"fm.csv"
            with path.open("w",newline="") as out:
                writer=csv.DictWriter(out,fieldnames=("uid","name","dob","kind","attr_source","ca"))
                writer.writeheader()
                writer.writerows([
                    {"uid":"42","name":"Player One","dob":"2000-01-01","kind":"player","attr_source":"db","ca":"120"},
                    {"uid":"42","name":"Staff Two","dob":"1970-01-01","kind":"staff","attr_source":"db","ca":"145"},
                    {"uid":"43","name":"Player Three","dob":"2001-01-01","kind":"player","attr_source":"db","ca":"80"},
                ])
            _,uids=load_fm(path,set(),{"42","43"})
            self.assertNotIn("42",uids)
            self.assertEqual(uids["43"]["name"],"Player Three")


if __name__ == "__main__":
    unittest.main()
