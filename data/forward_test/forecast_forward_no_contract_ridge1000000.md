# Forward Test — Forecast model (no_contract_ridge1000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 21, dataset 1, feature-set 3, calibration 1, ridge 1000000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=3, model_version=21, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5093** |
| RMSE (partial-period log-ratio) | 0.5147 |
| MAE (partial-period log-ratio) | 0.3927 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4681 vs -0.3127 |
| 80% conformal interval partial-horizon coverage (radius 0.6002) | 0.8000 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.0971 | -0.4393 | -0.3127 |
| 1 | 723 | -0.0045 | -0.3144 | -0.2320 |
| 2 | 723 | 0.0517 | -0.2282 | -0.1525 |
| 3 | 723 | 0.1018 | -0.1492 | -0.0863 |
| 4 | 723 | 0.1455 | -0.0895 | -0.0053 |
| 5 | 723 | 0.1876 | -0.0066 | 0.0949 |
| 6 | 723 | 0.2325 | 0.0350 | 0.1565 |
| 7 | 723 | 0.2865 | 0.1246 | 0.3203 |
| 8 | 723 | 0.3579 | 0.2580 | 0.5471 |
| 9 | 723 | 0.4706 | 0.5266 | 1.4681 |

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
| 10 | 0.8000 | 0.7000 | 7.4333 |
| 25 | 0.7600 | 0.7200 | 6.9433 |
| 50 | 0.6600 | 0.6400 | 5.4167 |
| 100 | 0.6700 | 0.6500 | 3.6780 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3993** |
| RMSE (partial-period log-ratio) | 0.6453 |
| MAE (partial-period log-ratio) | 0.4864 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.7096 vs 0.0609 |
| 80% conformal interval partial-horizon coverage (radius 0.6002) | 0.7596 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1885 | 0.0085 | 0.0609 |
| 1 | 21 | 0.2423 | -0.1376 | -0.0439 |
| 2 | 21 | 0.2862 | 0.0894 | 0.1931 |
| 3 | 21 | 0.3222 | 0.0600 | 0.1061 |
| 4 | 21 | 0.3471 | 0.1996 | 0.3971 |
| 5 | 21 | 0.3817 | 0.4927 | 1.1741 |
| 6 | 21 | 0.4162 | 0.4524 | 2.0567 |
| 7 | 21 | 0.4438 | 0.4412 | 0.7455 |
| 8 | 21 | 0.4845 | 0.7312 | 2.0907 |
| 9 | 19 | 0.5276 | 0.7614 | 2.7096 |

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
| 10 | 0.5000 | 0.5000 | 2.4483 |
| 25 | 0.6000 | 0.6000 | 3.1153 |
| 50 | 0.6000 | 0.5600 | 2.0211 |
| 100 | 0.5700 | 0.5100 | 1.6822 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4957** |
| RMSE (partial-period log-ratio) | 0.5020 |
| MAE (partial-period log-ratio) | 0.3790 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5270 vs -0.2801 |
| 80% conformal interval partial-horizon coverage (radius 0.6002) | 0.8168 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.0881 | -0.3928 | -0.2801 |
| 1 | 684 | 0.0032 | -0.2900 | -0.2177 |
| 2 | 684 | 0.0607 | -0.1684 | -0.1026 |
| 3 | 684 | 0.1100 | -0.1125 | -0.0533 |
| 4 | 684 | 0.1531 | -0.0448 | 0.0335 |
| 5 | 684 | 0.1953 | 0.0230 | 0.1291 |
| 6 | 684 | 0.2400 | 0.0559 | 0.1779 |
| 7 | 684 | 0.2937 | 0.1373 | 0.3402 |
| 8 | 684 | 0.3649 | 0.2878 | 0.5992 |
| 9 | 679 | 0.4744 | 0.5447 | 1.5270 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5161 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1754 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4678 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3582 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2602 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3640 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.7000 | 7.4333 |
| 25 | 0.7600 | 0.7200 | 6.9433 |
| 50 | 0.6600 | 0.6400 | 5.4167 |
| 100 | 0.6700 | 0.6500 | 3.6780 |


## Notes

- The conformal 80% interval radius (0.6002) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5921, MAE 0.4102, Spearman IC 0.4983.
