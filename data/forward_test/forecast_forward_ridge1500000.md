# Forward Test — Forecast model (ridge1500000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 1500000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5100** |
| RMSE (partial-period log-ratio) | 0.5272 |
| MAE (partial-period log-ratio) | 0.4063 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4413 vs -0.3119 |
| 80% conformal interval partial-horizon coverage (radius 0.6129) | 0.7988 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.0459 | -0.4376 | -0.3119 |
| 1 | 723 | 0.0370 | -0.3199 | -0.2370 |
| 2 | 723 | 0.0871 | -0.2300 | -0.1541 |
| 3 | 723 | 0.1300 | -0.1510 | -0.0845 |
| 4 | 723 | 0.1679 | -0.0720 | 0.0090 |
| 5 | 723 | 0.2049 | -0.0215 | 0.0769 |
| 6 | 723 | 0.2434 | 0.0315 | 0.1541 |
| 7 | 723 | 0.2897 | 0.1387 | 0.3308 |
| 8 | 723 | 0.3502 | 0.2620 | 0.5735 |
| 9 | 723 | 0.4432 | 0.5168 | 1.4413 |

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
| 10 | 0.7000 | 0.7000 | 8.3000 |
| 25 | 0.7200 | 0.6800 | 5.2333 |
| 50 | 0.7000 | 0.6800 | 5.0970 |
| 100 | 0.6200 | 0.6100 | 3.4454 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3951** |
| RMSE (partial-period log-ratio) | 0.6508 |
| MAE (partial-period log-ratio) | 0.4877 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.0465 vs 0.1133 |
| 80% conformal interval partial-horizon coverage (radius 0.6129) | 0.7596 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.2058 | 0.0604 | 0.1133 |
| 1 | 21 | 0.2505 | -0.1822 | -0.0884 |
| 2 | 21 | 0.2905 | 0.0915 | 0.1831 |
| 3 | 21 | 0.3200 | 0.1303 | 0.3144 |
| 4 | 21 | 0.3431 | 0.3079 | 0.6879 |
| 5 | 21 | 0.3704 | 0.2580 | 0.5142 |
| 6 | 21 | 0.3963 | 0.4748 | 2.1877 |
| 7 | 21 | 0.4228 | 0.5005 | 1.5264 |
| 8 | 21 | 0.4515 | 0.6908 | 1.9415 |
| 9 | 19 | 0.4848 | 0.7673 | 2.0465 |

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
| 10 | 0.5000 | 0.5000 | 1.3483 |
| 25 | 0.6000 | 0.6000 | 2.3793 |
| 50 | 0.6400 | 0.6000 | 2.0698 |
| 100 | 0.5700 | 0.5100 | 1.6812 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4959** |
| RMSE (partial-period log-ratio) | 0.5134 |
| MAE (partial-period log-ratio) | 0.3916 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4596 vs -0.2841 |
| 80% conformal interval partial-horizon coverage (radius 0.6129) | 0.8159 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.0375 | -0.3957 | -0.2841 |
| 1 | 684 | 0.0439 | -0.2849 | -0.2119 |
| 2 | 684 | 0.0950 | -0.1668 | -0.1007 |
| 3 | 684 | 0.1372 | -0.1265 | -0.0658 |
| 4 | 684 | 0.1748 | -0.0304 | 0.0458 |
| 5 | 684 | 0.2117 | 0.0223 | 0.1283 |
| 6 | 684 | 0.2497 | 0.0496 | 0.1690 |
| 7 | 684 | 0.2960 | 0.1437 | 0.3465 |
| 8 | 684 | 0.3561 | 0.2973 | 0.6660 |
| 9 | 679 | 0.4463 | 0.5315 | 1.4596 |

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
| 10 | 0.7000 | 0.7000 | 8.3000 |
| 25 | 0.7200 | 0.6800 | 5.2333 |
| 50 | 0.7000 | 0.6800 | 5.0970 |
| 100 | 0.6200 | 0.6100 | 3.4454 |


## Notes

- The conformal 80% interval radius (0.6129) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5995, MAE 0.4189, Spearman IC 0.5030.
