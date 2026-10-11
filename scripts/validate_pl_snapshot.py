#!/usr/bin/env python3
"""Hard validation of the committed player MC snapshot and eligible roster mapping."""
import argparse,json,math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source',default='artifacts/premier-league-2026-10-11/player_paths.json')
parser.add_argument('--audit',default='artifacts/premier-league-2026-10-11/fm_matched_squads.json')
a=parser.parse_args()
result=json.loads((ROOT/a.source).read_text())
roster=json.loads((ROOT/a.audit).read_text())
assert len(result['fixtures'])>0
names=set()
for f in result['fixtures']:
    assert f['paths']>0 and len(f['15_minute_windows'])==6
    assert (f['home'],f['away']) not in names
    names.add((f['home'],f['away']))
    assert all(math.isfinite(f[k]) and 0<=f[k]<=1 for k in ('home_win','draw','away_win'))
    assert abs(sum(f[k] for k in ('home_win','draw','away_win'))-1)<1e-5
    assert f['registered_fm_players']['home']>=14 and f['registered_fm_players']['away']>=14
    home_total=away_total=0.0
    for idx,w in enumerate(f['15_minute_windows']):
        assert w['minute_start']==15*idx and w['minute_end']==15*(idx+1)
        for key in ('p_any_goal','p_home_goal','p_away_goal','p_red_card','p_substitution'):
            assert 0<=w[key]<=1,(f['home'],w,key)
        assert 0<=w['p_any_goal']<=w['p_home_goal']+w['p_away_goal']+1e-9
        assert abs(sum(w['end_of_window_outcomes'])-1)<1e-5
        home_total+=w['expected_home_goals'];away_total+=w['expected_away_goals']
    assert abs(home_total-f['expected_home_goals'])<1e-4
    assert abs(away_total-f['expected_away_goals'])<1e-4
    print(f['home'],f['away'],f['paths'],'paths',f'home={f["home_win"]:.3f} draw={f["draw"]:.3f} away={f["away_win"]:.3f}')
assert len(roster['teams'])>=2
seen={}
for club,r in roster['teams'].items():
    for p in r['matched']:
        assert p['fm_uid'] not in seen or p['registration']=='u21' or seen[p['fm_uid']]['registration']=='u21'
        seen[p['fm_uid']]=p
print('Validated matchweek simulation snapshot and matched official squad identity sets.')
