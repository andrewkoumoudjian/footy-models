# Forward Test — Forecast model (ridge3000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 10, dataset 1, feature-set 1, calibration 1, ridge 3000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=10, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4614** |
| RMSE (partial-period log-ratio) | 0.4854 |
| MAE (partial-period log-ratio) | 0.3567 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4537 vs -0.2668 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.8140 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2506 | -0.3657 | -0.2668 |
| 1 | 723 | -0.1312 | -0.2799 | -0.1917 |
| 2 | 723 | -0.0654 | -0.2209 | -0.1384 |
| 3 | 723 | -0.0109 | -0.1536 | -0.0644 |
| 4 | 723 | 0.0409 | -0.0979 | -0.0138 |
| 5 | 723 | 0.0901 | -0.0607 | 0.0374 |
| 6 | 723 | 0.1436 | 0.0349 | 0.1590 |
| 7 | 723 | 0.2112 | 0.0947 | 0.2595 |
| 8 | 723 | 0.3101 | 0.2422 | 0.5636 |
| 9 | 723 | 0.5636 | 0.5240 | 1.4537 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5007 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1687 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4772 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3568 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2642 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3555 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.5617 |
| 25 | 0.7200 | 0.6800 | 5.8890 |
| 50 | 0.7000 | 0.6600 | 4.5776 |
| 100 | 0.6500 | 0.6300 | 3.7032 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3977** |
| RMSE (partial-period log-ratio) | 0.6247 |
| MAE (partial-period log-ratio) | 0.4461 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9426 vs -0.1133 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.7596 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | -0.0390 | -0.1863 | -0.1133 |
| 1 | 21 | 0.0659 | 0.0573 | 0.1424 |
| 2 | 21 | 0.1620 | 0.0417 | 0.0862 |
| 3 | 21 | 0.2175 | 0.2309 | 0.3737 |
| 4 | 21 | 0.2692 | 0.3545 | 1.0124 |
| 5 | 21 | 0.3205 | 0.2192 | 0.4311 |
| 6 | 21 | 0.3999 | 0.5082 | 0.9489 |
| 7 | 21 | 0.4867 | 0.6045 | 2.4357 |
| 8 | 21 | 0.5930 | 0.5580 | 2.1569 |
| 9 | 19 | 0.7799 | 0.7053 | 1.9426 |

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
| 10 | 0.8000 | 0.8000 | 3.1467 |
| 25 | 0.5600 | 0.5600 | 1.6364 |
| 50 | 0.5400 | 0.5000 | 2.0718 |
| 100 | 0.5700 | 0.4900 | 1.6125 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4572** |
| RMSE (partial-period log-ratio) | 0.4755 |
| MAE (partial-period log-ratio) | 0.3456 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5210 vs -0.2483 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.8269 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2427 | -0.3384 | -0.2483 |
| 1 | 684 | -0.1243 | -0.2373 | -0.1594 |
| 2 | 684 | -0.0591 | -0.1864 | -0.1083 |
| 3 | 684 | -0.0047 | -0.1157 | -0.0332 |
| 4 | 684 | 0.0474 | -0.0654 | 0.0172 |
| 5 | 684 | 0.0970 | -0.0089 | 0.0855 |
| 6 | 684 | 0.1517 | 0.0554 | 0.1759 |
| 7 | 684 | 0.2201 | 0.1274 | 0.3159 |
| 8 | 684 | 0.3205 | 0.2636 | 0.5868 |
| 9 | 679 | 0.5744 | 0.5459 | 1.5210 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5132 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1725 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4649 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3553 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2573 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3611 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.5617 |
| 25 | 0.7200 | 0.6800 | 5.8890 |
| 50 | 0.7000 | 0.6600 | 4.5776 |
| 100 | 0.6500 | 0.6300 | 3.7032 |


## Notes

- The conformal 80% interval radius (0.5910) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5556, MAE 0.3871, Spearman IC 0.5290.
