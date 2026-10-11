# Forward Test — Tail-aware forecast model (tail_tau080_basket)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 3, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; objective quantile-0.8; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player; realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 337 days (min 52, max 1053); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=3, calibration_version=1, seed=42

## Scored pool (n = 809)

### Headline metrics

| metric | value |
|---|---|
| n | 809 |
| Spearman IC (tail score vs realized partial-period log-ratio) | **0.4636** |
| high-quantile coverage P(realized log-ratio <= tail score) | 0.7367 (target 0.8) |
| hit rate (realized partial return >= +50%) | 0.4227 |
| hit rate (realized partial return >= +100%) | 0.2942 |
| mean realized partial return | 1.0804 |
| median realized partial return | 0.2857 |
| top-decile vs bottom-decile mean return | 3.9530 vs -0.0802 |
| top-decile vs bottom-decile median return | 2.0000 vs -0.1667 |

### Decile table (ranked by tail score; realized = partial window)

| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 rate | spike100 rate |
|---|---|---|---|---|---|---|---|
| 0 | 81 | 0.8095 | 1.1091 | 3.9530 | 2.0000 | 0.7654 | 0.6667 |
| 1 | 81 | 0.7662 | 0.6931 | 1.7378 | 0.7000 | 0.6173 | 0.4815 |
| 2 | 81 | 0.7352 | 0.5371 | 1.4939 | 0.5714 | 0.5679 | 0.4198 |
| 3 | 81 | 0.7051 | 0.4569 | 0.8914 | 0.5000 | 0.5062 | 0.3333 |
| 4 | 81 | 0.6771 | 0.4574 | 0.9180 | 0.5000 | 0.5185 | 0.3827 |
| 5 | 81 | 0.6528 | 0.2594 | 0.5201 | 0.1667 | 0.3210 | 0.1975 |
| 6 | 81 | 0.6312 | 0.3498 | 0.8853 | 0.2857 | 0.3827 | 0.2469 |
| 7 | 81 | 0.6059 | 0.0996 | 0.2113 | 0.0000 | 0.2222 | 0.0864 |
| 8 | 81 | 0.5685 | 0.1214 | 0.2538 | 0.0000 | 0.2346 | 0.1111 |
| 9 | 80 | 0.5060 | -0.1705 | -0.0749 | -0.1667 | 0.0875 | 0.0125 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (tail score)** | 2.0000 | 0.7654 | — | — |
| TM value (cheapest first) | 4.0000 | 1.0000 | -2.0000 | -0.2346 |
| FM CA (highest first) | 0.0000 | 0.2469 | 2.0000 | 0.5185 |
| FM PA (highest first) | 0.3333 | 0.4198 | 1.6667 | 0.3457 |
| age+minutes (young + proven minutes) | 0.1429 | 0.2840 | 1.8571 | 0.4815 |
| random price-bucket (seeded) | 0.2000 | 0.3580 | 1.8000 | 0.4074 |

### Top-N realized performance (by tail score)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 1.0000 | 0.9000 | 8.0000 |
| 25 | 0.8800 | 0.8000 | 6.0419 |
| 50 | 0.8800 | 0.7600 | 4.5738 |
| 100 | 0.8800 | 0.7300 | 3.4298 |


## Notes

- The tail score is the fitted conditional quantile-0.8 of the 12-month log value ratio; higher = stronger spike signal. Coverage measures the share of realized partial-period log-ratios at or below the prediction; for a well-calibrated quantile model this is close to 0.8 (partial-window dispersion is narrower, so slight under-coverage is expected here).
- Walk-forward (pooled OOF, same config): rank IC 0.4970, q-coverage 0.8140, top-decile spike50 0.6391 / spike100 0.5378; top-decile median return 1.0000 (mean 3.5452).
