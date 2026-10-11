#!/usr/bin/env python3
"""Deterministic README cards built from the committed second-resolved path summary."""
import argparse
import json
from pathlib import Path
from functools import lru_cache
from PIL import Image,ImageDraw,ImageFont

ROOT=Path(__file__).resolve().parents[1]
COLORS={'bg':'#0B1220','panel':'#121D30','edge':'#263952','text':'#EFF7FF',
        'muted':'#99AEC3','home':'#5EA7F3','draw':'#A9AEC2','away':'#F2B76E','green':'#5BD6AA'}
FONT='/usr/share/fonts/noto/NotoSans-Regular.ttf'
BOLD='/usr/share/fonts/noto/NotoSans-Bold.ttf'

@lru_cache(maxsize=64)
def font(px,bold=False):
    preferred = BOLD if bold else FONT
    for candidate in (preferred,'DejaVuSans-Bold.ttf' if bold else 'DejaVuSans.ttf'):
        try:return ImageFont.truetype(candidate,px)
        except OSError:pass
    return ImageFont.load_default(size=px)

def txt(d,xy,s,sz=24,color='text',bold=False,anchor=None):
    d.text(xy,str(s),font=font(sz,bold),fill=COLORS.get(color,color),anchor=anchor)
def rect(d,box,fill,radius=16,outline=None):
    d.rounded_rectangle(box,radius=radius,fill=COLORS.get(fill,fill),outline=COLORS.get(outline,outline) if outline else None,width=2)
def forecast_card(f,i,asof):
    W,H=1320,780
    im=Image.new('RGB',(W,H),COLORS['bg']); d=ImageDraw.Draw(im)
    rect(d,(28,22,W-28,H-22),'panel',24,'edge')
    txt(d,(62,55),'PREMIER LEAGUE  /  MATCH SIMULATION',22,'green',True)
    txt(d,(W-64,55),f'{asof} PRE-MATCH SNAPSHOT',18,'muted',anchor='ra')
    home,away=f['home'],f['away']
    txt(d,(62,116),f'{home}  vs  {away}',40,bold=True)
    txt(d,(62,175),f'{f["date"]}    ·    {f["paths"]:,} independent second-resolved paths',19,'muted')
    vals=[f['home_win'],f['draw'],f['away_win']]
    labels=[f'{home} WIN','DRAW',f'{away} WIN'];cs=['home','draw','away']
    left,right=62,W-62;top=241;barw=right-left
    txt(d,(left,209),'FULL-TIME OUTCOME DISTRIBUTION',18,'muted',True)
    cursor=left
    for p,col in zip(vals,cs):
        width=barw*p
        d.rectangle((int(cursor),top,int(cursor+width),top+42), fill=COLORS[col]);cursor+=width
    for j,(p,label,col) in enumerate(zip(vals,labels,cs)):
        x=left+j*(barw/3)
        txt(d,(x,303),f'{p*100:.0f}%',34,col,True)
        txt(d,(x,353),label,16,'muted')
    txt(d,(left,401),'MODELLED 15-MINUTE EVENTS',20,'green',True)
    txt(d,(left,431),'Window',16,'muted');txt(d,(310,431),'Any goal',16,'muted')
    txt(d,(600,431),'Home / away goals',16,'muted');txt(d,(920,431),'Red card',16,'muted');txt(d,(1065,431),'Top event*',16,'muted')
    for j,w in enumerate(f['15_minute_windows']):
        y=465+j*42
        d.rectangle((left,y-3,right,y+34),fill=('#1B2A40' if j%2==0 else COLORS['panel']))
        txt(d,(left+12,y+2),f'{w["minute_start"]:02d}–{w["minute_end"]:02d}',18,'text',True)
        prob=w['p_any_goal'];rect(d,(310,y+8,520,y+23),'edge',6)
        if prob>0:d.rounded_rectangle((310,y+8,310+max(2,210*prob),y+23),radius=5,fill=COLORS['green'])
        txt(d,(535,y+1),f'{prob*100:.0f}%',17,'green',True)
        txt(d,(600,y+1),f'{w["expected_home_goals"]:.2f}  /  {w["expected_away_goals"]:.2f}',18)
        txt(d,(920,y+1),f'{w["p_red_card"]*100:.0f}%',18,'muted')
        event_probs={'Goal':w['p_any_goal'],'Red':w['p_red_card'],'Sub':w['p_substitution']}
        top=max(event_probs, key=event_probs.get)
        txt(d,(1065,y+1),f'{top} {event_probs[top]:.0%}',17,'green',True)
    txt(d,(left,H-70),'*Top event among goal / red card / substitution · scenario priors unvalidated',14,'muted')
    txt(d,(W-64,H-70),'FOOTY MODELS  /  RESEARCH',16,'green',True,anchor='ra')
    return im

