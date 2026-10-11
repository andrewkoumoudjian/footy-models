# Forward Test — Forecast model (ridge1000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 1000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4555** |
| RMSE (partial-period log-ratio) | 0.4857 |
| MAE (partial-period log-ratio) | 0.3561 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4542 vs -0.2665 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.8136 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2582 | -0.3660 | -0.2665 |
| 1 | 723 | -0.1361 | -0.2862 | -0.1980 |
| 2 | 723 | -0.0689 | -0.2089 | -0.1224 |
| 3 | 723 | -0.0140 | -0.1393 | -0.0543 |
| 4 | 723 | 0.0382 | -0.1013 | -0.0139 |
| 5 | 723 | 0.0861 | -0.0585 | 0.0456 |
| 6 | 723 | 0.1390 | 0.0359 | 0.1542 |
| 7 | 723 | 0.2055 | 0.0950 | 0.2694 |
| 8 | 723 | 0.3025 | 0.2231 | 0.5298 |
| 9 | 723 | 0.5547 | 0.5232 | 1.4542 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4286 | 0.4979 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.4286 | 0.1660 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.6786 | 0.4744 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.5397 | 0.3541 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.4286 | 0.2614 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5119 | 0.3527 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.4617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.6660 |
| 100 | 0.6400 | 0.6100 | 3.5824 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3920** |
| RMSE (partial-period log-ratio) | 0.6268 |
| MAE (partial-period log-ratio) | 0.4434 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.8336 vs 0.0034 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.7692 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | -0.0627 | -0.1172 | 0.0034 |
| 1 | 21 | 0.0469 | -0.0568 | -0.0397 |
| 2 | 21 | 0.1385 | 0.1546 | 0.3005 |
| 3 | 21 | 0.1984 | 0.1504 | 0.2092 |
| 4 | 21 | 0.2516 | 0.3564 | 0.9805 |
| 5 | 21 | 0.2995 | 0.2941 | 0.6083 |
| 6 | 21 | 0.3793 | 0.4277 | 0.7816 |
| 7 | 21 | 0.4676 | 0.6241 | 2.4718 |
| 8 | 21 | 0.5736 | 0.5835 | 2.2571 |
| 9 | 19 | 0.7797 | 0.6735 | 1.8336 |

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
| 10 | 0.8000 | 0.8000 | 2.9267 |
| 25 | 0.6000 | 0.6000 | 2.8764 |
| 50 | 0.5400 | 0.5000 | 2.0851 |
| 100 | 0.5800 | 0.5000 | 1.6234 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4526** |
| RMSE (partial-period log-ratio) | 0.4758 |
| MAE (partial-period log-ratio) | 0.3450 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5095 vs -0.2457 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.8268 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2508 | -0.3361 | -0.2457 |
| 1 | 684 | -0.1294 | -0.2466 | -0.1688 |
| 2 | 684 | -0.0629 | -0.1745 | -0.0927 |
| 3 | 684 | -0.0083 | -0.1063 | -0.0246 |
| 4 | 684 | 0.0446 | -0.0651 | 0.0154 |
| 5 | 684 | 0.0923 | -0.0109 | 0.0924 |
| 6 | 684 | 0.1471 | 0.0610 | 0.1775 |
| 7 | 684 | 0.2137 | 0.1283 | 0.3159 |
| 8 | 684 | 0.3129 | 0.2502 | 0.5742 |
| 9 | 679 | 0.5652 | 0.5402 | 1.5095 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5073 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1667 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4591 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3494 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2515 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3553 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.4617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.6660 |
| 100 | 0.6400 | 0.6100 | 3.5824 |


## Notes

- The conformal 80% interval radius (0.5910) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5554, MAE 0.3868, Spearman IC 0.5291.
