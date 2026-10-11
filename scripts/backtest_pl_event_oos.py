#!/usr/bin/env python3
"""Leakage-safe chronological Premier League EVENT baseline backtest on StatsBomb open data.

This tests WHAT can be validated with the available 2015/16 Premier League
coverage, NOT the 2026 FM26 Rust player simulator. Only previous fixture dates
are used for player identity/rate priors and minute hazard forecasts. Same-day
fixtures are processed together (no within-day cross-game information).

Observed: StatsBomb shots, scored shots, registered goal-assist passes and
90 minute events (stoppage aggregated into the last regulation minute).
Future/current squads and FM26 are forbidden for this historical test.
"""
from __future__ import annotations
import argparse,json,math,random
from collections import defaultdict,Counter
from datetime import date
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
KINDS=('shots','goals','assists')

def match_events(match, events):
    teams=[match['home_team']['home_team_name'],match['away_team']['away_team_name']]
    counts={t:Counter() for t in teams}
    players={t:defaultdict(Counter) for t in teams}
    minutes={t:{metric:[0]*90 for metric in KINDS} for t in teams}
    player_names={}
    goals=Counter()
    for e in events:
        t=e.get('team',{}).get('name')
        if t not in counts:continue
        kind=e.get('type',{}).get('name')
        metrics=[]
        if kind=='Shot':
            metrics=['shots']
            if e.get('shot',{}).get('outcome',{}).get('name')=='Goal':metrics.append('goals')
        elif kind=='Pass' and e.get('pass',{}).get('goal_assist') is True:
            metrics=['assists']
        elif kind=='Own Goal Against':
            # Own goals increment opposition score but cannot be attributed
            # to an attacker or to a valid assisting player.
            goals[teams[1] if t==teams[0] else teams[0]]+=1
        if not metrics:continue
        who=e.get('player',{}).get('id')
        if not who:continue
        player_names[str(who)]=e['player'].get('name','')
        clock=min(89,max(0,int(e.get('minute',0))))
        for metric in metrics:
            counts[t][metric]+=1
            players[t][str(who)][metric]+=1
            minutes[t][metric][clock]+=1
            if metric=='goals':goals[t]+=1
    return {'id':match['match_id'],'date':match['match_date'],'home':teams[0],'away':teams[1],
            'score':[match['home_score'],match['away_score']],
            'observed_scores_from_goals':[goals[t] for t in teams],
            'teams':{t:{'counts':dict(counts[t]),'players':{k:dict(v) for k,v in players[t].items()},
                        'minutes':minutes[t]} for t in teams},'names':player_names}

def load_games(matches_file,events_dir):
    matches=json.loads(matches_file.read_text())
    results=[];missed=0
    for m in matches:
        file=events_dir/f"{m['match_id']}.json"
        if not file.exists():missed+=1;continue
        results.append(match_events(m,json.loads(file.read_text())))
    results.sort(key=lambda g:(g['date'],g['id']))
    if not results:raise ValueError('no StatsBomb fixture events to evaluate')
    return results,missed

def fit(history):
    team_games=Counter();team_totals=defaultdict(Counter)
    player_totals=defaultdict(Counter);player_apps=Counter();minute_totals={k:[0]*90 for k in KINDS}
    league_totals=Counter()
    for g in history:
        for team in (g['home'],g['away']):
            team_games[team]+=1
            rows=g['teams'][team]
            for k in KINDS:
                team_totals[team][k]+=rows['counts'].get(k,0)
                league_totals[k]+=rows['counts'].get(k,0)
                for minute,n in enumerate(rows['minutes'][k]):minute_totals[k][minute]+=n
            for uid,v in rows['players'].items():
                player_apps[(team,uid)]+=1
                for k in KINDS:player_totals[(team,uid)][k]+=v.get(k,0)
    return team_games,team_totals,player_totals,player_apps,minute_totals,league_totals

def brier(p,o):return (p-float(bool(o)))**2

