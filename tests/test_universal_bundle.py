"""Universal bundle: fixture parsing, causality, 20-club universe, ID safety."""
import json
import sys
import unittest
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from build_universal_bundle import (
    ESPN_TO_CANONICAL,
    ROLE_PRIORS,
    build_bundle,
    canonical_club,
    fixtures_from_schedule,
    fixtures_from_tsv,
    provisional_id,
    role_of,
)
from fm_staff_rescue import load_rescue_map

SQUADS = ROOT / "data/pl/squads_2026-10-11_full.json"
ESPN = [ROOT / "data/ml/math_engine/espn_frozen_20261011.json",
        ROOT / "data/ml/math_engine/espn_extension_20261012.json"]
XWALK = ROOT / "data/ml/math_engine/espn_pipeline_final_oct12/crosswalk.json"
HISTORY = ROOT / "data/pl/asof_team_stats.csv"
RESULTS = ROOT / "data/pl/results.tsv"
LEDGER = ROOT / "data/identity/reep/epl_canonical_clean_v11_evidence.json"


def _ledger_entry(name, kind, source, dob, tier, uid="999"):
    return {
        "club": "Arsenal", "official_name": name,
        "status": "candidate_uid_attached_attributes_quarantined",
        "accepted_candidate": {
            "fm_uid": uid, "native_kind": kind, "native_attr_source": source,
            "native_dob_agrees_with_tm": dob,
            "native_name_evidence_tier": tier,
        },
    }


def _union_row(kind="staff", source="db", finishing="14", pace="12"):
    return {"uid": "999", "kind": kind, "attr_source": source,
            "attr_Finishing": finishing, "attr_Pace": pace}


class StaffRescuePolicyTests(unittest.TestCase):
    def rescue(self, entries, rows):
        import tempfile
        ledger = {"players": entries}
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "ledger.json"
            path.write_text(json.dumps(ledger))
            return load_rescue_map(path, rows)

    def test_admits_bridge_corroroborated_staff_db_only(self):
        out = self.rescue([_ledger_entry("A", "staff", "db", True, 4)],
                          {"999": _union_row()})
        self.assertIn(("Arsenal", "A"), out)
        self.assertEqual(out[("Arsenal", "A")]["fm_uid"], "999")

    def test_rejects_weak_name_conflict_derived_and_empty(self):
        cases = [
            _ledger_entry("T", "staff", "db", True, 1, uid="1"),
            _ledger_entry("D", "staff", "db", False, 4, uid="2"),
            _ledger_entry("V", "staff", "derived", True, 4, uid="3"),
            _ledger_entry("E", "staff", "db", True, 4, uid="4"),
            _ledger_entry("P", "player", "db", True, 4, uid="5"),
        ]
        rows = {"1": _union_row(), "2": _union_row(), "3": _union_row(),
                "4": _union_row(finishing="", pace=""),
                "5": _union_row(kind="player")}
        self.assertEqual(self.rescue(cases, rows), {})


def board(*events):
    return {"events": [
        {"id": str(1000 + i), "date": day + "T14:00Z", "name": f"{h} at {a}",
         "status": {"type": {"description": "Scheduled", "completed": False}},
         "competitions": [{"competitors": [
             {"homeAway": "home", "team": {"displayName": h}},
             {"homeAway": "away", "team": {"displayName": a}}]}]}
        for i, (day, h, a) in enumerate(events)]}


