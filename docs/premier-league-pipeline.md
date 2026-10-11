# Premier League player-driven prediction pipeline

## Goal

A reproducible, dated pre-match snapshot that fetches historical match results; refits the independent goals baseline; matches the **current official registered squads** to the **existing FM26 database** (by verified name/UID); applies dated absences; constructs player priors; calls the **existing second-resolved stateful player/psychology engine**; validates the full-match Monte Carlo paths and six 15-minute event windows; and writes machine-readable data, five interactive HTML charts (plus SVG + PNG exports), and a Markdown match report.

This is a fully wired **data-processing and artifact pipeline**, **not** a claim that FM-derived Premier League goal/event rates have been statistically validated or trained on live per-player data. The team-goal benchmark and the player-resolved engine have different inputs and calibration. See [forecast code inventory](game-forecasting-inventory.md).

## One-command interface

From the repo root, with Rust, Python 3, `requests`, `beautifulsoup4`, `kaleido` with Chromium/Chrome, and FM26 men attributes locally available:

```bash
python3 -m pip install -r requirements-pl-pipeline.txt

# Check the entire plan, no file modifications:
python3 scripts/run_pl_pipeline.py --dry-run --offline \
  --fm-file /path/to/fm26_men_full_attrs.csv

# Full dated run: refresh training data, match FM26 squads, simulate matches, draw timelines:
python3 scripts/run_pl_pipeline.py --offline --paths 72 --seed 42 \
  --fm-file /path/to/fm26_men_full_attrs.csv \
  --fixtures configs/pl_2026-10-11.json \
  --availability configs/pl_availability_2026-10-11.json

# Rebuild figures/tables from the *committed* snapshot on a machine with Plotly/Kaleido + Chromium;
# no raw FM database, no network, no refit, no simulation needed:
python3 scripts/run_pl_pipeline.py --render-only
```

If caches are not on disk, omit `--offline`: the underlying scripts download results from football-data.co.uk and the official registered squad page. The user's raw FM CSV and derived player data remain local and untracked. **Use a current dated `--availability` file and fixture config for every future as-of date.** A registered squad does not prove a player is fit or selected. The pipeline refuses to carry forward the October 10 injury list into a later snapshot without an explicit matching date. For a later season, provide a new official `--registration-url`; the 2026–27 registrations cannot be reused silently.

The Rust CLI supports `--paths`, `--seed`, `--as-of`, `--snapshot-as-of`, `--results`, `--players`, `--baseline`, `--output`, and `--only` to reproduce or isolate any specific fixture. The Python runner supports `--snapshot-dir`, `--images-dir`, and `--report` for scratch runs or a new matchweek without touching the pinned artifacts.

## Pipeline stages and saved scripts

| Stage | Script / module | Contract / output |
|---|---|---|
| 1. Result data and regularized reference | `scripts/forecast_premier_league.py` + `apps/plforecast` | Date-causal E0/E1 fit, calibration report, independent baseline probabilities and goal intensities in `data/pl/forecasts.json` |
| 2. Registered player-to-FM IDs | `scripts/match_premier_league_fm26.py` | Official club membership, conservative match confidence, unresolved IDs retained in `data/pl/squads_<as-of>.json`; full FM vectors **never committed** |
| 3. Player inputs | `scripts/prepare_pl_player_inputs.py` | Filters loans and confirmed unavailable players; normalizes historical team data; labels FM-derived per-player shot, xG and exposure **priors** in `data/pl/*.csv` |
| 4. Second-resolved match Monte Carlo | `apps/matchforecast/src/main_pl.rs` (thin binary over the shared `driver.rs` library module), `crates/match/src/second_resolved_mc.rs` | Same full player match engine as existing MLS app. Each path samples lineups, latent player metrics, mental changes, score state, goals, red cards and substitutions. Save fixture/full-match and 15-minute path marginals in `artifacts/premier-league-<first-date>/player_paths.json` |
| 5. Guards | `scripts/validate_pl_snapshot.py` | Full-time probability sum, window continuity and total-goal conservation, event probabilities, identifiable squads |
| 6. Source-faithful timeline images | `scripts/render_pl_timeline.py` | 4 interactive Plotly HTML timeline charts plus PNG/SVG previews, one interactive matchweek overview with PNG/SVG, and `momentum_15min.json` with numeric reproducible pressure indexes |
| 7. Readable report | `scripts/write_pl_timeline_report.py` | One report containing full-time outlook, each 15-minute event and momentum row, and model limitations |
| 8. Orchestration | `scripts/run_pl_pipeline.py` | Executes all stages, logs commands, snapshots minimal source/UID provenance and hashed inputs, or renders saved data only |

