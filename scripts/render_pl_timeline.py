#!/usr/bin/env python3
"""Plotly Premier League Monte Carlo timeline charts (interactive HTML + PNG + SVG).

Default Plotly styling on purpose: the figures use the standard template,
colorway and legend conventions so the numbers read cleanly instead of being
hidden under a bespoke theme.

Only quantities provided by the 1-second player simulator enter the figures.
In particular, momentum is an *explicit derived attacking-pressure index* and
not observed possession, win-probability swing, ball movement, or event tracking:
  pressure(team,t) = expected_shots(t) + 3 * expected_goals(t)
  momentum(t) = 100 * (home_pressure - away_pressure) / total_pressure

Outputs per matchweek: four match timelines + a matchweek overview (interactive
HTML, PNG and SVG) plus `momentum_15min.json` with the derived traces used by
`write_pl_timeline_report.py`. PNG/SVG export requires kaleido + Chromium;
`--no-png` writes the HTML only.
"""
from __future__ import annotations
import argparse
import json
import math
from pathlib import Path

import plotly.graph_objects as go
from plotly.subplots import make_subplots

ROOT = Path(__file__).resolve().parents[1]
W = 1400
H = 1440
HOME_COLOR = '#636EFA'   # plotly default colorway[0]
AWAY_COLOR = '#EF553B'   # plotly default colorway[1]


def momentum(window):
    keys = ('expected_home_goals', 'expected_away_goals', 'expected_home_shots', 'expected_away_shots')
    if not all(math.isfinite(float(window[k])) for k in keys):
        raise ValueError('nonfinite attacking-pressure input')
    if any(float(window[k]) < 0 for k in keys):
        raise ValueError('negative goal/shot expectation')
    h = window['expected_home_shots'] + 3 * window['expected_home_goals']
    a = window['expected_away_shots'] + 3 * window['expected_away_goals']
    return 100 * (h - a) / (h + a) if h + a > 1e-12 else 0.0


def derived(f):
    rows = []
    for i, w in enumerate(f['15_minute_windows']):
        if (w['minute_start'], w['minute_end']) != (15 * i, 15 * (i + 1)):
            raise ValueError('noncontiguous 15-minute windows')
        for key in ('p_any_goal', 'p_home_goal', 'p_away_goal', 'p_red_card', 'p_substitution'):
            if not 0.0 <= w[key] <= 1.0:
                raise ValueError(f'probability out of range: {key}')
        if abs(sum(w['end_of_window_outcomes']) - 1.0) > 1e-5:
            raise ValueError('score-state probabilities do not sum to one')
        m = momentum(w)
        rows.append({'window_start': w['minute_start'], 'window_end': w['minute_end'],
                     'momentum_proxy': round(m, 5),
                     'home_pressure': w['expected_home_shots'] + 3 * w['expected_home_goals'],
                     'away_pressure': w['expected_away_shots'] + 3 * w['expected_away_goals'],
                     'expected_home_shots': w['expected_home_shots'], 'expected_away_shots': w['expected_away_shots'],
                     'expected_home_goals': w['expected_home_goals'], 'expected_away_goals': w['expected_away_goals'],
                     'probability_any_goal': w['p_any_goal'], 'probability_home_goal': w['p_home_goal'],
                     'probability_away_goal': w['p_away_goal'], 'probability_red_card': w['p_red_card'],
                     'probability_substitution_trigger': w['p_substitution'],
                     'home_lead': w['end_of_window_outcomes'][0], 'draw_state': w['end_of_window_outcomes'][1],
                     'away_lead': w['end_of_window_outcomes'][2]})
    if len(rows) != 6:
        raise ValueError('six 15-minute simulation windows required')
    ft = [f['home_win'], f['draw'], f['away_win']]
    if any(abs(got - want) > 1e-6 for got, want in
           zip([rows[-1]['home_lead'], rows[-1]['draw_state'], rows[-1]['away_lead']], ft)):
        raise ValueError('final window score-state must equal full-time outcome marginals')
    return rows


def clock_axis(fig, row=None, col=None):
    minutes = (0, 15, 30, 45, 60, 75, 90)
    fig.update_xaxes(range=[-3, 93], tickvals=list(minutes), ticktext=[f'{m}′' for m in minutes],
                     title_text='minute', row=row, col=col)


