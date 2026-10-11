# Forward Test — Tail-aware forecast model (tail_tau090_basket)

- **As-of anchor**: 2025-11-06 (FM26 database vintage / start of FM26)
- **Realized oracle end**: 2026-06-12 (last Transfermarkt valuation in the read-only overlay)
- **Model**: version 3, dataset 1, feature-set 1, calibration 1, ridge 1000000, seed 42; objective quantile-0.9; trained through 2025-06-12 (rows observed on or before this date)
- **Entry**: last TM valuation on or before 2025-11-06 per player; realized = last TM valuation in (entry, 2026-06-12]; players with no realized valuation after entry are dropped (censored), never encoded as zero
- **Realized window (measured)**: median horizon 337 days (min 52, max 1053); realized valuation dates span 2025-11-07 to 2026-06-11
- **BacktestRunRecord**: cutoff=2025-11-06, dataset_version=1, feature_set_version=1, model_version=3, calibration_version=1, seed=42

## Scored pool (n = 809)

### Headline metrics

| metric | value |
|---|---|
| n | 809 |
| Spearman IC (tail score vs realized partial-period log-ratio) | **0.4829** |
| high-quantile coverage P(realized log-ratio <= tail score) | 0.8504 (target 0.9) |
| hit rate (realized partial return >= +50%) | 0.4227 |
| hit rate (realized partial return >= +100%) | 0.2942 |
| mean realized partial return | 1.0804 |
| median realized partial return | 0.2857 |
| top-decile vs bottom-decile mean return | 4.1425 vs -0.0798 |
| top-decile vs bottom-decile median return | 2.3333 vs -0.1667 |

### Decile table (ranked by tail score; realized = partial window)

| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 rate | spike100 rate |
|---|---|---|---|---|---|---|---|
| 0 | 81 | 1.1563 | 1.1849 | 4.1425 | 2.3333 | 0.7901 | 0.7284 |
| 1 | 81 | 1.1174 | 0.6631 | 1.7590 | 0.6667 | 0.6173 | 0.4444 |
| 2 | 81 | 1.0897 | 0.5344 | 1.4040 | 0.6000 | 0.5802 | 0.4321 |
| 3 | 81 | 1.0641 | 0.5093 | 1.0087 | 0.5000 | 0.5432 | 0.3457 |
| 4 | 81 | 1.0421 | 0.4492 | 0.9962 | 0.4545 | 0.4938 | 0.3704 |
| 5 | 81 | 1.0221 | 0.2746 | 0.6164 | 0.1429 | 0.3210 | 0.1975 |
| 6 | 81 | 1.0067 | 0.2439 | 0.4656 | 0.2000 | 0.3333 | 0.2222 |
| 7 | 81 | 0.9865 | 0.1284 | 0.2582 | 0.0667 | 0.2222 | 0.0988 |
| 8 | 81 | 0.9593 | 0.1030 | 0.2190 | 0.0000 | 0.2346 | 0.0864 |
| 9 | 80 | 0.9098 | -0.1776 | -0.0800 | -0.1667 | 0.0875 | 0.0125 |

### Baseline comparison (top decile by signal; hit = partial return >= +50%)

| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (tail score)** | 2.3333 | 0.7901 | — | — |
| TM value (cheapest first) | 4.0000 | 1.0000 | -1.6667 | -0.2099 |
| FM CA (highest first) | 0.0000 | 0.2469 | 2.3333 | 0.5432 |
| FM PA (highest first) | 0.3333 | 0.4198 | 2.0000 | 0.3704 |
| age+minutes (young + proven minutes) | 0.1429 | 0.2840 | 2.1905 | 0.5062 |
| random price-bucket (seeded) | 0.2000 | 0.3580 | 2.1333 | 0.4321 |

### Top-N realized performance (by tail score)

| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 1.0000 | 0.9000 | 11.7667 |
| 25 | 0.9200 | 0.8000 | 7.1713 |
| 50 | 0.9000 | 0.8200 | 5.3906 |
| 100 | 0.8800 | 0.7500 | 3.5306 |


## Notes

- The tail score is the fitted conditional quantile-0.9 of the 12-month log value ratio; higher = stronger spike signal. Coverage measures the share of realized partial-period log-ratios at or below the prediction; for a well-calibrated quantile model this is close to 0.9 (partial-window dispersion is narrower, so slight under-coverage is expected here).
- Walk-forward (pooled OOF, same config): rank IC 0.4895, q-coverage 0.9040, top-decile spike50 0.6402 / spike100 0.5405; top-decile median return 1.0000 (mean 3.5346).
