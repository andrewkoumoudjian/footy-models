# Forward Test — Surge model (surge_ridge1000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 1000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.6582** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.8605 vs -0.1150 |
| top-decile vs bottom-decile median realized return | 0.2500 vs -0.2000 |
| ECE (10 bins) | 0.0472 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0316 | 0.0612 | -0.1150 |
| 1 | 343 | 0.0614 | 0.1137 | -0.0102 |
| 2 | 343 | 0.0882 | 0.1516 | 0.1540 |
| 3 | 343 | 0.1162 | 0.1633 | 0.1668 |
| 4 | 343 | 0.1434 | 0.2128 | 0.2537 |
| 5 | 343 | 0.1721 | 0.2187 | 0.4078 |
| 6 | 343 | 0.2046 | 0.2478 | 0.3219 |
| 7 | 343 | 0.2443 | 0.2799 | 0.6051 |
| 8 | 343 | 0.3013 | 0.3353 | 0.5077 |
| 9 | 342 | 0.4171 | 0.3860 | 0.8630 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.2500 | 0.3644 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.5500 | -0.5714 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5000 | 0.3440 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.3500 | 0.2128 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.2500 | 0.1137 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.2500 | 0.1720 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.4000 | 0.9443 |
| 25 | 0.7200 | 0.4800 | 1.6703 |
| 50 | 0.6000 | 0.3800 | 1.1515 |
| 100 | 0.6200 | 0.3500 | 1.1028 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.5943** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1972 vs 0.0230 |
| top-decile vs bottom-decile median realized return | 0.1429 vs 0.0000 |
| ECE (10 bins) | 0.0841 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.0721 | 0.1350 | 0.0247 |
| 1 | 163 | 0.1114 | 0.0613 | -0.0133 |
| 2 | 163 | 0.1378 | 0.1166 | 0.0202 |
| 3 | 163 | 0.1618 | 0.1043 | 0.0301 |
| 4 | 163 | 0.1868 | 0.1043 | -0.0026 |
| 5 | 163 | 0.2137 | 0.2025 | 0.1207 |
| 6 | 163 | 0.2436 | 0.1534 | 0.0832 |
| 7 | 163 | 0.2828 | 0.1963 | 0.1334 |
| 8 | 163 | 0.3345 | 0.1718 | 0.1012 |
| 9 | 157 | 0.4422 | 0.2293 | 0.1888 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.1429 | 0.2160 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.1071 | -0.1481 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.1429 | 0.1049 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.1429 | 0.1111 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.1429 | 0.0247 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.1429 | 0.0926 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.3000 | 0.3354 |
| 25 | 0.4800 | 0.2000 | 0.1884 |
| 50 | 0.5400 | 0.1800 | 0.1780 |
| 100 | 0.5200 | 0.1800 | 0.1729 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7766, AUC without FM features 0.7643.
