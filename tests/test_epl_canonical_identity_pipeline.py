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
    build_full_native_name_index, candidate_for_row, choose_candidate,
    evidence_vector, index_reep, native_full_legal_fallback,
    native_pair_fallback, rank_person, read_sortitoutsi_corrob, recover,
    shared_name,
)


def registered(name):
    return {'official_name':name,'registration':'registered',
            'fm_uid':None,'loaned_out':False}


def selection(**kw):
    """Minimal candidate dict shaped like rank_person's output."""
    base={'reep_id':'rp','fm_uid':'1','native_dob_agrees_with_tm':False,
          'native_kind':'staff','tm_club_compatible':False,
          'native_name_evidence_tier':2,'alias_name_evidence_tier':2,
          'score':8,'sufficient':False}
    base.update(kw)
    return base


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

    # --- clean-run coverage-regression gates (previously 952 vs 1,228) -------

    def test_transfermarkt_dob_without_current_club_is_still_sufficient(self):
        """Goretzka: TM club is a stale pre-transfer snapshot, DOB still ties."""
        rec={'names':{'Leon Christoph Goretzka'},'uids':{'91104807'}}
        native={'91104807':[{'uid':'91104807','name':'Leon Christoph Goretzka',
                             'dob':'1995-02-06','kind':'player','attr_source':'db',
                             'ca':'170','pa':'175','attributes':{}}]}
        tm={'1':{'name':'Leon Goretzka','date_of_birth':'1995-02-06',
                 'current_club_name':'FC Bayern Munich','player_id':'1'}}
        got=rank_person('Goretzka, Leon Christop','Aston Villa','rp',rec,{'1'},tm,native)
        self.assertTrue(got['sufficient'])
        self.assertTrue(got['native_dob_agrees_with_tm'])
        self.assertFalse(got['tm_club_compatible'])

    def test_unique_reep_identity_without_transfermarkt_is_provisional(self):
        """A lone Reep entity with one typed FM bridge is a valid discovery."""
        c=selection()
        chosen=choose_candidate([c])
        self.assertIs(chosen,c)
        self.assertTrue(c['sufficient'])
        self.assertEqual(c['identity_source'],'unique_reep_typed_fm_bridge')

    def test_equally_supported_reep_identities_are_not_arbitrarily_chosen(self):
        a=selection(reep_id='rpa',fm_uid='1',sufficient=True,score=20)
        b=selection(reep_id='rpb',fm_uid='2',sufficient=True,score=20)
        self.assertIsNone(choose_candidate([a,b]))

    def test_player_kind_and_club_override_a_higher_raw_score(self):
        """Reece James: a staff-kind same-name record must not outrank the
        corroborated player-kind record merely on raw score."""
        staff=selection(reep_id='rpa',fm_uid='staff',sufficient=True,score=27,
                        native_dob_agrees_with_tm=True,native_kind='staff',
                        tm_club_compatible=False,native_name_evidence_tier=4)
        player=selection(reep_id='rpb',fm_uid='player',sufficient=True,score=26,
                         native_dob_agrees_with_tm=True,native_kind='player',
                         tm_club_compatible=True,native_name_evidence_tier=4)
        self.assertEqual(choose_candidate([staff,player])['fm_uid'],'player')

    def test_independent_dob_disambiguates_same_name_candidates(self):
        confirmed=selection(reep_id='rpa',fm_uid='1',sufficient=True,score=27,
                            native_dob_agrees_with_tm=True,native_kind='player',
                            tm_club_compatible=True,native_name_evidence_tier=4)
        unconfirmed=selection(reep_id='rpb',fm_uid='2',sufficient=True,score=26)
        self.assertEqual(choose_candidate([confirmed,unconfirmed])['fm_uid'],'1')

    def test_full_legal_name_beats_generic_player_kind_name(self):
        """Gomes da Silva: a tier-4 staff-kind record must not lose to a
        tier-2 player-kind 'Joao Silva' merely on native kind."""
        strong=selection(reep_id='rpa',fm_uid='A',sufficient=True,score=23,
                         native_dob_agrees_with_tm=True,native_kind='staff',
                         tm_club_compatible=False,native_name_evidence_tier=0,
                         alias_name_evidence_tier=4)
        weak=selection(reep_id='rpb',fm_uid='B',sufficient=True,score=15,
                       native_dob_agrees_with_tm=True,native_kind='player',
                       tm_club_compatible=False,native_name_evidence_tier=1,
                       alias_name_evidence_tier=2)
        self.assertEqual(choose_candidate([strong,weak])['fm_uid'],'A')

    def test_comparable_name_evidence_uses_club_and_kind(self):
        staff=selection(reep_id='rpa',fm_uid='staff',sufficient=True,score=27,
                        native_dob_agrees_with_tm=True,native_kind='staff',
                        tm_club_compatible=False,native_name_evidence_tier=4,
                        alias_name_evidence_tier=4)
        player=selection(reep_id='rpb',fm_uid='player',sufficient=True,score=26,
                         native_dob_agrees_with_tm=True,native_kind='player',
                         tm_club_compatible=True,native_name_evidence_tier=4,
                         alias_name_evidence_tier=3)
        self.assertEqual(choose_candidate([staff,player])['fm_uid'],'player')

    def test_two_token_exact_legal_name_is_attached_without_attributes(self):
        """Caelan Avenell: exact 2-token legal name was wrongly rejected."""
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'fm.csv'
            with path.open('w',newline='') as h:
                w=csv.DictWriter(h,fieldnames=['uid','name','dob','kind','attr_source'])
                w.writeheader()
                w.writerow({'uid':'2000140968','name':'Caelan Avenell',
                            'dob':'2004-10-08','kind':'player','attr_source':'db'})
            roster={'teams':{'Brentford':{'players':[registered('Avenell, Caelan*')]}}}
            index=build_full_native_name_index(path,roster)
            got=native_full_legal_fallback('Avenell, Caelan*',
                                           index[('Brentford','Avenell, Caelan*')])
            self.assertEqual(got['fm_uid'],'2000140968')
            self.assertFalse(got['can_use_native_player_attributes'])

    def test_native_index_grades_and_rejects_namesakes(self):
        """Exact = 0, subset = 1, near-spelling middle = 2, and a genuinely
        different middle name (a namesake) must NOT match."""
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'fm.csv'
            with path.open('w',newline='') as h:
                w=csv.DictWriter(h,fieldnames=['uid','name','dob','kind','attr_source'])
                w.writeheader()
                w.writerow({'uid':'2000274223','name':'Josiah King',
                            'dob':'2001-04-21','kind':'staff','attr_source':'staff'})
                w.writerow({'uid':'9','name':'John Roderick Daniel Dodds',
                            'dob':'2001-04-21','kind':'player','attr_source':'db'})
                w.writerow({'uid':'8','name':'Caelan Avenell',
                            'dob':'2004-10-08','kind':'player','attr_source':'db'})
                w.writerow({'uid':'7','name':'Gabriel Filipe Passos Magalhaes',
                            'dob':'1997-12-19','kind':'player','attr_source':'db'})
            roster={'teams':{'Arsenal':{'players':[
                registered('King, Josiah Barack'),registered('Dodds, John Rodrick Daniel'),
                registered('Avenell, Caelan*'),registered('Dos Santos Magalhaes, Gabriel')]}}}
            index=build_full_native_name_index(path,roster)
            self.assertTrue(any(e['grade']==1 for e in index[('Arsenal','King, Josiah Barack')]))
            self.assertTrue(any(e['grade']==2 for e in index[('Arsenal','Dodds, John Rodrick Daniel')]))
            self.assertTrue(any(e['grade']==0 for e in index[('Arsenal','Avenell, Caelan*')]))
            self.assertFalse(index[('Arsenal','Dos Santos Magalhaes, Gabriel')],
                             'a different-middle namesake must not match')

    def test_namesake_with_different_middle_is_not_attached(self):
        """Gabriel dos Santos Magalhaes must not resolve to Gabriel Filipe
        Passos Magalhaes; a near-spelling middle still grades as 2."""
        from recover_epl_remaining_identities import _name_grade
        self.assertIsNone(_name_grade(
            ('gabriel','dos','santos','magalhaes'),
            ('gabriel','filipe','passos','magalhaes')))
        self.assertEqual(_name_grade(
            ('john','rodrick','daniel','dodds'),
            ('john','roderick','daniel','dodds')),2)

    def test_native_pair_fallback_requires_global_uniqueness(self):
        entries=[{'uid':'1','fm_name':'Josiah King','native_kind':'staff',
                  'native_dob':'2001-04-21','grade':2},
                 {'uid':'2','fm_name':'Josiah King','native_kind':'player',
                  'native_dob':'2002-01-01','grade':2}]
        self.assertIsNone(native_pair_fallback('King, Josiah Barack',entries))
        got=native_pair_fallback('King, Josiah Barack',[entries[0]])
        self.assertEqual(got['fm_uid'],'1')
        self.assertFalse(got['can_use_native_player_attributes'])


if __name__=='__main__':unittest.main()
