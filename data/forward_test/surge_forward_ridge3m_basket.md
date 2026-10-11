# Forward Test — Surge model (surge_ridge3m_basket)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 3, dataset 1, feature-set 1, calibration 1, ridge 3000000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (809 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 337 days (min 52, max 1053); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=3, calibration_version=1, seed=42

## Full candidate pool (n = 809)

### Headline metrics

| metric | value |
|---|---|
| n | 809 |
| AUC (model score vs partial-window surge label) | **0.5401** |
| partial surge rate (peak >= 1.5x entry within window) | 0.4351 |
| hit rate (realized partial return >= +50%) | 0.4227 |
| mean realized partial return | 1.0804 |
| top-decile vs bottom-decile mean realized return | 0.3617 vs 1.0856 |
| top-decile vs bottom-decile median realized return | 0.1250 vs 0.0769 |
| ECE (10 bins) | 0.1030 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 81 | 0.2292 | 0.2963 | 1.0856 |
| 1 | 81 | 0.2783 | 0.3333 | 0.5415 |
| 2 | 81 | 0.3124 | 0.4198 | 1.4494 |
| 3 | 81 | 0.3377 | 0.4568 | 1.0979 |
| 4 | 81 | 0.3593 | 0.4938 | 1.8155 |
| 5 | 81 | 0.3824 | 0.5185 | 1.1712 |
| 6 | 81 | 0.4047 | 0.4815 | 1.3524 |
| 7 | 81 | 0.4329 | 0.5926 | 1.1278 |
| 8 | 81 | 0.4696 | 0.4198 | 0.7963 |
| 9 | 80 | 0.5382 | 0.3375 | 0.3573 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.1250 | 0.3333 | — | — |
| TM value (cheapest first) | 4.0000 | 1.0000 | -3.8750 | -0.6667 |
| FM CA (highest first) | 0.0000 | 0.2469 | 0.1250 | 0.0864 |
| FM PA (highest first) | 0.3333 | 0.4198 | -0.2083 | -0.0864 |
| age+minutes (young + proven minutes) | 0.1429 | 0.2840 | -0.0179 | 0.0494 |
| random price-bucket (seeded) | 0.2000 | 0.3580 | -0.0750 | -0.0247 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.2000 | 0.1408 |
| 25 | 0.4000 | 0.1600 | 0.0820 |
| 50 | 0.5000 | 0.3200 | 0.3402 |
| 100 | 0.5800 | 0.3600 | 0.5505 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 470)

### Headline metrics

| metric | value |
|---|---|
| n | 470 |
| AUC (model score vs partial-window surge label) | **0.5941** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1979 |
| hit rate (realized partial return >= +50%) | 0.1830 |
| mean realized partial return | 0.1227 |
| top-decile vs bottom-decile mean realized return | 0.1136 vs 0.0056 |
| top-decile vs bottom-decile median realized return | 0.0000 vs 0.0000 |
| ECE (10 bins) | 0.1791 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 47 | 0.2276 | 0.0851 | 0.0056 |
| 1 | 47 | 0.2739 | 0.1064 | 0.0517 |
| 2 | 47 | 0.3106 | 0.1277 | 0.0426 |
| 3 | 47 | 0.3383 | 0.1915 | 0.0850 |
| 4 | 47 | 0.3622 | 0.2340 | 0.1459 |
| 5 | 47 | 0.3861 | 0.2766 | 0.1937 |
| 6 | 47 | 0.4081 | 0.2340 | 0.1263 |
| 7 | 47 | 0.4392 | 0.2766 | 0.2261 |
| 8 | 47 | 0.4763 | 0.2553 | 0.2360 |
| 9 | 47 | 0.5472 | 0.1915 | 0.1136 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.1702 | — | — |
| TM value (cheapest first) | 0.2500 | 0.4255 | -0.2500 | -0.2553 |
| FM CA (highest first) | 0.0000 | 0.1277 | 0.0000 | 0.0426 |
| FM PA (highest first) | 0.0000 | 0.1489 | 0.0000 | 0.0213 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1489 | 0.0000 | 0.0213 |
| random price-bucket (seeded) | 0.0000 | 0.1489 | 0.0000 | 0.0213 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.3000 | 0.2000 | 0.1167 |
| 25 | 0.3600 | 0.1600 | 0.1060 |
| 50 | 0.4200 | 0.1800 | 0.1263 |
| 100 | 0.4900 | 0.2000 | 0.1745 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 337 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7533, AUC without FM features 0.7464.
