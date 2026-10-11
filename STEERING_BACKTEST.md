# STEERING — Backtest & Optimize the Footy Models (with a REAL forward test)

## Mission

Iteratively backtest and optimize the footy-models pipeline (value-surge classifier + 12-month value forecast) using the repo's existing Rust primitives. The centerpiece is a **REAL forward test**: the FM26 database snapshot is from the start of FM26 (~Nov 2025), and the Transfermarkt overlay now has realized valuations through **2026-06-12**. That means ~7 months of REAL market data exist after the model's as-of — the models' 12-month predictions can be scored against what actually happened, not just against walk-forward history.

Read `architecture.md` and `AGENTS.md` before touching crates.

## Data inventory (paths are exact — use these, do not rebuild)

- `/home/drewclaw/footy-models/data/fm_tm_joined.csv` — 14,783 players, FM26 attrs joined to TM valuations (CORE asset; **MERGING IS DONE — DO NOT RE-MERGE**)
- `/home/drewclaw/footy-models/data/forecast/train.tsv` — 340,870 rows, 12-month log-ratio target, causal windows (MAX_TRAIN_T=2025-06-12)
- `/home/drewclaw/footy-models/data/forecast/train_matrix.bin` + `candidates_matrix.bin` + `model.bin` — the trained forecast model (ridge, λ=1000, seed 42, walk-forward AUC/IC: Spearman IC 0.529 pooled OOF)
- `/home/drewclaw/footy-models/data/surge/model.bin` — the trained value-surge classifier (walk-forward AUC 0.791)
- `/home/drewclaw/footy-models/data/forecast_candidates.csv`, `data/forecast_u23_top100.csv`, `data/proposals_top100.csv` — candidate pools + current deliverables
- `/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb` — **READ-ONLY** Transfermarkt overlay: `player_valuations` (656,301 rows, 2000-01-20 → **2026-06-12**), `players`, `transfers`, etc. This is the forward-test oracle.
- `/home/drewclaw/footy-models/data/extract_tm.py` — example of DuckDB access (duckdb.connect(read_only=True))

## Primitives to use (existing, tested)

- `crates/backtest` — `walk_forward_splits`, `enforce_cutoff`, `log_ratio_metrics` (RMSE/MAE/Spearman IC), `regression_deciles`, `reliability_deciles`, `expected_calibration_error`, `gem_metrics`, `compare_to_baselines`, `regression_metrics` (interval coverage), `subgroup_metrics`, `BacktestRunRecord` (version/seed records). Read its AGENTS.md.
- `crates/forecast` — `Rng64`, `VolatilityModel` (Ewma/Garch/Mf2Garch), `StochasticProcess`, `FootballPathModel`, `monte_carlo`, `monte_carlo_paths` (path-level series, added recently), `PriceDistribution`, `MonteCarloSummary`.
- `crates/models` — ridge/IRLS logistic, robust statistics.
- `apps/train` — `forecast-fit` (ridge R, seed, --cutoffs, --horizon-days), `forecast-report`, `surge-fit`, `surge-report`, materialize subcommands.
- `apps/score` — `forecast` / `surge` subcommands (candidates_matrix.bin + candidates_meta.tsv → top-100 CSV).

## Required work (in order)

### 1. Pin the as-of date (start here, it's quick)
Determine the exact valuation as-of used for the current candidates (FM26 snapshot ≈ start of FM26, ~Nov 2025). Check `forecast_candidates.csv` / `fm_tm_joined.csv` valuation dates and the FM26 DB vintage. Record the date; it defines the forward-test anchor.