def timeline_figure(f, as_of):
    data = derived(f)
    labels = [f"{r['window_start']}–{r['window_end']}′" for r in data]
    xs = [15 * i + 7.5 for i in range(6)]
    fig = make_subplots(rows=4, cols=1, shared_xaxes=True, vertical_spacing=0.075,
                        row_heights=[0.22, 0.22, 0.28, 0.28],
                        subplot_titles=('Attacking momentum proxy (derived)',
                                        'Expected shots per 15 min',
                                        'Event probability by clock window',
                                        'Score state after each window'))
    fig.add_trace(go.Bar(x=xs, y=[r['momentum_proxy'] for r in data], name='momentum',
                         customdata=labels,
                         hovertemplate='window %{customdata}<br>momentum %{y:+.1f}<extra></extra>'),
                  row=1, col=1)
    fig.update_yaxes(title_text='index', row=1, col=1)

    fig.add_trace(go.Bar(x=xs, y=[r['expected_home_shots'] for r in data], name=f'{f["home"]} shots',
                         customdata=labels, hovertemplate='%{y:.2f} shots (window %{customdata})<extra></extra>'),
                  row=2, col=1)
    fig.add_trace(go.Bar(x=xs, y=[r['expected_away_shots'] for r in data], name=f'{f["away"]} shots',
                         customdata=labels, hovertemplate='%{y:.2f} shots (window %{customdata})<extra></extra>'),
                  row=2, col=1)
    fig.update_yaxes(title_text='shots', row=2, col=1)

    for label, key in [('any goal', 'probability_any_goal'), (f'{f["home"]} goal', 'probability_home_goal'),
                       (f'{f["away"]} goal', 'probability_away_goal'), ('sub trigger', 'probability_substitution_trigger'),
                       ('red card', 'probability_red_card')]:
        fig.add_trace(go.Scatter(x=xs, y=[r[key] * 100 for r in data], mode='lines+markers', name=label,
                                 customdata=labels,
                                 hovertemplate=f'{label} %{{y:.1f}}% (window %{{customdata}})<extra></extra>'),
                      row=3, col=1)
    fig.update_yaxes(title_text='probability %', range=[0, 100], row=3, col=1)

    ends = [15 * (i + 1) for i in range(6)]
    for label, key in [(f'{f["home"]} lead', 'home_lead'), ('level', 'draw_state'), (f'{f["away"]} lead', 'away_lead')]:
        kickoff = 1.0 if key == 'draw_state' else 0.0
        fig.add_trace(go.Scatter(x=[0] + ends, y=[kickoff * 100] + [r[key] * 100 for r in data],
                                 mode='lines+markers', name=label,
                                 hovertemplate=f'{label} %{{y:.1f}}% after %{{x}}′<extra></extra>'),
                      row=4, col=1)
    fig.update_yaxes(title_text='probability %', range=[0, 100], row=4, col=1)
    clock_axis(fig)

    fig.update_layout(
        title=dict(text=f"{f['home']} vs {f['away']} — 15-minute simulated windows",
                   subtitle=dict(text=f"{f['date']} · pre-match snapshot {as_of} · {f['paths']} second-resolved paths · "
                                      "scenario estimate from registered squads + FM26 player priors; not tracking data")),
        barmode='group', bargap=0.35, hovermode='x unified',
        legend=dict(orientation='h', yanchor='bottom', y=1.02, x=0),
        height=H, width=W, margin=dict(t=110, b=70, l=70, r=40))
    return fig


