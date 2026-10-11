# Forward Test — Forecast model (ridge30000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 30000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4814** |
| RMSE (partial-period log-ratio) | 0.4874 |
| MAE (partial-period log-ratio) | 0.3627 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.5122 vs -0.2632 |
| 80% conformal interval partial-horizon coverage (radius 0.5948) | 0.8109 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2392 | -0.3645 | -0.2632 |
| 1 | 723 | -0.1213 | -0.3161 | -0.2245 |
| 2 | 723 | -0.0521 | -0.2196 | -0.1424 |
| 3 | 723 | 0.0052 | -0.1591 | -0.0731 |
| 4 | 723 | 0.0626 | -0.1031 | -0.0245 |
| 5 | 723 | 0.1189 | -0.0450 | 0.0481 |
| 6 | 723 | 0.1766 | 0.0114 | 0.1202 |
| 7 | 723 | 0.2485 | 0.1135 | 0.2980 |
| 8 | 723 | 0.3529 | 0.2545 | 0.5472 |
| 9 | 723 | 0.6097 | 0.5451 | 1.5122 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5145 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1826 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4910 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3707 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2780 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3693 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 6.9500 |
| 25 | 0.7600 | 0.7200 | 5.7013 |
| 50 | 0.6200 | 0.6000 | 4.6713 |
| 100 | 0.6500 | 0.6200 | 3.5555 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4166** |
| RMSE (partial-period log-ratio) | 0.6261 |
| MAE (partial-period log-ratio) | 0.4684 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9888 vs -0.0823 |
| 80% conformal interval partial-horizon coverage (radius 0.5948) | 0.6971 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.0597 | -0.1404 | -0.0823 |
| 1 | 21 | 0.1626 | 0.0419 | 0.1353 |
| 2 | 21 | 0.2336 | 0.0110 | 0.0721 |
| 3 | 21 | 0.2948 | 0.2309 | 0.5049 |
| 4 | 21 | 0.3535 | 0.2373 | 0.6136 |
| 5 | 21 | 0.4112 | 0.2724 | 0.5489 |
| 6 | 21 | 0.4686 | 0.4580 | 0.9015 |
| 7 | 21 | 0.5657 | 0.5785 | 1.7262 |
| 8 | 21 | 0.6540 | 0.6638 | 3.0121 |
| 9 | 19 | 0.8043 | 0.7437 | 1.9888 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 1.0000 | 0.5238 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 1.0000 | 0.0952 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.6667 | 0.1905 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.8750 | 0.2857 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 1.0000 | 0.2857 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 1.0000 | 0.1429 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.7000 | 3.0217 |
| 25 | 0.5600 | 0.5600 | 1.6635 |
| 50 | 0.6000 | 0.6000 | 2.6475 |
| 100 | 0.6100 | 0.5100 | 1.6742 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4726** |
| RMSE (partial-period log-ratio) | 0.4777 |
| MAE (partial-period log-ratio) | 0.3519 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5729 vs -0.2382 |
| 80% conformal interval partial-horizon coverage (radius 0.5948) | 0.8238 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2305 | -0.3284 | -0.2382 |
| 1 | 684 | -0.1130 | -0.2732 | -0.1933 |
| 2 | 684 | -0.0438 | -0.1870 | -0.1145 |
| 3 | 684 | 0.0133 | -0.1074 | -0.0289 |
| 4 | 684 | 0.0719 | -0.0733 | 0.0051 |
| 5 | 684 | 0.1275 | -0.0149 | 0.0717 |
| 6 | 684 | 0.1860 | 0.0453 | 0.1662 |
| 7 | 684 | 0.2586 | 0.1326 | 0.3081 |
| 8 | 684 | 0.3646 | 0.2842 | 0.6044 |
| 9 | 679 | 0.6202 | 0.5625 | 1.5729 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5278 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1871 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4795 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3699 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2719 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3757 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.5000 | 6.9500 |
| 25 | 0.7600 | 0.7200 | 5.7013 |
| 50 | 0.6200 | 0.6000 | 4.6713 |
| 100 | 0.6500 | 0.6200 | 3.5555 |


## Notes

- The conformal 80% interval radius (0.5948) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5583, MAE 0.3895, Spearman IC 0.5262.
