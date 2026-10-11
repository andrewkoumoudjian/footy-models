#!/usr/bin/env python3
"""Visualize *actual historical held-out* event-minute counts vs causal forecast.

This is an OOS baseline, not the contemporary FM26 model. Deliberately uses
model predictions already stored before reading observed outcomes in score JSON.
"""
import argparse,json
from pathlib import Path
import plotly.graph_objects as go
from plotly.subplots import make_subplots
ROOT=Path(__file__).resolve().parents[1]

def main():
    a=argparse.ArgumentParser(description=__doc__)
    a.add_argument('--source',default='artifacts/validation/premier-league-events-2015-16-oos.json')
    a.add_argument('--output',default='assets/validation/pl-2015-16-event-oos')
    a=a.parse_args();data=json.loads((ROOT/a.source).read_text());games=data['games']
    def agg(metric):
        if any(metric not in g for g in games):raise ValueError('rebuild OOS snapshot with updated minute arrays')
        return [sum(g[metric][i] for g in games) for i in range(90)]
    fig=make_subplots(rows=2,cols=1,shared_xaxes=True,vertical_spacing=.16,
        subplot_titles=('Goals · historical held-out real events vs pre-match expectation',
                        'Registered assists · held-out real events vs pre-match expectation'))
    for row,metrics in ((1,('actual_goal_by_minute','expected_goal_by_minute')),
                        (2,('actual_assist_by_minute','expected_assist_by_minute'))):
        truth,expected=[agg(k) for k in metrics]
        # Discrete window totals: no smooth temporal interpolation of observed events.
        intervals=[(i*15,(i+1)*15) for i in range(6)]
        x=[7.5+i*15 for i in range(6)]
        yy=[sum(truth[i:j]) for i,j in intervals]
        pp=[sum(expected[i:j]) for i,j in intervals]
        fig.add_trace(go.Bar(x=x,y=yy,width=6,name=('Goals' if row==1 else 'Assists')+' actually observed',
                  marker_color='#777777',hovertemplate='real events %{y:.0f}<extra></extra>'),row=row,col=1)
        fig.add_trace(go.Bar(x=x,y=pp,width=6,name=('Goals' if row==1 else 'Assists')+' OOS expected',
                  marker_color='#3779bb',hovertemplate='OOS expectation %{y:.2f}<extra></extra>'),row=row,col=1)
    fig.update_xaxes(tickvals=[0,15,30,45,60,75,90],range=[0,90],title_text='match minute (regulation 15-minute bins)',row=2,col=1)
    fig.update_yaxes(title_text='events across '+str(len(games))+' matches')
    fig.update_layout(template='plotly_white',height=850,width=1340,barmode='group',
       title=f"ACTUAL vs PRE-MATCH event rates · {len(games)} held-out PL 2015/16 fixtures · NOT FM26 validation",
       legend=dict(orientation='h',y=1.055,x=0))
    target=ROOT/a.output;target.parent.mkdir(parents=True,exist_ok=True)
    fig.write_html(target.with_suffix('.html'),include_plotlyjs='cdn')
    fig.write_image(target.with_suffix('.png'))
    fig.write_image(target.with_suffix('.svg'))
    print('Rendered actual-vs-forecast OOS graph',target)

if __name__=='__main__':main()
