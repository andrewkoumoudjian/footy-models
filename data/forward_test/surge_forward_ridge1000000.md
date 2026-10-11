# Forward Test — Surge model (surge_ridge1000000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.7210** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.8354 vs -0.1888 |
| top-decile vs bottom-decile median realized return | 0.3000 vs -0.2500 |
| ECE (10 bins) | 0.0260 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0544 | 0.0379 | -0.1888 |
| 1 | 343 | 0.0873 | 0.0787 | -0.0744 |
| 2 | 343 | 0.1171 | 0.1050 | 0.0504 |
| 3 | 343 | 0.1462 | 0.1312 | 0.0509 |
| 4 | 343 | 0.1768 | 0.1633 | 0.1985 |
| 5 | 343 | 0.2117 | 0.2274 | 0.3004 |
| 6 | 343 | 0.2525 | 0.2682 | 0.3901 |
| 7 | 343 | 0.3047 | 0.3061 | 0.5238 |
| 8 | 343 | 0.3763 | 0.4140 | 1.0662 |
| 9 | 342 | 0.5040 | 0.4386 | 0.8376 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.3000 | 0.4198 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.5000 | -0.5160 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5500 | 0.3994 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.4000 | 0.2682 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.3000 | 0.1691 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.3000 | 0.2274 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.3000 | 0.2000 | 0.1342 |
| 25 | 0.4000 | 0.2400 | 0.1910 |
| 50 | 0.4800 | 0.2600 | 0.2098 |
| 100 | 0.5400 | 0.3000 | 0.3522 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6233** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1709 vs -0.0531 |
| top-decile vs bottom-decile median realized return | 0.0000 vs -0.0833 |
| ECE (10 bins) | 0.1263 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.1069 | 0.0613 | -0.0521 |
| 1 | 163 | 0.1446 | 0.1043 | 0.0020 |
| 2 | 163 | 0.1723 | 0.0920 | 0.0239 |
| 3 | 163 | 0.1996 | 0.1288 | 0.0159 |
| 4 | 163 | 0.2280 | 0.1166 | 0.0441 |
| 5 | 163 | 0.2576 | 0.1656 | 0.1019 |
| 6 | 163 | 0.2951 | 0.1350 | 0.0891 |
| 7 | 163 | 0.3456 | 0.1902 | 0.1205 |
| 8 | 163 | 0.4115 | 0.2577 | 0.1770 |
| 9 | 157 | 0.5349 | 0.2229 | 0.1633 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.2099 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1543 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.0988 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.1049 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0185 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.0864 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.2000 | 0.1000 | 0.0675 |
| 25 | 0.3600 | 0.2400 | 0.1813 |
| 50 | 0.4400 | 0.2000 | 0.1520 |
| 100 | 0.5100 | 0.1800 | 0.1605 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7603, AUC without FM features 0.7534.