class FixtureParsingTests(unittest.TestCase):
    def squads(self):
        return json.loads(SQUADS.read_text())["teams"]

    def test_schedule_maps_espn_names_and_sorts(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "board.json"
            path.write_text(json.dumps(board(
                ("2026-10-18", "Manchester United", "Leeds United"),
                ("2026-10-17", "Coventry City", "Tottenham Hotspur"))))
            fixtures = fixtures_from_schedule([path], self.squads(), date(2026, 10, 12))
            self.assertEqual([(f["home"], f["away"]) for f in fixtures],
                             [("Coventry", "Tottenham"), ("Man United", "Leeds")])
            self.assertTrue(all(f["day"] > date(2026, 10, 12).toordinal()
                                for f in fixtures))

    def test_schedule_rejects_completed_and_unknown(self):
        import tempfile
        squads = self.squads()
        with tempfile.TemporaryDirectory() as tmp:
            done = Path(tmp) / "done.json"
            done.write_text(json.dumps({"events": [{
                "id": "1", "date": "2026-10-17T14:00Z", "name": "X",
                "status": {"type": {"description": "Final", "completed": True}},
                "competitions": [{"competitors": []}]}]}))
            with self.assertRaises(ValueError):
                fixtures_from_schedule([done], squads, date(2026, 10, 12))
            strange = Path(tmp) / "strange.json"
            strange.write_text(json.dumps(board(("2026-10-17", "Atlantis FC", "Arsenal"))))
            with self.assertRaises(ValueError):
                fixtures_from_schedule([strange], squads, date(2026, 10, 12))

    def test_canonical_map_covers_both_name_styles(self):
        self.assertEqual(canonical_club("Nottingham Forest"), "Nott'm Forest")
        self.assertEqual(canonical_club("Arsenal"), "Arsenal")
        with self.assertRaises(ValueError):
            canonical_club("Manchester City Reserves")

    def test_role_and_prior_contract(self):
        self.assertEqual(role_of("Goalkeeper"), "goalkeeper")
        self.assertEqual(role_of(None), "unknown")
        self.assertEqual(role_of("sweeper"), "unknown")
        self.assertAlmostEqual(ROLE_PRIORS["attack"], 2.20)

    def test_provisional_ids_never_collide_with_fm_space(self):
        squads = json.loads(SQUADS.read_text())["teams"]
        fm_ids = {int(p["fm_uid"]) for players in squads.values()
                  for p in players["players"] if p.get("fm_uid")}
        self.assertTrue(all(uid < 1 << 63 for uid in fm_ids))
        first = provisional_id("Arsenal", "Test Player")
        self.assertEqual(first, provisional_id("Arsenal", "Test Player"))
        self.assertGreaterEqual(first, 1 << 63)
        self.assertNotEqual(first, provisional_id("Arsenal", "Other Player"))


@unittest.skipUnless(SQUADS.exists() and HISTORY.exists(), "needs local data")
class UniversalBundleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        squads = json.loads(SQUADS.read_text())["teams"]
        fixtures = [{"date": "2026-10-17", "day": date(2026, 10, 17).toordinal(),
                     "home": "Arsenal", "away": "Chelsea"}]
        cls.bundle = build_bundle(
            squads_path=SQUADS, fixtures=fixtures, as_of=date(2026, 10, 12),
            espn_paths=[p for p in ESPN if p.exists()], crosswalk_path=XWALK,
            history_path=HISTORY, results_path=RESULTS)
        cls.squads = squads

    def test_full_registered_universe_retained(self):
        eligible = sum(1 for players in self.squads.values()
                       for p in players["players"] if not p.get("loaned_out"))
        self.assertEqual(len(self.bundle["players"]), eligible)
        self.assertEqual(len({p["club"] for p in self.bundle["players"]}), 20)

    def test_ids_unique_and_pooled_features_well_formed(self):
        ids = [p["id"] for p in self.bundle["players"]]
        self.assertEqual(len(ids), len(set(ids)))
        for p in self.bundle["players"]:
            self.assertEqual(len(p["pooled_features"]), 11)
            self.assertTrue(all(map(__import__("math").isfinite, p["pooled_features"])))
            self.assertGreater(p["shots_per90"], 0)
            self.assertGreater(p["xg_per90"], 0)

    def test_every_fixture_club_has_feasible_pool(self):
        for club in ("Arsenal", "Chelsea"):
            pool = [p for p in self.bundle["players"] if p["club"] == club]
            self.assertGreaterEqual(sum(p["is_goalkeeper"] for p in pool), 1)
            self.assertGreaterEqual(sum(not p["is_goalkeeper"] for p in pool), 10)

    def test_histories_strictly_pre_cutoff(self):
        self.assertTrue(all(r["day"] < date(2026, 10, 12).toordinal()
                            for r in self.bundle["history"]))
        self.assertTrue(all(r["day"] < date(2026, 10, 17).toordinal()
                            for r in self.bundle["championship"]))
        self.assertGreater(len(self.bundle["history"]), 1000)

    def test_tsv_rejects_non_future_and_unknown_clubs(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            past = Path(tmp) / "past.tsv"
            past.write_text("date\thome\taway\n2026-10-10\tArsenal\tChelsea\n")
            with self.assertRaises(ValueError):
                fixtures_from_tsv(past, self.squads, date(2026, 10, 12))
            strange = Path(tmp) / "strange.tsv"
            strange.write_text("date\thome\taway\n2026-10-17\tArsenal\tAtlantis\n")
            with self.assertRaises(ValueError):
                fixtures_from_tsv(strange, self.squads, date(2026, 10, 12))

    @unittest.skipUnless(LEDGER.exists(), "needs canonical identity ledger")
    def test_ledger_rescue_materially_raises_verified_coverage(self):
        fixtures = [{"date": "2026-10-17", "day": date(2026, 10, 17).toordinal(),
                     "home": "Arsenal", "away": "Chelsea"}]
        rescued_bundle = build_bundle(
            squads_path=SQUADS, fixtures=fixtures, as_of=date(2026, 10, 12),
            espn_paths=[p for p in ESPN if p.exists()], crosswalk_path=XWALK,
            history_path=HISTORY, results_path=RESULTS, ledger_path=LEDGER)
        cov = rescued_bundle["bundle_provenance"]["coverage"]
        self.assertGreaterEqual(cov.get("fm_rescued_staff", 0), 100)
        self.assertGreater(cov["fm_verified"], self.bundle["bundle_provenance"]
                           ["coverage"]["fm_verified"])
        for p in rescued_bundle["players"]:
            if p.get("fm_rescued_staff"):
                self.assertEqual(p["fm_source"], "db")
                self.assertEqual(p["pooled_features"][-1], 1.0)
                self.assertTrue(any(v != 0.0 for v in p["pooled_features"][1:-2]))


if __name__ == "__main__":
    unittest.main()
