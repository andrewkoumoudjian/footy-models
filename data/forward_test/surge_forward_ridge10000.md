# Forward Test — Surge model (surge_ridge10000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 10000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.6579** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.9158 vs -0.0797 |
| top-decile vs bottom-decile median realized return | 0.2500 vs -0.2000 |
| ECE (10 bins) | 0.0470 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0380 | 0.0758 | -0.0797 |
| 1 | 343 | 0.0666 | 0.1283 | 0.0126 |
| 2 | 343 | 0.0912 | 0.1370 | 0.1247 |
| 3 | 343 | 0.1167 | 0.1603 | 0.1859 |
| 4 | 343 | 0.1422 | 0.1866 | 0.1808 |
| 5 | 343 | 0.1697 | 0.2303 | 0.3558 |
| 6 | 343 | 0.2027 | 0.2449 | 0.3363 |
| 7 | 343 | 0.2419 | 0.2886 | 0.6458 |
| 8 | 343 | 0.2994 | 0.3382 | 0.4830 |
| 9 | 342 | 0.4158 | 0.3801 | 0.9097 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.2500 | 0.3673 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.5500 | -0.5685 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5000 | 0.3469 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.3500 | 0.2157 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.2500 | 0.1166 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.2500 | 0.1749 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.4000 | 0.9610 |
| 25 | 0.6800 | 0.4400 | 1.0515 |
| 50 | 0.6000 | 0.3600 | 1.1457 |
| 100 | 0.6300 | 0.3700 | 0.7911 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.5951** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1612 vs 0.0207 |
| top-decile vs bottom-decile median realized return | 0.0000 vs 0.0000 |
| ECE (10 bins) | 0.0795 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.0701 | 0.1288 | 0.0175 |
| 1 | 163 | 0.1081 | 0.0798 | -0.0058 |
| 2 | 163 | 0.1331 | 0.0859 | -0.0055 |
| 3 | 163 | 0.1568 | 0.1350 | 0.0557 |
| 4 | 163 | 0.1822 | 0.0920 | 0.0040 |
| 5 | 163 | 0.2106 | 0.1779 | 0.0987 |
| 6 | 163 | 0.2407 | 0.1534 | 0.0820 |
| 7 | 163 | 0.2791 | 0.2086 | 0.1521 |
| 8 | 163 | 0.3338 | 0.1963 | 0.1210 |
| 9 | 157 | 0.4410 | 0.2166 | 0.1660 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.1914 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1728 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.0802 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.0864 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0000 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.0679 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.2000 | 0.2087 |
| 25 | 0.4800 | 0.2000 | 0.1918 |
| 50 | 0.5400 | 0.2000 | 0.1940 |
| 100 | 0.5400 | 0.2100 | 0.1987 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7759, AUC without FM features 0.7639.
