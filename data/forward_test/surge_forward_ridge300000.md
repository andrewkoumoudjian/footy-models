# Forward Test — Surge model (surge_ridge300000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 300000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.6937** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.7464 vs -0.1487 |
| top-decile vs bottom-decile median realized return | 0.2500 vs -0.2000 |
| ECE (10 bins) | 0.0267 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0454 | 0.0496 | -0.1487 |
| 1 | 343 | 0.0754 | 0.1020 | -0.0010 |
| 2 | 343 | 0.1022 | 0.1195 | 0.0861 |
| 3 | 343 | 0.1314 | 0.1574 | 0.1199 |
| 4 | 343 | 0.1611 | 0.1633 | 0.1983 |
| 5 | 343 | 0.1930 | 0.2332 | 0.3057 |
| 6 | 343 | 0.2323 | 0.2362 | 0.3332 |
| 7 | 343 | 0.2835 | 0.2945 | 0.4592 |
| 8 | 343 | 0.3559 | 0.4082 | 1.0620 |
| 9 | 342 | 0.4830 | 0.4064 | 0.7398 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.2500 | 0.3907 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.5500 | -0.5452 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5000 | 0.3703 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.3500 | 0.2391 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.2500 | 0.1399 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.2500 | 0.1983 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.2000 | 0.2342 |
| 25 | 0.5600 | 0.3200 | 0.3828 |
| 50 | 0.5200 | 0.2600 | 0.3606 |
| 100 | 0.5800 | 0.3400 | 0.6825 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6099** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1749 vs -0.0222 |
| top-decile vs bottom-decile median realized return | 0.0000 vs 0.0000 |
| ECE (10 bins) | 0.1044 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.0870 | 0.0920 | -0.0221 |
| 1 | 163 | 0.1267 | 0.0982 | 0.0272 |
| 2 | 163 | 0.1555 | 0.0982 | -0.0033 |
| 3 | 163 | 0.1840 | 0.0982 | 0.0073 |
| 4 | 163 | 0.2107 | 0.1656 | 0.0943 |
| 5 | 163 | 0.2418 | 0.1411 | 0.0670 |
| 6 | 163 | 0.2774 | 0.1166 | 0.0502 |
| 7 | 163 | 0.3275 | 0.2393 | 0.1757 |
| 8 | 163 | 0.3946 | 0.1840 | 0.1179 |
| 9 | 157 | 0.5125 | 0.2420 | 0.1716 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.2160 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1481 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.1049 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.1111 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0247 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.0926 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.1000 | 0.1342 |
| 25 | 0.4000 | 0.2000 | 0.1748 |
| 50 | 0.4400 | 0.1200 | 0.1305 |
| 100 | 0.5100 | 0.2000 | 0.1654 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7677, AUC without FM features 0.7595.
