# Forward Test — Forecast model (ridge100)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 100, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4514** |
| RMSE (partial-period log-ratio) | 0.4861 |
| MAE (partial-period log-ratio) | 0.3561 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4496 vs -0.2679 |
| 80% conformal interval partial-horizon coverage (radius 0.5902) | 0.8129 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2687 | -0.3675 | -0.2679 |
| 1 | 723 | -0.1415 | -0.2805 | -0.1867 |
| 2 | 723 | -0.0717 | -0.2060 | -0.1278 |
| 3 | 723 | -0.0160 | -0.1342 | -0.0475 |
| 4 | 723 | 0.0375 | -0.1004 | -0.0115 |
| 5 | 723 | 0.0844 | -0.0527 | 0.0549 |
| 6 | 723 | 0.1378 | 0.0285 | 0.1481 |
| 7 | 723 | 0.2031 | 0.0929 | 0.2701 |
| 8 | 723 | 0.2994 | 0.2192 | 0.5167 |
| 9 | 723 | 0.5512 | 0.5178 | 1.4496 |

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
| 10 | 0.8000 | 0.8000 | 6.3617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.6660 |
| 100 | 0.6400 | 0.6100 | 3.5749 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3860** |
| RMSE (partial-period log-ratio) | 0.6286 |
| MAE (partial-period log-ratio) | 0.4418 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.8336 vs 0.0436 |
| 80% conformal interval partial-horizon coverage (radius 0.5902) | 0.7740 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | -0.0790 | -0.0649 | 0.0436 |
| 1 | 21 | 0.0330 | -0.0998 | -0.0745 |
| 2 | 21 | 0.1248 | 0.0986 | 0.2315 |
| 3 | 21 | 0.1870 | 0.1971 | 0.2727 |
| 4 | 21 | 0.2371 | 0.3483 | 0.7781 |
| 5 | 21 | 0.2870 | 0.2829 | 0.7948 |
| 6 | 21 | 0.3664 | 0.6607 | 1.8975 |
| 7 | 21 | 0.4547 | 0.4974 | 1.5583 |
| 8 | 21 | 0.5631 | 0.4964 | 2.0706 |
| 9 | 19 | 0.7841 | 0.6735 | 1.8336 |

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
| 100 | 0.5600 | 0.4900 | 1.6092 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4494** |
| RMSE (partial-period log-ratio) | 0.4761 |
| MAE (partial-period log-ratio) | 0.3449 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4959 vs -0.2461 |
| 80% conformal interval partial-horizon coverage (radius 0.5902) | 0.8260 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2618 | -0.3369 | -0.2461 |
| 1 | 684 | -0.1352 | -0.2429 | -0.1581 |
| 2 | 684 | -0.0656 | -0.1784 | -0.1010 |
| 3 | 684 | -0.0102 | -0.0989 | -0.0223 |
| 4 | 684 | 0.0435 | -0.0588 | 0.0243 |
| 5 | 684 | 0.0905 | -0.0127 | 0.0907 |
| 6 | 684 | 0.1456 | 0.0622 | 0.1928 |
| 7 | 684 | 0.2110 | 0.1340 | 0.3146 |
| 8 | 684 | 0.3097 | 0.2421 | 0.5621 |
| 9 | 679 | 0.5616 | 0.5304 | 1.4959 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4722 | 0.5000 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.4722 | 0.1594 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.6722 | 0.4518 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.4722 | 0.3421 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.4722 | 0.2442 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.4722 | 0.3480 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.3617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.6660 |
| 100 | 0.6400 | 0.6100 | 3.5749 |


## Notes

- The conformal 80% interval radius (0.5902) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5555, MAE 0.3869, Spearman IC 0.5287.
