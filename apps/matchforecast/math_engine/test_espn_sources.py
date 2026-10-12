"""Provider-level clock, identity and completion gates; all fixtures synthetic."""
import copy
from datetime import date, datetime, timezone
import hashlib
import json
import tempfile
from pathlib import Path
import unittest

from attach_espn import (merge, name_key, resolve_roster_identity,
                         verify_future_schedule, verify_raw_archive)
from espn_feed import normalize_match


def toy_match():
    sides = []
    for tid, name, start in [('1','Coventry City',100),('2','Newcastle United',200)]:
        lineup = []
        for i in range(12):
            lineup.append({
                'athlete': {'id':str(start+i),'fullName':f'Player {start+i}',
                            'position':{'name':'Goalkeeper' if i == 0 else 'Midfielder'}},
                'starter':i<11, 'subbedIn':i==11,'subbedOut':i==10,
                'stats':[{'name':'appearances','value':1 if i<11 or i==11 else 0},
                         {'name':'totalShots','value':1 if i<11 or i==11 else 0},
                         {'name':'totalGoals','value':0}],
            })
        sides.append({'team':{'id':tid,'displayName':name},'roster':lineup})
    event = {'id':'401','date':'2026-09-12T14:00:00Z'}
    competition = {
        'date':'2026-09-12T14:00:00Z',
        'status':{'type':{'completed':True}},
        'competitors':[
            {'homeAway':'home','team':{'id':'1','displayName':'Coventry City'}},
            {'homeAway':'away','team':{'id':'2','displayName':'Newcastle United'}},
        ],
    }
    events = []
    for team_id, number in [('1',100),('2',200)]:
        events.append({'team':{'id':team_id},'type':{'type':'substitution'},
                       'clock':{'value':3600},
                       'participants':[{'athlete':{'id':str(number+11)}},
                                       {'athlete':{'id':str(number+10)}}]})
    return {
        'header':{'id':'401','competitions':[competition]},
        'rosters':sides,'keyEvents':events,
    }, event


class ESPNSourceTests(unittest.TestCase):
    def test_completed_time_and_substitution_preserve_990_player_minutes(self):
        raw, event = toy_match()
        result = normalize_match(raw,event,date(2026,10,11))
        self.assertEqual(result['verified_completed_substitutions'],2)
        self.assertEqual(len(result['players']),24)
        for team in ('Coventry City','Newcastle United'):
            team_players=[p for p in result['players'] if p['club']==team]
            self.assertEqual(sum(p['started'] for p in team_players),11)
            self.assertEqual(sum(p['appeared'] for p in team_players),12)
            self.assertAlmostEqual(sum(p['minutes'] for p in team_players),990)
            self.assertAlmostEqual(team_players[10]['minutes'],60)
            self.assertAlmostEqual(team_players[11]['minutes'],30)

    def test_wrong_match_identity_and_future_target_rejected(self):
        raw, event = toy_match()
        event['id']='elsewhere'
        with self.assertRaisesRegex(ValueError,'ID mismatch'):
            normalize_match(raw,event,date(2026,10,11))
        event['id']='401'
        with self.assertRaisesRegex(ValueError,'same-day'):
            normalize_match(raw,event,date(2026,9,12))

    def test_unobserved_extra_player_fails_closed(self):
        raw,event=toy_match()
        raw['rosters'][0]['roster'][11]['subbedIn']=True
        raw['keyEvents']=raw['keyEvents'][1:]
        with self.assertRaisesRegex(ValueError,'appearance event lacks'):
            normalize_match(raw,event,date(2026,10,11))

    def test_dismissal_reduces_minutes_without_invented_replacement(self):
        raw,event=toy_match()
        raw['keyEvents'].append({
            'team':{'id':'1'},'type':{'type':'red-card','text':'Red Card'},
            'clock':{'value':4500},
            'participants':[{'athlete':{'id':'100'}}],
        })
        result=normalize_match(raw,event,date(2026,10,11))
        self.assertEqual(result['verified_red_cards'],1)
        self.assertAlmostEqual(sum(p['minutes'] for p in result['players']
                                   if p['club']=='Coventry City'),975)

    def test_unique_registered_legal_name_tokens_not_name_order(self):
        teams = {'Newcastle':{'20':'Anthony Elanga',
                              '21':'Harvey Barnes'}}
        names = {('Newcastle',name_key(v)):{k}
                 for k,v in teams['Newcastle'].items()}
        athlete,rule=resolve_roster_identity(
            'Elanga, Anthony David Junior*', None, 'Newcastle',names, teams)
        self.assertEqual(athlete,'20')
        self.assertIn('legal',rule)
        ambiguous={('Newcastle',name_key('Anthony Elanga')):{'20','30'}}
        athlete,rule=resolve_roster_identity(
            'Elanga, Anthony David Junior*','Anthony Elanga','Newcastle',
            ambiguous,{'Newcastle':{'20':'Anthony Elanga','30':'Anthony Elanga'}})
        self.assertIsNone(athlete)

    def test_archive_after_cutoff_is_not_asof_and_no_score_label_splice(self):
        from attach_espn import validate_snapshot
        base={'schema':'math_shot_goal_v1','as_of':'2026-10-10',
              'fixtures':[{'date':'2026-10-12','home':'Coventry','away':'Newcastle'}]}
        espn={'schema':'espn_player_appearances_v1',
              'retrieved_completed_at_utc':'2026-10-12T02:30:00Z',
              'as_of_exclusive_calendar_date':'2026-10-11','matches':[]}
        squads={'as_of':'2026-10-10','teams':{}}
        kickoff=datetime(2026,10,12,19,tzinfo=timezone.utc)
        with self.assertRaisesRegex(ValueError,'AFTER'):
            validate_snapshot(base,espn,squads,datetime(
                2026,10,12,1,tzinfo=timezone.utc),kickoff)
        validate_snapshot(base,espn,squads,datetime(
            2026,10,12,3,tzinfo=timezone.utc),kickoff)

    def test_schedule_identity_and_kickoff_are_verified(self):
        fixture={'date':'2026-10-12','home':'Coventry','away':'Newcastle'}
        event={'id':'123', 'date':'2026-10-12T19:00:00Z',
               'status':{'type':{'completed':False}},
               'competitions':[{'competitors':[
                   {'homeAway':'home','team':{'displayName':'Coventry City'}},
                   {'homeAway':'away','team':{'displayName':'Newcastle United'}},
               ]}]}
        board={'leagues':[{'slug':'eng.1'}],'events':[event]}
        dt=datetime(2026,10,12,19,tzinfo=timezone.utc)
        self.assertEqual(verify_future_schedule(board,fixture,dt),'123')
        with self.assertRaisesRegex(ValueError,'timestamp differs'):
            verify_future_schedule(board,fixture,dt.replace(hour=18))
        event['status']['type']['completed']=True
        with self.assertRaisesRegex(ValueError,'already completed'):
            verify_future_schedule(board,fixture,dt)

    def test_mutation_to_archived_raw_file_is_detected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'summary.json'
            p.write_text('{"old":"source"}')
            digest=hashlib.sha256(p.read_bytes()).hexdigest()
            manifest={'raw_source_sha256':{str(p):digest}}
            verify_raw_archive(manifest)
            p.write_text('{"new":"source"}')
            with self.assertRaisesRegex(ValueError,'mutated'):
                verify_raw_archive(manifest)


if __name__ == '__main__':
    unittest.main()
