# Forward Test — Tail-aware forecast model (tail_exp090_basket)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 3, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; objective expectile-0.9; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player; realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 337 days (min 52, max 1053); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=3, calibration_version=1, seed=42

## Scored pool (n = 809)

### Headline metrics

| metric | value |
|---|---|
| n | 809 |
| Spearman IC (tail score vs realized partial-period log-ratio) | **0.4858** |
| high-quantile coverage P(realized log-ratio <= tail score) | 0.8356 (target 0.9) |
| hit rate (realized partial return >= +50%) | 0.4227 |
| hit rate (realized partial return >= +100%) | 0.2942 |
| mean realized partial return | 1.0804 |
| median realized partial return | 0.2857 |
| top-decile vs bottom-decile mean return | 4.1682 vs -0.0635 |
| top-decile vs bottom-decile median return | 2.3333 vs -0.1333 |

### Decile table (ranked by tail score; realized = partial window)

| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 rate | spike100 rate |
|---|---|---|---|---|---|---|---|
| 0 | 81 | 1.1211 | 1.2064 | 4.1682 | 2.3333 | 0.8025 | 0.7531 |
| 1 | 81 | 1.0553 | 0.6011 | 1.6561 | 0.6000 | 0.5679 | 0.3827 |
| 2 | 81 | 1.0109 | 0.5733 | 1.4647 | 0.6000 | 0.5679 | 0.4444 |
| 3 | 81 | 0.9719 | 0.5302 | 1.0421 | 0.6364 | 0.6049 | 0.3704 |
| 4 | 81 | 0.9383 | 0.4319 | 1.0061 | 0.3750 | 0.4691 | 0.3580 |
| 5 | 81 | 0.9092 | 0.2598 | 0.4855 | 0.2857 | 0.3457 | 0.2099 |
| 6 | 81 | 0.8844 | 0.3018 | 0.6574 | 0.2000 | 0.3333 | 0.2469 |
| 7 | 81 | 0.8564 | 0.0598 | 0.1358 | 0.0000 | 0.1975 | 0.0617 |
| 8 | 81 | 0.8192 | 0.1075 | 0.2381 | 0.0000 | 0.2469 | 0.0988 |
| 9 | 80 | 0.7492 | -0.1584 | -0.0643 | -0.1333 | 0.0875 | 0.0125 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (tail score)** | 2.3333 | 0.8025 | — | — |
| TM value (cheapest first) | 4.0000 | 1.0000 | -1.6667 | -0.1975 |
| FM CA (highest first) | 0.0000 | 0.2469 | 2.3333 | 0.5556 |
| FM PA (highest first) | 0.3333 | 0.4198 | 2.0000 | 0.3827 |
| age+minutes (young + proven minutes) | 0.1429 | 0.2840 | 2.1905 | 0.5185 |
| random price-bucket (seeded) | 0.2000 | 0.3580 | 2.1333 | 0.4444 |

### Top-N realized performance (by tail score)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 1.0000 | 0.9000 | 13.7333 |
| 25 | 0.9200 | 0.8400 | 7.3213 |
| 50 | 0.9200 | 0.8400 | 5.5506 |
| 100 | 0.9100 | 0.7800 | 4.0937 |


## Notes

- The tail score is the fitted conditional expectile-0.9 of the 12-month log value ratio; higher = stronger spike signal. Coverage measures the share of realized partial-period log-ratios at or below the prediction; for a well-calibrated expectile model this is close to 0.9 (partial-window dispersion is narrower, so slight under-coverage is expected here).
- Walk-forward (pooled OOF, same config): rank IC 0.4989, q-coverage 0.8807, top-decile spike50 0.6470 / spike100 0.5481; top-decile median return 1.0000 (mean 3.6262).
