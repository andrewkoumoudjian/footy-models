# Premier League player-driven temporal forecasts — 2026-10-10

**Status: experimental simulation scenarios, not observed match timelines or validated betting probabilities.**

Source: [second-resolved Monte Carlo snapshot](../artifacts/premier-league-2026-10-11/player_paths.json), rendered with [`render_pl_timeline.py`](../scripts/render_pl_timeline.py).

## Full-time predictions

| Fixture | Monte Carlo paths | Home | Draw | Away | Expected goals |
|---|---:|---:|---:|---:|---|
| Crystal Palace – Nott'm Forest | 72 | 27.8% | 19.4% | 52.8% | 0.88 – 1.38 |
| Hull – Everton | 72 | 44.4% | 34.7% | 20.8% | 0.90 – 0.50 |
| Liverpool – Man City | 72 | 38.9% | 29.2% | 31.9% | 1.35 – 1.44 |
| Coventry – Newcastle | 72 | 26.4% | 23.6% | 50.0% | 1.12 – 1.81 |

## 15-minute event and momentum traces

Each row is an **aggregate across complete match paths** in that clock window, not a prediction of one particular event occurring at a precise minute. P(any goal), P(home goal), P(away goal), P(red card) and P(substitution trigger) are overlapping marginal events; do **not** sum them.

### Crystal Palace vs Nott'm Forest

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | -8.9 | 0.14/0.24 | 1.99/2.17 | 33.3% | 12.5% | 23.6% | 6.9% | 6.9% | 10% / 69% / 21% |
| 15–30 | +0.3 | 0.14/0.24 | 2.25/1.94 | 34.7% | 13.9% | 22.2% | 1.4% | 2.8% | 19% / 44% / 36% |
| 30–45 | -4.9 | 0.14/0.32 | 2.26/2.00 | 36.1% | 13.9% | 27.8% | 5.6% | 8.3% | 21% / 35% / 44% |
| 45–60 | -2.6 | 0.15/0.17 | 1.89/1.97 | 25.0% | 15.3% | 13.9% | 6.9% | 55.6% | 24% / 32% / 44% |
| 60–75 | -4.3 | 0.22/0.24 | 1.97/2.17 | 41.7% | 20.8% | 23.6% | 8.3% | 100.0% | 22% / 29% / 49% |
| 75–90 | -16.1 | 0.08/0.18 | 1.78/2.26 | 22.2% | 6.9% | 15.3% | 5.6% | 83.3% | 28% / 19% / 53% |

### Hull vs Everton

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | +1.1 | 0.14/0.08 | 2.25/2.36 | 20.8% | 13.9% | 8.3% | 5.6% | 6.9% | 12% / 81% / 7% |
| 15–30 | -9.8 | 0.11/0.08 | 2.04/2.64 | 16.7% | 11.1% | 6.9% | 8.3% | 9.7% | 21% / 68% / 11% |
| 30–45 | +10.4 | 0.19/0.10 | 2.44/2.17 | 27.8% | 19.4% | 9.7% | 0.0% | 8.3% | 29% / 57% / 14% |
| 45–60 | +0.3 | 0.11/0.11 | 1.92/1.90 | 20.8% | 9.7% | 11.1% | 8.3% | 52.8% | 31% / 53% / 17% |
| 60–75 | +6.5 | 0.21/0.04 | 1.88/2.07 | 18.1% | 16.7% | 4.2% | 1.4% | 100.0% | 40% / 43% / 17% |
| 75–90 | +3.2 | 0.14/0.08 | 2.06/2.07 | 22.2% | 13.9% | 8.3% | 2.8% | 73.6% | 44% / 35% / 21% |

