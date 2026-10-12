"""Historical player panel: fixture-level completeness, identities and as-of isolation."""

import unittest

from scripts.build_historical_player_panel import (
    Fixture, Observation, build_panel, validate_fixture, require_count,
)


def fixture(day, match_id, team="Team A", source="statsbomb", new=False):
    other = "Team B"
    participants = {}
    for club in (team, other):
        entries = {}
        for index in range(11):
            suffix = "other" if club == other else "new" if new and index == 0 else "original"
            player_id = f"{source}:{suffix}-{club}-{index}"
            entries[player_id] = Observation(player_id, str(index), True, 90.,
                                             0, None, None, "")
        entries[f"{source}:bench-{club}"] = Observation(
            f"{source}:bench-{club}", "bench", False, 0., 0, None, None, "",
        )
        participants[club] = entries
    return Fixture(source, "test-league", match_id, day, team, other, participants)


class HistoricalPanelTests(unittest.TestCase):
    def test_no_same_day_feature_leakage(self):
        a = fixture("2026-01-01", "1")
        b = fixture("2026-01-02", "2")
        c = fixture("2026-01-02", "3")
        rows, labels, audit = build_panel([a, b, c], max_age_days={"statsbomb": 45})
        target = [row for row in rows if row["match_id"] in ("2", "3")]
        self.assertEqual(len(target), 48)
        self.assertTrue(all(row["prior_listings"] == 1 for row in target))
        self.assertTrue(all(row["club_fixtures"] == 1 for row in target))
        self.assertTrue(all(row["latest_history_date"] == "2026-01-01" for row in target))
        self.assertTrue(audit["chronological_invariant"])
        self.assertEqual(len(labels), 72)

    def test_first_listed_player_has_no_target_row(self):
        a = fixture("2026-01-01", "1")
        b = fixture("2026-01-03", "2", new=True)
        rows, labels, _ = build_panel([a, b], max_age_days={"statsbomb": 45})
        prior = [row for row in rows if row["match_id"] == "2" and row["team"] == "Team A"]
        self.assertEqual(len(prior), 12)
        self.assertTrue(any(row["started"] == 0 for row in prior))
        self.assertFalse(any("new-" in row["player_id"] for row in prior))
        self.assertTrue(any("new-" in row["player_id"] for row in labels))

    def test_stale_players_are_not_imputed_as_absent(self):
        a = fixture("2026-01-01", "1")
        b = fixture("2026-04-01", "2")
        rows, _, _ = build_panel([a, b], max_age_days={"statsbomb": 45})
        self.assertEqual(rows, [])

    def test_provider_qualified_ids_do_not_cross_join(self):
        a = fixture("2026-01-01", "1", source="statsbomb")
        b = fixture("2026-01-02", "2", source="sofascore")
        rows, labels, audit = build_panel(
            [a, b], max_age_days={"statsbomb": 45, "sofascore": 45},
        )
        self.assertEqual(rows, [])
        self.assertEqual(audit["observed_player_matches_by_source"], {
            "sofascore": 24, "statsbomb": 24,
        })
        self.assertEqual(len(labels), 48)

    def test_role_comes_only_from_previous_match(self):
        a = fixture("2026-01-01", "1")
        pid = "statsbomb:original-Team A-0"
        a.teams["Team A"][pid] = Observation(pid, "keeper", True, 90., 0, 0, 0, "Goalkeeper")
        b = fixture("2026-01-02", "2")
        b.teams["Team A"][pid] = Observation(pid, "keeper", True, 90., 0, 0, 0, "Forward")
        rows, labels, _ = build_panel([a, b], max_age_days={"statsbomb": 45})
        row = next(row for row in rows if row["player_id"] == pid)
        self.assertEqual(row["position"], "Goalkeeper")
        later_label = next(row for row in labels if row["match_id"] == "2" and row["player_id"] == pid)
        self.assertEqual(later_label["position"], "Forward")

    def test_missing_stat_does_not_become_observed_zero(self):
        self.assertIsNone(require_count(None, "shots"))
        self.assertEqual(require_count(0, "shots"), 0)
        with self.assertRaises(ValueError):
            require_count(-2, "shots")

    def test_unconfirmed_xi_rejected(self):
        fx = fixture("2026-01-01", "1")
        fx.teams["Team A"].pop("statsbomb:original-Team A-0")
        with self.assertRaisesRegex(ValueError, "no confirmed XI"):
            validate_fixture(fx)


if __name__ == "__main__":
    unittest.main()