`README.md` embeds automatically generated PNG time-series graphs; the matching SVG can be zoomed losslessly and Plotly HTML can be opened locally for tooltips, zooming, and trace toggles. Static images come from the same Plotly figure and JSON trace source. HTML pages reference Plotly.js on its CDN (requires network when opened); committed PNG and SVG previews are self-contained. Legacy summary card generation is retained in `scripts/legacy/render_pl_forecast_cards_2026.py`, but `render_pl_forecast_cards.py` now redirects to the proper time-graph generator.

## What the time series means

**Time resolution:** six non-overlapping regulation-time bins: 0–15, 15–30, 30–45, 45–60, 60–75, 75–90 minutes. Model paths resolve second-by-second internally; **each chart shows the aggregate probability or expectation within each 15-minute bin**, never a fabricated minute of occurrence.

**Events genuinely recorded by the simulated paths:** home goals, away goals, expected home/away shot counts (model player-metric increments), any goal, red-card incidence, substitution/injury-trigger incidence, and end-of-window probabilities of home leading, level, or away leading. Injury and tactical substitution triggers are aggregated into substitution-trigger incidence; a trigger is not necessarily a completed substitution. Their traces are **model projections**, not observed match events.

**Momentum** is not a native trained variable and not real-world possession. It is a transparent attacking-pressure index derived from outputs: `P(team,t) = E[shots(team,t)] + 3 × E[goals(team,t)]`; `M(t) = 100 × (P(home,t) − P(away,t)) / (P(home,t) + P(away,t))` if denominator positive, otherwise zero. Its arbitrary weight 3 is a **visualization choice**, not a backtested or optimized parameter. Positive means more modelled home pressure, negative means more modelled away pressure. The figure displays its exact formula.

**Deliberately absent:** corners, yellow cards, possession, fouls, xA, ball locations, confirmed XI, true minute-by-minute probabilities. Those values are not in the saved player simulator's 15-minute output and MUST NOT be made up. When later tracking data and distinct metrics are added, expand the engine event schema and make these new series optional/explicit.

**Calibration limit:** The original player-model MLS holdout Brier was 0.7646 over 12 fixtures, worse than uniform 0.667. The separate results+FM team baseline performed better on its own league holdout but cannot validate the player-model event trajectories. A 72-path result probability near 50% has approximately ±12 points 95% Monte Carlo standard error, *in addition* to model uncertainty. For serious inference, fix input calibration first, then run far more independent paths.

## Run boundaries and reproducibility

The cutoff is in the fixture JSON (`as_of`, inclusive for results), with fixtures strictly later. The player model gets a simulation start of the **following calendar date** to exclude same-day leakage when fitting team statistics. Every full run is deterministic for a fixed seed, code/CPU environment, inputs and model version; the manifest records SHA-256 for local inputs, dated sources and commit version without publishing restricted data. Do not refit on final fixture outcomes and present the result as a pre-match forecast. The snapshot dated 2026-10-10 was completed before the October 11–12 fixtures.

To build a later matchweek, copy the fixture and availability JSON, change dates/teams and source provenance, supply an updated registration URL/squad file when relevant, then use the same pipeline command. Do not change or overwrite frozen past snapshots: use `--snapshot-dir`, `--images-dir` and `--report` if necessary.
