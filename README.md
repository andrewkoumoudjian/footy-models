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

**Player-level validation update:** The engine now has a separate 90-bin-per-match output for player-attributed shots and goals, provisional assister allocations, and minute events. See the [real historical held-out event audit](docs/premier-league-player-event-oos-validation.md). On 24 actual Premier League 2015–16 event matches, historical player/team event-rate baselines **failed to establish improvement** for goal and assist timing; this does **not** validate the 2026 FM26 engine. The original 72-path October snapshot does not contain native per-minute player events: those require an explicit rerun, never guessed interpolation.

### Native one-minute player forecasts — diagnostic only

The upgraded engine now produces **90 actual simulation-minute windows**, and per-player shot and scorer totals. The first four-fixture integration run below uses **only eight independent paths per match** to test instrumentation. **It is not a statistically meaningful forecast and must not replace the frozen 72-path results above.** Assist values are provisional conditional assignments using an unvalidated assisted-goal prior, **not measured assists or expected assists fitted from Premier League data**.

![Eight-path Liverpool vs Manchester City player goals and provisional assists with native 1-minute goal/shot timelines](assets/validation/pl-minute-sample-8paths/03-liverpool.png)

[Palace–Forest diagnostic](assets/validation/pl-minute-sample-8paths/01-crystal-palace.html) · [Hull–Everton diagnostic](assets/validation/pl-minute-sample-8paths/02-hull.html) · [Liverpool–City diagnostic](assets/validation/pl-minute-sample-8paths/03-liverpool.html) · [Coventry–Newcastle diagnostic](assets/validation/pl-minute-sample-8paths/04-coventry.html). Each includes a matching player-goals/shots/assist-credit CSV generated by [`render_pl_player_minute.py`](scripts/render_pl_player_minute.py).

### Held-out real Premier League validation

![Observed goals and registered assists versus pre-match event-rate forecasts in 24 Premier League historical out-of-sample matches](assets/validation/pl-2015-16-event-oos.png)

The [24-match historical player-event OOS audit](docs/premier-league-player-event-oos-validation.md) compares genuinely observed goals, assists and minute windows to a **separately trained, strictly chronological historical-rate baseline**. Goal and assist timing forecasts do **not** outperform simple previous-only references. This **does not yet validate the current FM26 player simulator**. It shows exactly what still needs calibration and preserves the evaluation code for an as-of historical model backtest.

### Same-engine historical backtest (observed-stat-only contract)

The [same-engine walk-forward backtest](docs/historical-same-engine-backtest.md) runs the **actual Rust second-resolved engine** on the cached 2015/16 StatsBomb subset with a strictly causal observed-stat-only input contract (no FM26 attributes, no current squads, no future information), scores held-out fixtures against causal Poisson and previous-only player/timing baselines with matchday-block bootstrap intervals, and records the engine fingerprint in [`artifacts/validation/pl-hist-same-engine-oos.json`](artifacts/validation/pl-hist-same-engine-oos.json). This is the first genuinely same-engine OOS evidence in the repository; it validates (or fails to validate) the **engine on historical inputs**, and explicitly does **not** validate the FM26-enhanced 2026 pipeline.

On 25 held-out fixtures (2016-02-13 → 2016-05-17): player **goal occurrence**, player **shot volume**, **one-minute goal occurrence** and substitution incidence beat their causal references with 95% bootstrap intervals below zero. Match **W/D/L**, **exact score** and **team-goal** distributions do **not** beat the causal regularized-Poisson baseline (exact score is clearly worse), and the lineup sampler's **start/appearance** marginals are clearly worse than its own input prior. The 25-fixture, two-club-dominated sample is far below the 200-fixture release threshold, so every target remains `research_only` regardless of outcome. Two frozen calibration experiments are recorded in the same document: a global fit-period goal/home correction was **rejected** (W/D/L and 15-minute losses significantly worse OOS; the fit-period overprediction did not persist), while a fit-period selection-marginal correction reduced the lineup-sampler compression and improved player minutes MAE, and is retained as an explicit, non-default research flag.

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

## Forecast targets and accuracy gates

The [versioned target contract](configs/forecast_targets_v1.json) and [forecast-design plan](docs/forecast-targets.md) define **what** we should predict and **how** to evaluate it: match W/D/L and joint goals first, then player participation, player shots/goals/primary assists, then calibrated 15-minute events and score-state paths. Native one-minute data are kept for development, but must earn their own historical event-timing calibration. Momentum is an explicitly derived display proxy, not a separately calibrated football statistic.

The [machine-readable release audit](artifacts/validation/forecast-release-gates-2026-10-11.json) prevents claiming these 72-path experimental outputs are calibrated. `python3 scripts/check_forecast_release.py` produces it; future runs call the same gate automatically. The [sealed historical primary-assist prior experiment](artifacts/validation/primary-assist-prior-2015-16-oos.json) evaluated a 0.61 candidate against the existing 0.68 default; it **failed** the OOS improvement requirement, so the simulator's 0.68 remains an unvalidated, explicit configurable prior, not an approved recalibration. No results from an unrelated historical model are allowed to validate the FM26 simulator.

## Code map

- [`crates/match`](crates/match/src/lib.rs): source-neutral player match graph, lineup sampling, FM player attributes, dynamic mental state, second-resolved Monte Carlo and outcome calibration.
- [`apps/matchforecast`](apps/matchforecast): production player match driver, MLS adapter, Premier League adapter and walk-forward backtest.
- [`apps/plforecast`](apps/plforecast): refitted team-goals baseline for external scoring/comparison; this is not the player-event simulation.
- [`crates/forecast`](crates/forecast/src/lib.rs): stochastic primitives, player participation, future-state and transfer-market forecasting.

Other apps and crates cover ingestion, performance, scouting/recruitment, financial valuation and simulation. They are separate from the match-result models described above.
