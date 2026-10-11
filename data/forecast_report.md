# Footy-Models — 12-Month Market-Value Forecast (U23, FM PA >= 140, MV <= 1M)

## 1. Objective

Forecast each candidate's Transfermarkt market value 12 months ahead from its latest valuation date and rank by the expected value gap (`predicted_12m_mv - current_mv`, EUR and %). Target population: age < 23 (floating years as of 2026-06-12), effective FM potential ability >= 140 (negative PA band codes converted to band midpoints first), 0 < current_mv <= 1,000,000 EUR, and `attr_source` in {db, derived}. Outputs are point forecasts with split-conformal 80% intervals, never bare point predictions.

## 2. Model spec

- Regression of the 12-month **log value ratio** `log_ratio = log(mv_12m / mv_t)` (not classification), where `mv_12m` is the valuation closest to `t+365` within `(t, t+365]`.
- Prediction: `predicted_12m_mv = mv_t * exp(predicted_log_ratio)`.
- Ridge OLS (closed-form normal equations + Gaussian elimination, deterministic), L2 `ridge = 1000000.0000`, seed `42`; every feature is standardized (mean/std fit on training rows) with a paired missingness-indicator slot -> 94 model inputs. Missing is not zero.
- Model version 2, dataset version 1, feature-set version 1, calibration version 1, trained through 2025-06-12 (rows observed on or before this date).
- Uncertainty: split-conformal 80% interval on the log ratio (absolute-residual quantile from out-of-fold predictions), reported per player as `interval_10` / `interval_90` on `predicted_12m_mv`; radius 0.6009 on 144578 calibration rows (measured empirical OOF coverage 0.8000).
- Hyperparameter sweep: ridge was swept over {10, 100, 300, 1000, 3000, 10000, 30000, 100000, 300000, 1000000, 3000000} on the pooled out-of-fold metrics AND the real forward test (as-of 2025-11-06 to 2026-06-12). Walk-forward pooled Spearman IC declines gently with ridge (0.529 at 1000 to 0.506 at 1e6), but the real forward-test IC improves monotonically from 0.456 (ridge 1000) to 0.510 (ridge 1e6) on the full forward pool (n=7230), and 0.392 to 0.399 on the deployable U23/PA>=140 population (n=208). The forward-test winner (ridge 1000000) is used for the deployed model (see `data/backtest_report.md` for the full optimization log).

## 3. Target definition

For every historical Transfermarkt valuation observation `(player, t, mv_t)` with 10k <= mv_t <= 1M: `log_ratio = log(mv_12m / mv_t)` where `mv_12m` is the market value at the valuation observation closest to `t+365` within `(t, t+365]`. Rows are kept only when the valuation series extends >= 365 days past `t` and at least one valuation is observed in `(t, t+365]` — censored outcomes are dropped, never encoded as zeros. All features are computed from observations strictly before `t`.

## 4. Features (47)

### Market state (causal valuation history) (11)

`log_mv, log_mv_180d_ratio, log_mv_365d_ratio, log_peak_prior_ratio, log_min_prior_ratio, log_n_vals_prior, log_days_since_first_val, val_freq, log_days_since_last_val, last_change_dir, mv_vol_log`

### Performance (appearances, causal windows) (8)

`perf_present_730, log_mins_365, log_apps_365, g_per90_365, a_per90_365, log_mins_180, mins_trend, g_per90_180`

### Transfer history (5)

`ever_transferred, log_days_since_transfer, log_transfer_fee, free_transfer, log_mv_transfer_ratio`

### Player state (6)

`age, age_sq, pos_gk, pos_df, pos_mf, height_cm`

### Competition context (4)

`comp_mapped, first_tier, confed_uefa, confed_sam`

### FM26 development/ability state (13)

`fm_present, fm_ca, fm_pa, fm_pa_gap, fm_ca_age_pct, fm_sig_technical, fm_sig_physical, fm_sig_mental, fm_sig_defensive, fm_sig_attacking, fm_sig_creative, fm_attr_db, fm_attr_derived`

Every feature is standardized with a paired missingness-indicator slot, so **missing is not zero**: an absent value contributes a 0 standardized value plus a 1.0 missingness indicator; measured zeros and absences stay distinct. Historical players without FM26 linkage contribute `fm_present=0` and missing FM slots.

## 5. Walk-forward validation

Strictly causal walk-forward: train on rows with `as_of <= cutoff`, test on rows with `as_of` in `(cutoff, cutoff+365]`. Cutoffs are the same as the surge model: 2020-06-01 ... 2024-06-01. No random row splits.

