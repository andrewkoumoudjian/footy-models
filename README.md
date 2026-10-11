# Footy Models

Player-focused football modeling in Rust: FM26 attributes, historical player/team performance, lineup uncertainty, contextual interactions, a state-dependent one-second match simulator, and reproducible match forecasts.

## Premier League — 11–12 October 2026

**Player-driven match timelines: 15-minute attacking momentum, expected shots, event probabilities, and evolving score state.**

![Premier League forecasts: player-driven full-match Monte Carlo overview](assets/predictions/2026-10-11/overview.png)

The simulation excludes players explicitly reported injured or suspended in the [October 9 team-news snapshot](https://www.theguardian.com/football/2026/oct/09/premier-league-team-news-predicted-lineups-for-the-weekend-action); doubtful players remain in the stochastic lineup pool.

After matching 391 FM26 records across the eight official registered squads, the simulated eligible pools retain **323 players** once loaned-out registrations and confirmed injuries/suspensions are excluded. This snapshot uses the existing **[`simulate_second_resolved_dynamic_mental_match`](crates/match/src/second_resolved_api.rs)** engine with FM26 players matched to [2026–27 registered Premier League squads](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists). Each fixture simulates a full 90-minute game with six observed-in-simulation 15-minute event windows, randomized lineups, goals, substitutions, red cards and changing player mental state.

The charts show **four aligned time-series panels** for each simulated 90-minute match: (1) attacking momentum derived from modeled shots and goals (positive bars = home pressure, negative bars = away pressure), (2) projected shot volume per 15 minutes, (3) probability of modeled goals, red cards and substitution triggers in each interval, and (4) the chance of each team leading or a level score at the end of each interval. **They are simulated scenarios, not live match-event data.** Open the SVG for lossless zoom, or the matching Plotly HTML locally for event-probability tooltips and trace toggles (the HTML loads Plotly.js from a CDN).

### Match timelines

**Crystal Palace vs Nottingham Forest**

![Crystal Palace vs Nottingham Forest: second-resolved Monte Carlo match timeline](assets/predictions/2026-10-11/01-crystal-palace.png)

[Vector SVG](assets/predictions/2026-10-11/01-crystal-palace.svg) · [Interactive HTML](assets/predictions/2026-10-11/01-crystal-palace.html)

**Hull vs Everton**

![Hull vs Everton: second-resolved Monte Carlo match timeline](assets/predictions/2026-10-11/02-hull.png)

[Vector SVG](assets/predictions/2026-10-11/02-hull.svg) · [Interactive HTML](assets/predictions/2026-10-11/02-hull.html)

**Liverpool vs Manchester City**

![Liverpool vs Manchester City: second-resolved Monte Carlo match timeline](assets/predictions/2026-10-11/03-liverpool.png)

[Vector SVG](assets/predictions/2026-10-11/03-liverpool.svg) · [Interactive HTML](assets/predictions/2026-10-11/03-liverpool.html)

**Coventry vs Newcastle**

![Coventry vs Newcastle: second-resolved Monte Carlo match timeline](assets/predictions/2026-10-11/04-coventry.png)

[Vector SVG](assets/predictions/2026-10-11/04-coventry.svg) · [Interactive HTML](assets/predictions/2026-10-11/04-coventry.html)

The snapshots, including every 15-minute probability, team/player forecast and provenance details, are stored in [`artifacts/premier-league-2026-10-11`](artifacts/premier-league-2026-10-11). A [readable window-by-window table](docs/premier-league-player-forecasts-2026-10-11.md) lists each event probability. The [independent, historically fitted results + FM benchmark](docs/premier-league-forecast-2026-10-11.md) is preserved separately rather than passed off as player-resolved simulation.

**Interpretation:** The player-simulation results are **experimental scenario estimates**, not independently validated Premier League betting probabilities. The registered squad source is not a starting XI: some players' 2026–27 minutes, injuries and suspensions are unavailable. FM26 attributes and prior-season exposure supply labeled uncertain proxies for player event rates and selection. The first player-resolved engine has weak historical MLS holdout calibration (see [`BACKTEST_RESULTS_MLS.md`](BACKTEST_RESULTS_MLS.md)); with 72 paths, the binomial Monte Carlo standard error can reach 5.9 percentage points (approximately ±12 points at 95% confidence) even before model uncertainty. More Monte Carlo paths improve random sampling precision, not predictive validity. The reported 15-minute goal/red-card/substitution probabilities are event summaries from the same stateful simulated paths, not independent post-hoc Poisson draws.

### Reproduce — one command

The full pipeline is saved in [`scripts/run_pl_pipeline.py`](scripts/run_pl_pipeline.py). It runs match-data refitting, official squad → FM26 UID matching, injury and loan exclusions, the **original stateful second-resolved engine**, validation, timelines, and a source-audited report:

```bash
# Recreate PNG/SVG match timelines from the committed simulation JSON;
# no raw source datasets, FM database, refit, or Monte Carlo computation needed:
python3 scripts/run_pl_pipeline.py --render-only

# Regenerate the entire matchweek from local historical CSVs and FM26 input:
python3 scripts/run_pl_pipeline.py --offline --paths 72 --seed 42 \
  --fixtures configs/pl_2026-10-11.json \
  --availability configs/pl_availability_2026-10-11.json \
  --fm-file /path/to/fm26_men_full_attrs.csv

# See exactly what commands the full pipeline would run without changing files:
python3 scripts/run_pl_pipeline.py --dry-run --offline \
  --fm-file /path/to/fm26_men_full_attrs.csv
```

[Full data pipeline documentation](docs/premier-league-pipeline.md) · [Machine-readable 15-minute event and momentum data](assets/predictions/2026-10-11/momentum_15min.json) · [All forecast script inventory](docs/game-forecasting-inventory.md). Python's `requests` and `beautifulsoup4` are used for roster retrieval; `plotly` creates interactive HTML timelines, and `kaleido` with Chromium exports matching PNG/SVG static figures. Raw licensed player data and fetched source datasets stay out of Git.

## Code map

- [`crates/match`](crates/match/src/lib.rs): source-neutral player match graph, lineup sampling, FM player attributes, dynamic mental state, second-resolved Monte Carlo and outcome calibration.
- [`apps/matchforecast`](apps/matchforecast): production player match driver, MLS adapter, Premier League adapter and walk-forward backtest.
- [`apps/plforecast`](apps/plforecast): refitted team-goals baseline for external scoring/comparison; this is not the player-event simulation.
- [`crates/forecast`](crates/forecast/src/lib.rs): stochastic primitives, player participation, future-state and transfer-market forecasting.

Other apps and crates cover ingestion, performance, scouting/recruitment, financial valuation and simulation. They are separate from the match-result models described above.
