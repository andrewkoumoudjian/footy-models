"""Fail-closed shared EPL/MLS feature contract tests (no network required)."""
import copy
from datetime import datetime, timezone
import unittest

from unified_features import assemble, FEATURE_NAMES, FORBIDDEN, team_profile


def data():
    squad=[]
    for club in ("Liverpool","Inter Miami"):
        for i in range(14):
            row={
                "canonical_player_id":f"fm26:{1000+100*i+(0 if club=='Liverpool' else 1)}",
                "team":club,"role":"GK" if i<2 else "MF",
                "age_years":24.0,
            }
            if i<8:
                row["fm"]={
                    "kind":"player","attr_source":"db",
                    "snapshot_at":"2026-03-01T00:00:00Z",
                    "ca":125,"pa":150,
                    "attributes":{"Composure":14,"Leadership":12+i,"Teamwork":12,"Finishing":8},
                }
            if i==2:
                row["sofascore"]={
                    "published_at":"2026-10-10T19:00:00Z",
                    "features":{"rating":7.2,"totalShots":2},
                }
                row["fbref"]={
                    "published_at":"2026-10-10T20:00:00Z",
                    "features":{"shots_per90":1.8},
                }
                row["market_context"]={
                    "published_at":"2026-09-01T00:00:00Z",
                    "features":{"log_observed_market_value_eur":15.0},
                }
            squad.append(row)
    return {
        "schema":"unified_math_fixture_v1", "league":"EPL",
        "feature_cutoff_utc":"2026-10-11T21:00:00Z",
        "fixture_kickoff_utc":"2026-10-12T19:00:00Z",
        "clubs":["Liverpool","Inter Miami"],
        "registered_players":squad,
        "team_recent":{
            "Liverpool":{"published_at":"2026-10-10T21:00:00Z",
                         "features":{"rating":7.1,"xg":1.6,"xga":0.9}},
        },
        "prior_confirmed_co_starts":[
            {"published_at":"2026-09-01T10:00:00Z",
             "club":"Liverpool","completed_co_starts":3},
        ],
    }


class UnifiedFeatureTests(unittest.TestCase):
    def test_keeps_all_registered_players_even_with_missing_fm(self):
        report=assemble(data())
        self.assertEqual(len(report["players"]),28)
        self.assertEqual(tuple(report["feature_names"]),FEATURE_NAMES)
        self.assertTrue(all(len(r["features"]) == len(FEATURE_NAMES) for r in report["players"]))
        self.assertFalse(report["players"][13]["mask"]["fm_mental_Composure"])
        self.assertTrue(report["players"][2]["mask"]["sofascore_rating"])
        self.assertTrue(report["players"][2]["mask"]["fbref_shots_per90"])
        self.assertEqual(report["teams"]["Liverpool"]["team_mental_coverage"],8/14)
        self.assertEqual(report["players"][2]["features"]["teammate_co_start_familiarity"],3)

    def test_reconstructed_mental_is_separate_from_observed_team_mean(self):
        obj=data()
        obj["registered_players"][1]["fm"]["attr_source"]="derived"
        res=assemble(obj)
        self.assertEqual(res["teams"]["Liverpool"]["team_mental_coverage"],7/14)
        self.assertEqual(res["players"][1]["features"]["fm_attributes_reconstructed"],1)
        self.assertEqual(res["players"][1]["features"]["fm_attributes_observed"],0)

    def test_unpublished_future_stats_and_future_lineups_rejected(self):
        obj=data()
        obj["registered_players"][2]["sofascore"]["published_at"]="2026-10-12T22:00:00Z"
        with self.assertRaisesRegex(ValueError,"not available"):
            assemble(obj)
        obj=data()
        obj["prior_confirmed_co_starts"][0]["published_at"]="2026-10-12T21:00:00Z"
        with self.assertRaisesRegex(ValueError,"future co-start"):
            assemble(obj)

    def test_future_market_model_output_is_not_feature(self):
        obj=data()
        obj["registered_players"][2]["market_context"]["features"]["future_value_12m"]=200_000_000
        with self.assertRaisesRegex(ValueError,"future target"):
            assemble(obj)
        self.assertIn("future_value_12m",FORBIDDEN)

    def test_player_identity_is_only_metadata_not_training_vector(self):
        res=assemble(data())
        self.assertFalse(any("player_id" in field for field in res["feature_names"]))
        self.assertFalse(any("team_id" in field for field in res["feature_names"]))
        self.assertFalse(any("fm_uid" in field for field in res["feature_names"]))

    def test_unregistered_other_club_or_duplicate_identity_rejected(self):
        obj=data()
        obj["registered_players"][0]["team"]="Third Club"
        with self.assertRaisesRegex(ValueError,"neither"):
            assemble(obj)
        obj=data()
        obj["registered_players"][0]["canonical_player_id"]=obj["registered_players"][1]["canonical_player_id"]
        with self.assertRaisesRegex(ValueError,"duplicate player"):
            assemble(obj)


if __name__=="__main__":
    unittest.main()
