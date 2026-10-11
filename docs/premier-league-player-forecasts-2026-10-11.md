# Premier League player-driven temporal forecasts — 2026-10-10

**Status: experimental simulation scenarios, not observed match timelines or validated betting probabilities.**

Source: [second-resolved Monte Carlo snapshot](../artifacts/premier-league-2026-10-11/player_paths.json), rendered with [`render_pl_timeline.py`](../scripts/render_pl_timeline.py).

## Full-time predictions

| Fixture | Monte Carlo paths | Home | Draw | Away | Expected goals |
|---|---:|---:|---:|---:|---|
| Crystal Palace – Nott'm Forest | 72 | 45.8% | 18.1% | 36.1% | 1.32 – 1.31 |
| Hull – Everton | 72 | 38.9% | 30.6% | 30.6% | 0.65 – 0.62 |
| Liverpool – Man City | 72 | 37.5% | 18.1% | 44.4% | 1.43 – 1.64 |
| Coventry – Newcastle | 72 | 25.0% | 25.0% | 50.0% | 1.17 – 1.83 |

## 15-minute event and momentum traces

Each row is an **aggregate across complete match paths** in that clock window, not a prediction of one particular event occurring at a precise minute. P(any goal), P(home goal), P(away goal), P(red card) and P(substitution trigger) are overlapping marginal events; do **not** sum them.

### Crystal Palace vs Nott'm Forest

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | -0.4 | 0.36/0.24 | 2.00/2.40 | 48.6% | 30.6% | 23.6% | 8.3% | 6.9% | 28% / 54% / 18% |
| 15–30 | +16.5 | 0.24/0.18 | 2.33/1.64 | 31.9% | 20.8% | 15.3% | 4.2% | 6.9% | 40% / 36% / 24% |
| 30–45 | -0.5 | 0.18/0.24 | 2.01/1.88 | 31.9% | 16.7% | 19.4% | 9.7% | 1.4% | 40% / 32% / 28% |
| 45–60 | +8.5 | 0.18/0.12 | 2.19/1.93 | 23.6% | 15.3% | 9.7% | 9.7% | 58.3% | 39% / 29% / 32% |
| 60–75 | -7.0 | 0.14/0.31 | 1.90/1.75 | 38.9% | 13.9% | 26.4% | 2.8% | 100.0% | 39% / 22% / 39% |
| 75–90 | -2.2 | 0.22/0.22 | 1.85/1.96 | 34.7% | 18.1% | 19.4% | 9.7% | 86.1% | 46% / 18% / 36% |

### Hull vs Everton

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | +3.8 | 0.15/0.03 | 1.99/2.18 | 15.3% | 12.5% | 2.8% | 8.3% | 4.2% | 12% / 85% / 3% |
| 15–30 | -21.7 | 0.06/0.08 | 1.51/2.36 | 12.5% | 4.2% | 8.3% | 4.2% | 2.8% | 15% / 74% / 11% |
| 30–45 | -6.3 | 0.11/0.10 | 1.62/1.93 | 20.8% | 11.1% | 9.7% | 13.9% | 6.9% | 26% / 56% / 18% |
| 45–60 | -9.6 | 0.11/0.10 | 1.83/2.33 | 18.1% | 9.7% | 8.3% | 6.9% | 51.4% | 31% / 47% / 22% |
| 60–75 | -9.0 | 0.14/0.14 | 1.89/2.35 | 22.2% | 12.5% | 12.5% | 6.9% | 100.0% | 39% / 36% / 25% |
| 75–90 | -17.6 | 0.08/0.18 | 1.74/2.29 | 22.2% | 8.3% | 16.7% | 4.2% | 88.9% | 39% / 31% / 31% |

### Liverpool vs Man City

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | +8.2 | 0.21/0.17 | 2.40/2.07 | 33.3% | 19.4% | 16.7% | 8.3% | 2.8% | 17% / 69% / 14% |
| 15–30 | +7.1 | 0.22/0.19 | 2.26/1.96 | 33.3% | 18.1% | 19.4% | 6.9% | 6.9% | 26% / 50% / 24% |
| 30–45 | +0.7 | 0.29/0.33 | 2.14/1.97 | 45.8% | 25.0% | 26.4% | 8.3% | 11.1% | 33% / 33% / 33% |
| 45–60 | -3.0 | 0.22/0.38 | 2.43/2.17 | 45.8% | 20.8% | 27.8% | 4.2% | 61.1% | 36% / 24% / 40% |
| 60–75 | -13.2 | 0.21/0.28 | 1.81/2.33 | 38.9% | 19.4% | 26.4% | 4.2% | 100.0% | 33% / 22% / 44% |
| 75–90 | +3.6 | 0.28/0.29 | 2.00/1.76 | 43.1% | 25.0% | 25.0% | 12.5% | 84.7% | 38% / 18% / 44% |

### Coventry vs Newcastle

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | -4.7 | 0.19/0.36 | 2.11/1.88 | 40.3% | 18.1% | 29.2% | 4.2% | 8.3% | 11% / 62% / 26% |
| 15–30 | -7.9 | 0.18/0.26 | 1.88/2.04 | 34.7% | 18.1% | 23.6% | 9.7% | 2.8% | 14% / 49% / 38% |
| 30–45 | -4.4 | 0.26/0.32 | 1.92/2.00 | 47.2% | 23.6% | 27.8% | 8.3% | 6.9% | 24% / 31% / 46% |
| 45–60 | -3.9 | 0.18/0.24 | 1.83/1.86 | 40.3% | 18.1% | 23.6% | 8.3% | 51.4% | 24% / 31% / 46% |
| 60–75 | -13.3 | 0.18/0.33 | 1.62/1.83 | 43.1% | 18.1% | 27.8% | 8.3% | 100.0% | 25% / 22% / 53% |
| 75–90 | -6.6 | 0.17/0.32 | 1.96/1.85 | 40.3% | 15.3% | 26.4% | 9.7% | 87.5% | 25% / 25% / 50% |

## Method: what the momentum graph actually means

The **attacking-pressure momentum proxy** is a derived descriptive index of modeled shot and goal activity per 15-minute window. It is not a modeled possession share, team pressing tracking metric, ball location, or real football momentum observation.

For team `i` and clock interval `t`: `P(i,t) = E[shots(i,t)] + 3 × E[goals(i,t)]`. Momentum = `100 × (P(home,t) − P(away,t)) / (P(home,t) + P(away,t))`, or zero if both are zero. Positive means more simulated home attacking pressure, negative means more away pressure.

The plots do **not** invent corners, yellow cards, possession, passes, expected assists, fouls, or within-interval exact event timestamps. The simulator reports goals, shots, red cards, substitution triggers, and score-state probabilities. These are the event series plotted.

Squad membership comes from the [official 2026–27 registered lists](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists), matched by FM unique ID or high-confidence name. Confirmed absences in [October 9 team news](https://www.theguardian.com/football/2026/oct/09/premier-league-team-news-predicted-lineups-for-the-weekend-action) are excluded. 2026–27 player event-rate and lineup availability data are incomplete; proxies come from FM26 attributes and limited prior-season inputs.

The existing player-model MLS validation reported Brier 0.7646 on 12 matches, worse than a uniform categorical forecast. With only 72 paths, a single categorical win-probability estimate near 50% has approximate 95% Monte Carlo sampling error of ±12 percentage points, excluding model error. More paths improve precision, not the correctness of assumed event rates.

For the separate, fitted team-goal baseline, see [the historical model report](premier-league-forecast-2026-10-11.md). For the full workflow, see [pipeline documentation](premier-league-pipeline.md).
