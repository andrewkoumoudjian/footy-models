# Forward Test — Forecast model (ridge750000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 750000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5094** |
| RMSE (partial-period log-ratio) | 0.5126 |
| MAE (partial-period log-ratio) | 0.3913 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4544 vs -0.3078 |
| 80% conformal interval partial-horizon coverage (radius 0.5943) | 0.7975 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.1155 | -0.4317 | -0.3078 |
| 1 | 723 | -0.0170 | -0.3276 | -0.2407 |
| 2 | 723 | 0.0437 | -0.2236 | -0.1515 |
| 3 | 723 | 0.0971 | -0.1446 | -0.0743 |
| 4 | 723 | 0.1441 | -0.0849 | -0.0059 |
| 5 | 723 | 0.1909 | -0.0251 | 0.0562 |
| 6 | 723 | 0.2396 | 0.0322 | 0.1719 |
| 7 | 723 | 0.2973 | 0.1426 | 0.3156 |
| 8 | 723 | 0.3760 | 0.2537 | 0.5802 |
| 9 | 723 | 0.5101 | 0.5262 | 1.4544 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5062 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1743 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4827 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3624 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2697 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3610 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 4.3000 |
| 25 | 0.6800 | 0.6400 | 6.1833 |
| 50 | 0.6600 | 0.6400 | 4.9633 |
| 100 | 0.6700 | 0.6600 | 3.4577 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3974** |
| RMSE (partial-period log-ratio) | 0.6454 |
| MAE (partial-period log-ratio) | 0.4911 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9763 vs 0.0665 |
| 80% conformal interval partial-horizon coverage (radius 0.5943) | 0.7548 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1970 | 0.0149 | 0.0665 |
| 1 | 21 | 0.2521 | -0.1697 | -0.0892 |
| 2 | 21 | 0.2991 | 0.1181 | 0.2462 |
| 3 | 21 | 0.3402 | 0.0496 | 0.0866 |
| 4 | 21 | 0.3679 | 0.4437 | 1.0062 |
| 5 | 21 | 0.4073 | 0.1986 | 0.3963 |
| 6 | 21 | 0.4502 | 0.5615 | 2.2988 |
| 7 | 21 | 0.4797 | 0.2624 | 0.4907 |
| 8 | 21 | 0.5263 | 0.8771 | 2.9415 |
| 9 | 19 | 0.5768 | 0.7405 | 1.9763 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.6000 | 0.5714 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 0.6000 | 0.1429 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.2667 | 0.2381 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.4750 | 0.3333 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 0.6000 | 0.3333 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 0.6000 | 0.1905 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 1.3150 |
| 25 | 0.6000 | 0.6000 | 1.8220 |
| 50 | 0.6200 | 0.5800 | 2.0394 |
| 100 | 0.5700 | 0.5100 | 1.6672 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4961** |
| RMSE (partial-period log-ratio) | 0.5002 |
| MAE (partial-period log-ratio) | 0.3780 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5049 vs -0.2808 |
| 80% conformal interval partial-horizon coverage (radius 0.5943) | 0.8140 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.1056 | -0.3897 | -0.2808 |
| 1 | 684 | -0.0086 | -0.2903 | -0.2113 |
| 2 | 684 | 0.0531 | -0.1735 | -0.1097 |
| 3 | 684 | 0.1060 | -0.0984 | -0.0373 |
| 4 | 684 | 0.1522 | -0.0476 | 0.0294 |
| 5 | 684 | 0.1994 | 0.0033 | 0.1003 |
| 6 | 684 | 0.2475 | 0.0604 | 0.1917 |
| 7 | 684 | 0.3053 | 0.1482 | 0.3278 |
| 8 | 684 | 0.3841 | 0.2894 | 0.6381 |
| 9 | 679 | 0.5146 | 0.5384 | 1.5049 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5146 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1740 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4664 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3567 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2588 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3626 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 4.3000 |
| 25 | 0.6800 | 0.6400 | 6.1833 |
| 50 | 0.6600 | 0.6400 | 4.9633 |
| 100 | 0.6700 | 0.6600 | 3.4577 |


## Notes

- The conformal 80% interval radius (0.5943) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5848, MAE 0.4077, Spearman IC 0.5087.
