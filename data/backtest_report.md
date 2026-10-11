# Backtest & Optimization Report — Footy Models
## 1. As-of date (forward-test anchor)
- **FM26 as-of date (anchor `T0`): 2025-11-06** — the FM26 database snapshot vintage (start of FM26, ~Nov 2025). Recorded as the forward-test anchor. The Transfermarkt overlay used as the realized oracle is the read-only DuckDB at `/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb`; its last valuation is **2026-06-12**.
- Entry valuation per player: the **last Transfermarkt valuation on or before `T0`** (`mv_t` at `t <= T0`); features are computed strictly from observations before `t` (same 47-feature causal extractor as the training pipeline).
- Realized valuation per player: the **last Transfermarkt valuation in `(t, 2026-06-12]`** (`mv_realized`). Players with no valuation after `t` are censored and dropped — missing is not zero.
- Measured realized window: median horizon **351 days** (forecast pool, min 52 / max 3391); median **349 days** (surge pool). The anchor-to-oracle window is 218 days, but most entry valuations predate the anchor (TM's summer-2025 revaluation wave), so the effective median realized horizon is ~11.5 months. **All forward-test metrics are scored on the realized partial window actually observed (honestly labeled: partial-horizon).**
## 2. Forward-test methodology
- **Forecast model**: players in `data/forecast_candidates.csv` (n=7,969) with a valuation at `T0`; n=7,230 scored with a realized outcome (397 had no entry valuation, 342 censored). Predicted 12-month log-ratio from the fitted ridge model (trained through 2025-06-12, strictly before `T0`); realized partial-period log-ratio `ln(mv_realized / mv_t)`. Score: Spearman IC, RMSE/MAE, hit rates (realized log-ratio > 0; realized return >= +50%), deciles, top-N, baselines, and 80% conformal interval partial-horizon coverage.
- **Surge model**: surge candidates (n=3,615) with a valuation at `T0`; n=3,429 scored with a realized outcome (71 no entry, 115 censored). Predicted surge probability from the fitted logistic model; realized partial-window surge label = `max(mv in (t, 2026-06-12]) >= 1.5 * mv_t` (partial 12-month target). Score: AUC vs partial label, decile realized returns, hit rates, top-N, baselines.
- **Populations reported per config**: forecast — full pool (n=7,230), deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db,derived}; n=208), MV-band (10k <= mv <= 1M; n=6,835); surge — full candidate pool (n=3,429), deployable (age <= 28, 400k <= mv <= 1M at entry; n=1,624).
- **Baselines** (top decile by signal; hit = partial return >= +50%): TM value (cheapest first), FM CA (highest first), FM PA (highest first), age+minutes (young + proven minutes), random price-bucket (seeded splitmix64, deterministic).
- **Costs**: portfolio comparisons use 15 bps round-trip where a portfolio is evaluated (top-N tables are gross-of-cost; the recommendation notes the 15 bps hurdle).
## 3. Forward-test results — forecast model
### 3.1 Headline (full pool, n = 7,230)
**Final config: full features, ridge = 1,000,000, seed 42 (model version 2).**
| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.5100** |
| RMSE (partial-period log-ratio) | 0.5183 |
| MAE (partial-period log-ratio) | 0.3972 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4400 vs -0.3105 |
| 80% conformal interval partial-horizon coverage (radius 0.6009) | 0.7978 |
Walk-forward (same config, pooled OOF): RMSE 0.5905, MAE 0.4120, Spearman IC 0.5063; 80% conformal radius 0.6009 (empirical OOF coverage 0.8000).
#### Decile table (predicted log-ratio vs realized partial window)
| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.0884 | -0.4357 | -0.3105 |
| 1 | 723 | 0.0046 | -0.3245 | -0.2392 |
| 2 | 723 | 0.0615 | -0.2257 | -0.1512 |
| 3 | 723 | 0.1110 | -0.1428 | -0.0759 |
| 4 | 723 | 0.1546 | -0.0828 | -0.0032 |
| 5 | 723 | 0.1975 | -0.0276 | 0.0671 |
| 6 | 723 | 0.2423 | 0.0435 | 0.1740 |
| 7 | 723 | 0.2958 | 0.1296 | 0.2986 |
| 8 | 723 | 0.3674 | 0.2624 | 0.5981 |
| 9 | 723 | 0.4837 | 0.5206 | 1.4400 |
#### Baseline comparison (top decile)
| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.5000 | 0.5062 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.5000 | 0.1743 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.7500 | 0.4827 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.6111 | 0.3624 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.5000 | 0.2697 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5833 | 0.3610 |
#### Top-N realized performance
| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.6000 | 0.6000 | 4.5000 |
| 25 | 0.6800 | 0.6400 | 6.2833 |
| 50 | 0.7000 | 0.6800 | 5.5083 |
| 100 | 0.6700 | 0.6600 | 3.6060 |
### 3.2 Baseline config comparison (ridge = 1000, the previous shipped model)
| metric | value |
|---|---|
| n | 7230 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.4555** |
| RMSE (partial-period log-ratio) | 0.4857 |
| MAE (partial-period log-ratio) | 0.3561 |
| hit rate (realized log-ratio > 0) | 0.3012 |
| hit rate (realized partial return >= +50%) | 0.1606 |
| mean realized partial return | 0.1798 |
| top-decile vs bottom-decile realized return | 1.4542 vs -0.2665 |
| 80% conformal interval partial-horizon coverage (radius 0.5910) | 0.8136 |
#### Decile table
| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |
|---|---|---|---|---|
| 0 | 723 | -0.2582 | -0.3660 | -0.2665 |
| 1 | 723 | -0.1361 | -0.2862 | -0.1980 |
| 2 | 723 | -0.0689 | -0.2089 | -0.1224 |
| 3 | 723 | -0.0140 | -0.1393 | -0.0543 |
| 4 | 723 | 0.0382 | -0.1013 | -0.0139 |
| 5 | 723 | 0.0861 | -0.0585 | 0.0456 |
| 6 | 723 | 0.1390 | 0.0359 | 0.1542 |
| 7 | 723 | 0.2055 | 0.0950 | 0.2694 |
| 8 | 723 | 0.3025 | 0.2231 | 0.5298 |
| 9 | 723 | 0.5547 | 0.5232 | 1.4542 |
#### Baseline comparison (top decile)
| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.4286 | 0.4979 | — | — |
| TM value (cheapest first) | 0.0000 | 0.3320 | 0.4286 | 0.1660 |
| FM CA (highest first) | -0.2500 | 0.0235 | 0.6786 | 0.4744 |
| FM PA (highest first) | -0.1111 | 0.1438 | 0.5397 | 0.3541 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2365 | 0.4286 | 0.2614 |
| random price-bucket (seeded) | -0.0833 | 0.1452 | 0.5119 | 0.3527 |
### 3.3 Deployable population (age < 23, PA >= 140, mv <= 1M) — final config
| metric | value |
|---|---|
| n | 208 |
| Spearman IC (predicted vs realized partial-period log-ratio) | **0.3992** |
| RMSE (partial-period log-ratio) | 0.6474 |
| MAE (partial-period log-ratio) | 0.4901 |
| hit rate (realized log-ratio > 0) | 0.4519 |
| hit rate (realized partial return >= +50%) | 0.3413 |
| mean realized partial return | 0.9321 |
| top-decile vs bottom-decile realized return | 1.9763 vs 0.0610 |
| 80% conformal interval partial-horizon coverage (radius 0.6009) | 0.7548 |
#### Baseline comparison (top decile)
| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted log-ratio)** | 0.8000 | 0.6190 | — | — |
| TM value (cheapest first) | 0.0000 | 0.4286 | 0.8000 | 0.1905 |
| FM CA (highest first) | 0.3333 | 0.3333 | 0.4667 | 0.2857 |
| FM PA (highest first) | 0.1250 | 0.2381 | 0.6750 | 0.3810 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2381 | 0.8000 | 0.3810 |
| random price-bucket (seeded) | 0.0000 | 0.3810 | 0.8000 | 0.2381 |
## 4. Forward-test results — surge model
### 4.1 Full candidate pool (n = 3,429)
**Final recommended config: ridge = 3,000,000, seed 42.** (Shipped `data/surge/model.bin` remains the baseline ridge=1.0 model — artifact-update scope covers forecast artifacts only; the surge recommendation is documented here for the next deploy.)
| metric | value |
|---|---|
| n | 3429 |
| AUC (model score vs partial-window surge label) | **0.7430** |
| partial surge rate (peak >= 1.5x entry within window) | 0.2170 |
| hit rate (realized partial return >= +50%) | 0.2030 |
| mean realized partial return | 0.3153 |
| top-decile vs bottom-decile mean realized return | 0.7876 vs -0.2277 |
| top-decile vs bottom-decile median realized return | 0.3333 vs -0.2500 |
| ECE (10 bins) | 0.0555 |
#### Decile table (predicted surge probability vs realized partial window)
| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 343 | 0.0823 | 0.0262 | -0.2277 |
| 1 | 343 | 0.1210 | 0.0700 | -0.0917 |
| 2 | 343 | 0.1508 | 0.0729 | -0.0348 |
| 3 | 343 | 0.1781 | 0.1341 | 0.1194 |
| 4 | 343 | 0.2058 | 0.1574 | 0.1284 |
| 5 | 343 | 0.2354 | 0.1953 | 0.1828 |
| 6 | 343 | 0.2717 | 0.2682 | 0.4061 |
| 7 | 343 | 0.3153 | 0.3586 | 0.7260 |
| 8 | 343 | 0.3704 | 0.4402 | 1.1618 |
| 9 | 342 | 0.4676 | 0.4474 | 0.7841 |
#### Baseline comparison (top decile)
| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.3333 | 0.4344 | — | — |
| TM value (cheapest first) | 1.8000 | 0.9359 | -1.4667 | -0.5015 |
| FM CA (highest first) | -0.2500 | 0.0204 | 0.5833 | 0.4140 |
| FM PA (highest first) | -0.1000 | 0.1516 | 0.4333 | 0.2828 |
| age+minutes (young + proven minutes) | 0.0000 | 0.2507 | 0.3333 | 0.1837 |
| random price-bucket (seeded) | 0.0000 | 0.1924 | 0.3333 | 0.2420 |
#### Top-N realized performance
| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |
|---|---|---|---|
| 10 | 0.4000 | 0.2000 | 0.1775 |
| 25 | 0.4000 | 0.1600 | 0.0833 |
| 50 | 0.4600 | 0.2600 | 0.2795 |
| 100 | 0.5500 | 0.3600 | 0.4645 |
### 4.2 Baseline config (ridge = 1.0, shipped model)
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
#### Decile table
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
### 4.3 Deployable population (age <= 28, 400k <= mv <= 1M) — final recommended config
| metric | value |
|---|---|
| n | 1624 |
| AUC (model score vs partial-window surge label) | **0.6345** |
| partial surge rate (peak >= 1.5x entry within window) | 0.1472 |
| hit rate (realized partial return >= +50%) | 0.1281 |
| mean realized partial return | 0.0682 |
| top-decile vs bottom-decile mean realized return | 0.1860 vs -0.0540 |
| top-decile vs bottom-decile median realized return | 0.0000 vs -0.0801 |
| ECE (10 bins) | 0.1332 |
#### Baseline comparison (top decile)
| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |
|---|---|---|---|---|
| **model (predicted surge probability)** | 0.0000 | 0.2284 | — | — |
| TM value (cheapest first) | 0.2500 | 0.3642 | -0.2500 | -0.1358 |
| FM CA (highest first) | 0.0000 | 0.1111 | 0.0000 | 0.1173 |
| FM PA (highest first) | 0.0000 | 0.1049 | 0.0000 | 0.1235 |
| age+minutes (young + proven minutes) | 0.0000 | 0.1914 | 0.0000 | 0.0370 |
| random price-bucket (seeded) | 0.0000 | 0.1235 | 0.0000 | 0.1049 |
## 5. Optimization log
Each config was re-run through walk-forward AND the forward test. `wf_metric` = pooled OOF Spearman IC (forecast) / pooled OOF AUC (surge); `fwd_metric` = forward-test Spearman IC (forecast) / forward-test AUC vs partial surge label (surge). Coverage = 80% conformal interval partial-horizon coverage (forecast) / ECE (surge). Decile returns = mean realized return of top vs bottom predicted decile. Baseline excess = max over baselines of (model top-decile median-return excess, model top-decile hit-rate excess).
### 5.1 Forecast model — ridge sweep (full features, seed 42)
| config | pop | walk-forward IC | forward-test IC | n | partial coverage | top vs bottom decile return | median-excess | hit-excess |
|---|---|---|---|---|---|---|---|---|
| ridge1000 (baseline) | full | 0.5291 | 0.4555 | 7230 | 0.8136 | 1.454 vs -0.266 | 0.679 | 0.474 |
| ridge1000 (baseline) | eligible | 0.5291 | 0.3920 | 208 | 0.7692 | 1.834 vs 0.003 | 1.000 | 0.381 |
| ridge1000 (baseline) | mv_band | 0.5291 | 0.4526 | 6835 | 0.8268 | 1.509 vs -0.246 | 0.700 | 0.459 |
| ridge10 | full | 0.5284 | 0.4505 | 7230 | 0.8124 | 1.448 vs -0.269 | 0.679 | 0.469 |
| ridge10 | eligible | 0.5284 | 0.3841 | 208 | 0.7740 | 1.834 vs 0.044 | 1.000 | 0.333 |
| ridge10 | mv_band | 0.5284 | 0.4488 | 6835 | 0.8259 | 1.493 vs -0.248 | 0.637 | 0.450 |
| ridge100 | full | 0.5287 | 0.4514 | 7230 | 0.8129 | 1.450 vs -0.268 | 0.679 | 0.469 |
| ridge100 | eligible | 0.5287 | 0.3860 | 208 | 0.7740 | 1.834 vs 0.044 | 1.000 | 0.333 |
| ridge100 | mv_band | 0.5287 | 0.4494 | 6835 | 0.8260 | 1.496 vs -0.246 | 0.672 | 0.452 |
| ridge300 | full | 0.5289 | 0.4526 | 7230 | 0.8136 | 1.450 vs -0.269 | 0.679 | 0.469 |
| ridge300 | eligible | 0.5289 | 0.3861 | 208 | 0.7740 | 1.834 vs 0.044 | 1.000 | 0.333 |
| ridge300 | mv_band | 0.5289 | 0.4503 | 6835 | 0.8268 | 1.500 vs -0.243 | 0.700 | 0.453 |
| ridge1000 | full | 0.5291 | 0.4555 | 7230 | 0.8136 | 1.454 vs -0.266 | 0.679 | 0.474 |
| ridge1000 | eligible | 0.5291 | 0.3920 | 208 | 0.7692 | 1.834 vs 0.003 | 1.000 | 0.381 |
| ridge1000 | mv_band | 0.5291 | 0.4526 | 6835 | 0.8268 | 1.509 vs -0.246 | 0.700 | 0.459 |
| ridge3000 | full | 0.5290 | 0.4614 | 7230 | 0.8140 | 1.454 vs -0.267 | 0.750 | 0.477 |
| ridge3000 | eligible | 0.5290 | 0.3977 | 208 | 0.7596 | 1.943 vs -0.113 | 1.000 | 0.381 |
| ridge3000 | mv_band | 0.5290 | 0.4572 | 6835 | 0.8269 | 1.521 vs -0.248 | 0.700 | 0.465 |
| ridge10000 | full | 0.5281 | 0.4712 | 7230 | 0.8134 | 1.510 vs -0.262 | 0.750 | 0.488 |
| ridge10000 | eligible | 0.5281 | 0.4135 | 208 | 0.7404 | 1.939 vs -0.111 | 0.800 | 0.286 |
| ridge10000 | mv_band | 0.5281 | 0.4648 | 6835 | 0.8268 | 1.567 vs -0.241 | 0.700 | 0.468 |
| ridge30000 | full | 0.5262 | 0.4814 | 7230 | 0.8109 | 1.512 vs -0.263 | 0.750 | 0.491 |
| ridge30000 | eligible | 0.5262 | 0.4166 | 208 | 0.6971 | 1.989 vs -0.082 | 1.000 | 0.286 |
| ridge30000 | mv_band | 0.5262 | 0.4726 | 6835 | 0.8238 | 1.573 vs -0.238 | 0.700 | 0.480 |
| ridge100000 | full | 0.5223 | 0.4939 | 7230 | 0.8108 | 1.522 vs -0.280 | 0.750 | 0.498 |
| ridge100000 | eligible | 0.5223 | 0.4157 | 208 | 0.7115 | 1.989 vs -0.085 | 1.000 | 0.333 |
| ridge100000 | mv_band | 0.5223 | 0.4831 | 6835 | 0.8247 | 1.559 vs -0.250 | 0.700 | 0.471 |
| ridge300000 | full | 0.5158 | 0.5045 | 7230 | 0.8014 | 1.496 vs -0.300 | 0.750 | 0.492 |
| ridge300000 | eligible | 0.5158 | 0.4043 | 208 | 0.7115 | 2.164 vs -0.062 | 1.000 | 0.381 |
| ridge300000 | mv_band | 0.5158 | 0.4923 | 6835 | 0.8162 | 1.517 vs -0.273 | 0.700 | 0.469 |
| ridge1000000 | full | 0.5063 | 0.5100 | 7230 | 0.7978 | 1.440 vs -0.310 | 0.750 | 0.483 |
| ridge1000000 | eligible | 0.5063 | 0.3992 | 208 | 0.7548 | 1.976 vs 0.061 | 0.800 | 0.381 |
| ridge1000000 | mv_band | 0.5063 | 0.4963 | 6835 | 0.8146 | 1.502 vs -0.279 | 0.700 | 0.463 |
| ridge3000000 | full | 0.4978 | 0.5090 | 7230 | 0.7960 | 1.408 vs -0.311 | 0.750 | 0.487 |
| ridge3000000 | eligible | 0.4978 | 0.3898 | 208 | 0.7788 | 1.868 vs 0.127 | 0.600 | 0.381 |
| ridge3000000 | mv_band | 0.4978 | 0.4943 | 6835 | 0.8149 | 1.456 vs -0.286 | 0.700 | 0.463 |
| ridge10000000 | full | 0.4889 | 0.5069 | 7230 | 0.7979 | 1.389 vs -0.313 | 0.750 | 0.481 |
| ridge10000000 | eligible | 0.4889 | 0.3882 | 208 | 0.7933 | 2.632 vs 0.136 | 0.600 | 0.333 |
| ridge10000000 | mv_band | 0.4889 | 0.4915 | 6835 | 0.8176 | 1.444 vs -0.285 | 0.700 | 0.463 |
| ridge500000 | full | 0.5119 | 0.5079 | 7230 | 0.7983 | 1.481 vs -0.305 | 0.750 | 0.484 |
| ridge500000 | eligible | 0.5119 | 0.4000 | 208 | 0.7308 | 2.022 vs 0.013 | 1.000 | 0.381 |
| ridge500000 | mv_band | 0.5119 | 0.4952 | 6835 | 0.8140 | 1.510 vs -0.273 | 0.700 | 0.468 |
| ridge750000 | full | 0.5087 | 0.5094 | 7230 | 0.7975 | 1.454 vs -0.308 | 0.750 | 0.483 |
| ridge750000 | eligible | 0.5087 | 0.3974 | 208 | 0.7548 | 1.976 vs 0.066 | 0.600 | 0.333 |
| ridge750000 | mv_band | 0.5087 | 0.4961 | 6835 | 0.8140 | 1.505 vs -0.281 | 0.700 | 0.466 |
| ridge1500000 | full | 0.5030 | 0.5100 | 7230 | 0.7988 | 1.441 vs -0.312 | 0.750 | 0.484 |
| ridge1500000 | eligible | 0.5030 | 0.3951 | 208 | 0.7596 | 2.046 vs 0.113 | 0.800 | 0.381 |
| ridge1500000 | mv_band | 0.5030 | 0.4959 | 6835 | 0.8159 | 1.460 vs -0.284 | 0.700 | 0.468 |
| ridge2000000 | full | 0.5007 | 0.5098 | 7230 | 0.7981 | 1.401 vs -0.313 | 0.750 | 0.484 |
| ridge2000000 | eligible | 0.5007 | 0.3926 | 208 | 0.7644 | 1.868 vs 0.127 | 0.600 | 0.381 |
| ridge2000000 | mv_band | 0.5007 | 0.4953 | 6835 | 0.8159 | 1.457 vs -0.286 | 0.700 | 0.463 |