def overview_figure(fixtures, as_of):
    # subplot_titles fill row-major across both columns: title the left panel only.
    titles = []
    for f in fixtures:
        titles += [f"{f['home']} – {f['away']} ({f['date']})", '']
    mom_max = 5 * max(5, math.ceil(max(abs(r['momentum_proxy'])
                                       for f in fixtures for r in derived(f)) / 5))
    fig = make_subplots(rows=len(fixtures), cols=2, column_widths=[0.30, 0.70],
                        horizontal_spacing=0.07, vertical_spacing=0.09,
                        subplot_titles=titles)
    for i, f in enumerate(fixtures):
        row = i + 1
        data = derived(f)
        fig.add_trace(go.Bar(x=['home', 'draw', 'away'],
                             y=[f['home_win'] * 100, f['draw'] * 100, f['away_win'] * 100],
                             text=[f'{f["home_win"]:.0%}', f'{f["draw"]:.0%}', f'{f["away_win"]:.0%}'],
                             textposition='outside', name=f"{f['home']} vs {f['away']}",
                             showlegend=False, hovertemplate='%{y:.1f}%<extra></extra>'), row=row, col=1)
        fig.update_yaxes(range=[0, 118], title_text='%', row=row, col=1)
        fig.add_trace(go.Bar(x=[15 * k + 7.5 for k in range(6)],
                             y=[r['momentum_proxy'] for r in data], name='momentum',
                             showlegend=False,
                             customdata=[f"{r['window_start']}–{r['window_end']}′" for r in data],
                             hovertemplate='window %{customdata}<br>momentum %{y:+.1f}<extra></extra>'),
                      row=row, col=2)
        fig.update_yaxes(title_text='momentum', range=[-mom_max, mom_max], row=row, col=2)
        clock_axis(fig, row=row, col=2)
    fig.update_layout(
        title=dict(text='Premier League — simulated momentum & match outcomes',
                   subtitle=dict(text=f'As of {as_of} · default-styled Plotly overview of the four match timelines · '
                                      'momentum is derived from simulated shots/goals, not observed possession')),
        barmode='group', height=980, width=W, margin=dict(t=130, b=60, l=70, r=40),
        legend=dict(orientation='h', yanchor='bottom', y=1.02, x=0))
    return fig


def export(fig, stem, png=True):
    stem.parent.mkdir(parents=True, exist_ok=True)
    fig.write_html(str(stem.with_suffix('.html')), include_plotlyjs='cdn',
                   full_html=True, config={'displaylogo': False})
    if png:
        try:
            fig.write_image(str(stem.with_suffix('.png')), format='png', scale=1)
            fig.write_image(str(stem.with_suffix('.svg')), format='svg')
        except Exception as exc:  # kaleido/chromium missing
            raise RuntimeError(f'PNG/SVG conversion requires kaleido + chromium: {exc}') from exc


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', default='artifacts/premier-league-2026-10-11/player_paths.json')
    parser.add_argument('--out-dir', default='assets/predictions/2026-10-11')
    parser.add_argument('--no-png', action='store_true', help='write interactive HTML only')
    args = parser.parse_args()
    data = json.loads((ROOT / args.source).read_text())
    fixtures = data['fixtures']
    out = ROOT / args.out_dir
    if not fixtures:
        raise ValueError('no matches')
    export(overview_figure(fixtures, data['as_of']), out / 'overview', not args.no_png)
    all_derived = {}
    for i, f in enumerate(fixtures):
        slug = f'{i + 1:02d}-' + f['home'].lower().replace(' ', '-').replace("'", '')
        export(timeline_figure(f, data['as_of']), out / slug, not args.no_png)
        all_derived[f"{f['home']} vs {f['away']}"] = derived(f)
    (out / 'momentum_15min.json').write_text(json.dumps({
        'snapshot': args.source, 'as_of': data['as_of'], 'paths_per_match': [f['paths'] for f in fixtures],
        'momentum_formula': '100 * ((expected_home_shots + 3 * expected_home_goals) - (expected_away_shots + 3 * expected_away_goals)) / combined_shot_goal_pressure',
        'momentum_units': 'signed percentage-like pressure index bounded -100 to 100; this is not observed possession',
        'events_included': ['goals', 'shots', 'red cards', 'substitution triggers', 'score-state'],
        'not_modelled': ['yellow cards', 'corners', 'possession', 'fouls', 'exact per-event timestamps'],
        'fixtures': all_derived}, indent=2, ensure_ascii=False) + '\n')
    print(f'Generated {len(fixtures)} Plotly time graphs + matchweek overview (HTML' + ('' if args.no_png else ' + PNG + SVG') + f'), {out}')


if __name__ == '__main__':
    main()
