# Forward Test — Forecast model (no_fm_ridge1000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 21, dataset 1, feature-set 2, calibration 1, ridge 1000000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=2, model_version=21, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5175** |
| RMSE (partial-period log-ratio) | 0.5080 |
| MAE (partial-period log-ratio) | 0.3857 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.3844 vs -0.3238 |
| 80% conformal interval partial-horizon coverage (radius 0.6017) | 0.8093 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.1349 | -0.4510 | -0.3238 |
| 1 | 723 | -0.0319 | -0.3242 | -0.2429 |
| 2 | 723 | 0.0320 | -0.2174 | -0.1453 |
| 3 | 723 | 0.0863 | -0.1585 | -0.0850 |
| 4 | 723 | 0.1344 | -0.0659 | 0.0071 |
| 5 | 723 | 0.1836 | -0.0223 | 0.0717 |
| 6 | 723 | 0.2350 | 0.0222 | 0.1228 |
| 7 | 723 | 0.2952 | 0.1555 | 0.3480 |
| 8 | 723 | 0.3747 | 0.2693 | 0.6611 |
| 9 | 723 | 0.4976 | 0.5094 | 1.3844 |

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
| 10 | 0.9000 | 0.9000 | 9.5500 |
| 25 | 0.6800 | 0.6800 | 6.3400 |
| 50 | 0.6800 | 0.6800 | 4.2487 |
| 100 | 0.6700 | 0.6600 | 3.3940 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3634** |
| RMSE (partial-period log-ratio) | 0.6500 |
| MAE (partial-period log-ratio) | 0.4943 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.7254 vs 0.1593 |
| 80% conformal interval partial-horizon coverage (radius 0.6017) | 0.7644 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1793 | 0.0798 | 0.1593 |
| 1 | 21 | 0.2523 | -0.1848 | -0.1150 |
| 2 | 21 | 0.2950 | 0.0927 | 0.1468 |
| 3 | 21 | 0.3353 | 0.2263 | 0.5137 |
| 4 | 21 | 0.3657 | 0.1217 | 0.4113 |
| 5 | 21 | 0.4047 | 0.5232 | 2.3093 |
| 6 | 21 | 0.4425 | 0.4650 | 1.3980 |
| 7 | 21 | 0.4741 | 0.4486 | 0.8605 |
| 8 | 21 | 0.5032 | 0.4914 | 1.0820 |
| 9 | 19 | 0.5444 | 0.8425 | 2.7254 |

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
| 10 | 0.7000 | 0.7000 | 4.0017 |
| 25 | 0.6000 | 0.6000 | 2.1413 |
| 50 | 0.5800 | 0.5600 | 1.7368 |
| 100 | 0.5600 | 0.5100 | 1.6776 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4988** |
| RMSE (partial-period log-ratio) | 0.4974 |
| MAE (partial-period log-ratio) | 0.3738 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4343 vs -0.2950 |
| 80% conformal interval partial-horizon coverage (radius 0.6017) | 0.8240 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.1229 | -0.4072 | -0.2950 |
| 1 | 684 | -0.0213 | -0.2865 | -0.2145 |
| 2 | 684 | 0.0429 | -0.1567 | -0.0969 |
| 3 | 684 | 0.0967 | -0.1180 | -0.0478 |
| 4 | 684 | 0.1446 | -0.0246 | 0.0444 |
| 5 | 684 | 0.1930 | -0.0013 | 0.0973 |
| 6 | 684 | 0.2444 | 0.0545 | 0.1673 |
| 7 | 684 | 0.3036 | 0.1664 | 0.3597 |
| 8 | 684 | 0.3829 | 0.2898 | 0.7037 |
| 9 | 679 | 0.5015 | 0.5237 | 1.4343 |

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
| 10 | 0.9000 | 0.9000 | 9.5500 |
| 25 | 0.6800 | 0.6800 | 6.3400 |
| 50 | 0.6800 | 0.6800 | 4.2487 |
| 100 | 0.6700 | 0.6600 | 3.3940 |


## Notes

- The conformal 80% interval radius (0.6017) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5936, MAE 0.4131, Spearman IC 0.4941.