### Liverpool vs Man City

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | -1.5 | 0.21/0.26 | 2.51/2.44 | 37.5% | 20.8% | 20.8% | 5.6% | 6.9% | 17% / 64% / 19% |
| 15–30 | +6.5 | 0.25/0.15 | 2.57/2.46 | 33.3% | 20.8% | 13.9% | 5.6% | 6.9% | 26% / 56% / 18% |
| 30–45 | -8.2 | 0.15/0.28 | 2.28/2.39 | 38.9% | 12.5% | 26.4% | 6.9% | 6.9% | 31% / 40% / 29% |
| 45–60 | -15.6 | 0.15/0.25 | 2.28/3.00 | 37.5% | 15.3% | 23.6% | 2.8% | 50.0% | 29% / 32% / 39% |
| 60–75 | -9.3 | 0.28/0.31 | 2.14/2.67 | 47.2% | 25.0% | 27.8% | 12.5% | 100.0% | 31% / 35% / 35% |
| 75–90 | +2.8 | 0.31/0.19 | 2.10/2.26 | 40.3% | 26.4% | 15.3% | 8.3% | 88.9% | 39% / 29% / 32% |

### Coventry vs Newcastle

| Minutes | Momentum index | Goals home/away | Shots home/away | Any goal | Home goal | Away goal | Red card | Sub trigger | Home lead / Level / Away lead after window |
|---|---:|---|---|---:|---:|---:|---:|---:|---|
| 00–15 | -1.9 | 0.17/0.15 | 2.01/2.15 | 31.9% | 16.7% | 15.3% | 4.2% | 5.6% | 17% / 68% / 15% |
| 15–30 | -14.9 | 0.21/0.36 | 1.79/2.18 | 44.4% | 19.4% | 27.8% | 6.9% | 11.1% | 28% / 42% / 31% |
| 30–45 | -4.0 | 0.24/0.36 | 2.26/2.14 | 41.7% | 20.8% | 29.2% | 9.7% | 2.8% | 29% / 33% / 38% |
| 45–60 | -13.8 | 0.12/0.24 | 1.93/2.33 | 31.9% | 12.5% | 20.8% | 2.8% | 56.9% | 29% / 29% / 42% |
| 60–75 | -9.8 | 0.17/0.39 | 2.11/2.01 | 38.9% | 12.5% | 33.3% | 5.6% | 100.0% | 29% / 22% / 49% |
| 75–90 | -3.2 | 0.22/0.31 | 2.07/2.00 | 45.8% | 20.8% | 26.4% | 8.3% | 83.3% | 26% / 24% / 50% |

## Method: what the momentum graph actually means

The **attacking-pressure momentum proxy** is a derived descriptive index of modeled shot and goal activity per 15-minute window. It is not a modeled possession share, team pressing tracking metric, ball location, or real football momentum observation.

For team `i` and clock interval `t`: `P(i,t) = E[shots(i,t)] + 3 × E[goals(i,t)]`. Momentum = `100 × (P(home,t) − P(away,t)) / (P(home,t) + P(away,t))`, or zero if both are zero. Positive means more simulated home attacking pressure, negative means more away pressure.

The plots do **not** invent corners, yellow cards, possession, passes, expected assists, fouls, or within-interval exact event timestamps. The simulator reports goals, shots, red cards, substitution triggers, and score-state probabilities. These are the event series plotted.

Squad membership comes from the [official 2026–27 registered lists](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists), matched by FM unique ID or high-confidence name. Confirmed absences in [October 9 team news](https://www.theguardian.com/football/2026/oct/09/premier-league-team-news-predicted-lineups-for-the-weekend-action) are excluded. 2026–27 player event-rate and lineup availability data are incomplete; proxies come from FM26 attributes and limited prior-season inputs.

The existing player-model MLS validation reported Brier 0.7646 on 12 matches, worse than a uniform categorical forecast. With only 72 paths, a single categorical win-probability estimate near 50% has approximate 95% Monte Carlo sampling error of ±12 percentage points, excluding model error. More paths improve precision, not the correctness of assumed event rates.

For the separate, fitted team-goal baseline, see [the historical model report](premier-league-forecast-2026-10-11.md). For the full workflow, see [pipeline documentation](premier-league-pipeline.md).
