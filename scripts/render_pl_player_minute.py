#!/usr/bin/env python3
"""Render actual simulator ONE-MINUTE marginal events and conditional player credit.

Requires minute fields emitted by the second-resolved Rust MC after Oct-2026
upgrade; fails rather than resampling fabricated 1-minute values from 15-minute
windows. Scorers are assigned conditional on match goals using player xG/shot
weights. Assists are provisional: P(assist|goal)=0.68, conditional assistant
weighted by xA; this is NOT empirically fitted/validated yet.
"""
import argparse,csv,json,math
from pathlib import Path
from collections import defaultdict
import plotly.graph_objects as go
from plotly.subplots import make_subplots
ROOT=Path(__file__).resolve().parents[1]

def reconstruct(f):
    minutes=f.get('minutes_1min')
    events=f.get('player_minute_events')
    if minutes is None or events is None or len(minutes)!=90:
        raise ValueError('one-minute player events absent; rerun updated Rust simulator, do not interpolate')
    for j,w in enumerate(f['15_minute_windows']):
        for k in ('expected_home_goals','expected_away_goals','expected_home_shots','expected_away_shots'):
            if abs(sum(t[k] for t in minutes[15*j:15*(j+1)])-w[k])>1e-6:
                raise ValueError(f'1-minute/15-minute reconciliation failed: {k} window {j}')
    player_stats=defaultdict(lambda:{'goals':0.,'assists_proxy':0.,'shots':0.,'name':'','team':''})
    for e in events:
        if not 0<=e['minute']<90:raise ValueError('invalid minute')
        p=player_stats[str(e['uid'])]
        p['name'],p['team']=e['name'],e['team']
        p['goals']+=e['expected_goals']
        p['assists_proxy']+=e['expected_assists_proxy']
        p['shots']+=e['expected_shots']
    total=sum(p['goals'] for p in player_stats.values())
    if abs(total+f['unattributed_goals_per_match']-f['expected_home_goals']-f['expected_away_goals'])>1e-6:
        raise ValueError('scorer and team goals mismatch')
    if sum(p['assists_proxy'] for p in player_stats.values())>total+1e-6:
        raise ValueError('assists exceed goals')
    return minutes,player_stats

def figure(f):
    minutes,players=reconstruct(f)
    fig=make_subplots(rows=4,cols=1,shared_xaxes=True,vertical_spacing=.085,
                      subplot_titles=('Goal-event probability (per minute)','Expected shots per minute',
                                      'Expected scored goals by player (full match)',
                                      'Provisional assist attribution by player (full match)'),
                      row_heights=[.23,.23,.27,.27])
    x=[m+.5 for m in range(90)]
    for key,label,col in [('p_home_goal',f['home'],'#249fe6'),('p_away_goal',f['away'],'#df8a36'),
                          ('p_any_goal','any goal','#16a76a'),('p_red_card','red card','#b14d5c'),
                          ('p_substitution','sub trigger','#888888')]:
        fig.add_trace(go.Scatter(x=x,y=[100*m[key] for m in minutes],mode='lines',name=label+' P',
                       line=dict(color=col,width=2),hovertemplate='%{x:.0f} min · %{y:.1f}%<extra>'+label+'</extra>'),row=1,col=1)
    for key,label,col in [('expected_home_shots',f['home'],'#249fe6'),('expected_away_shots',f['away'],'#df8a36')]:
        fig.add_trace(go.Scatter(x=x,y=[m[key] for m in minutes],mode='lines',name=label+' shots',
                     line=dict(color=col,width=2),hovertemplate='%{x:.0f} min · %{y:.3f} shots<extra>'+label+'</extra>'),row=2,col=1)
    for row,metric,color in ((3,'goals','#249fe6'),(4,'assists_proxy','#df8a36')):
        ranked=sorted([(p[metric],p['name'],uid) for uid,p in players.items()],reverse=True)[:12]
        if ranked:
            fig.add_trace(go.Bar(x=[n for v,n,uid in ranked],y=[v for v,n,uid in ranked],
                         marker_color=color,name=('goals' if row==3 else 'assists proxy'),
                         hovertemplate='%{x}<br>%{y:.3f} per match<extra></extra>'),row=row,col=1)
    fig.update_xaxes(range=[0,90],tickvals=list(range(0,91,5)),title_text='match minute',row=2,col=1)
    fig.update_yaxes(title_text='probability %',row=1,col=1)
    fig.update_yaxes(title_text='expected shots',row=2,col=1)
    fig.update_yaxes(title_text='expected goals',row=3,col=1)
    fig.update_yaxes(title_text='expected assisted goals',row=4,col=1)
    fig.update_layout(height=1350,width=1450,
        title=f"{f['home']} vs {f['away']} · {f['paths']} paths · true 1-minute bins (exploratory)",
        template='plotly_white',legend=dict(orientation='h',y=1.045,x=0),
        margin=dict(l=70,r=50,b=100,t=135))
    return fig,players

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--source',required=True)
    ap.add_argument('--out-dir',default='assets/predictions/2026-10-11/minute-level')
    ap.add_argument('--png',action='store_true',help='also export static PNG (Kaleido+Chrome)')
    opt=ap.parse_args();src=ROOT/opt.source
    data=json.loads(src.read_text());out=ROOT/opt.out_dir;out.mkdir(parents=True,exist_ok=True)
    manifest=[]
    for i,f in enumerate(data['fixtures']):
        fig,players=figure(f)
        slug=f"{i+1:02d}-"+f['home'].lower().replace(' ','-').replace("'",'')
        fig.write_html(out/(slug+'.html'),include_plotlyjs='cdn',config={'displaylogo':False})
        if opt.png:fig.write_image(out/(slug+'.png'))
        with (out/(slug+'-players.csv')).open('w',newline='') as stream:
            cols=['uid','name','team','expected_goals','expected_assists_proxy','expected_shots']
            writer=csv.DictWriter(stream,fieldnames=cols,lineterminator='\n');writer.writeheader()
            for uid,p in sorted(players.items(),key=lambda x:(-x[1]['goals'],-x[1]['assists_proxy'])):
                writer.writerow({'uid':uid,'name':p['name'],'team':p['team'],
                                 'expected_goals':round(p['goals'],6),
                                 'expected_assists_proxy':round(p['assists_proxy'],6),
                                 'expected_shots':round(p['shots'],6)})
        manifest.append({'fixture':f["home"]+' vs '+f['away'],'paths':f['paths'],
                         'expected_goals':f['expected_home_goals']+f['expected_away_goals'],
                         'unattributed_goals':f['unattributed_goals_per_match']})
    (out/'manifest.json').write_text(json.dumps({'source':opt.source,'status':'provisional scorer and uncalibrated assists; do not interpret as OOS-validated','fixtures':manifest},indent=2)+'\n')
    print('Rendered one-minute player reports for',len(manifest),'fixtures to',out)

if __name__=='__main__':main()
