# Forward Test — Tail-aware forecast model (tail_exp090_full)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 3, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; objective expectile-0.9; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player; realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 351 days (min 52, max 3391); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=3, calibration_version=1, seed=42

## Scored pool (n = 7230)

### Headline metrics

| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (tail score vs realized partial-period log-ratio) | **0.5033** |
| high-quantile coverage P(realized log-ratio <= tail score) | 0.9480 (target 0.9) |
| hit rate (realized partial return >= +50%) | 0.1606 |
| hit rate (realized partial return >= +100%) | 0.0992 |
| mean realized partial return | 0.1798 |
| median realized partial return | 0.0000 |
| top-decile vs bottom-decile mean return | 1.4557 vs -0.3081 |
| top-decile vs bottom-decile median return | 0.5000 vs -0.3333 |

### Decile table (ranked by tail score; realized = partial window)

| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 rate | spike100 rate |
|---|---|---|---|---|---|---|---|
| 0 | 723 | 1.0791 | 0.5197 | 1.4557 | 0.5000 | 0.5048 | 0.3928 |
| 1 | 723 | 0.9507 | 0.2684 | 0.6036 | 0.1250 | 0.3568 | 0.2420 |
| 2 | 723 | 0.8760 | 0.1261 | 0.2969 | 0.0000 | 0.2462 | 0.1397 |
| 3 | 723 | 0.8228 | 0.0380 | 0.1684 | 0.0000 | 0.1867 | 0.0885 |
| 4 | 723 | 0.7758 | -0.0279 | 0.0625 | 0.0000 | 0.1369 | 0.0636 |
| 5 | 723 | 0.7317 | -0.0790 | -0.0056 | 0.0000 | 0.0913 | 0.0373 |
| 6 | 723 | 0.6864 | -0.1595 | -0.0890 | -0.1250 | 0.0401 | 0.0152 |
| 7 | 723 | 0.6367 | -0.2237 | -0.1539 | -0.2000 | 0.0290 | 0.0083 |
| 8 | 723 | 0.5784 | -0.3143 | -0.2323 | -0.2500 | 0.0111 | 0.0028 |
| 9 | 723 | 0.4783 | -0.4307 | -0.3081 | -0.3333 | 0.0028 | 0.0014 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (tail score)** | 0.5000 | 0.5048 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1729 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4813 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3610 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2683 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3596 |

### Top-N realized performance (by tail score)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.7000 | 0.6000 | 8.7333 |
| 25 | 0.7200 | 0.6800 | 6.7233 |
| 50 | 0.6800 | 0.6600 | 4.9133 |
| 100 | 0.6800 | 0.6500 | 3.4400 |


## Notes

- The tail score is the fitted conditional expectile-0.9 of the 12-month log value ratio; higher = stronger spike signal. Coverage measures the share of realized partial-period log-ratios at or below the prediction; for a well-calibrated expectile model this is close to 0.9 (partial-window dispersion is narrower, so slight under-coverage is expected here).
- Walk-forward (pooled OOF, same config): rank IC 0.4989, q-coverage 0.8807, top-decile spike50 0.6470 / spike100 0.5481; top-decile median return 1.0000 (mean 3.6262).
