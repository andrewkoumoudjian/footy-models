# Forward Test — Forecast model (ridge100000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 100000, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4939** |
| RMSE (partial-period log-ratio) | 0.4909 |
| MAE (partial-period log-ratio) | 0.3676 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.5224 vs -0.2798 |
| 80% conformal interval partial-horizon coverage (radius 0.5969) | 0.8108 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2202 | -0.3898 | -0.2798 |
| 1 | 723 | -0.1069 | -0.3238 | -0.2347 |
| 2 | 723 | -0.0362 | -0.2177 | -0.1435 |
| 3 | 723 | 0.0235 | -0.1542 | -0.0766 |
| 4 | 723 | 0.0822 | -0.0968 | -0.0134 |
| 5 | 723 | 0.1404 | -0.0455 | 0.0445 |
| 6 | 723 | 0.2008 | 0.0194 | 0.1373 |
| 7 | 723 | 0.2713 | 0.1263 | 0.3123 |
| 8 | 723 | 0.3751 | 0.2487 | 0.5294 |
| 9 | 723 | 0.6110 | 0.5505 | 1.5224 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5214 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1895 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4979 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3776 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2849 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3762 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.7000 | 8.0000 |
| 25 | 0.7200 | 0.6800 | 5.7747 |
| 50 | 0.6400 | 0.6200 | 4.6767 |
| 100 | 0.6600 | 0.6400 | 3.5080 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4157** |
| RMSE (partial-period log-ratio) | 0.6326 |
| MAE (partial-period log-ratio) | 0.4818 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9888 vs -0.0852 |
| 80% conformal interval partial-horizon coverage (radius 0.5969) | 0.7115 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | 0.1183 | -0.1430 | -0.0852 |
| 1 | 21 | 0.2094 | 0.1144 | 0.2255 |
| 2 | 21 | 0.2675 | -0.0616 | 0.0046 |
| 3 | 21 | 0.3238 | 0.1031 | 0.2305 |
| 4 | 21 | 0.3803 | 0.4087 | 0.9119 |
| 5 | 21 | 0.4393 | 0.2578 | 0.5365 |
| 6 | 21 | 0.4900 | 0.4177 | 0.8583 |
| 7 | 21 | 0.5750 | 0.5824 | 2.8256 |
| 8 | 21 | 0.6488 | 0.6737 | 1.9246 |
| 9 | 19 | 0.7737 | 0.7437 | 1.9888 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 1.0000 | 0.5714 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 1.0000 | 0.1429 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.6667 | 0.2381 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.8750 | 0.3333 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 1.0000 | 0.3333 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 1.0000 | 0.1905 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.7000 | 3.0217 |
| 25 | 0.6000 | 0.6000 | 1.8022 |
| 50 | 0.6200 | 0.6000 | 2.6559 |
| 100 | 0.5900 | 0.5100 | 1.6650 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4831** |
| RMSE (partial-period log-ratio) | 0.4810 |
| MAE (partial-period log-ratio) | 0.3567 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.5591 vs -0.2503 |
| 80% conformal interval partial-horizon coverage (radius 0.5969) | 0.8247 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2102 | -0.3485 | -0.2503 |
| 1 | 684 | -0.0979 | -0.2842 | -0.2070 |
| 2 | 684 | -0.0270 | -0.1787 | -0.1144 |
| 3 | 684 | 0.0326 | -0.1142 | -0.0389 |
| 4 | 684 | 0.0922 | -0.0619 | 0.0116 |
| 5 | 684 | 0.1501 | -0.0081 | 0.0860 |
| 6 | 684 | 0.2104 | 0.0495 | 0.1689 |
| 7 | 684 | 0.2812 | 0.1494 | 0.3542 |
| 8 | 684 | 0.3867 | 0.2789 | 0.5844 |
| 9 | 679 | 0.6204 | 0.5582 | 1.5591 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5190 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.5000 | 0.1784 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.7000 | 0.4708 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.5000 | 0.3611 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.5000 | 0.2632 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.5000 | 0.3670 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.7000 | 8.0000 |
| 25 | 0.7200 | 0.6800 | 5.7747 |
| 50 | 0.6400 | 0.6200 | 4.6767 |
| 100 | 0.6600 | 0.6400 | 3.5080 |


## Notes

- The conformal 80% interval radius (0.5969) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5624, MAE 0.3920, Spearman IC 0.5223.
