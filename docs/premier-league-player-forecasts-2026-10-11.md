# Premier League player-resolved match predictions, 11–12 October 2026

**As of:** 2026-10-10 · **Engine:** `simulate_second_resolved_dynamic_mental_match` · **Paths per fixture:** 72

## Results

| Fixture | Home win | Draw | Away win | Simulated goals |
|---|---:|---:|---:|---|
| Crystal Palace vs Nott'm Forest | 46% | 18% | 36% | 1.32–1.31 |
| Hull vs Everton | 39% | 31% | 31% | 0.65–0.62 |
| Liverpool vs Man City | 38% | 18% | 44% | 1.43–1.64 |
| Coventry vs Newcastle | 25% | 25% | 50% | 1.17–1.83 |

**Model comparison:** The historical results + FM benchmark is saved as `artifacts/premier-league-2026-10-11/team_benchmark.json` and is not the simulator. Its stronger validation does not establish the accuracy of this player-level event model.

## Crystal Palace vs Nott'm Forest

| Minute | Most likely (goal/red/sub) | P(any goal) | P(home goal) | P(away goal) | P(red card) | P(substitution trigger) | Goals H–A | Shots H–A |
|---|---|---:|---:|---:|---:|---:|---|---|
| 00–15 | goal (49%) | 48.6% | 30.6% | 23.6% | 8.3% | 6.9% | 0.36–0.24 | 2.00–2.40 |
| 15–30 | goal (32%) | 31.9% | 20.8% | 15.3% | 4.2% | 6.9% | 0.24–0.18 | 2.33–1.64 |
| 30–45 | goal (32%) | 31.9% | 16.7% | 19.4% | 9.7% | 1.4% | 0.18–0.24 | 2.01–1.88 |
| 45–60 | substitution (58%) | 23.6% | 15.3% | 9.7% | 9.7% | 58.3% | 0.18–0.12 | 2.19–1.93 |
| 60–75 | substitution (100%) | 38.9% | 13.9% | 26.4% | 2.8% | 100.0% | 0.14–0.31 | 1.90–1.75 |
| 75–90 | substitution (86%) | 34.7% | 18.1% | 19.4% | 9.7% | 86.1% | 0.22–0.22 | 1.85–1.96 |

## Hull vs Everton

| Minute | Most likely (goal/red/sub) | P(any goal) | P(home goal) | P(away goal) | P(red card) | P(substitution trigger) | Goals H–A | Shots H–A |
|---|---|---:|---:|---:|---:|---:|---|---|
| 00–15 | goal (15%) | 15.3% | 12.5% | 2.8% | 8.3% | 4.2% | 0.15–0.03 | 1.99–2.18 |
| 15–30 | goal (12%) | 12.5% | 4.2% | 8.3% | 4.2% | 2.8% | 0.06–0.08 | 1.51–2.36 |
| 30–45 | goal (21%) | 20.8% | 11.1% | 9.7% | 13.9% | 6.9% | 0.11–0.10 | 1.62–1.93 |
| 45–60 | substitution (51%) | 18.1% | 9.7% | 8.3% | 6.9% | 51.4% | 0.11–0.10 | 1.83–2.33 |
| 60–75 | substitution (100%) | 22.2% | 12.5% | 12.5% | 6.9% | 100.0% | 0.14–0.14 | 1.89–2.35 |
| 75–90 | substitution (89%) | 22.2% | 8.3% | 16.7% | 4.2% | 88.9% | 0.08–0.18 | 1.74–2.29 |

## Liverpool vs Man City

| Minute | Most likely (goal/red/sub) | P(any goal) | P(home goal) | P(away goal) | P(red card) | P(substitution trigger) | Goals H–A | Shots H–A |
|---|---|---:|---:|---:|---:|---:|---|---|
| 00–15 | goal (33%) | 33.3% | 19.4% | 16.7% | 8.3% | 2.8% | 0.21–0.17 | 2.40–2.07 |
| 15–30 | goal (33%) | 33.3% | 18.1% | 19.4% | 6.9% | 6.9% | 0.22–0.19 | 2.26–1.96 |
| 30–45 | goal (46%) | 45.8% | 25.0% | 26.4% | 8.3% | 11.1% | 0.29–0.33 | 2.14–1.97 |
| 45–60 | substitution (61%) | 45.8% | 20.8% | 27.8% | 4.2% | 61.1% | 0.22–0.38 | 2.43–2.17 |
| 60–75 | substitution (100%) | 38.9% | 19.4% | 26.4% | 4.2% | 100.0% | 0.21–0.28 | 1.81–2.33 |
| 75–90 | substitution (85%) | 43.1% | 25.0% | 25.0% | 12.5% | 84.7% | 0.28–0.29 | 2.00–1.76 |

## Coventry vs Newcastle

| Minute | Most likely (goal/red/sub) | P(any goal) | P(home goal) | P(away goal) | P(red card) | P(substitution trigger) | Goals H–A | Shots H–A |
|---|---|---:|---:|---:|---:|---:|---|---|
| 00–15 | goal (40%) | 40.3% | 18.1% | 29.2% | 4.2% | 8.3% | 0.19–0.36 | 2.11–1.88 |
| 15–30 | goal (35%) | 34.7% | 18.1% | 23.6% | 9.7% | 2.8% | 0.18–0.26 | 1.88–2.04 |
| 30–45 | goal (47%) | 47.2% | 23.6% | 27.8% | 8.3% | 6.9% | 0.26–0.32 | 1.92–2.00 |
| 45–60 | substitution (51%) | 40.3% | 18.1% | 23.6% | 8.3% | 51.4% | 0.18–0.24 | 1.83–1.86 |
| 60–75 | substitution (100%) | 43.1% | 18.1% | 27.8% | 8.3% | 100.0% | 0.18–0.33 | 1.62–1.83 |
| 75–90 | substitution (88%) | 40.3% | 15.3% | 26.4% | 9.7% | 87.5% | 0.17–0.32 | 1.96–1.85 |

## Interpretation and provenance

The event classified as most likely is the largest marginal among goal, red card and substitution trigger, not an exhaustive ranking of every possible football event. The engine simulates full independent 90-minute player/state paths. All 15-minute event counts come from those exact paths; outcome marginals are direct counts. Goal, red-card and substitution-trigger events are not assumed independent, and rows across a window can overlap. End-of-window score-state probabilities appear in the source JSON.

Player clubs come from the [official 2026–27 registrations](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists), matched to FM26 UIDs and attributes. Known injuries/suspensions reported in [October 9 team news](https://www.theguardian.com/football/2026/oct/09/premier-league-team-news-predicted-lineups-for-the-weekend-action) were excluded; doubtful players remain uncertain. Player event rates/lineup exposure lack contemporaneous PL tracking and are FM/legacy-data priors. The original MLS backtest Brier score was 0.7646 on only 12 fixtures, worse than a uniform forecast. **This EPL application is exploratory and not calibrated to bookmaker-grade probability reliability.**

Each fixture uses a limited number of Monte Carlo paths: at 72 paths, sampling error alone can reach roughly ±12 percentage points at 95% confidence, before model error. More paths do not correct missing historical per-player data.

JSON: [`player_paths.json`](../artifacts/premier-league-2026-10-11/player_paths.json). Read the [code inventory](game-forecasting-inventory.md) for details on reused prediction components and limitations.
