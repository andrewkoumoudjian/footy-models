# Forward Test — Forecast model (ridge10)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 20, dataset 1, feature-set 1, calibration 1, ridge 10, seed 42; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player (7230 players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11; the anchor-to-end window is 218 days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=20, calibration_version=1, seed=42

## Full pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4505** |
| RMSE (partial-period log-ratio) | 0.4862 |
| MAE (partial-period log-ratio) | 0.3561 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4484 vs -0.2687 |
| 80% conformal interval partial-horizon coverage (radius 0.5900) | 0.8124 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2708 | -0.3689 | -0.2687 |
| 1 | 723 | -0.1425 | -0.2789 | -0.1859 |
| 2 | 723 | -0.0721 | -0.2085 | -0.1276 |
| 3 | 723 | -0.0162 | -0.1281 | -0.0430 |
| 4 | 723 | 0.0373 | -0.1053 | -0.0170 |
| 5 | 723 | 0.0842 | -0.0490 | 0.0578 |
| 6 | 723 | 0.1377 | 0.0304 | 0.1600 |
| 7 | 723 | 0.2028 | 0.0930 | 0.2618 |
| 8 | 723 | 0.2990 | 0.2151 | 0.5123 |
| 9 | 723 | 0.5508 | 0.5172 | 1.4484 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4286 | 0.4924 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.4286 | 0.1604 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.6786 | 0.4689 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.5397 | 0.3485 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.4286 | 0.2559 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5119 | 0.3472 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.3617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.4740 |
| 100 | 0.6500 | 0.6200 | 3.7249 |


## Deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db, derived}) (n = 208)

### Headline metrics

| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3841** |
| RMSE (partial-period log-ratio) | 0.6290 |
| MAE (partial-period log-ratio) | 0.4416 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.8336 vs 0.0436 |
| 80% conformal interval partial-horizon coverage (radius 0.5900) | 0.7740 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 21 | -0.0821 | -0.0649 | 0.0436 |
| 1 | 21 | 0.0299 | -0.0998 | -0.0745 |
| 2 | 21 | 0.1225 | 0.0986 | 0.2315 |
| 3 | 21 | 0.1855 | 0.1971 | 0.2727 |
| 4 | 21 | 0.2341 | 0.3483 | 0.7781 |
| 5 | 21 | 0.2852 | 0.2829 | 0.7948 |
| 6 | 21 | 0.3644 | 0.6937 | 1.9292 |
| 7 | 21 | 0.4519 | 0.4644 | 1.5266 |
| 8 | 21 | 0.5618 | 0.4964 | 2.0706 |
| 9 | 19 | 0.7849 | 0.6735 | 1.8336 |

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
| 10 | 0.8000 | 0.8000 | 2.9267 |
| 25 | 0.6000 | 0.6000 | 2.8764 |
| 50 | 0.5400 | 0.5000 | 2.0851 |
| 100 | 0.5600 | 0.4900 | 1.6092 |


## MV-band restricted (10k <= mv <= 1M, any age) (n = 6835)

### Headline metrics

| metric | value |
|---|---|
| n | 6835 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4488** |
| RMSE (partial-period log-ratio) | 0.4761 |
| MAE (partial-period log-ratio) | 0.3449 |
| hit rate (realized log-ratio > 0) | 0.3187 |
| hit rate (realized partial return >= +50%) | 0.1699 |
| mean realized partial return | 0.2144 |
| top-decile vs bottom-decile realized return | 1.4930 vs -0.2484 |
| 80% conformal interval partial-horizon coverage (radius 0.5900) | 0.8259 |

### Decile table (ranked by predicted 12m log-ratio; realized = partial window)

| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 684 | -0.2639 | -0.3395 | -0.2484 |
| 1 | 684 | -0.1363 | -0.2375 | -0.1533 |
| 2 | 684 | -0.0660 | -0.1801 | -0.1046 |
| 3 | 684 | -0.0105 | -0.1002 | -0.0211 |
| 4 | 684 | 0.0433 | -0.0573 | 0.0252 |
| 5 | 684 | 0.0903 | -0.0139 | 0.0896 |
| 6 | 684 | 0.1455 | 0.0631 | 0.1950 |
| 7 | 684 | 0.2107 | 0.1358 | 0.3189 |
| 8 | 684 | 0.3093 | 0.2409 | 0.5587 |
| 9 | 679 | 0.5612 | 0.5288 | 1.4930 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4365 | 0.4985 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3406 | 0.4365 | 0.1579 |
| FM CA (highest first) | -0.2000 | 0.0482 | 0.6365 | 0.4503 |
| FM PA (highest first) | 0.0000 | 0.1579 | 0.4365 | 0.3406 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2558 | 0.4365 | 0.2427 |
| random price-bucket (seeded) | 0.0000 | 0.1520 | 0.4365 | 0.3465 |

### Top-N realized performance (by predicted log-ratio)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.8000 | 6.3617 |
| 25 | 0.7200 | 0.6800 | 4.4890 |
| 50 | 0.6600 | 0.6200 | 4.4740 |
| 100 | 0.6500 | 0.6200 | 3.7249 |


## Notes

- The conformal 80% interval radius (0.5900) was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).
- Walk-forward (pooled OOF, same config): RMSE 0.5556, MAE 0.3870, Spearman IC 0.5284.
