# Forward Test — Forecast model (ridge500000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 500000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5079** |
| RMSE (partial-period log-ratio) | 0.5056 |
| MAE (partial-period log-ratio) | 0.3839 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4808 vs -0.3052 |
| 80% conformal interval partial-horizon coverage (radius 0.5932) | 0.7983 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.1479 | -0.4269 | -0.3052 |
| 1 | 723 | -0.0441 | -0.3290 | -0.2408 |
| 2 | 723 | 0.0202 | -0.2158 | -0.1506 |
| 3 | 723 | 0.0782 | -0.1481 | -0.0692 |
| 4 | 723 | 0.1290 | -0.0985 | -0.0295 |
| 5 | 723 | 0.1803 | -0.0156 | 0.0703 |
| 6 | 723 | 0.2334 | 0.0308 | 0.1670 |
| 7 | 723 | 0.2959 | 0.1243 | 0.2996 |
| 8 | 723 | 0.3834 | 0.2626 | 0.5757 |
| 9 | 723 | 0.5424 | 0.5333 | 1.4808 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5076 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1757 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4841 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3638 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2711 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3624 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 7.2000 |
| 25 | 0.6800 | 0.6400 | 6.3233 |
| 50 | 0.6400 | 0.6200 | 4.6733 |
| 100 | 0.6800 | 0.6600 | 3.4750 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4000** |
| RMSE (partial-period log-ratio) | 0.6427 |
| MAE (partial-period log-ratio) | 0.4909 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.0219 vs 0.0133 |
| 80% conformal interval partial-horizon coverage (radius 0.5932) | 0.7308 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1879 | -0.0407 | 0.0133 |
| 1 | 21 | 0.2474 | -0.0274 | 0.0952 |
| 2 | 21 | 0.2966 | 0.0314 | 0.1150 |
| 3 | 21 | 0.3456 | 0.0770 | 0.1442 |
| 4 | 21 | 0.3758 | 0.4163 | 0.9486 |
| 5 | 21 | 0.4229 | 0.1326 | 0.2534 |
| 6 | 21 | 0.4714 | 0.6253 | 1.2401 |
| 7 | 21 | 0.5089 | 0.2646 | 1.6923 |
| 8 | 21 | 0.5638 | 0.8584 | 2.9002 |
| 9 | 19 | 0.6273 | 0.7611 | 2.0219 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 1.0000 | 0.6190 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 1.0000 | 0.1905 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.6667 | 0.2857 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.8750 | 0.3810 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 1.0000 | 0.3810 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 1.0000 | 0.2381 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 1.3550 |
| 25 | 0.6000 | 0.6000 | 1.8715 |
| 50 | 0.6200 | 0.6000 | 2.0409 |
| 100 | 0.5600 | 0.5100 | 1.6639 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4952** |
| RMSE (partial-period log-ratio) | 0.4939 |
| MAE (partial-period log-ratio) | 0.3714 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5104 vs -0.2734 |
| 80% conformal interval partial-horizon coverage (radius 0.5932) | 0.8140 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.1376 | -0.3826 | -0.2734 |
| 1 | 684 | -0.0351 | -0.2939 | -0.2150 |
| 2 | 684 | 0.0299 | -0.1758 | -0.1151 |
| 3 | 684 | 0.0875 | -0.0956 | -0.0309 |
| 4 | 684 | 0.1377 | -0.0446 | 0.0357 |
| 5 | 684 | 0.1896 | -0.0194 | 0.0593 |
| 6 | 684 | 0.2421 | 0.0812 | 0.2298 |
| 7 | 684 | 0.3046 | 0.1400 | 0.3188 |
| 8 | 684 | 0.3926 | 0.2888 | 0.6335 |
| 9 | 679 | 0.5480 | 0.5421 | 1.5104 |

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
| 10 | 0.6000 | 0.6000 | 7.2000 |
| 25 | 0.6800 | 0.6400 | 6.3233 |
| 50 | 0.6400 | 0.6200 | 4.6733 |
| 100 | 0.6800 | 0.6600 | 3.4750 |


## Notes

- The conformal 80% interval radius (0.5932) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5779, MAE 0.4026, Spearman IC 0.5119.