| cutoff | train rows | test rows | RMSE | MAE | Spearman IC |
|---|---|---|---|---|---|
| 2020-06-01 | 196092 | 32314 | 0.5637 | 0.3993 | 0.4986 |
| 2021-06-01 | 228406 | 35839 | 0.5577 | 0.3976 | 0.5003 |
| 2022-06-01 | 264245 | 26472 | 0.5984 | 0.4103 | 0.5025 |
| 2023-06-01 | 290717 | 25707 | 0.6350 | 0.4409 | 0.5087 |
| 2024-06-01 | 316424 | 24246 | 0.6147 | 0.4214 | 0.5083 |

- Pooled out-of-fold RMSE: **0.5905**
- Pooled out-of-fold MAE: **0.4120**
- Pooled out-of-fold Spearman IC (predicted vs realized log_ratio): **0.5063**
- Conformal 80% interval radius: 0.6009 (coverage 0.8000 on 144578 calibration rows)

## 6. Decile table (pooled out-of-fold, ranked by predicted log_ratio)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 14458 | -0.0715 | -0.2969 | -0.1983 |
| 1 | 14458 | 0.0301 | -0.1443 | -0.0717 |
| 2 | 14458 | 0.0915 | -0.0415 | 0.0455 |
| 3 | 14458 | 0.1412 | 0.0460 | 0.1732 |
| 4 | 14458 | 0.1860 | 0.1092 | 0.2787 |
| 5 | 14458 | 0.2306 | 0.1776 | 0.4274 |
| 6 | 14458 | 0.2779 | 0.2642 | 0.6219 |
| 7 | 14458 | 0.3316 | 0.3732 | 0.9096 |
| 8 | 14458 | 0.3983 | 0.5244 | 1.4628 |
| 9 | 14456 | 0.4915 | 0.8730 | 3.6310 |

A monotonically increasing predicted-to-realized log_ratio across deciles confirms the ranking separates expected appreciators from the rest.

## 7. Filter counts (scoring-time population)

- Age < 23: **2250** candidates.
- Age < 23 AND effective PA >= 140: **212** candidates.
- Age < 23 AND PA >= 140 AND 0 < current_mv <= 1M AND attr_source in {db, derived}: **212** candidates — the ranked population.

## 8. Top-100 mix

- 100 rows in `data/forecast_u23_top100_v2.csv` (deployed-model re-scoring; the original `data/forecast_u23_top100.csv` is kept intact); current_mv 150000 – 1000000 (mean 574500); gap_eur mean 196588.85.
- Age: min 17.2180 / mean 20.8683 / max 22.9240; effective PA: min 140.0000 / mean 154.6800 / max 175.0000.
- Position mix: MF 37, FW 34, GK 15, DF 14.
- Competition mix (top 10): premier-league 11, bundesliga 9, superliga 7, j1-league 6, jupiler-pro-league 6, eliteserien 5, eredivisie 5, premier-liga 5, allsvenskan 4, major-league-soccer 4.
- Ranking: `gap_eur` descending, ties broken by `player_id` ascending; exactly 100 rows.

## 9. Limitations

- FM26 attributes are a single snapshot (summer 2026); historical training rows attach current FM state as a stable proxy and encode absence with missingness indicators. The model cannot observe attribute evolution over time.
- `attr_source=derived` attributes are statistically generated from the distribution of real FM players and carry lower confidence.
- Transfermarkt data runs through 2026-06-12; candidates are scored as of their latest valuation date (each candidate's `t`).
- The forecast is a regression on a heavy-tailed 12-month return distribution; 80% conformal intervals are wide and coverage is empirical, not guaranteed per player.
- Negative FM PA codes (randomized potential bands) are converted to band midpoints for filtering/reporting only (documented conversion: -95→185, -85→175, -75→165, -65→155, -55→145, -45→135, -35→125, -25→115, -15→105, -10→185, -9→165, -8→145, -7→125, -6→105, -5→85, -4→65, -3→45, -2→30, -1→20). The model itself treats negative PA as missing.
- Free transfers are never encoded as zero-fee permanent transfers; `free_transfer` is a separate indicator and `log_transfer_fee` is missing for free moves.
- Current Transfermarkt value is never used as a model feature beyond the causal `log_mv` reference in the training/candidate matrices; market-perceived value, fundamental value, and forecast value remain distinct outputs.

## 10. Reproducibility

- Fixed seed 42, ridge 1000000.0000, deterministic closed-form OLS; same input → byte-identical model artifact, ranked CSV, and report.
- Source TSV checksum: `7718532073002000228`.
- Data plumbing (train.tsv, candidates.tsv, candidates_meta.tsv) is immutable for this run; enforced by `cargo test --release` and `scripts/gate_check_forecast.sh`.
