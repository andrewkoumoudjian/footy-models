# STEERING — Refine Models to Surface MORE Value-Spike Candidates (Basket: U23, TM €400k–€1.2M)

## Mission

The user's basket is U23 players with Transfermarkt value €400k–€1.2M (948 scored players; `data/forecast_36m_top100.csv` is the current deliverable ranked by expected 36-month % gain). The user believes there should be **more players in this basket who will experience MASSIVE value spikes**. The current ranking is drift-based (compounded 12m log forecast) — it ranks expected appreciation, which underweights tail/spike events. Your mission: **refine the models to produce a spike-focused ranking of this basket that demonstrably surfaces more true spike candidates**, validated on the REAL forward window (as-of 2025-11-06 → realized TM through 2026-06-12).

Read `architecture.md` + `AGENTS.md` first. Use the existing Rust primitives; extend them where needed.

## Context you inherit (all verified)

- Forecast v2 model: `data/forecast/model.bin` (ridge=1e6, seed 42; forward-test Spearman IC 0.5100; SHA-256 `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f`). Do NOT modify it.
- Surge classifier (value-surge: P(≥1.5× in 12m)): shipped `data/surge/model.bin` (ridge=1.0; forward-test AUC 0.6566 full / 0.5920 deployable). The previous optimization run RECOMMENDED ridge=3,000,000 (forward AUC 0.7430 full / 0.6345 deployable) but did not deploy it — that recommendation is in `data/backtest_report.md` §4/§5.3.
- Forward-test infra: `apps/forward` (`footy-forward forecast|surge ...`), data files under `data/forward_test/` (forecast_* and surge_* candidates/realized/meta), `data/forward_test/metrics_log.tsv`.
- Training: `apps/train` (`surge-fit --matrix ... --ridge R --seed N [--cutoffs ...] --output model.bin`, `surge-materialize`, `surge-materialize-candidates`, `forecast-*` variants). Dataset 1 / feature-set 1 versions.
- Backtest metrics: `crates/backtest` — `reliability_deciles`, `gem_metrics`, `compare_to_baselines`, `expected_calibration_error`, `log_ratio_metrics`, `spearman_ic`, `regression_deciles`.
- Basket pool (this run's population): U23 (`age < 23`), `400000 <= current_mv <= 1200000`, `attr_source != 'staff'` — 948 players; source `data/fm_tm_joined.csv` (MERGING IS DONE — DO NOT RE-MERGE). `data/forecast_36m_all.csv` has all 948 scored.

## Required work

### 1. Wire spike/tail events INTO the forecaster (centerpiece — do this first)

The user's point is correct and structural: the deployed v2 forecaster is a ridge regression on the 12-month log-ratio — an MSE-style conditional-MEAN model. It predicts the center of the distribution and systematically underweights the tail, which is exactly why the 36m top-100 looks thin on spike candidates. The forecaster itself must become tail-aware. Build a NEW forecast artifact (never modify `data/forecast/model.bin`) with a tail-wired objective, using the same 47-feature causal schema and the same walk-forward + real forward-test harness:

- **Primary direction — tail-aware forecaster.** Extend `crates/models` / `apps/train` with one or more of:
  - quantile regression on the log-ratio at high quantiles (e.g. τ = 0.80, 0.90) via pinball loss — the fitted high-quantile becomes the spike score;
  - expectile/weighted loss that up-weights positive-tail errors;
  - a two-part model: P(spike) head (peak ≥ +50% / +100% within 12m) + conditional spike magnitude head, jointly fit with a tuned weight (default λ_spike = 0.3, sweep);
  - or any defensible tail objective — but it must be a property of the FORECAST MODEL, not a separate post-hoc classifier.
- Validate each variant on (a) walk-forward pooled OOF (rank IC, decile monotonicity, tail decile hit rates) and (b) the REAL forward window (as-of 2025-11-06 → 2026-06-12): tail hit rates at ≥+50% and ≥+100%, top-decile realized mean/median return, high-quantile calibration. The decision metric is forward-test tail accuracy.
- Output a per-player `tail_score` (the high-quantile / spike probability) for the 948-player basket.

### 2. Spike ranking of the basket (complementary, do after step 1)
Score the basket with the surge classifier as a SECOND signal:
- Fit the recommended surge config ridge=3,000,000 (seed 42) as a NEW artifact `data/surge/model_ridge3m.bin` (do NOT overwrite the shipped `data/surge/model.bin`).
- Score the 948 basket players → `data/surge_basket_ranked.csv` with surge probability; validate on the forward window (AUC + decile spike rates).
- This is an input to the ensemble (step 3), not the primary ranking.

### 3. Combine / decide the final ranking
The deliverable ranking should maximize realized spike rate among the top 100 while keeping ranking quality. Evaluate on the forward window (pick by evidence):
- Tail-aware forecast score alone
- Tail score × surge probability (joint)
- min/rank-average of tail score and surge z-scores
- Any defensible ensemble — report exactly how it is computed.
Decision criterion = REAL forward test: spike hit rates (≥ +50% and ≥ +100%), top-decile realized mean/median return, baseline comparisons (TM value, FM CA, FM PA, age+minutes, random price-bucket). Prefer models that also hold up in walk-forward; if a model wins only on the forward window, require monotone/multi-config evidence before choosing it.

### 4. Deliverables
- `data/spike_refined_top100.csv` — the FINAL top-100 spike candidates in the basket: player_id, name, club, competition, country, position, pos_cls, age, current_mv, fm_ca, fm_pa, attr_source, spike_score (final ranking score, name it precisely), predicted_12m_mv (v2 forecast), 36m expected gain, surge_prob, rationale/key_strengths.
- `data/spike_refinement_report.md` — methodology (population, models compared, configs, seeds), forward-test comparison table (old 36m top-100 vs new spike top-100: spike hit rates ≥50%/≥100%, mean/median return, deciles, baselines), ablation log, the final choice + why, honest limitations (single realization window).
- New model artifacts as described (never overwrite shipped ones).
- `cargo test --release` green. Do not touch `data/forecast_u23_top100*.csv`, `data/forecast_36m_top100.csv`, `data/forecast/model.bin`, `data/surge/model.bin`.

## Non-negotiables

- **Long-running fits MUST run in the background with polling.** The watchdog kills the whole run if the session file shows no growth for 15 minutes. A foreground fit that takes >15 min looks like a stall and gets killed, losing all progress (this already happened twice). For any fit/scoring step that may take >5 minutes:
  1. Launch it detached: `nohup ./target/release/footy-train ... > /tmp/fit_<name>.log 2>&1 & echo $! > /tmp/fit_<name>.pid`
  2. Poll the log (`tail -n 5 /tmp/fit_<name>.log`) and emit a short session message on each poll, so the session keeps growing.
  3. Only continue when the log shows completion (or exit-code check on the pid).
  This keeps the watchdog happy and preserves long computations.
- **Validate runtime on a subset first.** Before any full-matrix fit, run the same command on a 50k-row subsample (or a few thousand rows for candidates) to confirm it completes in reasonable time and produces sane outputs. Only then run the full matrix.
- Walk-forward / forward-test only. No random row splits for headline evidence.
- Strict as-of: features from past only; realized measured strictly after entry. Censored players dropped, not zeroed.
- Deterministic: seed 42 everywhere; report seeds.
- attr_source honesty (db vs derived flagged) everywhere.
- Missing is not zero. 15 bps costs where portfolios are evaluated.
- DO NOT RE-MERGE. DO NOT modify the deployed v2 forecast model or shipped surge model. DuckDB read-only.
- Rust-first for scoring; Python only for reporting/analysis.

## Gate

`bash scripts/gate_check_refine.sh` (on disk). It requires:
1. `cargo test --release` passes (footy-backtest, footy-forecast, footy-models).
2. `data/spike_refined_top100.csv`: exactly 100 rows; every row age < 23, 400000 ≤ current_mv ≤ 1200000, attr_source != 'staff'; no NaN; has a spike_score column.
3. `data/spike_refinement_report.md` exists and contains: basket population definition, forward-test comparison table (old vs new top-100 spike hit rates at +50% and +100%), the final model config + seed, baseline comparison, limitations.
4. The refined top-100's forward-test spike hit rate (≥ +50%) is ≥ the old 36m top-100's hit rate on the same window (or the report states honestly that it is not and explains why the old ranking remains).
5. Deployed models untouched (SHA-256 of `data/forecast/model.bin` still `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f`; `data/surge/model.bin` mtime unchanged from before the run — record it at run start).
6. Determinism: seed statement in the report.

Iterate until the gate passes. Passing gate = done.
