# Forward Test — Forecast model (ridge1000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5100** |
| RMSE (partial-period log-ratio) | 0.5183 |
| MAE (partial-period log-ratio) | 0.3972 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4400 vs -0.3105 |
| 80% conformal interval partial-horizon coverage (radius 0.6009) | 0.7978 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.0884 | -0.4357 | -0.3105 |
| 1 | 723 | 0.0046 | -0.3245 | -0.2392 |
| 2 | 723 | 0.0615 | -0.2257 | -0.1512 |
| 3 | 723 | 0.1110 | -0.1428 | -0.0759 |
| 4 | 723 | 0.1546 | -0.0828 | -0.0032 |
| 5 | 723 | 0.1975 | -0.0276 | 0.0671 |
| 6 | 723 | 0.2423 | 0.0435 | 0.1740 |
| 7 | 723 | 0.2958 | 0.1296 | 0.2986 |
| 8 | 723 | 0.3674 | 0.2624 | 0.5981 |
| 9 | 723 | 0.4837 | 0.5206 | 1.4400 |

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
| 10 | 0.6000 | 0.6000 | 4.5000 |
| 25 | 0.6800 | 0.6400 | 6.2833 |
| 50 | 0.7000 | 0.6800 | 5.5083 |
| 100 | 0.6700 | 0.6600 | 3.6060 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3992** |
| RMSE (partial-period log-ratio) | 0.6474 |
| MAE (partial-period log-ratio) | 0.4901 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9763 vs 0.0610 |
| 80% conformal interval partial-horizon coverage (radius 0.6009) | 0.7548 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.2016 | 0.0051 | 0.0610 |
| 1 | 21 | 0.2526 | -0.1598 | -0.0837 |
| 2 | 21 | 0.2980 | 0.1102 | 0.2169 |
| 3 | 21 | 0.3331 | 0.0575 | 0.1160 |
| 4 | 21 | 0.3596 | 0.3409 | 0.8355 |
| 5 | 21 | 0.3936 | 0.3014 | 0.5669 |
| 6 | 21 | 0.4296 | 0.4798 | 2.1877 |
| 7 | 21 | 0.4574 | 0.4868 | 1.5066 |
| 8 | 21 | 0.4963 | 0.7345 | 2.0367 |
| 9 | 19 | 0.5393 | 0.7405 | 1.9763 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.8000 | 0.6190 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 0.8000 | 0.1905 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.4667 | 0.2857 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.6750 | 0.3810 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 0.8000 | 0.3810 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 0.8000 | 0.2381 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 1.7683 |
| 25 | 0.5600 | 0.5600 | 1.7553 |
| 50 | 0.6200 | 0.5800 | 2.0494 |
| 100 | 0.5700 | 0.5100 | 1.6672 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4963** |
| RMSE (partial-period log-ratio) | 0.5053 |
| MAE (partial-period log-ratio) | 0.3834 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5022 vs -0.2793 |
| 80% conformal interval partial-horizon coverage (radius 0.6009) | 0.8146 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.0790 | -0.3881 | -0.2793 |
| 1 | 684 | 0.0124 | -0.2955 | -0.2188 |
| 2 | 684 | 0.0705 | -0.1734 | -0.1084 |
| 3 | 684 | 0.1191 | -0.1084 | -0.0474 |
| 4 | 684 | 0.1623 | -0.0395 | 0.0367 |
| 5 | 684 | 0.2054 | 0.0199 | 0.1235 |
| 6 | 684 | 0.2496 | 0.0504 | 0.1737 |
| 7 | 684 | 0.3032 | 0.1531 | 0.3611 |
| 8 | 684 | 0.3745 | 0.2846 | 0.6098 |
| 9 | 679 | 0.4877 | 0.5372 | 1.5022 |

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
| 10 | 0.6000 | 0.6000 | 4.5000 |
| 25 | 0.6800 | 0.6400 | 6.2833 |
| 50 | 0.7000 | 0.6800 | 5.5083 |
| 100 | 0.6700 | 0.6600 | 3.6060 |


## Notes

- The conformal 80% interval radius (0.6009) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5905, MAE 0.4120, Spearman IC 0.5063.
