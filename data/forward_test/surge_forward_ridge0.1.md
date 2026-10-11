# Forward Test — Surge model (surge_ridge0.1)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 0.1, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.6566** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.8133 vs -0.1284 |
| top-decile vs bottom-decile median realized return | 0.2000 vs -0.2000 |
| ECE (10 bins) | 0.0485 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0294 | 0.0583 | -0.1284 |
| 1 | 343 | 0.0593 | 0.1195 | 0.0037 |
| 2 | 343 | 0.0871 | 0.1487 | 0.1636 |
| 3 | 343 | 0.1157 | 0.1778 | 0.1772 |
| 4 | 343 | 0.1437 | 0.2012 | 0.2553 |
| 5 | 343 | 0.1724 | 0.2157 | 0.3980 |
| 6 | 343 | 0.2046 | 0.2624 | 0.5151 |
| 7 | 343 | 0.2451 | 0.2770 | 0.4233 |
| 8 | 343 | 0.3014 | 0.3440 | 0.5298 |
| 9 | 342 | 0.4182 | 0.3655 | 0.8170 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.2000 | 0.3440 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.6000 | -0.5918 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.4500 | 0.3236 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.3000 | 0.1924 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.2000 | 0.0933 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.2000 | 0.1516 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.3000 | 0.7818 |
| 25 | 0.6800 | 0.4800 | 1.6410 |
| 50 | 0.6000 | 0.3800 | 1.0398 |
| 100 | 0.6000 | 0.3500 | 1.0937 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.5920** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1730 vs 0.0283 |
| top-decile vs bottom-decile median realized return | 0.1250 vs 0.0000 |
| ECE (10 bins) | 0.0847 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.0726 | 0.1411 | 0.0322 |
| 1 | 163 | 0.1121 | 0.0613 | -0.0149 |
| 2 | 163 | 0.1393 | 0.1104 | 0.0169 |
| 3 | 163 | 0.1630 | 0.0920 | 0.0131 |
| 4 | 163 | 0.1872 | 0.1104 | 0.0124 |
| 5 | 163 | 0.2145 | 0.2086 | 0.1168 |
| 6 | 163 | 0.2446 | 0.1534 | 0.0853 |
| 7 | 163 | 0.2846 | 0.1963 | 0.1314 |
| 8 | 163 | 0.3338 | 0.1840 | 0.1200 |
| 9 | 157 | 0.4436 | 0.2166 | 0.1727 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.1250 | 0.1914 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.1250 | -0.1728 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.1250 | 0.0802 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.1250 | 0.0864 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.1250 | 0.0000 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.1250 | 0.0679 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.2000 | 0.1492 |
| 25 | 0.4800 | 0.2000 | 0.1884 |
| 50 | 0.5400 | 0.1600 | 0.1610 |
| 100 | 0.5400 | 0.1900 | 0.1849 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7765, AUC without FM features 0.7643.