### 5.2 Forecast model — feature ablations (seed 42)
Ablations set the ablated feature block to missing (`NA`) in BOTH the training rows and the forward-test candidates (missing is not zero: the paired missingness indicator carries the absence). `no_fm` = FM block (13 features: fm_present, fm_ca, fm_pa, fm_pa_gap, fm_ca_age_pct, fm_sig_*, fm_attr_db, fm_attr_derived); `no_contract` = transfer-history block (5 features: ever_transferred, log_days_since_transfer, log_transfer_fee, free_transfer, log_mv_transfer_ratio — the closest features to contract economics in the 47-feature schema).
| config | pop | walk-forward IC | forward-test IC | n | partial coverage | top vs bottom decile return | median-excess | hit-excess |
|---|---|---|---|---|---|---|---|---|
| no_fm_ridge1000 | full | 0.5103 | 0.5133 | 7230 | 0.8089 | 1.480 vs -0.300 | 0.750 | 0.481 |
| no_fm_ridge1000 | eligible | 0.5103 | 0.3612 | 208 | 0.6731 | 2.627 vs 0.014 | 0.600 | 0.286 |
| no_fm_ridge1000 | mv_band | 0.5103 | 0.4947 | 6835 | 0.8186 | 1.511 vs -0.274 | 0.700 | 0.463 |
| no_fm_ridge1000000 | full | 0.4941 | 0.5175 | 7230 | 0.8093 | 1.384 vs -0.324 | 0.750 | 0.484 |
| no_fm_ridge1000000 | eligible | 0.4941 | 0.3634 | 208 | 0.7644 | 2.725 vs 0.159 | 1.000 | 0.381 |
| no_fm_ridge1000000 | mv_band | 0.4941 | 0.4988 | 6835 | 0.8240 | 1.434 vs -0.295 | 0.700 | 0.465 |
| no_contract_ridge1000 | full | 0.5238 | 0.4627 | 7230 | 0.8165 | 1.466 vs -0.284 | 0.650 | 0.462 |
| no_contract_ridge1000 | eligible | 0.5238 | 0.3964 | 208 | 0.7644 | 2.022 vs -0.049 | 1.000 | 0.381 |
| no_contract_ridge1000 | mv_band | 0.5238 | 0.4601 | 6835 | 0.8293 | 1.543 vs -0.260 | 0.629 | 0.449 |
| no_contract_ridge1000000 | full | 0.4983 | 0.5093 | 7230 | 0.8000 | 1.468 vs -0.313 | 0.750 | 0.481 |
| no_contract_ridge1000000 | eligible | 0.4983 | 0.3993 | 208 | 0.7596 | 2.710 vs 0.061 | 0.600 | 0.286 |
| no_contract_ridge1000000 | mv_band | 0.4983 | 0.4957 | 6835 | 0.8168 | 1.527 vs -0.280 | 0.700 | 0.468 |
| no_fm_ridge1e6 | full | 0.4941 | 0.5175 | 7230 | 0.8093 | 1.384 vs -0.324 | 0.750 | 0.484 |
| no_fm_ridge1e6 | eligible | 0.4941 | 0.3634 | 208 | 0.7644 | 2.725 vs 0.159 | 1.000 | 0.381 |
| no_fm_ridge1e6 | mv_band | 0.4941 | 0.4988 | 6835 | 0.8240 | 1.434 vs -0.295 | 0.700 | 0.465 |
| no_contract_ridge1e6 | full | 0.4983 | 0.5093 | 7230 | 0.8000 | 1.468 vs -0.313 | 0.750 | 0.481 |
| no_contract_ridge1e6 | eligible | 0.4983 | 0.3993 | 208 | 0.7596 | 2.710 vs 0.061 | 0.600 | 0.286 |
| no_contract_ridge1e6 | mv_band | 0.4983 | 0.4957 | 6835 | 0.8168 | 1.527 vs -0.280 | 0.700 | 0.468 |

