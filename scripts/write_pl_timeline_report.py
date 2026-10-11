#!/usr/bin/env python3
"""Human-readable, source-conserving temporal match forecasts and model limitations."""
import argparse
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--source',default='artifacts/premier-league-2026-10-11/player_paths.json')
    ap.add_argument('--derived',default='assets/predictions/2026-10-11/momentum_15min.json')
    ap.add_argument('--output',default='docs/premier-league-player-forecasts-2026-10-11.md')
    o=ap.parse_args()
    src=json.loads((ROOT/o.source).read_text())
    derived=json.loads((ROOT/o.derived).read_text())
    if src['as_of']!=derived['as_of']:raise ValueError('forecast and visualization as-of dates differ')
    fixtures=src['fixtures'];data=derived['fixtures']
    lines=[f"# Premier League player-driven temporal forecasts — {src['as_of']}",'',
        '**Status: experimental simulation scenarios, not observed match timelines or validated betting probabilities.**','',
        f"Source: [second-resolved Monte Carlo snapshot](../{o.source}), rendered with [`render_pl_timeline.py`](../scripts/render_pl_timeline.py).",'',
        '## Full-time predictions','',
        '| Fixture | Monte Carlo paths | Home | Draw | Away | Expected goals |',
        '|---|---:|---:|---:|---:|---|']
    for f in fixtures:
        lines.append(f"| {f['home']} – {f['away']} | {f['paths']} | {f['home_win']:.1%} | {f['draw']:.1%} | {f['away_win']:.1%} | {f['expected_home_goals']:.2f} – {f['expected_away_goals']:.2f} |")
    lines += ['','## 15-minute event and momentum traces','',
        'Each row is an **aggregate across complete match paths** in that clock window, not a prediction of one particular event occurring at a precise minute. P(any goal), P(home goal), P(away goal), P(red card) and P(substitution trigger) are overlapping marginal events; do **not** sum them.','']
    for f in fixtures:
        name=f"{f['home']} vs {f['away']}"
        rows=data[name]
        lines += [f'### {name}', '',
            '| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |',
            '|---|---:|---|---|---:|---:|---:|---:|---:|---|']
        for w in rows:
            lines.append(f"| {w['window_start']:02d}–{w['window_end']:02d} | {w['momentum_proxy']:+.1f} | "
                f"{w['expected_home_goals']:.2f}/{w['expected_away_goals']:.2f} | "
                f"{w['expected_home_shots']:.2f}/{w['expected_away_shots']:.2f} | "
                f"{w['probability_any_goal']:.1%} | {w['probability_home_goal']:.1%} | "
                f"{w['probability_away_goal']:.1%} | {w['probability_red_card']:.1%} | "
                f"{w['probability_substitution_trigger']:.1%} | "
                f"{w['home_lead']:.0%} / {w['draw_state']:.0%} / {w['away_lead']:.0%} |")
        lines.append('')
    lines += ['## Method: what the momentum graph actually means','',
        'The **attacking-pressure momentum proxy** is a derived descriptive index of modeled shot and goal activity per 15-minute window. It is not a modeled possession share, team pressing tracking metric, ball location, or real football momentum observation.',
        '', 'For team `i` and clock interval `t`: `P(i,t) = E[shots(i,t)] + 3 × E[goals(i,t)]`. Momentum = `100 × (P(home,t) − P(away,t)) / (P(home,t) + P(away,t))`, or zero if both are zero. Positive means more simulated home attacking pressure, negative means more away pressure.',
        '', 'The plots do **not** invent corners, yellow cards, possession, passes, expected assists, fouls, or within-interval exact event timestamps. The simulator reports goals, shots, red cards, substitution triggers, and score-state probabilities. These are the event series plotted.',
        '', 'Squad membership comes from the [official 2026–27 registered lists](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists), matched by FM unique ID or high-confidence name. Confirmed absences in [October 9 team news](https://www.theguardian.com/football/2026/oct/09/premier-league-team-news-predicted-lineups-for-the-weekend-action) are excluded. 2026–27 player event-rate and lineup availability data are incomplete; proxies come from FM26 attributes and limited prior-season inputs.',
        '', 'The existing player-model MLS validation reported Brier 0.7646 on 12 matches, worse than a uniform categorical forecast. With only 72 paths, a single categorical win-probability estimate near 50% has approximate 95% Monte Carlo sampling error of ±12 percentage points, excluding model error. More paths improve precision, not the correctness of assumed event rates.',
        '', 'For the separate, fitted team-goal baseline, see [the historical model report](premier-league-forecast-2026-10-11.md). For the full workflow, see [pipeline documentation](premier-league-pipeline.md).', '']
    target=ROOT/o.output
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text('\n'.join(lines),encoding='utf-8')
    print('Wrote',target,len(fixtures),'match timeline tables')

if __name__=='__main__':main()
