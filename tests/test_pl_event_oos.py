"""Synthetic regression checks for the StatsBomb event-label / pre-kickoff baseline."""
import json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from backtest_pl_event_oos import match_events,fit,score_game

class EventBacktestTests(unittest.TestCase):
    def test_event_labels_clock_and_assist_are_distinct_from_xa(self):
        game={'match_id':4,'match_date':'2015-09-01','home_team':{'home_team_name':'A'},
              'away_team':{'away_team_name':'B'},'home_score':1,'away_score':0}
        base={'team':{'name':'A'},'player':{'id':42,'name':'Forward'},'minute':4,'second':31}
        examples=[{**base,'type':{'name':'Shot'},'shot':{'outcome':{'name':'Goal'}}},
                  {**base,'type':{'name':'Pass'},'pass':{'goal_assist':True},'player':{'id':43,'name':'Creator'},'minute':3},
                  {**base,'type':{'name':'Pass'},'pass':{'goal_assist':False}},
                  {**base,'type':{'name':'Shot'},'shot':{'outcome':{'name':'Saved'}},'minute':92},
                  {**base,'team':{'name':'B'},'type':{'name':'Shot'},'shot':{'outcome':{'name':'Saved'}}}]
        parsed=match_events(game,examples)
        self.assertEqual(parsed['teams']['A']['counts']['goals'],1)
        self.assertEqual(parsed['teams']['A']['counts']['assists'],1)
        self.assertEqual(parsed['teams']['A']['counts']['shots'],2)
        self.assertEqual(parsed['teams']['A']['minutes']['shots'][89],1)
        self.assertEqual(parsed['teams']['A']['players']['43']['assists'],1)
        self.assertEqual(parsed['observed_scores_from_goals'],[1,0])

    def test_test_match_cannot_change_existing_prior_forecast(self):
        def fake(day,mid,home_goals,shot_count):
            return {'id':mid,'date':day,'home':'A','away':'B','score':[home_goals,0],'observed_scores_from_goals':[home_goals,0],
                    'teams':{t:{'counts':{'goals':home_goals if t=='A' else 0,
                                          'shots':shot_count if t=='A' else 0},
                                 'players':{'4':{'shots':shot_count}} if t=='A' else {},
                                 'minutes':{k:[0]*90 for k in ('shots','goals','assists')}} for t in ('A','B')}}
        hist=[fake(f'2015-08-{i:02d}',i,1,i) for i in range(1,16)]
        forecast=score_game(fake('2015-09-12',22,1,5),fit(hist))
        revised=score_game(fake('2015-09-12',22,7,100),fit(hist))
        self.assertEqual(forecast['predicted_goals'],revised['predicted_goals'])
        self.assertEqual(forecast['prior_games'],{'A':15,'B':15})
        self.assertEqual(forecast['predicted_goals'],revised['predicted_goals'])

    def test_saved_holdout_has_no_future_fm(self):
        data=json.loads((ROOT/'artifacts/validation/premier-league-events-2015-16-oos.json').read_text())
        self.assertTrue(data['split']['no_FM26_used'])
        dates=[g['date'] for g in data['games']]
        self.assertEqual(dates,sorted(dates))
        self.assertEqual(len(dates),24)
        self.assertEqual(data['split']['earliest_test_date'],'2016-02-13')
        self.assertEqual(data['split']['latest_test_date'],'2016-05-17')
        for game in data['games']:
            self.assertEqual(game['actual_score'],game['observed_goals'])

if __name__=='__main__':unittest.main()
