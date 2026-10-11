# Forward Test — Forecast model (ridge2000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 2000000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5098** |
| RMSE (partial-period log-ratio) | 0.5339 |
| MAE (partial-period log-ratio) | 0.4130 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4007 vs -0.3130 |
| 80% conformal interval partial-horizon coverage (radius 0.6198) | 0.7981 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.0140 | -0.4387 | -0.3130 |
| 1 | 723 | 0.0602 | -0.3169 | -0.2335 |
| 2 | 723 | 0.1046 | -0.2254 | -0.1491 |
| 3 | 723 | 0.1424 | -0.1646 | -0.0967 |
| 4 | 723 | 0.1759 | -0.0670 | 0.0118 |
| 5 | 723 | 0.2085 | -0.0201 | 0.0781 |
| 6 | 723 | 0.2422 | 0.0358 | 0.1599 |
| 7 | 723 | 0.2828 | 0.1355 | 0.3236 |
| 8 | 723 | 0.3352 | 0.2671 | 0.6161 |
| 9 | 723 | 0.4133 | 0.5115 | 1.4007 |

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
| 10 | 0.8000 | 0.8000 | 9.5000 |
| 25 | 0.7600 | 0.7600 | 6.1400 |
| 50 | 0.7200 | 0.7000 | 4.4237 |
| 100 | 0.6200 | 0.6100 | 3.4259 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3926** |
| RMSE (partial-period log-ratio) | 0.6535 |
| MAE (partial-period log-ratio) | 0.4864 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.8675 vs 0.1266 |
| 80% conformal interval partial-horizon coverage (radius 0.6198) | 0.7644 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.2072 | 0.0445 | 0.1266 |
| 1 | 21 | 0.2473 | -0.1557 | -0.0921 |
| 2 | 21 | 0.2827 | 0.1275 | 0.2331 |
| 3 | 21 | 0.3085 | 0.1314 | 0.3124 |
| 4 | 21 | 0.3291 | 0.2601 | 0.6303 |
| 5 | 21 | 0.3517 | 0.3346 | 0.7589 |
| 6 | 21 | 0.3716 | 0.3981 | 1.9430 |
| 7 | 21 | 0.3959 | 0.5752 | 1.6502 |
| 8 | 21 | 0.4192 | 0.6704 | 1.9796 |
| 9 | 19 | 0.4466 | 0.7074 | 1.8675 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.6000 | 0.6190 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 0.6000 | 0.1905 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.2667 | 0.2857 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.4750 | 0.3810 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 0.6000 | 0.3810 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 0.6000 | 0.2381 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 1.4283 |
| 25 | 0.6400 | 0.6400 | 2.4460 |
| 50 | 0.6600 | 0.6200 | 2.1498 |
| 100 | 0.5700 | 0.5100 | 1.6837 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4953** |
| RMSE (partial-period log-ratio) | 0.5195 |
| MAE (partial-period log-ratio) | 0.3977 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4567 vs -0.2860 |
| 80% conformal interval partial-horizon coverage (radius 0.6198) | 0.8159 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.0066 | -0.3970 | -0.2860 |
| 1 | 684 | 0.0664 | -0.2795 | -0.2065 |
| 2 | 684 | 0.1117 | -0.1726 | -0.1055 |
| 3 | 684 | 0.1489 | -0.1261 | -0.0649 |
| 4 | 684 | 0.1820 | -0.0277 | 0.0454 |
| 5 | 684 | 0.2145 | 0.0128 | 0.1251 |
| 6 | 684 | 0.2478 | 0.0568 | 0.1723 |
| 7 | 684 | 0.2882 | 0.1517 | 0.3580 |
| 8 | 684 | 0.3402 | 0.2933 | 0.6582 |
| 9 | 679 | 0.4159 | 0.5286 | 1.4567 |

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
| 10 | 0.8000 | 0.8000 | 9.5000 |
| 25 | 0.7600 | 0.7600 | 6.1400 |
| 50 | 0.7200 | 0.7000 | 4.4237 |
| 100 | 0.6200 | 0.6100 | 3.4259 |


## Notes

- The conformal 80% interval radius (0.6198) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.6063, MAE 0.4243, Spearman IC 0.5007.
