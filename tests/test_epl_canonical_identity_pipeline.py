"""Regression gates for the actual EPL roster builder, not a UUID patchlist."""
import csv
import inspect
import sys
import tempfile
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
import match_premier_league_fm26 as matching
from recover_epl_remaining_identities import (
    build_full_native_name_index, candidate_for_row, index_reep,
    native_full_legal_fallback, rank_person, read_sortitoutsi_corrob,
    recover, shared_name,
)


def registered(name):
    return {'official_name':name,'registration':'registered',
            'fm_uid':None,'loaned_out':False}


class CanonicalIdentityPipelineTests(unittest.TestCase):
    def test_normalization_does_not_erase_unicode_or_patronymic(self):
        self.assertEqual(matching.norm('Martin Ødegaard'),'martin odegaard')
        self.assertEqual(matching.norm('Đorđe Petrović'),'djordje petrovic')
        self.assertEqual(matching.norm("Iliman N'Diaye"),'iliman ndiaye')
        self.assertEqual(matching.norm('Mykhailo Mudryk'),'mykhailo mudryk')

    def test_near_transliteration_needs_independent_tm_dob(self):
        self.assertGreater(shared_name('Petrovic, Djorde','Đorđe Petrović'),0)
        rec={'names':{'Đorđe Petrović'},'uids':{'62216198'}}
        native={'62216198':[{'uid':'62216198','name':'Đorđe Petrović',
                             'dob':'1999-10-08','kind':'player','attr_source':'db',
                             'ca':'141','pa':'160','attributes':{'Composure':14.0}}]}
        tm={'465555':{'name':'Djordje Petrovic','date_of_birth':'1999-10-08',
                     'current_club_name':'AFC Bournemouth','player_id':'465555'}}
        candidate=rank_person('Petrovic, Djorde','Bournemouth','rp1',rec,{'465555'},tm,native)
        self.assertTrue(candidate['sufficient'])
        self.assertTrue(candidate['can_use_native_player_attributes'])
        self.assertEqual(candidate['fm_uid'],'62216198')

    def test_wrong_native_name_does_not_become_an_attribute(self):
        rec={'names':{'Matheus Luiz Nunes'},'uids':{'83188780'}}
        native={'83188780':[{'uid':'83188780','name':'Jorge Manuel Domingues Maria Vital',
                             'dob':'1998-08-27','kind':'player','attr_source':'db',
                             'ca':'160','pa':'175','attributes':{'Leadership':20.0}}]}
        tm={'601883':{'name':'Matheus Nunes','date_of_birth':'1998-08-27',
                      'current_club_name':'Manchester City','player_id':'601883'}}
        selected=rank_person('Nunes, Matheus Luiz','Man City','rp2',rec,{'601883'},tm,native)
        self.assertTrue(selected['sufficient'])
        self.assertFalse(selected['can_use_native_player_attributes'])
        roster={'as_of':'2026-10-11','teams':{'Man City':{'players':[registered('Nunes, Matheus Luiz')]}}}
        found,review=recover(roster,{}, {'matheus':{'rp2'},'surname:nunes':{'rp2'}},
                             {'rp2':rec},{'rp2':{'601883'}},tm,native)
        player=found['teams']['Man City']['players'][0]
        self.assertEqual(player['fm_uid'],'83188780')
        self.assertFalse(player['fm_attributes_verified'])
        self.assertNotIn('attributes',player)
        self.assertNotIn('ca',player)

    def test_actual_matcher_does_not_call_retired_name_scorer(self):
        source=inspect.getsource(matching.run)
        self.assertIn('resolve_official_roster',source)
        self.assertNotIn('best_match(',source)

    def test_native_exact_full_legal_search_not_restricted_to_reep(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'fm.csv'
            with path.open('w',newline='') as h:
                w=csv.DictWriter(h,fieldnames=['uid','name','dob','kind','attr_source']);w.writeheader()
                w.writerow({'uid':'19058734','name':'Alisson Ramsés Becker','dob':'1992-10-02',
                            'kind':'player','attr_source':'db'})
            roster={'teams':{'Liverpool':{'players':[registered('Becker, Alisson Ramses')]}}}
            result=build_full_native_name_index(path,roster)
            evidence=native_full_legal_fallback('Becker, Alisson Ramses',
                                                result[('Liverpool','Becker, Alisson Ramses')])
            self.assertEqual(evidence['fm_uid'],'19058734')
            self.assertFalse(evidence['can_use_native_player_attributes'])

    def test_sortitoutsi_uid_cannot_be_used_without_explicit_binary_bridge(self):
        with tempfile.TemporaryDirectory() as tmp:
            si=Path(tmp)/'si.csv';mapping=Path(tmp)/'map.csv'
            with si.open('w',newline='') as h:
                w=csv.DictWriter(h,fieldnames=['uid','Name','DOB']);w.writeheader()
                w.writerow({'uid':'999','Name':'Alisson Ramses Becker','DOB':'1992-10-02'})
            with mapping.open('w',newline='') as h:
                w=csv.DictWriter(h,fieldnames=['si_id','si_name','binary_uid','binary_name','binary_dob'])
                w.writeheader();w.writerow({'si_id':'999','si_name':'Alisson Ramses Becker',
                           'binary_uid':'19058734','binary_name':'Alisson Ramsés Becker',
                           'binary_dob':'1992-10-02'})
            native={'19058734':[{'uid':'19058734','name':'Alisson Ramsés Becker',
                                 'dob':'1992-10-02','kind':'player','attr_source':'db'}]}
            roster={'teams':{'Liverpool':{'players':[registered('Becker, Alisson Ramses')]}}}
            evidence=read_sortitoutsi_corrob(si,mapping,roster,native)
            self.assertEqual(evidence[('Liverpool','Becker, Alisson Ramses')][0]['uid'],'19058734')
            self.assertEqual(evidence[('Liverpool','Becker, Alisson Ramses')][0]['sortitoutsi_uid'],'999')
            mapping.write_text('si_id,si_name,binary_uid,binary_name,binary_dob\n')
            self.assertEqual(dict(read_sortitoutsi_corrob(si,mapping,roster,native)),{})

    def test_reconstructed_or_missing_attrs_never_marked_db_observed(self):
        rec={'names':{'Philip Walter Foden','Phil Foden'},'uids':{'1'}}
        native={'1':[{'uid':'1','name':'Philip Walter Foden','dob':'2000-05-28',
                      'kind':'player','attr_source':'derived','ca':'160','pa':'180',
                      'attributes':{'Teamwork':18.0}}]}
        tm={'2':{'name':'Phil Foden','date_of_birth':'2000-05-28',
                 'current_club_name':'Manchester City'}}
        candidate=rank_person('Foden, Philip Walter','Man City','rp1',rec,{'2'},tm,native)
        self.assertIsNotNone(candidate)
        self.assertFalse(candidate['can_use_native_player_attributes'] and
                         candidate.get('native_attr_source')=='db')


if __name__=='__main__':unittest.main()