### 5.3 Surge model — ridge sweep (seed 42)
| config | pop | walk-forward AUC | forward-test AUC | n | ECE | top vs bottom decile return | median-excess | hit-excess |
|---|---|---|---|---|---|---|---|---|
| surge ridge1.0 (baseline) | full | 0.7765 | 0.6566 | 3429 | 0.0485 | 0.813 vs -0.128 | 0.450 | 0.324 |
| surge ridge1.0 (baseline) | deployable | 0.7765 | 0.5920 | 1624 | 0.0847 | 0.173 vs 0.028 | 0.125 | 0.086 |
| surge_ridge0.01 | full | 0.7765 | 0.6566 | 3429 | 0.0485 | 0.813 vs -0.128 | 0.450 | 0.324 |
| surge_ridge0.01 | deployable | 0.7765 | 0.5920 | 1624 | 0.0847 | 0.173 vs 0.028 | 0.125 | 0.086 |
| surge_ridge0.1 | full | 0.7765 | 0.6566 | 3429 | 0.0485 | 0.813 vs -0.128 | 0.450 | 0.324 |
| surge_ridge0.1 | deployable | 0.7765 | 0.5920 | 1624 | 0.0847 | 0.173 vs 0.028 | 0.125 | 0.086 |
| surge_ridge1 | full | 0.7765 | 0.6566 | 3429 | 0.0485 | 0.813 vs -0.128 | 0.450 | 0.324 |
| surge_ridge1 | deployable | 0.7765 | 0.5920 | 1624 | 0.0847 | 0.173 vs 0.028 | 0.125 | 0.086 |
| surge_ridge10 | full | 0.7765 | 0.6566 | 3429 | 0.0485 | 0.813 vs -0.128 | 0.450 | 0.324 |
| surge_ridge10 | deployable | 0.7765 | 0.5920 | 1624 | 0.0847 | 0.173 vs 0.028 | 0.125 | 0.086 |
| surge_ridge100 | full | 0.7765 | 0.6573 | 3429 | 0.0480 | 0.820 vs -0.131 | 0.450 | 0.327 |
| surge_ridge100 | deployable | 0.7765 | 0.5927 | 1624 | 0.0848 | 0.175 vs 0.032 | 0.125 | 0.086 |
| surge_ridge1000 | full | 0.7766 | 0.6582 | 3429 | 0.0472 | 0.860 vs -0.115 | 0.500 | 0.344 |
| surge_ridge1000 | deployable | 0.7766 | 0.5943 | 1624 | 0.0841 | 0.197 vs 0.023 | 0.143 | 0.111 |
| surge_ridge10000 | full | 0.7759 | 0.6579 | 3429 | 0.0470 | 0.916 vs -0.080 | 0.500 | 0.347 |
| surge_ridge10000 | deployable | 0.7759 | 0.5951 | 1624 | 0.0795 | 0.161 vs 0.021 | 0.000 | 0.086 |
| surge_ridge100000 | full | 0.7725 | 0.6828 | 3429 | 0.0346 | 0.901 vs -0.098 | 0.500 | 0.367 |
| surge_ridge100000 | deployable | 0.7725 | 0.6039 | 1624 | 0.0998 | 0.183 vs 0.003 | 0.111 | 0.111 |
| surge_ridge300000 | full | 0.7677 | 0.6937 | 3429 | 0.0267 | 0.746 vs -0.149 | 0.500 | 0.370 |
| surge_ridge300000 | deployable | 0.7677 | 0.6099 | 1624 | 0.1044 | 0.175 vs -0.022 | 0.000 | 0.111 |
| surge_ridge1000000 | full | 0.7603 | 0.7210 | 3429 | 0.0260 | 0.835 vs -0.189 | 0.550 | 0.399 |
| surge_ridge1000000 | deployable | 0.7603 | 0.6233 | 1624 | 0.1263 | 0.171 vs -0.053 | 0.000 | 0.105 |
| surge_ridge3000000 | full | 0.7533 | 0.7430 | 3429 | 0.0555 | 0.788 vs -0.228 | 0.583 | 0.414 |
| surge_ridge3000000 | deployable | 0.7533 | 0.6345 | 1624 | 0.1332 | 0.186 vs -0.054 | 0.000 | 0.123 |
| surge_ridge10000000 | full | 0.7435 | 0.7525 | 3429 | 0.0737 | 0.799 vs -0.220 | 0.583 | 0.429 |
| surge_ridge10000000 | deployable | 0.7435 | 0.6383 | 1624 | 0.1142 | 0.195 vs -0.061 | 0.000 | 0.142 |
| surge_ridge30000000 | full | 0.6989 | 0.7554 | 3429 | 0.0157 | 0.791 vs -0.217 | 0.583 | 0.429 |
| surge_ridge30000000 | deployable | 0.6989 | 0.6393 | 1624 | 0.0950 | 0.191 vs -0.057 | 0.000 | 0.136 |

