# Forward Test — Forecast model (ridge10000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 10000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4712** |
| RMSE (partial-period log-ratio) | 0.4856 |
| MAE (partial-period log-ratio) | 0.3589 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.5096 vs -0.2620 |
| 80% conformal interval partial-horizon coverage (radius 0.5927) | 0.8134 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2444 | -0.3602 | -0.2620 |
| 1 | 723 | -0.1260 | -0.2967 | -0.2075 |
| 2 | 723 | -0.0600 | -0.2317 | -0.1505 |
| 3 | 723 | -0.0042 | -0.1497 | -0.0661 |
| 4 | 723 | 0.0496 | -0.1021 | -0.0195 |
| 5 | 723 | 0.1012 | -0.0643 | 0.0302 |
| 6 | 723 | 0.1576 | 0.0375 | 0.1634 |
| 7 | 723 | 0.2268 | 0.0967 | 0.2649 |
| 8 | 723 | 0.3287 | 0.2425 | 0.5354 |
| 9 | 723 | 0.5864 | 0.5451 | 1.5096 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5118 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1798 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4882 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3679 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2752 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3665 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 6.1167 |
| 25 | 0.7200 | 0.6800 | 6.1023 |
| 50 | 0.6600 | 0.6200 | 4.4913 |
| 100 | 0.6400 | 0.6200 | 3.6447 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4135** |
| RMSE (partial-period log-ratio) | 0.6232 |
| MAE (partial-period log-ratio) | 0.4555 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9388 vs -0.1109 |
| 80% conformal interval partial-horizon coverage (radius 0.5927) | 0.7404 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.0064 | -0.1841 | -0.1109 |
| 1 | 21 | 0.1103 | 0.0551 | 0.1401 |
| 2 | 21 | 0.1986 | 0.0925 | 0.1617 |
| 3 | 21 | 0.2561 | 0.0735 | 0.1970 |
| 4 | 21 | 0.3094 | 0.3525 | 0.8266 |
| 5 | 21 | 0.3620 | 0.2649 | 0.6745 |
| 6 | 21 | 0.4395 | 0.5526 | 0.9648 |
| 7 | 21 | 0.5221 | 0.4420 | 1.5029 |
| 8 | 21 | 0.6296 | 0.7231 | 3.1209 |
| 9 | 19 | 0.7932 | 0.7230 | 1.9388 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.8000 | 0.5238 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 0.8000 | 0.0952 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.4667 | 0.1905 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.6750 | 0.2857 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 0.8000 | 0.2857 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 0.8000 | 0.1429 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 3.1217 |
| 25 | 0.5600 | 0.5600 | 1.6364 |
| 50 | 0.5800 | 0.5600 | 2.5761 |
| 100 | 0.6000 | 0.5100 | 1.6792 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4648** |
| RMSE (partial-period log-ratio) | 0.4759 |
| MAE (partial-period log-ratio) | 0.3479 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5672 vs -0.2410 |
| 80% conformal interval partial-horizon coverage (radius 0.5927) | 0.8268 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2361 | -0.3316 | -0.2410 |
| 1 | 684 | -0.1187 | -0.2566 | -0.1798 |
| 2 | 684 | -0.0528 | -0.1871 | -0.1070 |
| 3 | 684 | 0.0031 | -0.1110 | -0.0388 |
| 4 | 684 | 0.0571 | -0.0651 | 0.0208 |
| 5 | 684 | 0.1089 | -0.0202 | 0.0738 |
| 6 | 684 | 0.1661 | 0.0570 | 0.1766 |
| 7 | 684 | 0.2363 | 0.1258 | 0.3003 |
| 8 | 684 | 0.3399 | 0.2685 | 0.5815 |
| 9 | 679 | 0.5972 | 0.5606 | 1.5672 |

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
| 10 | 0.6000 | 0.6000 | 6.1167 |
| 25 | 0.7200 | 0.6800 | 6.1023 |
| 50 | 0.6600 | 0.6200 | 4.4913 |
| 100 | 0.6400 | 0.6200 | 3.6447 |


## Notes

- The conformal 80% interval radius (0.5927) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5565, MAE 0.3881, Spearman IC 0.5281.