def score_game(game,state):
    team_games,team_totals,player_totals,apps,mins,league=state
    n_team_matches=sum(team_games.values())
    out={'id':game['id'],'date':game['date'],'home':game['home'],'away':game['away'],
         'actual_score':game['score'],'observed_goals':game['observed_scores_from_goals'],
         'predicted_goals':[],'prior_games':{},'covered_players':{},'unseen_player_actual_goals':0,
         'unseen_player_actual_assists':0,'player_count':0,
         'expected_goal_by_minute':[0.]*90,'actual_goal_by_minute':[0]*90,
         'expected_assist_by_minute':[0.]*90,'actual_assist_by_minute':[0]*90,
         'player_loss':Counter(),
         'player_reference_loss':Counter(),'goal_minute_loss':0.,'goal_minute_baseline_loss':0.,
         'goal_15min_loss':0.,'goal_15min_baseline_loss':0.,
         'assist_minute_loss':0.,'assist_minute_baseline_loss':0.,'shot_loss':0.,'shot_baseline_loss':0.,
         'score_goal_abs_error':0.,'goal_event_times':[],'player_scorers':[],'player_assisters':[]}
    league_rates={k:league[k]/max(n_team_matches,1) for k in KINDS}
    minute_distribution=[(mins['goals'][i]+1)/(sum(mins['goals'])+90) for i in range(90)]
    for team in (game['home'],game['away']):
        ng=team_games[team]
        out['prior_games'][team]=ng
        # Team prior: shrink 4 synthetic games toward PREVIOUS league rate.
        means={k:(team_totals[team][k]+4*league_rates[k])/(ng+4) for k in KINDS}
        out['predicted_goals'].append(round(means['goals'],3))
        out['score_goal_abs_error']+=abs(means['goals']-game['teams'][team]['counts'].get('goals',0))
        known=[uid for (name,uid),n in apps.items() if name==team]
        out['covered_players'][team]=len(known)
        actual=game['teams'][team]['players']
        out['unseen_player_actual_goals']+=sum(v.get('goals',0) for uid,v in actual.items() if uid not in known)
        out['unseen_player_actual_assists']+=sum(v.get('assists',0) for uid,v in actual.items() if uid not in known)
        # Each candidate player is derived only from matches before this day;
        # no test-match lineup is used. Known players can be absent this match.
        for uid in known:
            out['player_count']+=1
            app=apps[(team,uid)]
            selection=(app+1)/(ng+2) # finite prior for player appearance
            for k in KINDS:
                player_rate=(player_totals[(team,uid)][k]+0.25*league_rates[k])/(app+1)
                lam=selection*player_rate
                p=-math.expm1(-lam)
                observed=actual.get(uid,{}).get(k,0)
                out['player_loss'][k]+=brier(p,observed)
                # Previous-only uniform-player baseline (team-count independent).
                basic=-math.expm1(-league_rates[k]/max(len(known),1))
                out['player_reference_loss'][k]+=brier(basic,observed)
        for k in KINDS:
            out['shot_loss']+=abs(means[k]-game['teams'][team]['counts'].get(k,0)) if k=='shots' else 0.
            out['shot_baseline_loss']+=abs(league_rates[k]-game['teams'][team]['counts'].get(k,0)) if k=='shots' else 0.
        assist_distribution=[(mins['assists'][i]+1)/(sum(mins['assists'])+90) for i in range(90)]
        for minute in range(90):
            lam=means['goals']*minute_distribution[minute]
            forecast=-math.expm1(-lam)
            simple=-math.expm1(-league_rates['goals']/90)
            out['expected_goal_by_minute'][minute]+=lam
            out['actual_goal_by_minute'][minute]+=game['teams'][team]['minutes']['goals'][minute]
            truth=game['teams'][team]['minutes']['goals'][minute]>0
            out['goal_minute_loss']+=brier(forecast,truth)
            out['goal_minute_baseline_loss']+=brier(simple,truth)
            assister=-math.expm1(-means['assists']*assist_distribution[minute])
            basic_assist=-math.expm1(-league_rates['assists']/90)
            out['expected_assist_by_minute'][minute]+=means['assists']*assist_distribution[minute]
            out['actual_assist_by_minute'][minute]+=game['teams'][team]['minutes']['assists'][minute]
            actual_assist=game['teams'][team]['minutes']['assists'][minute]>0
            out['assist_minute_loss']+=brier(assister,actual_assist)
            out['assist_minute_baseline_loss']+=brier(basic_assist,actual_assist)
        for start in range(0,90,15):
            lam=means['goals']*sum(minute_distribution[start:start+15])
            forecast=-math.expm1(-lam)
            basic=-math.expm1(-league_rates['goals']/6)
            truth=sum(game['teams'][team]['minutes']['goals'][start:start+15])>0
            out['goal_15min_loss']+=brier(forecast,truth)
            out['goal_15min_baseline_loss']+=brier(basic,truth)
        for uid,metrics in actual.items():
            for metric,field in [('goals','player_scorers'),('assists','player_assisters')]:
                if metrics.get(metric,0):out[field].append({'player_id':uid,'count':metrics[metric],
                  'previously_seen':uid in known})
        out['goal_event_times']+=[{'team':team,'minute_index':minute,'goals':v}
            for minute,v in enumerate(game['teams'][team]['minutes']['goals']) if v]
    return out

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--matches',default='data/statsbomb/matches/pl_2015_16.json')
    ap.add_argument('--events',default='data/statsbomb/events')
    ap.add_argument('--holdout-matches',type=int,default=24)
    ap.add_argument('--output',default='artifacts/validation/premier-league-events-2015-16-oos.json')
    args=ap.parse_args()
    data,missing=load_games(ROOT/args.matches,ROOT/args.events)
    if not 2<=args.holdout_matches<len(data)-10:ap.error('holdout must leave at least 10 earlier matches')
    start=len(data)-args.holdout_matches
    results=[]
    for i in range(start,len(data)):
        game=data[i]
        # Strict match-day cutoff avoids same-day or same-match leakage.
        historical=[g for g in data if g['date']<game['date']]
        assert len(historical)>=10
        results.append(score_game(game,fit(historical)))
    n=len(results)
    def metric(k):return sum(row[k] for row in results)
    np=sum(r['player_count'] for r in results)
    metrics={'player_shot_occurrence_brier':metric('player_loss_shots') if False else sum(r['player_loss']['shots'] for r in results)/max(np,1),
        'player_goal_occurrence_brier':sum(r['player_loss']['goals'] for r in results)/max(np,1),
        'player_assist_occurrence_brier':sum(r['player_loss']['assists'] for r in results)/max(np,1),
        'reference_player_shot_brier':sum(r['player_reference_loss']['shots'] for r in results)/max(np,1),
        'reference_player_goal_brier':sum(r['player_reference_loss']['goals'] for r in results)/max(np,1),
        'reference_player_assist_brier':sum(r['player_reference_loss']['assists'] for r in results)/max(np,1),
        'team_shot_count_mae':metric('shot_loss')/(2*n),
        'reference_team_shot_mae':metric('shot_baseline_loss')/(2*n),
        'minute_goal_brier':metric('goal_minute_loss')/(180*n),
        'reference_minute_goal_brier':metric('goal_minute_baseline_loss')/(180*n),
        'minute_assist_brier':metric('assist_minute_loss')/(180*n),
        'reference_minute_assist_brier':metric('assist_minute_baseline_loss')/(180*n),
        'window15_goal_brier':metric('goal_15min_loss')/(12*n),
        'reference_window15_goal_brier':metric('goal_15min_baseline_loss')/(12*n),
        'team_goal_mae':metric('score_goal_abs_error')/(2*n),
        'prior_known_players_per_match':np/n,
        'unseen_player_actual_goals':metric('unseen_player_actual_goals'),
        'unseen_player_actual_assists':metric('unseen_player_actual_assists')}
    # Paired match-block uncertainty: outcomes within a fixture are correlated.
    # A negative delta favors the historical-rate model. No tuning on holdout.
    rng=random.Random(42)
    confidence={}
    comparisons={
        'minute_goal':('goal_minute_loss','goal_minute_baseline_loss',180),
        '15minute_goal':('goal_15min_loss','goal_15min_baseline_loss',12),
        'minute_assist':('assist_minute_loss','assist_minute_baseline_loss',180),
        'team_shots':('shot_loss','shot_baseline_loss',2),
    }
    for label,(a,b,denom) in comparisons.items():
        differences=[(r[a]-r[b])/denom for r in results]
        bootstrap=[]
        for _ in range(2000):
            bootstrap.append(sum(differences[rng.randrange(n)] for _ in range(n))/n)
        bootstrap.sort()
        confidence[label]={'model_minus_reference_mean':round(sum(differences)/n,7),
            'paired_fixture_bootstrap_95':[round(bootstrap[49],7),round(bootstrap[1949],7)],
            'clear_improvement_at_95':bootstrap[1949]<0}
    out={'source':'StatsBomb open data, English Premier League 2015/16 season, local event JSON',
       'dataset_matches_with_events':len(data),'metadata_matches_without_event_files':missing,
       'split':{'type':'chronological last-N matches; same-day excluded from prior fit',
                'holdout_games':n,'earliest_test_date':results[0]['date'],'latest_test_date':results[-1]['date'],
                'no_FM26_used':True,'confirmed_starting_XIs_at_prediction':False},
       'method':'causal, naive team/player rate hazard baseline fit only on games before each kickoff date',
       'IMPORTANT':'The match simulation from 2026 FM26 is NOT validated here. This is a real-data OOS benchmark and event-label pipeline; player OOS calibration of the Rust FM model remains open.',
       'metrics':{k:round(v,6) for k,v in metrics.items()},
       'paired_matchday_uncertainty_note':'Paired match bootstrap is illustrative; same-day dependency remains and data cover only 74/380 matches.',
       'paired_fixture_bootstrap':confidence,
       'games':results}
    target=ROOT/args.output;target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(json.dumps(out,indent=2,ensure_ascii=False)+'\n')
    print('OOS',n,'games:',results[0]['date'],'to',results[-1]['date'],'prior games strict < date')
    for key,value in out['metrics'].items():print(key, value)
    print('paired deltas',confidence)
    print('saved',target)

if __name__=='__main__':main()
