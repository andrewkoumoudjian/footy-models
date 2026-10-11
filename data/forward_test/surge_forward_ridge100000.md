# Forward Test — Surge model (surge_ridge100000)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 30, dataset 1, feature-set 1, calibration 1, ridge 100000, seed 42; trained through 2025-06-10 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per candidate (3429 surge candidates have one); realized = last TM valuation in (entry, 2026-06-12]; censored players dropped
- **Realized window (measured)**: median horizon 349 days (min 52, max 2434); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=30, calibration_version=1, seed=42

## Full candidate pool (n = 3429)

### Headline metrics

| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.6828** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.9006 vs -0.0984 |
| top-decile vs bottom-decile median realized return | 0.2500 vs -0.2000 |
| ECE (10 bins) | 0.0346 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0403 | 0.0700 | -0.0984 |
| 1 | 343 | 0.0693 | 0.1108 | -0.0292 |
| 2 | 343 | 0.0954 | 0.1195 | 0.1116 |
| 3 | 343 | 0.1234 | 0.1720 | 0.1390 |
| 4 | 343 | 0.1526 | 0.1283 | 0.0983 |
| 5 | 343 | 0.1829 | 0.2478 | 0.3803 |
| 6 | 343 | 0.2218 | 0.2274 | 0.2264 |
| 7 | 343 | 0.2709 | 0.3149 | 0.6435 |
| 8 | 343 | 0.3380 | 0.3761 | 0.7816 |
| 9 | 342 | 0.4658 | 0.4035 | 0.9017 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.2500 | 0.3878 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.5500 | -0.5481 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5000 | 0.3673 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.3500 | 0.2362 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.2500 | 0.1370 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.2500 | 0.1953 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.8000 | 0.5000 | 1.2267 |
| 25 | 0.6800 | 0.5200 | 1.1978 |
| 50 | 0.6600 | 0.3800 | 1.0704 |
| 100 | 0.6300 | 0.4000 | 1.2426 |


## Deployable population (age <= 28, 400k <= mv <= 1M at entry) (n = 1624)

### Headline metrics

| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6039** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1833 vs 0.0031 |
| top-decile vs bottom-decile median realized return | 0.1111 vs 0.0000 |
| ECE (10 bins) | 0.0998 |

### Decile table (ranked by predicted surge probability; realized = partial window)

| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 163 | 0.0759 | 0.1227 | 0.0031 |
| 1 | 163 | 0.1158 | 0.0736 | 0.0089 |
| 2 | 163 | 0.1445 | 0.0798 | -0.0200 |
| 3 | 163 | 0.1715 | 0.1350 | 0.0370 |
| 4 | 163 | 0.1986 | 0.1472 | 0.0763 |
| 5 | 163 | 0.2322 | 0.1104 | 0.0321 |
| 6 | 163 | 0.2667 | 0.1350 | 0.0810 |
| 7 | 163 | 0.3127 | 0.2393 | 0.1681 |
| 8 | 163 | 0.3752 | 0.1963 | 0.1172 |
| 9 | 157 | 0.4897 | 0.2357 | 0.1825 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.1111 | 0.2160 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.1389 | -0.1481 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.1111 | 0.1049 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.1111 | 0.1111 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.1111 | 0.0247 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.1111 | 0.0926 |

### Top-N realized performance (by predicted surge probability)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.5000 | 0.3000 | 0.2569 |
| 25 | 0.5200 | 0.1600 | 0.1950 |
| 50 | 0.5000 | 0.1200 | 0.1612 |
| 100 | 0.5200 | 0.2200 | 0.1903 |


## Notes

- Partial-window surge label: `max(valuation in (entry, 2026-06-12]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon 349 days), honestly labeled.
- Walk-forward (pooled OOF, same config): AUC 0.7725, AUC without FM features 0.7627.
