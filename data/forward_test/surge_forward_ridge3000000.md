# Forward Test — Surge model (surge_ridge3000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 3000000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.7430** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.7876 vs -0.2277 |
| top-decile vs bottom-decile median realized return | 0.3333 vs -0.2500 |
| ECE (10 bins) | 0.0555 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0823 | 0.0262 | -0.2277 |
| 1 | 343 | 0.1210 | 0.0700 | -0.0917 |
| 2 | 343 | 0.1508 | 0.0729 | -0.0348 |
| 3 | 343 | 0.1781 | 0.1341 | 0.1194 |
| 4 | 343 | 0.2058 | 0.1574 | 0.1284 |
| 5 | 343 | 0.2354 | 0.1953 | 0.1828 |
| 6 | 343 | 0.2717 | 0.2682 | 0.4061 |
| 7 | 343 | 0.3153 | 0.3586 | 0.7260 |
| 8 | 343 | 0.3704 | 0.4402 | 1.1618 |
| 9 | 342 | 0.4676 | 0.4474 | 0.7841 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.3333 | 0.4344 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.4667 | -0.5015 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5833 | 0.4140 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.4333 | 0.2828 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.3333 | 0.1837 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.3333 | 0.2420 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.2000 | 0.1775 |
| 25 | 0.4000 | 0.1600 | 0.0833 |
| 50 | 0.4600 | 0.2600 | 0.2795 |
| 100 | 0.5500 | 0.3600 | 0.4645 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6345** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1860 vs -0.0540 |
| top-decile vs bottom-decile median realized return | 0.0000 vs -0.0801 |
| ECE (10 bins) | 0.1332 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.1410 | 0.0675 | -0.0503 |
| 1 | 163 | 0.1770 | 0.0736 | -0.0304 |
| 2 | 163 | 0.2016 | 0.0982 | 0.0126 |
| 3 | 163 | 0.2231 | 0.1227 | 0.0378 |
| 4 | 163 | 0.2457 | 0.1227 | 0.0494 |
| 5 | 163 | 0.2735 | 0.1472 | 0.0870 |
| 6 | 163 | 0.3044 | 0.1718 | 0.1122 |
| 7 | 163 | 0.3425 | 0.1779 | 0.1293 |
| 8 | 163 | 0.3938 | 0.2515 | 0.1510 |
| 9 | 157 | 0.4893 | 0.2420 | 0.1877 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.2284 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1358 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.1173 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.1235 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0370 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.1049 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.3000 | 0.2000 | 0.1192 |
| 25 | 0.3600 | 0.1600 | 0.1100 |
| 50 | 0.3600 | 0.1600 | 0.1033 |
| 100 | 0.4800 | 0.2200 | 0.1791 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7533, AUC without FM features 0.7464.
