"""Causal external football results and pre-match odds reference ingestion."""
import csv
from datetime import date, datetime, timezone
import io
import json
from pathlib import Path
import sys
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from import_pl_historical_results import parse_archive  # noqa: E402
from polymarket_prematch import attach_engine_comparison, select_prematch_quotes  # noqa: E402
from fit_pl_red_card_prior import fit_prior, evaluate  # noqa: E402


class ExternalEplResultsTests(unittest.TestCase):
    def archive(self, rows):
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as archive:
            archive.writestr("results.csv", rows)
        return output.getvalue()

    def test_parses_results_and_preserves_missing_shots(self):
        header = "Season,DateTime,HomeTeam,AwayTeam,FTHG,FTAG,FTR,HS,AS,HR,AR\n"
        rows = "2000-01,2000-08-02T12:00:00Z,A,B,1,1,D,NA,NA,0,0\n"
        result = parse_archive(self.archive(header + rows))
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["date"], "2000-08-02")
        self.assertIsNone(result[0]["hs"])
        self.assertEqual(result[0]["hr"], 0)

    def test_rejects_inconsistent_results_and_duplicate_matches(self):
        header = "Season,DateTime,HomeTeam,AwayTeam,FTHG,FTAG,FTR\n"
        row = "2000-01,2000-08-02T12:00:00Z,A,B,2,1,A\n"
        with self.assertRaises(ValueError):
            parse_archive(self.archive(header + row))
        row = "2000-01,2000-08-02T12:00:00Z,A,B,2,1,H\n"
        with self.assertRaises(ValueError):
            parse_archive(self.archive(header + row + row))


class PolymarketAsOfTests(unittest.TestCase):
    def setUp(self):
        self.spec = {
            "fixture_id": "2026-10-12|Coventry|Newcastle", "event_slug": "fixture-market",
            "settlement": "regulation_90_minutes_plus_stoppage_1x2",
            "kickoff_utc": "2026-10-12T19:00:00Z", "snapshot_utc": "2026-10-12T15:00:00Z",
            "yes_tokens": {"home": "1", "draw": "2", "away": "3"},
        }
        self.cutoff = int(datetime(2026, 10, 12, 15, tzinfo=timezone.utc).timestamp())
        self.histories = {
            side: [{"t": self.cutoff - 90, "p": p}, {"t": self.cutoff + 90, "p": 0.999}]
            for side, p in (("home", 0.25), ("draw", 0.27), ("away", 0.50))
        }

    def test_uses_prematch_quote_only_and_normalizes(self):
        result = select_prematch_quotes(self.spec, self.histories)
        self.assertAlmostEqual(sum(result["normalized_reference"].values()), 1.0)
        self.assertAlmostEqual(result["normalized_reference"]["home"], 0.25 / 1.02)
        self.assertEqual(result["raw_probability_sum"], 1.02)
        self.assertEqual(result["quote_timestamps_unix"]["home"], self.cutoff - 90)

    def test_rejects_lookahead_stale_and_two_way_contract(self):
        self.spec["snapshot_utc"] = self.spec["kickoff_utc"]
        with self.assertRaisesRegex(ValueError, "precede kickoff"):
            select_prematch_quotes(self.spec, self.histories)
        self.spec["snapshot_utc"] = "2026-10-12T15:00:00Z"
        with self.assertRaisesRegex(ValueError, "stale"):
            select_prematch_quotes(self.spec, self.histories, max_age_minutes=1)
        self.spec["settlement"] = "two_way_moneyline"
        with self.assertRaisesRegex(ValueError, "settlement"):
            select_prematch_quotes(self.spec, self.histories)

    def test_rejects_unrelated_market_tokens_and_missing_draw(self):
        self.spec["yes_tokens"]["away"] = "1"
        with self.assertRaisesRegex(ValueError, "distinct"):
            select_prematch_quotes(self.spec, self.histories)
        self.spec["yes_tokens"]["away"] = "3"
        self.histories["draw"] = []
        with self.assertRaisesRegex(ValueError, "no as-of quote"):
            select_prematch_quotes(self.spec, self.histories)

    def test_reference_comparison_does_not_rewrite_engine_and_rejects_later_odds(self):
        market = select_prematch_quotes(self.spec, self.histories)
        engine = {"as_of": "2026-10-12", "fixtures": [
            {"date": "2026-10-12", "home": "Coventry", "away": "Newcastle",
             "home_win": 0.30, "draw": 0.30, "away_win": 0.40}]}
        original = json.dumps(engine, sort_keys=True)
        out = attach_engine_comparison(market, engine)
        self.assertAlmostEqual(out["market_minus_engine"]["home"], 0.25/1.02-0.30)
        self.assertEqual(original, json.dumps(engine, sort_keys=True))
        engine["as_of"] = "2026-10-11"
        with self.assertRaisesRegex(ValueError, "newer"):
            attach_engine_comparison(market, engine)


class CausalIncidentPriorTests(unittest.TestCase):
    def test_future_labels_do_not_change_past_red_prior(self):
        train = [(date(2015, 1, 1), int(index % 8 == 0)) for index in range(400)]
        start, end = date(2014, 1, 1), date(2015, 2, 1)
        first = fit_prior(train, start, end)
        # Future fixtures are visible to the evaluator, but forbidden as inputs.
        future = [(date(2017, 1, 1), 1)] * 300
        self.assertEqual(first, fit_prior(train + future, start, end))
        self.assertAlmostEqual(
            1 - (1 - first["per_player_per90_probability"])**22,
            first["match_red_probability"], places=12)
        held_out = [0] * 7 + [1]
        self.assertLess(evaluate(first, held_out)["fitted_brier"],
                        evaluate(first, held_out)["default_brier"])


if __name__ == "__main__":
    unittest.main()
