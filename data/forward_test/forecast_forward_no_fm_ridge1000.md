# Forward Test — Forecast model (no_fm_ridge1000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 21, dataset 1, feature-set 2, calibration 1, ridge 1000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=2, model_version=21, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5133** |
| RMSE (partial-period log-ratio) | 0.4997 |
| MAE (partial-period log-ratio) | 0.3747 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4796 vs -0.3004 |
| 80% conformal interval partial-horizon coverage (radius 0.6095) | 0.8089 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2877 | -0.4049 | -0.3004 |
| 1 | 723 | -0.1435 | -0.3400 | -0.2417 |
| 2 | 723 | -0.0504 | -0.2307 | -0.1641 |
| 3 | 723 | 0.0312 | -0.1533 | -0.0733 |
| 4 | 723 | 0.1011 | -0.0822 | -0.0061 |
| 5 | 723 | 0.1710 | -0.0308 | 0.0604 |
| 6 | 723 | 0.2508 | 0.0272 | 0.1377 |
| 7 | 723 | 0.3404 | 0.1344 | 0.2845 |
| 8 | 723 | 0.4668 | 0.2649 | 0.6213 |
| 9 | 723 | 0.7830 | 0.5325 | 1.4796 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5048 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1729 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4813 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3610 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2683 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3596 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.7000 | 6.7500 |
| 25 | 0.5600 | 0.5600 | 4.5333 |
| 50 | 0.6200 | 0.6200 | 4.1167 |
| 100 | 0.5800 | 0.5700 | 3.0907 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3612** |
| RMSE (partial-period log-ratio) | 0.6651 |
| MAE (partial-period log-ratio) | 0.5251 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.6274 vs 0.0137 |
| 80% conformal interval partial-horizon coverage (radius 0.6095) | 0.6731 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1298 | -0.0640 | 0.0137 |
| 1 | 21 | 0.2550 | 0.0791 | 0.1504 |
| 2 | 21 | 0.3268 | -0.0505 | 0.0351 |
| 3 | 21 | 0.3943 | 0.0814 | 0.2567 |
| 4 | 21 | 0.4586 | 0.5284 | 1.7444 |
| 5 | 21 | 0.5126 | 0.3017 | 0.5146 |
| 6 | 21 | 0.5828 | 0.3600 | 0.8643 |
| 7 | 21 | 0.6765 | 0.4930 | 0.8766 |
| 8 | 21 | 0.8366 | 0.6241 | 2.3986 |
| 9 | 19 | 0.9982 | 0.7437 | 2.6274 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.6000 | 0.5238 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 0.6000 | 0.0952 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.2667 | 0.1905 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.4750 | 0.2857 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 0.6000 | 0.2857 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 0.6000 | 0.1429 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.4000 | 0.9750 |
| 25 | 0.4800 | 0.4800 | 2.2155 |
| 50 | 0.5400 | 0.5200 | 2.1925 |
| 100 | 0.5400 | 0.4800 | 1.4632 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4947** |
| RMSE (partial-period log-ratio) | 0.4933 |
| MAE (partial-period log-ratio) | 0.3674 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5115 vs -0.2737 |
| 80% conformal interval partial-horizon coverage (radius 0.6095) | 0.8186 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2669 | -0.3630 | -0.2737 |
| 1 | 684 | -0.1273 | -0.2851 | -0.1998 |
| 2 | 684 | -0.0342 | -0.1993 | -0.1412 |
| 3 | 684 | 0.0471 | -0.1050 | -0.0281 |
| 4 | 684 | 0.1144 | -0.0508 | 0.0275 |
| 5 | 684 | 0.1858 | 0.0133 | 0.1104 |
| 6 | 684 | 0.2652 | 0.0496 | 0.1566 |
| 7 | 684 | 0.3539 | 0.1522 | 0.3179 |
| 8 | 684 | 0.4808 | 0.2886 | 0.6719 |
| 9 | 679 | 0.7969 | 0.5396 | 1.5115 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5117 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1711 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4635 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3538 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2558 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3596 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.7000 | 6.7500 |
| 25 | 0.5600 | 0.5600 | 4.5333 |
| 50 | 0.6200 | 0.6200 | 4.1167 |
| 100 | 0.5800 | 0.5700 | 3.0907 |


## Notes

- The conformal 80% interval radius (0.6095) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5666, MAE 0.3979, Spearman IC 0.5103.
