# Forward Test — Forecast model (ridge300000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 300000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5045** |
| RMSE (partial-period log-ratio) | 0.4988 |
| MAE (partial-period log-ratio) | 0.3766 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4963 vs -0.2995 |
| 80% conformal interval partial-horizon coverage (radius 0.5956) | 0.8014 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.1790 | -0.4199 | -0.2995 |
| 1 | 723 | -0.0717 | -0.3163 | -0.2329 |
| 2 | 723 | -0.0045 | -0.2338 | -0.1651 |
| 3 | 723 | 0.0569 | -0.1406 | -0.0608 |
| 4 | 723 | 0.1109 | -0.0897 | -0.0153 |
| 5 | 723 | 0.1660 | -0.0402 | 0.0438 |
| 6 | 723 | 0.2237 | 0.0345 | 0.1649 |
| 7 | 723 | 0.2898 | 0.1277 | 0.3047 |
| 8 | 723 | 0.3856 | 0.2526 | 0.5618 |
| 9 | 723 | 0.5743 | 0.5427 | 1.4963 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5159 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1840 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4924 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3721 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2794 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3707 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 7.2000 |
| 25 | 0.6800 | 0.6400 | 7.1267 |
| 50 | 0.6800 | 0.6600 | 4.7173 |
| 100 | 0.7100 | 0.6900 | 3.6067 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4043** |
| RMSE (partial-period log-ratio) | 0.6395 |
| MAE (partial-period log-ratio) | 0.4890 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 2.1643 vs -0.0621 |
| 80% conformal interval partial-horizon coverage (radius 0.5956) | 0.7115 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1696 | -0.1204 | -0.0621 |
| 1 | 21 | 0.2385 | 0.0779 | 0.1667 |
| 2 | 21 | 0.2906 | -0.0370 | 0.0555 |
| 3 | 21 | 0.3422 | 0.1409 | 0.2323 |
| 4 | 21 | 0.3829 | 0.4015 | 0.9300 |
| 5 | 21 | 0.4344 | 0.2059 | 0.4816 |
| 6 | 21 | 0.4872 | 0.5456 | 1.0060 |
| 7 | 21 | 0.5398 | 0.2870 | 1.7209 |
| 8 | 21 | 0.6012 | 0.8051 | 2.7429 |
| 9 | 19 | 0.6842 | 0.7953 | 2.1643 |

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
| 10 | 0.6000 | 0.6000 | 1.5217 |
| 25 | 0.6000 | 0.6000 | 1.8715 |
| 50 | 0.6000 | 0.5800 | 2.0259 |
| 100 | 0.5600 | 0.5000 | 1.6520 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4923** |
| RMSE (partial-period log-ratio) | 0.4879 |
| MAE (partial-period log-ratio) | 0.3648 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5173 vs -0.2729 |
| 80% conformal interval partial-horizon coverage (radius 0.5956) | 0.8162 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.1685 | -0.3818 | -0.2729 |
| 1 | 684 | -0.0625 | -0.2757 | -0.2020 |
| 2 | 684 | 0.0053 | -0.1883 | -0.1270 |
| 3 | 684 | 0.0665 | -0.0970 | -0.0242 |
| 4 | 684 | 0.1204 | -0.0530 | 0.0164 |
| 5 | 684 | 0.1757 | -0.0069 | 0.0875 |
| 6 | 684 | 0.2331 | 0.0701 | 0.1995 |
| 7 | 684 | 0.2992 | 0.1406 | 0.3307 |
| 8 | 684 | 0.3958 | 0.2870 | 0.6279 |
| 9 | 679 | 0.5813 | 0.5454 | 1.5173 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5175 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1769 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4693 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3596 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2617 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3655 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 7.2000 |
| 25 | 0.6800 | 0.6400 | 7.1267 |
| 50 | 0.6800 | 0.6600 | 4.7173 |
| 100 | 0.7100 | 0.6900 | 3.6067 |


## Notes

- The conformal 80% interval radius (0.5956) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5711, MAE 0.3977, Spearman IC 0.5158.
