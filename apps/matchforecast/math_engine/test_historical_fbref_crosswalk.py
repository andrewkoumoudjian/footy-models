"""No current FM-to-official identity is silently promoted by historical IDs."""
from datetime import datetime, timezone
import copy
import unittest

from historical_fbref_crosswalk import build


def sample_bundle():
    teams={}
    for club_no in range(20):
        players=[]
        for j in range(76):
            uid=f"official_epl:{club_no:02d}:{j:03d}"
            ids=["abcd1234"] if club_no==0 and j==0 else []
            players.append({
                "canonical_player_id":uid,
                "provider_ids":{"fbref_person_ids_candidate":ids}
            })
        teams[f"club_{club_no:02d}"]={"players":players}
    return {"schema":"unified_epl_mls_player_set_v1",
            "league":"EPL","roster_scope":"full_league",
            "team_count":20,"player_count":1520,
            "forecast_cutoff_date":"2026-10-11",
            "teams":teams}


class HistoricalFBrefTests(unittest.TestCase):
    def test_provider_id_joins_earlier_season_but_not_model_features(self):
        history=[{
            "league":"EPL","player_id":"abcd1234","season":"2024-2025",
            "squad":"Chelsea","mp":"28","starts":"24","min":"2050",
            "gls":"4","ast":"2",
        }]
        quality={"schema":"fbref_understat_season_v1",
                 "players":[{"league":"EPL","fbref_player_id":"abcd1234",
                             "season":"2024-2025","available_from":"2025-07-01",
                             "club":"Chelsea","understat_shots":45.0,
                             "understat_xg":5.2,"understat_npxg":4.3}]}
        result=build(sample_bundle(),history,quality,
                     datetime(2026,10,12,12,tzinfo=timezone.utc))
        self.assertEqual(result["candidate_coverage"]["matched_provider_fbref_id_previous_season"],1)
        self.assertEqual(result["candidate_coverage"]["with_prior_understat_xg_rows"],1)
        entry=result["historical_candidate_records"][0]
        self.assertEqual(entry["identity_for_current_registration"],"CANDIDATE_ONLY_UNVERIFIED")
        self.assertFalse(entry["training_feature_admission"])
        self.assertTrue(result["retains_original_248_feature_schema"])
        self.assertFalse(result["source_is_match_log"])
        self.assertEqual(entry["prior_understat_crossverified_club_season_rows"][0]["xg"],5.2)

    def test_cannot_backdate_into_original_snapshot(self):
        with self.assertRaisesRegex(ValueError,"LATER-vintage"):
            build(sample_bundle(),[],None,datetime(2026,10,11,tzinfo=timezone.utc))

    def test_rejects_fixture_subset_and_inconsistent_start_count(self):
        truncated=sample_bundle()
        truncated["roster_scope"]="fixture_subset"
        with self.assertRaisesRegex(ValueError,"20-club"):
            build(truncated,[],None,datetime(2026,10,12,tzinfo=timezone.utc))
        broken=[{"league":"EPL","player_id":"abcd1234",
                 "season":"2024-2025","squad":"Chelsea",
                 "mp":"10","starts":"14","min":"90","gls":"0","ast":"0"}]
        with self.assertRaisesRegex(ValueError,"starts exceed"):
            build(sample_bundle(),broken,None,datetime(2026,10,12,tzinfo=timezone.utc))

    def test_current_season_aggregates_dont_count_as_past(self):
        future=[{"league":"EPL","player_id":"abcd1234",
                 "season":"2026-2027","squad":"Chelsea",
                 "mp":"38","starts":"38","min":"3200","gls":"10","ast":"10"}]
        result=build(sample_bundle(),future,None,
                     datetime(2026,10,12,tzinfo=timezone.utc))
        self.assertEqual(result["candidate_coverage"]["linked_id_no_prior_season_rows"],1)
        self.assertFalse(result["historical_candidate_records"])


if __name__=="__main__":
    unittest.main()
