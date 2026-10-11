# Forward Test — Forecast model (ridge10000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 10000000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5069** |
| RMSE (partial-period log-ratio) | 0.5658 |
| MAE (partial-period log-ratio) | 0.4420 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.3893 vs -0.3125 |
| 80% conformal interval partial-horizon coverage (radius 0.6578) | 0.7979 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | 0.1315 | -0.4386 | -0.3125 |
| 1 | 723 | 0.1582 | -0.3153 | -0.2319 |
| 2 | 723 | 0.1735 | -0.2181 | -0.1438 |
| 3 | 723 | 0.1864 | -0.1648 | -0.0875 |
| 4 | 723 | 0.1980 | -0.0659 | 0.0015 |
| 5 | 723 | 0.2090 | -0.0252 | 0.0875 |
| 6 | 723 | 0.2204 | 0.0349 | 0.1539 |
| 7 | 723 | 0.2337 | 0.1372 | 0.3285 |
| 8 | 723 | 0.2506 | 0.2686 | 0.6131 |
| 9 | 723 | 0.2734 | 0.5041 | 1.3893 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5048 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1729 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4813 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3610 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2683 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3596 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 2.6875 |
| 25 | 0.6800 | 0.6800 | 5.6657 |
| 50 | 0.6000 | 0.6000 | 3.7439 |
| 100 | 0.6600 | 0.6300 | 3.0604 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3882** |
| RMSE (partial-period log-ratio) | 0.6718 |
| MAE (partial-period log-ratio) | 0.4823 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.6325 vs 0.1364 |
| 80% conformal interval partial-horizon coverage (radius 0.6578) | 0.7933 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.2052 | 0.0592 | 0.1364 |
| 1 | 21 | 0.2206 | -0.1391 | -0.0762 |
| 2 | 21 | 0.2322 | -0.0072 | 0.0411 |
| 3 | 21 | 0.2417 | 0.2435 | 0.4866 |
| 4 | 21 | 0.2482 | 0.2484 | 0.6224 |
| 5 | 21 | 0.2540 | 0.3164 | 1.9401 |
| 6 | 21 | 0.2599 | 0.3025 | 0.5937 |
| 7 | 21 | 0.2667 | 0.7103 | 1.8851 |
| 8 | 21 | 0.2740 | 0.5706 | 1.2206 |
| 9 | 19 | 0.2822 | 0.7976 | 2.6325 |

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
| 10 | 0.6000 | 0.6000 | 1.1217 |
| 25 | 0.6000 | 0.5600 | 2.0993 |
| 50 | 0.6600 | 0.6200 | 1.7631 |
| 100 | 0.5600 | 0.5100 | 1.6812 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4915** |
| RMSE (partial-period log-ratio) | 0.5489 |
| MAE (partial-period log-ratio) | 0.4241 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4443 vs -0.2852 |
| 80% conformal interval partial-horizon coverage (radius 0.6578) | 0.8176 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | 0.1342 | -0.3971 | -0.2852 |
| 1 | 684 | 0.1604 | -0.2715 | -0.1984 |
| 2 | 684 | 0.1760 | -0.1823 | -0.1186 |
| 3 | 684 | 0.1887 | -0.1145 | -0.0434 |
| 4 | 684 | 0.2000 | -0.0334 | 0.0351 |
| 5 | 684 | 0.2111 | 0.0149 | 0.1344 |
| 6 | 684 | 0.2222 | 0.0501 | 0.1643 |
| 7 | 684 | 0.2355 | 0.1720 | 0.3857 |
| 8 | 684 | 0.2521 | 0.2803 | 0.6345 |
| 9 | 679 | 0.2741 | 0.5216 | 1.4443 |

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
| 10 | 0.5000 | 0.5000 | 2.6875 |
| 25 | 0.6800 | 0.6800 | 5.6657 |
| 50 | 0.6000 | 0.6000 | 3.7439 |
| 100 | 0.6600 | 0.6300 | 3.0604 |


## Notes

- The conformal 80% interval radius (0.6578) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.6384, MAE 0.4503, Spearman IC 0.4889.
