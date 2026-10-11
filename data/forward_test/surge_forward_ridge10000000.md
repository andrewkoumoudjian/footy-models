# Forward Test — Surge model (surge_ridge10000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 10000000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.7525** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.7993 vs -0.2197 |
| top-decile vs bottom-decile median realized return | 0.3333 vs -0.2500 |
| ECE (10 bins) | 0.0737 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.1451 | 0.0321 | -0.2197 |
| 1 | 343 | 0.1763 | 0.0525 | -0.1178 |
| 2 | 343 | 0.1957 | 0.0671 | -0.0412 |
| 3 | 343 | 0.2118 | 0.1341 | 0.0713 |
| 4 | 343 | 0.2275 | 0.1545 | 0.1734 |
| 5 | 343 | 0.2429 | 0.1953 | 0.1488 |
| 6 | 343 | 0.2616 | 0.2682 | 0.3464 |
| 7 | 343 | 0.2826 | 0.3557 | 0.7656 |
| 8 | 343 | 0.3077 | 0.4519 | 1.2377 |
| 9 | 342 | 0.3518 | 0.4591 | 0.7900 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.3333 | 0.4490 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.4667 | -0.4869 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5833 | 0.4286 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.4333 | 0.2974 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.3333 | 0.1983 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.3333 | 0.2566 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.2000 | 0.1308 |
| 25 | 0.4400 | 0.1600 | 0.0953 |
| 50 | 0.4200 | 0.2400 | 0.2299 |
| 100 | 0.5400 | 0.3800 | 0.5355 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6383** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1947 vs -0.0609 |
| top-decile vs bottom-decile median realized return | 0.0000 vs -0.1000 |
| ECE (10 bins) | 0.1142 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.1882 | 0.0552 | -0.0590 |
| 1 | 163 | 0.2106 | 0.0859 | -0.0204 |
| 2 | 163 | 0.2244 | 0.0982 | -0.0061 |
| 3 | 163 | 0.2365 | 0.0982 | 0.0266 |
| 4 | 163 | 0.2472 | 0.1534 | 0.0958 |
| 5 | 163 | 0.2618 | 0.1534 | 0.0674 |
| 6 | 163 | 0.2766 | 0.1472 | 0.1051 |
| 7 | 163 | 0.2944 | 0.1656 | 0.1102 |
| 8 | 163 | 0.3168 | 0.2638 | 0.1766 |
| 9 | 157 | 0.3610 | 0.2548 | 0.1903 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.2469 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1173 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.1358 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.1420 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0556 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.1235 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.2000 | 0.1553 |
| 25 | 0.3600 | 0.1600 | 0.1029 |
| 50 | 0.3400 | 0.1800 | 0.0892 |
| 100 | 0.4500 | 0.2300 | 0.1833 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7435, AUC without FM features 0.7350.
