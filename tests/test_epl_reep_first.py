"""Verify that Reep aliases, not FM name-token heuristics, discover players."""

import sys
import unittest
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from resolve_epl_reep import candidate_key, label_match, resolve_one, resolve_roster
from match_premier_league_fm26 import TEAM_ALIASES


def entry(uid, label, *, kind='player', attr_source='db'):
    native = {'uid': uid, 'name': label, 'dob': '1999-05-07',
              'kind':kind, 'attr_source':attr_source, 'ca':'155', 'pa':'167',
              'attributes': {'Finishing':15.0}}
    return native


def registered(club, name):
    return {'official_name': name, 'registration':'registered',
            'registered_club':club, 'match_method':'unmatched'}


class EPLReepFirstTests(unittest.TestCase):
    def setUp(self):
        self.index = defaultdict(set)
        self.entries = {}
        self.native = {}

    def add(self, rid, uid, label, *aliases, kind='player'):
        names = {label, *aliases}
        self.entries[rid] = {'uids':{uid}, 'names':names, 'rungs':{'name-dob'}}
        self.native[uid] = [entry(uid,label,kind=kind)]
        for name in names:
            self.index[candidate_key(name)].add(rid)

    def test_missing_famous_names_resolve_by_expanded_names(self):
        cases = [
            ('Gakpo, Cody', 'Cody Mathès Gakpo', '37062185'),
            ('Saka, Bukayo*', 'Bukayo Ayoyinka Temidayo Saka', '28122642'),
            ('Doku, Jeremy Baffour', 'Jérémy Baffour Doku', '18106083'),
            ('Frimpong, Jeremie*', 'Jeremie Agyekum Frimpong', '28113828'),
            ('Chiesa, Federico', 'Federico Chiesa', '43161651'),
            ('Havertz, Kai', 'Kai Lukas Havertz', '91151081'),
            ('Milenkovic, Nikola', 'Nikola Milenković', '62191195'),
        ]
        for i,(official, label, uid) in enumerate(cases):
            self.add(f'rp{i}',uid,label)
            with self.subTest(name=official):
                result,candidates = resolve_one(official,self.index,self.entries,self.native)
                self.assertIsNotNone(result)
                self.assertEqual(result['fm_uid'],uid)

    def test_unique_name_collision_must_be_quarantined(self):
        self.add('rp1','41','John Michael Smith')
        self.add('rp2','42','John Alex Smith')
        selected,choices=resolve_one('Smith, John',self.index,self.entries,self.native)
        self.assertIsNone(selected)
        self.assertEqual(len(choices),2)

    def test_no_popularity_or_ca_disambiguation(self):
        self.add('rp1','41','John Michael Smith')
        self.add('rp2','42','John Alex Smith')
        self.native['41'][0]['ca']='195'
        self.native['42'][0]['ca']='51'
        selected,_=resolve_one('Smith, John',self.index,self.entries,self.native)
        self.assertIsNone(selected)

    def test_rejects_contradictory_native_fm_record(self):
        self.add('rp1','99','Matheus Luiz Nunes')
        self.native['99'][0]['name'] = 'Jorge Manuel Domingues Maria Vital'
        selected, choices = resolve_one('Nunes, Matheus Luiz',self.index,self.entries,self.native)
        self.assertIsNone(selected)
        self.assertEqual(choices[0]['status'],'native_fm_name_contradicts_reep_alias')

    def test_unicode_and_long_legal_names(self):
        self.add('rp1','100','Martin Ødegaard')
        result,_=resolve_one('Odegaard, Martin',self.index,self.entries,self.native)
        self.assertEqual(result['fm_uid'],'100')
        self.add('rp2','101','Bruno Guimarães')
        result,_=resolve_one('Guimaraes Rodriguez Moura, Bruno',self.index,self.entries,self.native)
        self.assertEqual(result['fm_uid'],'101')

    def test_full_roster_recovers_reep_missing_and_keeps_old_legacy(self):
        self.add('rp1','37062185','Cody Mathès Gakpo')
        teams={club:{'registered_count':0,'matched_count':0,'players':[], 'unmatched':[]}
               for club in TEAM_ALIASES}
        teams['Liverpool']['players']=[registered('Liverpool','Gakpo, Cody')]
        teams['Liverpool']['registered_count']=1
        teams['Man City']['players']=[{**registered('Man City','Haaland, Erling'), 'fm_uid':'29179241'}]
        teams['Man City']['registered_count']=1
        original={'roster_scope':'full_league','as_of':'2026-10-11','teams':teams}
        result=resolve_roster(original,self.index,self.entries,self.native)
        recovered=result['teams']['Liverpool']['players'][0]
        self.assertEqual(recovered['fm_uid'],'37062185')
        self.assertTrue(recovered['fm_attributes_verified'])
        self.assertEqual(recovered['attributes']['Finishing'],15.)
        self.assertIn('not_dob_verified',recovered['identity_verification'])
        self.assertEqual(result['teams']['Man City']['players'][0]['fm_uid'],'29179241')
        self.assertEqual(result['teams']['Man City']['players'][0]['identity_verification'],
                         'legacy_name_only_unverified')

    def test_cross_club_duplicate_uid_is_not_accepted(self):
        self.add('rp1','37062185','Cody Mathès Gakpo')
        teams={club:{'registered_count':0,'matched_count':0,'players':[],'unmatched':[]}
               for club in TEAM_ALIASES}
        for club in ('Liverpool','Arsenal'):
            teams[club]['players']=[registered(club,'Gakpo, Cody')]
            teams[club]['registered_count']=1
        result=resolve_roster({'roster_scope':'full_league','teams':teams},self.index,self.entries,self.native)
        self.assertEqual(result['teams']['Liverpool']['matched_count'],0)
        self.assertEqual(result['teams']['Arsenal']['matched_count'],0)
        self.assertEqual(result['resolution_summary']['colliding_registration_rows_quarantined'],2)


if __name__ == '__main__':unittest.main()
