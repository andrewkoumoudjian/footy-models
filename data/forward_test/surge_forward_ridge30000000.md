# Forward Test — Surge model (surge_ridge30000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 30000000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.7554** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.7914 vs -0.2169 |
| top-decile vs bottom-decile median realized return | 0.3333 vs -0.2500 |
| ECE (10 bins) | 0.0157 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.1915 | 0.0321 | -0.2169 |
| 1 | 343 | 0.2069 | 0.0466 | -0.1268 |
| 2 | 343 | 0.2155 | 0.0758 | -0.0354 |
| 3 | 343 | 0.2224 | 0.1224 | 0.0551 |
| 4 | 343 | 0.2288 | 0.1545 | 0.1711 |
| 5 | 343 | 0.2350 | 0.1953 | 0.1425 |
| 6 | 343 | 0.2423 | 0.2711 | 0.3455 |
| 7 | 343 | 0.2502 | 0.3469 | 0.7388 |
| 8 | 343 | 0.2594 | 0.4636 | 1.2860 |
| 9 | 342 | 0.2750 | 0.4620 | 0.7946 |

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
| 10 | 0.4000 | 0.2000 | 0.1169 |
| 25 | 0.4800 | 0.2000 | 0.1282 |
| 50 | 0.5000 | 0.3000 | 0.4716 |
| 100 | 0.5400 | 0.3800 | 0.5280 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6393** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1913 vs -0.0573 |
| top-decile vs bottom-decile median realized return | 0.0000 vs -0.1000 |
| ECE (10 bins) | 0.0950 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.2119 | 0.0552 | -0.0543 |
| 1 | 163 | 0.2218 | 0.0859 | -0.0315 |
| 2 | 163 | 0.2274 | 0.0920 | -0.0057 |
| 3 | 163 | 0.2324 | 0.0982 | 0.0291 |
| 4 | 163 | 0.2367 | 0.1656 | 0.1022 |
| 5 | 163 | 0.2422 | 0.1472 | 0.0506 |
| 6 | 163 | 0.2478 | 0.1718 | 0.1382 |
| 7 | 163 | 0.2545 | 0.1288 | 0.0735 |
| 8 | 163 | 0.2624 | 0.2699 | 0.1832 |
| 9 | 157 | 0.2781 | 0.2611 | 0.2016 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.2407 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1235 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.1296 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.1358 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0494 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.1173 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.2000 | 0.1091 |
| 25 | 0.3600 | 0.1600 | 0.0895 |
| 50 | 0.3400 | 0.1800 | 0.0892 |
| 100 | 0.4500 | 0.2300 | 0.1819 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.6989, AUC without FM features 0.6845.