### 2. REAL forward test (highest value — do this before any optimization)
Build a forward-test evaluation (extend an app with a new subcommand, or add a thin new app; domain logic in crates/backtest where possible):
- For the forecast model: players in `forecast_candidates.csv` with a valuation at the FM26 as-of date. Predict the 12m log ratio (model.bin, or re-fit with the same config). Compare against the ACTUAL TM valuation at as-of + ~12 months — but realized data only extends to 2026-06-12, so measure the realized window you have (as-of → 2026-06-12, ~7 months; report the actual horizon in days). Score: Spearman IC (predicted vs realized partial-period log ratio), hit rates, top-decile vs bottom-decile realized returns, calibration of the 80% conformal interval on the partial window (report it as partial-horizon coverage, honestly labeled).
- For the surge classifier: same idea — does the top-decile proposal list from the FM26 as-of actually outperform the bottom-decile on realized TM appreciation through 2026-06-12?
- Compare against the required baselines (TM value, FM CA, FM PA, age+minutes, random price-bucket) — `compare_to_baselines`.
- Record `BacktestRunRecord` fields (dataset/feature/model/calibration versions, seed).

### 3. Optimize as it goes (iterate)
After the forward test exists, iterate optimizations — each one re-run through walk-forward AND the forward test:
- ridge λ sweep for forecast (current λ=1000; verify the plateau and check λ∈{10,100,300,1000,3000} on pooled OOF IC)
- feature ablations (what happens without FM attrs? without contract features? which features drive the forward test?)
- MV band / PA-band handling / horizon-days sensitivity
- top-N and threshold choices for the proposal list
- Keep a running optimization log table: config → walk-forward IC/AUC → forward-test IC/hit rate. **Do not stop at the first improvement**; keep iterating until 2 consecutive configs fail to improve forward-test IC by ≥0.01 or you run out of budget. Prefer the config that wins on the REAL forward test when walk-forward and forward-test disagree.

### 4. Deliverables
- `data/backtest_report.md` — as-of date, forward-test methodology (window, n players, horizons), forward-test results table (IC, hit rates, deciles, baselines, coverage), walk-forward metrics for the same configs, optimization log table, final recommendation (which config to use).
- Updated artifacts ONLY if an optimization beats the current model on forward-test IC: `data/forecast/model.bin` + re-scored `data/forecast_u23_top100.csv` (via `apps/score forecast`) + updated `data/forecast_report.md`.
- Keep `data/proposals_top100.csv` and `data/forecast_u23_top100.csv` intact unless you produce a strictly better replacement (then keep both, name the new one `forecast_u23_top100_v2.csv`).
- `cargo test --release` green.

## Non-negotiables

- Walk-forward / forward-test only. NEVER random row splits for headline evidence.
- Strict as-of discipline: no future observations in features (enforce_cutoff), no lookahead in the forward test (prediction made at as-of, realized measured after).
- Deterministic: fixed seeds, report them.
- The FM26 attrs are a STATIC snapshot (start of FM26). Do not pretend they update. `attr_source` honesty: flag db vs derived in any output.
- Missing data is not zero.
- Costs: 15 bps round-trip wherever a portfolio is evaluated.
- DO NOT RE-MERGE `fm_tm_joined.csv` or re-run the join — it caused a pathological Cartesian blowup before. Use the files as-is.
- Read the DuckDB read-only. No writes to it, no new tables in it.
- Do not install Python runtime deps into production scoring; Rust-first. Python is allowed only for one-off analysis/reporting, not for scoring paths.

## Gate

The run gate is `bash scripts/gate_check_backtest.sh` (already on disk). It requires:
1. `cargo test --release` passes for the workspace (footy-backtest, footy-forecast, footy-models at minimum).
2. `data/backtest_report.md` exists and contains: as-of date, forward-test window + n players, forward-test Spearman IC + hit rates + decile table, baseline comparison, optimization log table with ≥2 configs, and an explicit recommendation.
3. Determinism: report reruns with the same seed produce identical forward-test IC (verify by rerunning the forward test twice, or state the seed and show the identical output).
4. Existing deliverables intact (`data/proposals_top100.csv`, `data/forecast_u23_top100.csv` present and non-empty).
5. No NaN/infinite values in any generated report tables.

Iterate until the gate passes. The gate is the completion criterion — a passing gate means you are done.
