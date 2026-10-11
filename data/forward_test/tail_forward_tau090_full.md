# Forward Test — Tail-aware forecast model (tail_tau090_full)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 3, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; objective quantile-0.9; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player; realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=3, calibration_version=1, seed=42

## Scored pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (tail score vs realized partial-period log-ratio) | **0.5036** |
| high-quantile coverage P(realized log-ratio <= tail score) | 0.9589 (target 0.9) |
| hit rate (realized partial return >= +50%) | 0.1606 |
| hit rate (realized partial return >= +100%) | 0.0992 |
| mean realized partial return | 0.1798 |
| median realized partial return | 0.0000 |
| top-decile vs bottom-decile mean return | 1.4519 vs -0.3122 |
| top-decile vs bottom-decile median return | 0.5000 vs -0.3333 |

### Decile table (ranked by tail score; realized = partial window)

| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 rate | spike100 rate |
|---|---|---|---|---|---|---|---|
| 0 | 723 | 1.1313 | 0.5126 | 1.4519 | 0.5000 | 0.5021 | 0.3887 |
| 1 | 723 | 1.0510 | 0.2791 | 0.6134 | 0.1250 | 0.3568 | 0.2448 |
| 2 | 723 | 1.0005 | 0.1098 | 0.2705 | 0.0000 | 0.2379 | 0.1355 |
| 3 | 723 | 0.9637 | 0.0439 | 0.1761 | 0.0000 | 0.1867 | 0.0858 |
| 4 | 723 | 0.9311 | -0.0257 | 0.0636 | 0.0000 | 0.1466 | 0.0650 |
| 5 | 723 | 0.9001 | -0.0675 | 0.0140 | 0.0000 | 0.0927 | 0.0456 |
| 6 | 723 | 0.8684 | -0.1676 | -0.0995 | -0.1250 | 0.0415 | 0.0124 |
| 7 | 723 | 0.8331 | -0.2225 | -0.1520 | -0.2000 | 0.0290 | 0.0111 |
| 8 | 723 | 0.7920 | -0.3084 | -0.2277 | -0.2500 | 0.0111 | 0.0028 |
| 9 | 723 | 0.7195 | -0.4365 | -0.3122 | -0.3333 | 0.0014 | 0.0000 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (tail score)** | 0.5000 | 0.5021 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1701 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4786 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3582 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2656 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3568 |

### Top-N realized performance (by tail score)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.9000 | 0.9000 | 9.7000 |
| 25 | 0.8000 | 0.7600 | 5.4073 |
| 50 | 0.7400 | 0.7200 | 5.1270 |
| 100 | 0.6600 | 0.6400 | 3.1732 |


## Notes

- The tail score is the fitted conditional quantile-0.9 of the 12-month log value ratio; higher = stronger spike signal. Coverage measures the share of realized partial-period log-ratios at or below the prediction; for a well-calibrated quantile model this is close to 0.9 (partial-window dispersion is narrower, so slight under-coverage is expected here).
- Walk-forward (pooled OOF, same config): rank IC 0.4895, q-coverage 0.9040, top-decile spike50 0.0310 / spike100 0.0114; top-decile median return -0.2000 (mean -0.1827).
