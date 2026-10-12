"""Reep-first EPL identity evidence / FM binary fallback regressions."""
from collections import defaultdict
import csv
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/"scripts"))
from resolve_epl_reep_first import source_name_support,read_fm_subset,resolve
from build_unified_prematch_bundle import load_fm,epl_native_attribute_admissible


def write_csv(path,rows):
    cols=sorted({k for row in rows for k in row})
    with path.open("w",newline="") as file:
        writer=csv.DictWriter(file,fieldnames=cols)
        writer.writeheader();writer.writerows(rows)


class ReepResolutionTests(unittest.TestCase):
    def test_provider_id_does_not_authorize_unrelated_fm_psychology(self):
        valid={"name":"Calvin Chinedu Bassey","binary_record_source":"men_full_attrs"}
        different={"name":"David James Bassey","binary_record_source":"men_full_attrs"}
        stripped={"name":"","binary_record_source":"complete_v6_uid_exact",
                  "ca":167,"attributes":{"Composure":15}}
        self.assertTrue(epl_native_attribute_admissible(
            "Bassey, Calvin Chinedu",valid))
        self.assertFalse(epl_native_attribute_admissible(
            "Bassey, Calvin Chinedu",different))
        self.assertFalse(epl_native_attribute_admissible(
            "Bassey, Calvin Chinedu",stripped))
        self.assertFalse(epl_native_attribute_admissible(
            "Bassey, Calvin Chinedu",valid,
            {"fm_native_record_trusted_for_this_player":False}))
    def test_short_names_are_not_an_identity_without_typed_provider_link(self):
        self.assertTrue(source_name_support("Wissa, Yoane","Yoane Wissa Bileko"))
        self.assertTrue(source_name_support("Obi, Chidozie Martin","Chidozie Obi-Martin"))
        self.assertTrue(source_name_support("Asemota, Aimuamwonnsa Presley Owen",
                                            "Aimuamwonsa Presly Owen Asemota"))
        self.assertFalse(source_name_support("Smith, John","Jack Smith"))
        self.assertFalse(source_name_support("Gray, Harry Andrew","Harrison Gray"))

    def test_complete_v6_recovers_only_missing_native_uid(self):
        with tempfile.TemporaryDirectory() as tmp:
            a,b=Path(tmp)/"men.csv",Path(tmp)/"full.csv"
            write_csv(a,[dict(uid="100",name="Real A",kind="player",
                              attr_source="db",ca="130",pa="155",
                              attr_Decisions="12",attr_Composure="14")])
            write_csv(b,[dict(uid="100",name="",kind="player",
                              ca="1",pa="1",attr_Decisions="1"),
                         dict(uid="200",name="",kind="player",
                              ca="167",pa="175",attr_Decisions="15",
                              attr_Composure="14"),
                         dict(uid="300",name="",kind="staff",
                              ca="145",pa="170",attr_Decisions="15")])
            found,scanned=read_fm_subset(a,{"100","200","300"},b)
            self.assertEqual(scanned,1)
            self.assertEqual(found["100"][0]["ca"],"130")
            self.assertEqual(found["100"][0]["record_source"],"men_full_attrs")
            self.assertEqual(found["200"][0]["record_source"],
                             "complete_v6_raw_attribute_block")
            self.assertEqual(found["300"][0]["kind"],"staff")
            self.assertEqual(found["300"][0]["attr_source"],"db")
            _,byuid=load_fm(a,set(),{"100","200","300"},complete_fm_path=b)
            self.assertEqual(byuid["200"]["attributes"]["Decisions"],15.)
            self.assertEqual(byuid["100"]["attributes"]["Decisions"],12.)
            self.assertEqual(byuid["300"]["binary_record_source"],
                             "complete_v6_uid_exact")

    def test_duplicate_native_uid_does_not_accept_alt_fallback(self):
        with tempfile.TemporaryDirectory() as tmp:
            a,b=Path(tmp)/"men.csv",Path(tmp)/"full.csv"
            write_csv(a,[dict(uid="12",kind="player",name="A",ca="10"),
                         dict(uid="12",kind="staff",name="B",ca="20")])
            write_csv(b,[dict(uid="12",kind="player",name="",ca="180",
                              attr_Decisions="20")])
            _,ids=load_fm(a,set(),{"12"},complete_fm_path=b)
            self.assertNotIn("12",ids)

    def test_two_loan_contract_registrations_retained_same_person(self):
        persons=[
            {"club":c,"official_name":name,"registration":"u21",
             "old_fm_uid":"100",
             "strong_keys":set(),"short_keys":set()}
            for c,name in (
                ("Man City","Mfuni, Stephen-Nevin Mutanda* (Loan)"),
                ("Coventry","Mfuni, Stephen-Nevin Mutanda*"))]
        candidates={i:{"rp1":{"reep_id":"rp1","rank":0,
                              "evidence_name":"Stephen-Nevin Mutanda Mfuni",
                              "source_table":"players"}} for i in (0,1)}
        bridges={"rp1":{"fm":{("100","source")},
                          "sofascore":set(),"transfermarkt":set(),"fbref":set()}}
        records={"100":[{"fm_uid":"100","name":"Stephen-Nevin Mutanda Mfuni",
                         "dob":"2008-01-01","kind":"player",
                         "attr_source":"db","record_source":"men_full_attrs"}]}
        by_club,counts=resolve(persons,candidates,bridges,records)
        self.assertEqual(counts["same_person_in_multiple_club_registrations"],2)
        self.assertEqual(by_club["Man City"][0]["new_fm_uid"],"100")
        self.assertTrue(by_club["Coventry"][0]["multi_club_eligibility_unresolved"])

    def test_reep_collision_different_players_rejected(self):
        persons=[
            {"club":c,"official_name":name,"registration":"registered",
             "old_fm_uid":None,"strong_keys":set(),"short_keys":set()}
            for c,name in (
                ("Coventry","Gray, Harrison Steven"),
                ("Leeds","Gray, Harry Andrew"))]
        candidates={i:{"rpX":{"reep_id":"rpX","rank":0,"evidence_name":"Gray",
                              "source_table":"aliases"}} for i in (0,1)}
        bridges={"rpX":{"fm":set(),"sofascore":set(),"transfermarkt":set(),"fbref":set()}}
        by_club,counts=resolve(persons,candidates,bridges,{})
        self.assertEqual(counts["cross_registration_reep_collision"],2)
        self.assertIsNone(by_club["Leeds"][0]["new_reep_id"])


if __name__=="__main__":
    unittest.main()
