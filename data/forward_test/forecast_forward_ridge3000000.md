# Forward Test — Forecast model (ridge3000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 3000000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5090** |
| RMSE (partial-period log-ratio) | 0.5433 |
| MAE (partial-period log-ratio) | 0.4221 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4083 vs -0.3115 |
| 80% conformal interval partial-horizon coverage (radius 0.6294) | 0.7960 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | 0.0302 | -0.4364 | -0.3115 |
| 1 | 723 | 0.0913 | -0.3196 | -0.2365 |
| 2 | 723 | 0.1273 | -0.2191 | -0.1423 |
| 3 | 723 | 0.1578 | -0.1624 | -0.0948 |
| 4 | 723 | 0.1850 | -0.0686 | 0.0092 |
| 5 | 723 | 0.2111 | -0.0228 | 0.0795 |
| 6 | 723 | 0.2382 | 0.0297 | 0.1520 |
| 7 | 723 | 0.2706 | 0.1448 | 0.3379 |
| 8 | 723 | 0.3120 | 0.2555 | 0.5961 |
| 9 | 723 | 0.3715 | 0.5160 | 1.4083 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5104 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1784 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4869 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3665 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2739 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3651 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 9.4500 |
| 25 | 0.7200 | 0.7200 | 5.7267 |
| 50 | 0.6400 | 0.6200 | 4.1812 |
| 100 | 0.6300 | 0.6100 | 3.0955 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3898** |
| RMSE (partial-period log-ratio) | 0.6579 |
| MAE (partial-period log-ratio) | 0.4851 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.8675 vs 0.1266 |
| 80% conformal interval partial-horizon coverage (radius 0.6294) | 0.7788 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.2074 | 0.0445 | 0.1266 |
| 1 | 21 | 0.2414 | -0.1557 | -0.0921 |
| 2 | 21 | 0.2693 | 0.0795 | 0.1716 |
| 3 | 21 | 0.2902 | 0.1794 | 0.3739 |
| 4 | 21 | 0.3072 | 0.2464 | 0.6184 |
| 5 | 21 | 0.3239 | 0.3308 | 0.7497 |
| 6 | 21 | 0.3382 | 0.4157 | 1.9642 |
| 7 | 21 | 0.3574 | 0.6168 | 1.7137 |
| 8 | 21 | 0.3746 | 0.6287 | 1.9161 |
| 9 | 19 | 0.3959 | 0.7074 | 1.8675 |

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
| 10 | 0.5000 | 0.5000 | 0.9283 |
| 25 | 0.6400 | 0.6000 | 2.3993 |
| 50 | 0.6600 | 0.6200 | 1.7864 |
| 100 | 0.5700 | 0.5100 | 1.6837 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4943** |
| RMSE (partial-period log-ratio) | 0.5280 |
| MAE (partial-period log-ratio) | 0.4059 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4559 vs -0.2856 |
| 80% conformal interval partial-horizon coverage (radius 0.6294) | 0.8149 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | 0.0363 | -0.3966 | -0.2856 |
| 1 | 684 | 0.0964 | -0.2788 | -0.2036 |
| 2 | 684 | 0.1331 | -0.1777 | -0.1131 |
| 3 | 684 | 0.1630 | -0.1171 | -0.0496 |
| 4 | 684 | 0.1899 | -0.0367 | 0.0320 |
| 5 | 684 | 0.2159 | 0.0207 | 0.1400 |
| 6 | 684 | 0.2427 | 0.0632 | 0.1781 |
| 7 | 684 | 0.2749 | 0.1476 | 0.3464 |
| 8 | 684 | 0.3159 | 0.2859 | 0.6523 |
| 9 | 679 | 0.3735 | 0.5298 | 1.4559 |

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
| 10 | 0.8000 | 0.8000 | 9.4500 |
| 25 | 0.7200 | 0.7200 | 5.7267 |
| 50 | 0.6400 | 0.6200 | 4.1812 |
| 100 | 0.6300 | 0.6100 | 3.0955 |


## Notes

- The conformal 80% interval radius (0.6294) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.6158, MAE 0.4322, Spearman IC 0.4978.