def overview(fixtures,asof):
    W,H=1320,705
    im=Image.new('RGB',(W,H),COLORS['bg']);d=ImageDraw.Draw(im)
    txt(d,(50,40),'FOOTY MODELS',24,'green',True)
    txt(d,(50,84),'Premier League · Matchweek forecasts',42,bold=True)
    txt(d,(50,146),f'Player-driven second-resolved Monte Carlo  |  as of {asof}',20,'muted')
    for idx,f in enumerate(fixtures):
        y=205+idx*112
        rect(d,(45,y,1270,y+94),'panel',16,'edge')
        txt(d,(67,y+16),f'{f["home"]}  –  {f["away"]}',25,bold=True)
        txt(d,(67,y+54),f'{f["paths"]} paths  ·  {f["date"]}',17,'muted')
        vals=[f['home_win'],f['draw'],f['away_win']]
        for j,(p,c,label) in enumerate(zip(vals,['home','draw','away'],['HOME','DRAW','AWAY'])):
            x=750+j*159
            txt(d,(x,y+13),f'{p*100:.0f}%',27,c,True)
            txt(d,(x,y+55),label,16,'muted')
    txt(d,(50,H-35),'Research scenario: FM26 player rates and registered squads; injury / lineup status unverified.',17,'muted')
    return im

def main():
    a=argparse.ArgumentParser();a.add_argument('--source',default='artifacts/premier-league-2026-10-11/player_paths.json');args=a.parse_args()
    file=ROOT/args.source; data=json.loads(file.read_text());f=data['fixtures']
    assert len(f)==4,'README snapshot should contain all four fixtures'
    out=ROOT/'assets/predictions/2026-10-11';out.mkdir(parents=True,exist_ok=True)
    overview(f,data['as_of']).save(out/'overview.png',optimize=True)
    for idx,item in enumerate(f):
        assert len(item['15_minute_windows'])==6
        name=f'{idx+1:02d}-'+item['home'].lower().replace(' ','-').replace("'",'')
        forecast_card(item,idx,data['as_of']).save(out/(name+'.png'),optimize=True)
    # Persist the same numeric output as readable per-interval tables, including
    # marginal event likelihoods (these categories can overlap).
    lines=['# Premier League player-resolved match predictions, 11–12 October 2026',
           '',f"**As of:** {data['as_of']} · **Engine:** `simulate_second_resolved_dynamic_mental_match` · **Paths per fixture:** {min(x['paths'] for x in f)}",
           '', '## Results', '',
           '| Fixture | Home win | Draw | Away win | Simulated goals |',
           '|---|---:|---:|---:|---|']
    for row in f:
        lines.append(f"| {row['home']} vs {row['away']} | {row['home_win']:.0%} | {row['draw']:.0%} | {row['away_win']:.0%} | {row['expected_home_goals']:.2f}–{row['expected_away_goals']:.2f} |")
    lines += ['', '**Model comparison:** The historical results + FM benchmark is saved as `artifacts/premier-league-2026-10-11/team_benchmark.json` and is not the simulator. Its stronger validation does not establish the accuracy of this player-level event model.', '']
    for row in f:
        lines += [f"## {row['home']} vs {row['away']}", '',
            '| Minute | Most likely (goal/red/sub) | P(any goal) | P(home goal) | P(away goal) | P(red card) | P(substitution trigger) | Goals H–A | Shots H–A |',
            '|---|---|---:|---:|---:|---:|---:|---|---|']
        for w in row['15_minute_windows']:
            marginal={'goal':w['p_any_goal'],'red card':w['p_red_card'],
                      'substitution':w['p_substitution']}
            most_likely=max(marginal,key=marginal.get)
            lines.append(f"| {w['minute_start']:02d}–{w['minute_end']:02d} | {most_likely} ({marginal[most_likely]:.0%}) | {w['p_any_goal']:.1%} | {w['p_home_goal']:.1%} | {w['p_away_goal']:.1%} | {w['p_red_card']:.1%} | {w['p_substitution']:.1%} | {w['expected_home_goals']:.2f}–{w['expected_away_goals']:.2f} | {w['expected_home_shots']:.2f}–{w['expected_away_shots']:.2f} |")
        lines.append('')
    lines += ['## Interpretation and provenance', '',
       'The event classified as most likely is the largest marginal among goal, red card and substitution trigger, not an exhaustive ranking of every possible football event. The engine simulates full independent 90-minute player/state paths. All 15-minute event counts come from those exact paths; outcome marginals are direct counts. Goal, red-card and substitution-trigger events are not assumed independent, and rows across a window can overlap. End-of-window score-state probabilities appear in the source JSON.',
       '', 'Player clubs come from the [official 2026–27 registrations](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists), matched to FM26 UIDs and attributes. Known injuries/suspensions reported in [October 9 team news](https://www.theguardian.com/football/2026/oct/09/premier-league-team-news-predicted-lineups-for-the-weekend-action) were excluded; doubtful players remain uncertain. Player event rates/lineup exposure lack contemporaneous PL tracking and are FM/legacy-data priors. The original MLS backtest Brier score was 0.7646 on only 12 fixtures, worse than a uniform forecast. **This EPL application is exploratory and not calibrated to bookmaker-grade probability reliability.**',
       '', 'Each fixture uses a limited number of Monte Carlo paths: at 72 paths, sampling error alone can reach roughly ±12 percentage points at 95% confidence, before model error. More paths do not correct missing historical per-player data.',
       '', 'JSON: [`player_paths.json`](../artifacts/premier-league-2026-10-11/player_paths.json). Read the [code inventory](game-forecasting-inventory.md) for details on reused prediction components and limitations.', '']
    report=ROOT/'docs/premier-league-player-forecasts-2026-10-11.md'
    report.write_text('\n'.join(lines),encoding='utf-8')
    print('Rendered',len(f)+1,'images to',out,'and forecast report to',report)

if __name__=='__main__':main()