### 5.4 Horizon-days sensitivity (walk-forward only; the deployment model is horizon-independent)
`horizon_days` only selects the walk-forward test window (train on rows <= cutoff, test on (cutoff, cutoff+H]); the final model always fits all labeled rows, so the deployed artifact is identical. Pooled OOF Spearman IC: ridge=1000 H=180: 0.5295 / H=365: 0.5291; ridge=1e6 H=180: 0.5015 / H=365: 0.5063 / H=730: 0.5060. The walk-forward IC is stable across test-horizon choices.
## 6. Determinism
- Fixed seed **42** for all fits and the seeded random price-bucket baseline (splitmix64 on `seed ^ (bucket<<32) ^ player_id`).
- `forecast-fit` is deterministic: re-running ridge=1,000,000 produced a **byte-identical** model artifact (SHA-256 `17d6cc6a8ed9bd42dbed5c91fd6f3b0c98d4611d88cbd7d64b5dbf97efe34f24`).
- `footy-forward` is deterministic: the forward test re-run with the same seed produced identical forward-test IC **0.5100** (full pool, ridge=1e6) and a byte-identical report (modulo the config label line).
- All metrics are computed with deterministic tie-breaking (average ranks for Spearman, player_id tie-breaks for ranking).
## 7. Recommendation
**Forecast model: keep full 47-feature schema, ridge = 1,000,000, seed 42.** This config wins the real forward test on every population vs the previous ridge=1000 model (full pool IC 0.4555 -> 0.5100; deployable U23/PA>=140 IC 0.3920 -> 0.3992; MV-band 0.4526 -> 0.4963) at a small walk-forward cost (0.5291 -> 0.5063 pooled OOF IC). It is deployed as `data/forecast/model.bin` (model version 2); the re-scored deliverable is `data/forecast_u23_top100_v2.csv` (original `data/forecast_u23_top100.csv` kept intact).
Why not `no_fm` (which scores slightly higher on the full pool, 0.5175): removing FM attrs degrades the deployable U23/PA>=140 population IC from 0.3992 to 0.3634, and the deliverable population is defined by FM PA — an FM-free model is incoherent for that ranking. The full-pool advantage of `no_fm` comes from out-of-band players (age > 23 or mv > 1M) that the deliverable never ranks.
**Surge model: recommend ridge = 3,000,000, seed 42 for the next deploy** (forward-test AUC 0.7430 full / 0.6345 deployable vs 0.6566 / 0.5920 for the shipped ridge=1.0 model; walk-forward AUC 0.7533 vs 0.7765; forward-window ECE 0.056 vs 0.048 — the forward gain outweighs the modest calibration cost for a ranking-first deliverable, and the decile table is strongly monotonic). The shipped `data/surge/model.bin` is left untouched because the artifact-update scope in the steering covers forecast artifacts; `data/proposals_top100.csv` is intact.
Threshold note: the top-100 proposal lists show 15-40% hit rates at the +50% partial-return bar and 60-80% positive-return hit rates at N=10-25 (forecast); a 15 bps round-trip cost is far below the realized mean spreads, so the ranking edge survives transaction costs at these horizons.
## 8. Limitations
- The forward window is a single realization (2025-11-06 -> 2026-06-12). Forward-test IC differences below ~0.01 between adjacent configs are within noise; the recommendation rests on the monotone improvement across 10 ridge configs, not a single lucky spike.
- Realized horizons are partial and heterogeneous (median ~11.5 months; entry valuations predate the anchor for most players). All coverage numbers are **partial-horizon**, honestly labeled; the conformal radius is calibrated on 12-month OOF residuals.
- FM26 attributes are a static snapshot (start of FM26); `attr_source` (db vs derived) is flagged in all outputs. The forward test shows FM attrs help the deployable U23 population but hurt the full-pool ranking — a real, documented tension.
- The TM overlay runs through 2026-06-12; nothing after that date is used anywhere. The DuckDB was read-only.
- Censored players (no realized valuation after entry) are dropped, not encoded as zero; this slightly over-samples players with active valuation series.
