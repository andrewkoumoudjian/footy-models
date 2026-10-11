# Forward Test — Forecast model (no_contract_ridge1000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 21, dataset 1, feature-set 3, calibration 1, ridge 1000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=3, model_version=21, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4627** |
| RMSE (partial-period log-ratio) | 0.4825 |
| MAE (partial-period log-ratio) | 0.3530 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4660 vs -0.2839 |
| 80% conformal interval partial-horizon coverage (radius 0.5868) | 0.8165 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2581 | -0.3948 | -0.2839 |
| 1 | 723 | -0.1427 | -0.2793 | -0.1979 |
| 2 | 723 | -0.0753 | -0.2244 | -0.1399 |
| 3 | 723 | -0.0215 | -0.1191 | -0.0427 |
| 4 | 723 | 0.0282 | -0.1204 | -0.0391 |
| 5 | 723 | 0.0798 | -0.0285 | 0.0720 |
| 6 | 723 | 0.1331 | 0.0227 | 0.1414 |
| 7 | 723 | 0.1989 | 0.1165 | 0.3069 |
| 8 | 723 | 0.2934 | 0.2287 | 0.5152 |
| 9 | 723 | 0.5399 | 0.5157 | 1.4660 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4000 | 0.4855 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.4000 | 0.1535 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.6500 | 0.4620 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.5111 | 0.3416 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.4000 | 0.2490 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.4833 | 0.3402 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 7.3617 |
| 25 | 0.6800 | 0.6400 | 5.5493 |
| 50 | 0.6600 | 0.6200 | 4.2160 |
| 100 | 0.6600 | 0.6300 | 3.6340 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3964** |
| RMSE (partial-period log-ratio) | 0.6251 |
| MAE (partial-period log-ratio) | 0.4422 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.0216 vs -0.0492 |
| 80% conformal interval partial-horizon coverage (radius 0.5868) | 0.7644 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | -0.0595 | -0.1146 | -0.0492 |
| 1 | 21 | 0.0410 | -0.0230 | 0.0575 |
| 2 | 21 | 0.1390 | 0.0933 | 0.1698 |
| 3 | 21 | 0.2025 | 0.2275 | 0.5174 |
| 4 | 21 | 0.2372 | 0.1259 | 0.2640 |
| 5 | 21 | 0.2875 | 0.5010 | 0.9799 |
| 6 | 21 | 0.3737 | 0.4283 | 0.9343 |
| 7 | 21 | 0.4671 | 0.3120 | 0.6659 |
| 8 | 21 | 0.5643 | 0.7941 | 3.8633 |
| 9 | 19 | 0.7509 | 0.7535 | 2.0216 |

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
| 10 | 0.8000 | 0.8000 | 2.9767 |
| 25 | 0.6400 | 0.6400 | 1.7564 |
| 50 | 0.5400 | 0.5000 | 2.4251 |
| 100 | 0.5800 | 0.5200 | 1.7208 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4601** |
| RMSE (partial-period log-ratio) | 0.4729 |
| MAE (partial-period log-ratio) | 0.3420 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5425 vs -0.2597 |
| 80% conformal interval partial-horizon coverage (radius 0.5868) | 0.8293 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2502 | -0.3605 | -0.2597 |
| 1 | 684 | -0.1361 | -0.2398 | -0.1697 |
| 2 | 684 | -0.0694 | -0.1871 | -0.1115 |
| 3 | 684 | -0.0158 | -0.0858 | -0.0143 |
| 4 | 684 | 0.0345 | -0.0829 | 0.0032 |
| 5 | 684 | 0.0864 | 0.0110 | 0.1124 |
| 6 | 684 | 0.1402 | 0.0642 | 0.1805 |
| 7 | 684 | 0.2073 | 0.1388 | 0.3355 |
| 8 | 684 | 0.3032 | 0.2431 | 0.5345 |
| 9 | 679 | 0.5499 | 0.5392 | 1.5425 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4286 | 0.4971 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.4286 | 0.1564 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.6286 | 0.4488 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.4286 | 0.3392 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.4286 | 0.2412 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.4286 | 0.3450 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 7.3617 |
| 25 | 0.6800 | 0.6400 | 5.5493 |
| 50 | 0.6600 | 0.6200 | 4.2160 |
| 100 | 0.6600 | 0.6300 | 3.6340 |


## Notes

- The conformal 80% interval radius (0.5868) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5574, MAE 0.3847, Spearman IC 0.5238.
