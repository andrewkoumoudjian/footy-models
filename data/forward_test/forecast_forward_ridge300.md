# Forward Test — Forecast model (ridge300)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 300, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4526** |
| RMSE (partial-period log-ratio) | 0.4859 |
| MAE (partial-period log-ratio) | 0.3560 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4497 vs -0.2691 |
| 80% conformal interval partial-horizon coverage (radius 0.5905) | 0.8136 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2652 | -0.3696 | -0.2691 |
| 1 | 723 | -0.1398 | -0.2794 | -0.1867 |
| 2 | 723 | -0.0709 | -0.2064 | -0.1282 |
| 3 | 723 | -0.0155 | -0.1422 | -0.0538 |
| 4 | 723 | 0.0376 | -0.0954 | -0.0093 |
| 5 | 723 | 0.0847 | -0.0514 | 0.0573 |
| 6 | 723 | 0.1380 | 0.0303 | 0.1506 |
| 7 | 723 | 0.2036 | 0.0921 | 0.2666 |
| 8 | 723 | 0.3000 | 0.2209 | 0.5209 |
| 9 | 723 | 0.5518 | 0.5183 | 1.4497 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4286 | 0.4924 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.4286 | 0.1604 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.6786 | 0.4689 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.5397 | 0.3485 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.4286 | 0.2559 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5119 | 0.3472 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.4617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.6660 |
| 100 | 0.6400 | 0.6100 | 3.7099 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3861** |
| RMSE (partial-period log-ratio) | 0.6281 |
| MAE (partial-period log-ratio) | 0.4423 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.8336 vs 0.0436 |
| 80% conformal interval partial-horizon coverage (radius 0.5905) | 0.7740 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | -0.0744 | -0.0649 | 0.0436 |
| 1 | 21 | 0.0368 | -0.0879 | -0.0609 |
| 2 | 21 | 0.1283 | 0.1334 | 0.2814 |
| 3 | 21 | 0.1898 | 0.1504 | 0.2092 |
| 4 | 21 | 0.2418 | 0.4387 | 1.0479 |
| 5 | 21 | 0.2897 | 0.1926 | 0.5250 |
| 6 | 21 | 0.3696 | 0.6607 | 1.8975 |
| 7 | 21 | 0.4584 | 0.4104 | 1.3718 |
| 8 | 21 | 0.5655 | 0.5835 | 2.2571 |
| 9 | 19 | 0.7825 | 0.6735 | 1.8336 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 1.0000 | 0.5714 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 1.0000 | 0.1429 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.6667 | 0.2381 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.8750 | 0.3333 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 1.0000 | 0.3333 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 1.0000 | 0.1905 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 2.9267 |
| 25 | 0.6000 | 0.6000 | 2.8764 |
| 50 | 0.5400 | 0.5000 | 2.0851 |
| 100 | 0.5600 | 0.4900 | 1.6159 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4503** |
| RMSE (partial-period log-ratio) | 0.4760 |
| MAE (partial-period log-ratio) | 0.3449 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5000 vs -0.2427 |
| 80% conformal interval partial-horizon coverage (radius 0.5905) | 0.8268 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2581 | -0.3322 | -0.2427 |
| 1 | 684 | -0.1334 | -0.2474 | -0.1611 |
| 2 | 684 | -0.0649 | -0.1751 | -0.0990 |
| 3 | 684 | -0.0096 | -0.1060 | -0.0292 |
| 4 | 684 | 0.0437 | -0.0596 | 0.0255 |
| 5 | 684 | 0.0909 | -0.0090 | 0.0937 |
| 6 | 684 | 0.1459 | 0.0584 | 0.1770 |
| 7 | 684 | 0.2116 | 0.1247 | 0.3134 |
| 8 | 684 | 0.3103 | 0.2525 | 0.5755 |
| 9 | 679 | 0.5623 | 0.5339 | 1.5000 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5015 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1608 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4532 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3436 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2456 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3494 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.4617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.6660 |
| 100 | 0.6400 | 0.6100 | 3.7099 |


## Notes

- The conformal 80% interval radius (0.5905) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5554, MAE 0.3868, Spearman IC 0.5289.
