"""Prevent FM/Sortitoutsi UID confusion and incorrect Transfermarkt joins."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from reconcile_epl_fm_sources import match_quality, adjudicate
from promote_epl_fm_tm_evidence import select_evidence, club_matches, name_consistent


class FullFMRecoveryTests(unittest.TestCase):
    def test_extended_legal_name(self):
        self.assertEqual(match_quality('Becker, Alisson Ramses','Alisson Ramsés Becker')[0],0)
        self.assertEqual(match_quality('Nyoni, Trey*','Trey Mourice Nyoni')[0],2)
        self.assertEqual(match_quality('Sangare, Ibrahim','Ibrahim Sangaré')[0],0)

    def test_sortitoutsi_uid_is_not_assumed_native_uid(self):
        records=[{'source':'sortitoutsi_fm26_roster','uid':'8123','sortitoutsi_uid':'5599',
                  'name':'Example Smith','dob':'2000-01-02','kind':'player','attr_source':'metadata',
                  'grade':0,'si_map_name':'Example Smith','si_map_dob':'2000-01-02',
                  'si_map_type':'full','si_name':'Example Smith'},
                 {'source':'fm26_men_full_attrs.csv','uid':'8123','name':'Example Smith',
                  'dob':'2000-01-02','kind':'player','attr_source':'db','grade':0}]
        chosen,proposals=adjudicate(records)
        self.assertEqual(chosen['uid'],'8123')
        self.assertEqual(chosen['sortitoutsi_ids'],['5599'])

    def test_same_name_without_dob_is_not_promoted(self):
        proposed={'uid':'27088231','fm_name':'Mathias Jensen',
                  'native_dob':'1996-01-01','native_kind':'player',
                  'native_attr_source':'db'}
        wrong={'uid':'2000142529','fm_name':'Mathias Jensen',
               'native_dob':'2005-04-14','native_kind':'player',
               'native_attr_source':'db'}
        rows={'27088231':[{'date_of_birth':'1996-01-01','name_tm':'Mathias Jensen',
                          'name':'Mathias Jensen','current_club_name':'Brentford FC',
                          'player_id':'319859'}],
              '2000142529':[{'date_of_birth':'2005-04-14','name_tm':'Mathias Jensen',
                            'name':'Mathias Jensen','current_club_name':'Bröndby IF',
                            'player_id':'695210'}]}
        selected,evidence=select_evidence('Jensen, Mathias','Brentford',[proposed,wrong],rows)
        self.assertEqual(selected['uid'],'27088231')
        self.assertEqual(len(evidence),1)

    def test_rejects_wrong_transfermarkt_person_same_uid(self):
        candidate={'uid':'2000095120','fm_name':'Gabriel Pawel Słonina',
                   'native_dob':'2005-05-15','native_kind':'player'}
        tm={'2000095120':[{'date_of_birth':'2005-05-15',
                           'name_tm':'Reed Baker-Whiting',
                           'name':'Gabriel Pawel Słonina',
                           'current_club_name':'Chelsea FC',
                           'player_id':'793957'}]}
        selected,_=select_evidence('Slonina, Gabriel Pawel','Chelsea',[candidate],tm)
        self.assertIsNone(selected)

    def test_rejects_wrong_club_and_wrong_dob(self):
        candidate={'uid':'28108035','fm_name':'Reece James','native_dob':'1999-12-08',
                   'native_kind':'player'}
        tm={'28108035':[{'date_of_birth':'1999-12-08','name_tm':'Reece James',
                          'name':'Reece James','current_club_name':'Manchester United',
                          'player_id':'472423'}]}
        selected,_=select_evidence('James, Reece','Chelsea',[candidate],tm)
        self.assertIsNone(selected)
        tm['28108035'][0]['current_club_name']='Chelsea FC'
        tm['28108035'][0]['date_of_birth']='2004-01-02'
        selected,_=select_evidence('James, Reece','Chelsea',[candidate],tm)
        self.assertIsNone(selected)

    def test_2026_club_normalization(self):
        self.assertTrue(club_matches('Chelsea','Chelsea FC'))
        self.assertTrue(club_matches('Liverpool','Liverpool FC'))
        self.assertTrue(club_matches('Nott\'m Forest','Nottingham Forest'))
        self.assertFalse(club_matches('Chelsea','Newcastle United'))
        self.assertTrue(name_consistent('Martin Ødegaard','Martin Odegaard'))


if __name__ == '__main__':unittest.main()
