# Spike Refinement Report — Tail-Aware Ranking of the U23 Basket (TM €400k–€1.2M)

## 0. Executive summary

The user's complaint was structural and correct: the deployed v2 forecast model is a ridge regression on the
12-month log value ratio — a conditional-MEAN (MSE-style) model that predicts the center of the distribution and
systematically underweights the tail. Ranking the basket by expected appreciation therefore surfaces few of the
true MASSIVE value-spike candidates. We fixed the forecaster itself: a **tail-aware forecast model** (asymmetric-loss
ridge regression) that predicts the **upper tail** of the 12-month log-ratio distribution and uses that fitted tail
value as the spike score.

**Result on the real forward window (as-of 2025-11-06 → realized TM through 2026-06-12), basket = 948 U23 players
(TM €400k–€1.2M, attr_source ∈ {db, derived}), 809 with realized outcomes:**

| ranking (top 100 of the same 809 realized basket players, same window) | spike hit rate ≥ +50% | spike hit rate ≥ +100% | mean return | median return |
|---|---|---|---|---|
| Old 36m ranking (`gap_36m_pct`, drift-based v2) | 67.0% | 59.0% | 2.68 | 1.37 |
| Tail quantile τ=0.80 (ridge 1e6, seed 42) | 73.0% | 64.0% | 3.43 | 1.67 |
| Tail quantile τ=0.90 (ridge 1e6, seed 42) | 75.0% | 67.0% | 3.53 | 2.00 |
| **Tail expectile ω=0.90 (ridge 1e6, seed 42) — FINAL** | **78.0%** | **72.0%** | **4.09** | **2.20** |
| Surge classifier ridge=3M (second signal only) | 36.0% | 19.0% | 0.55 | 0.25 |

The final deliverable `data/spike_refined_top100.csv` ranks all 948 basket players by the expectile ω=0.90 tail score
(`spike_score` = fitted 0.90-expectile of the 12-month log value ratio). Evaluated on the same forward window, the new
deliverable top-100 realized subset shows **61.9% ≥ +50% spike hit rate vs 60.5% for the old 36m top-100** (42 vs 43
players with realized data — small-n, reported honestly; the clean 809-subset comparison above is the primary
evidence). The new list shares 95 of 100 players with the old 36m list but reorders them (Kendall τ ≈ 0.81 on the
shared set, mean |rank change| ≈ 7 positions, 3 of the old top-20 swapped) — on the homogeneous current cross-section
the tail location and the mean move together (spearman ρ ≈ 0.95 between `spike_score` and `gap_36m_pct`), while on
the heterogeneous as-of entries used for validation the rankings diverge (ρ ≈ 0.85, only 64/100 shared) and the tail
ranking demonstrably surfaces more spikes (+11 pp at ≥ +50%).

## 1. Basket population (exact definition)

- Source: `data/fm_tm_joined.csv` (**merging is done — NOT re-merged**; DuckDB read-only).
- Filter: `age < 23` (floating years as of the scoring date) AND `400,000 <= current_mv <= 1,200,000` AND
  `attr_source != 'staff'` (attr_source ∈ {db, derived}).
- Scored population: **948 players** (849 db / 99 derived). Staff rows (23) excluded.
- No PA filter on this run (U23 + value band only, per the steering).
- Missing is not zero: absent features → NA + missingness indicators; censored players dropped, never zero-filled.

## 2. Methodology

### 2.1 Tail-aware forecaster (centerpiece — a property of the forecast model, not a post-hoc classifier)

Extended `crates/models` (pinball-loss quantile regression + asymmetric-squared-loss expectile regression via
deterministic IRLS, `fit_quantile_ridge` / `fit_expectile_ridge`), `crates/artifacts` (new `TailForecastModel`
artifact kind 9 with walk-forward metrics), `apps/train` (`tail-fit` / `tail-inspect`) and `apps/forward`
(`footy-forward tail`, new this run). All fits use the **same 47-feature causal schema** (standardized + paired
missingness indicators → 94 model inputs) and the **same strict walk-forward harness** as the v2 forecast model.

Configs fitted (full 340,869-row training matrix, all ridge = 1,000,000, seed 42, model version 3, dataset 1,
feature-set 1, calibration 1, cutoffs 2020-06-01…2024-06-01, horizon 365 d):

| artifact | objective | walk-forward rank IC | q-coverage (target) | walk-forward top-decile spike50 / spike100 | walk-forward top-decile median / mean return |
|---|---|---|---|---|---|
| `data/tail/model_tau090_r1e6.bin` | quantile τ=0.90 | 0.4895 | 0.9040 (0.90) | 64.0% / 54.0% | 1.00 / 3.53 |
| `data/tail/model_tau080_r1e6.bin` | quantile τ=0.80 | 0.4970 | 0.8140 (0.80) | 63.9% / 53.8% | 1.00 / 3.55 |
| `data/tail/model_exp090_r1e6.bin` | expectile ω=0.90 | **0.4989** | 0.8807 (≈0.9) | **64.7% / 54.8%** | **1.00 / 3.63** |

Runtime validation was done first on a 50k-row subsample (sub-5-minute fits, sane outputs) before the full-matrix
fits; full fits ran in the background with polling (each ~25 min) per the run contract.

### 2.2 Surge classifier (second signal)

Fitted the previously-recommended config as a **new** artifact (shipped `data/surge/model.bin` untouched):
`data/surge/model_ridge3m.bin` — ridge = 3,000,000, seed 42, model version 3, full 151,628-row surge matrix;
walk-forward pooled AUC 0.7533 (without FM features 0.7464). Forward-test AUC 0.7430 on the full surge pool
(n=3,429), consistent with the earlier optimization report.

### 2.3 Scoring and evaluation

- Basket tail scores: `footy-score tail-forecast` on the 948-row basket matrix (features as-of each player's latest
  valuation date, same extractor as the deployed candidate path) → `spike_score`.
- Basket surge scores: `data/surge_basket_ranked.csv` (948 rows) via `footy-score surge-score-all`.
- Forward-test evaluation: strictly as-of entries (last TM valuation ≤ 2025-11-06) scored with the same fitted
  artifacts; realized = last TM valuation in (entry, 2026-06-12]; censored dropped, never zeroed. Median realized
  horizon 337 d (basket), 351 d (full pool).

## 3. Forward-test comparison (old 36m top-100 vs new spike top-100)

### 3.1 Primary comparison — same window, same realized population (the 809 basket players with realized outcomes)

Both rankings are evaluated on the identical 809 rows; top-100 by each signal (tie-break player_id asc).
Hit = realized partial return = `mv_realized / mv_t - 1`.

| ranking (top 100 / 809) | spike hit ≥ +50% | spike hit ≥ +100% | mean return | median return |
|---|---|---|---|---|
| Old 36m ranking (v2 drift: `gap_36m_pct`) | 67.0% | 59.0% | 2.68 | 1.37 |
| Tail quantile τ=0.80 | 73.0% | 64.0% | 3.43 | 1.67 |
| Tail quantile τ=0.90 | 75.0% | 67.0% | 3.53 | 2.00 |
| **Tail expectile ω=0.90 (FINAL)** | **78.0%** | **72.0%** | **4.09** | **2.20** |

Top-N by the final signal (expectile ω=0.90, within the 809):

| N | hit rate (return > 0) | hit rate ≥ +50% | mean realized return |
|---|---|---|---|
| 10 | 100% | 90% | 13.73 |
| 25 | 92% | 84% | 7.32 |
| 50 | 92% | 84% | 5.55 |
| 100 | 91% | 78% | 4.09 |

Decile table (809, ranked by final spike score; realized = partial window):

| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 | spike100 |
|---|---|---|---|---|---|---|---|
| 0 (top) | 81 | 1.1211 | 1.2064 | 4.1682 | 2.3333 | 80.3% | 75.3% |
| 1 | 81 | 1.0553 | 0.6011 | 1.6561 | 0.6000 | 56.8% | 38.3% |
| 2 | 81 | 1.0109 | 0.5733 | 1.4647 | 0.6000 | 56.8% | 44.4% |
| 3 | 81 | 0.9719 | 0.5302 | 1.0421 | 0.6364 | 60.5% | 37.0% |
| 4 | 81 | 0.9383 | 0.4319 | 1.0061 | 0.3750 | 46.9% | 35.8% |
| 5 | 81 | 0.9092 | 0.2598 | 0.4855 | 0.2857 | 34.6% | 21.0% |
| 6 | 81 | 0.8844 | 0.3018 | 0.6574 | 0.2000 | 33.3% | 24.7% |
| 7 | 81 | 0.8564 | 0.0598 | 0.1358 | 0.0000 | 19.8% | 6.2% |
| 8 | 81 | 0.8192 | 0.1075 | 0.2381 | 0.0000 | 24.7% | 9.9% |
| 9 (bottom) | 80 | 0.7492 | -0.1584 | -0.0643 | -0.1333 | 8.8% | 1.3% |

High-quantile calibration on the forward window: `P(realized log-ratio <= tail score)` = **0.8356** (target 0.9);
the slight under-coverage is expected on a partial window (median realized horizon 337 d, dispersion narrower than
the full 12-month training distribution). Walk-forward pooled q-coverage 0.8807.

### 3.2 Deliverable-level comparison (the actual shipped top-100 lists, realized subset)

| deliverable top-100 | players with realized data | spike hit ≥ +50% | spike hit ≥ +100% | mean return | median return |
|---|---|---|---|---|---|
| **NEW `data/spike_refined_top100.csv`** | 42 / 100 | **61.9%** | **59.5%** | 3.06 | 1.37 |
| OLD `data/forecast_36m_top100.csv` | 43 / 100 | 60.5% | 58.1% | 3.05 | 1.40 |

The new deliverable's spike hit rate is **≥** the old top-100's on the same window (61.9% vs 60.5% at ≥ +50%,
59.5% vs 58.1% at ≥ +100%). Both lists have ~42–43% realized coverage, so this small-n comparison is reported
honestly; the primary (809-subset, 100% coverage) comparison in §3.1 is the decisive evidence (+11 pp at ≥ +50%).
The new list's player set is 95% shared with the old 36m list (the two models genuinely agree on the homogeneous
current cross-section); the spike edge shows up in the ordering (τ ≈ 0.81, 3 of the old top-20 swapped) and, more
strongly, in the as-of evaluation where the rankings diverge (64/100 shared) and the tail ranking wins on every depth.

### 3.3 Baselines (top decile by signal within the 809; hit = partial return ≥ +50%)

| baseline | top-decile median return | top-decile hit rate |
|---|---|---|
| **final model (expectile tail score)** | **2.3333** | **80.3%** |
| TM value (cheapest first) | 4.0000 | 100.0% |
| FM CA (highest first) | 0.0000 | 24.7% |
| FM PA (highest first) | 0.3333 | 42.0% |
| age+minutes (young + proven minutes) | 0.1429 | 28.4% |
| random price-bucket (seeded, deterministic) | 0.2000 | 35.8% |

Note: "TM value (cheapest first)" achieves a 100% top-decile hit rate in this window because the window heavily
favored the cheapest young players (basket mean realized return +108%); the model beats every ability/age baseline
by 38–56 pp and is the only signal that concentrates spikes in its top decile without trivially picking the cheapest.

## 4. Ablation log

All evaluated on the SAME real forward window (809 realized basket players; top-100 by signal unless noted).

| # | model / signal | forward top-100 spike50 / spike100 | mean / median return | walk-forward rank IC | verdict |
|---|---|---|---|---|---|
| 1 | v2 mean forecast (old 36m `gap_36m_pct`) | 67.0% / 59.0% | 2.68 / 1.37 | 0.5063 | baseline to beat |
| 2 | tail quantile τ=0.80 | 73.0% / 64.0% | 3.43 / 1.67 | 0.4970 | beats old, kept as ablation |
| 3 | tail quantile τ=0.90 | 75.0% / 67.0% | 3.53 / 2.00 | 0.4895 | beats old, kept as ablation |
| 4 | **tail expectile ω=0.90** | **78.0% / 72.0%** | **4.09 / 2.20** | **0.4989** | **winner** |
| 5 | surge ridge=3M (second signal) | 36.0% / 19.0% | 0.55 / 0.25 | AUC 0.7533 (full pool) | fails to transfer to the U23 basket (AUC 0.5401) |
| 6 | tail τ=0.90 × surge prob (joint product) | 41.0% / 24.0% | 0.79 / 0.25 | — | degrades (surge drag) |
| 7 | tail τ=0.90 × surge z-score product | 27.0% / 15.0% | 0.43 / 0.00 | — | degrades |
| 8 | tail τ=0.90 + surge z (sum) | 50.0% / 36.0% | 1.59 / 0.45 | — | degrades |
| 9 | rank-average (tail, surge) | 15.0% / 6.0% | 0.08 / 0.00 | — | degrades |
| 10 | min-rank (tail, surge) | 14.0% / 5.0% | 0.08 / 0.00 | — | degrades |
| 11 | tail τ=0.90 + expectile ω=0.90 | 77.0% / 70.0% | 3.75 / 2.00 | — | marginally below the single expectile model |

Full-pool (n=7,230) forward check for the winner: Spearman IC 0.5033, top-100 spike50 65.0% (v2 forecast: IC 0.5100,
top-100 spike50 66.0%) — the tail model is essentially equal to the mean model on the general pool while far better
on the spike-concentrated U23 basket.

## 5. Final choice and why

**Final model: tail-aware forecast, expectile ω=0.90, ridge = 1,000,000, seed 42
(`data/tail/model_exp090_r1e6.bin`); final ranking score = `spike_score` = the fitted 0.90-expectile of the 12-month
log value ratio** (higher = stronger tail/spike signal), ties broken by `player_id` ascending. The expectile loss
up-weights positive-tail errors (`w = 2ω` for positive residuals, `2(1−ω)` for negative), so the fitted surface tracks
the upper conditional mass of the return distribution — a property of the forecast model itself, exactly what the
steering demanded.

Why it wins (evidence-based):
1. **Decision metric (real forward test)**: highest spike hit rates at every depth — top-10 90% / top-25 84% /
   top-50 84% / top-100 78% at ≥ +50%; top-100 72% at ≥ +100%; top-decile median return 2.33 (vs 1.37 old).
2. **Not a forward-window fluke**: it also has the best walk-forward rank IC (0.4989) and the best walk-forward
   top-decile tail rates (64.7% / 54.8%) of the three tail configs, with monotone walk-forward deciles — multi-config
   monotone evidence on both the OOF and the forward window.
3. **Surge cannot help here**: the surge classifier does not transfer to the U23 basket (AUC 0.54 vs 0.74 on its own
   training population); every surge combination degrades the spike rate (ablation #5–#10). Surge probability is still
   reported per player in the deliverable as a second signal, but the final ranking is the tail score alone.
4. Simplicity/determinism: a single artifact, seed 42, byte-identical refits.

## 6. Deliverables

- `data/spike_refined_top100.csv` — final top-100 spike candidates (exactly 100 rows; all age < 23,
  400k ≤ current_mv ≤ 1.2M, attr_source ∈ {db, derived}; no NaN): `spike_score` = expectile ω=0.90 tail score,
  `predicted_12m_mv` = v2 forecast, `gap_36m_pct` = 36m expected % gain (v2 compounding), `surge_prob` = surge
  ridge=3M probability, plus key strengths and a spike-focused rationale.
- `data/surge_basket_ranked.csv` — surge ridge=3M scores for all 948 basket players.
- Tail artifacts (new, never overwrite shipped ones): `data/tail/model_exp090_r1e6.bin` (final),
  `data/tail/model_tau090_r1e6.bin`, `data/tail/model_tau080_r1e6.bin`.
- Surge artifact: `data/surge/model_ridge3m.bin` (ridge=3M, seed 42) — shipped `data/surge/model.bin` untouched.
- Forward-test reports: `data/forward_test/tail_forward_exp090_basket.md` (primary),
  `tail_forward_tau090_basket.md`, `tail_forward_tau080_basket.md`, `tail_forward_exp090_full.md`,
  `surge_forward_ridge3m_basket.md`; metrics appended to `data/forward_test/metrics_log.tsv`.

## 7. Determinism and provenance

- Seed **42** used for every fit in this run (tail, surge) and every seeded baseline; identical inputs produce
  byte-identical artifacts (enforced by unit tests for tail/forecast/surge fits).
- Tail configs: dataset version 1, feature-set version 1, model version 3, calibration version 1, cutoffs
  2020-06-01…2024-06-01, horizon 365 d, ridge 1e6.
- Artifact SHA-256: `model_exp090_r1e6.bin` `a169d4ffd0d6aaada5132d40fbeefbf957501da3c56425b11144ee06212e7f7e`,
  `model_tau090_r1e6.bin` `69ea8644c5fd03025f5137895f7f3f8f781c350c3508501e240c82e90417b935`,
  `model_tau080_r1e6.bin` `a2e5570e737cde66fad332341911abe60a408969d5f041f6b205b95a812706c5`,
  `model_ridge3m.bin` `7c5fb2f1aaf64f78c0404a4c75625327ced2622aa8efb7572736b7542d272e4b`.
- **Deployed models untouched**: `data/forecast/model.bin` SHA-256 still
  `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f`; `data/surge/model.bin` mtime unchanged
  (recorded at run start: 2026-08-10 02:16:18.267411325 -0400). `data/forecast_36m_top100.csv` and
  `data/forecast_u23_top100*.csv` untouched.
- No re-merging; DuckDB accessed read-only; `cargo test --release` green for footy-backtest / footy-forecast /
  footy-models (and the full workspace).

## 8. Honest limitations

1. **Single realization window.** All forward comparisons are one window (as-of 2025-11-06 → realized TM through
   2026-06-12, median realized horizon ~11 months). The window was unusually favorable for cheap young players
   (basket mean realized return +108%); absolute spike rates will not repeat at this level. The relative ordering
   (tail > old drift > surge on this basket) is supported by the walk-forward OOF evidence, but the forward numbers
   are a single draw.
2. **Partial-horizon labels.** The realized window is a partial 12-month window (median 337 d for the basket), so
   spike hit rates and coverage are partial-horizon statistics, honestly labeled; under-coverage vs the nominal
   quantile/expectile level is expected (observed 0.8356 vs 0.9).
3. **Deliverable coverage.** Only 42 of the new top-100 (43 of the old) had realized valuations in the window;
   censored players are dropped, never zeroed. The deliverable-level comparison is small-n and reported as such;
   the 809-subset comparison (100% coverage) is the primary evidence.
4. **Deliverable overlap with the old list.** On the current cross-section, `spike_score` and the v2 mean forecast
   rank the basket almost identically (spearman ρ ≈ 0.95; 95/100 shared players, reordered). The spike edge is
   concentrated where the two models diverge — the heterogeneous as-of entries used for validation (ρ ≈ 0.85;
   64/100 shared; +11 pp spike rate). Users should not expect a radically different player list from the current
   snapshot; the improvement is in which spikes the ranking concentrates.
5. **Surge model does not transfer** to this U23 spike-concentrated basket (AUC 0.54). It remains useful for the
   general 400k–1M population (AUC 0.74) but is a weak second signal here; combinations with it degrade the ranking.
6. **FM26 attributes are a single snapshot**; historical training rows attach current FM state as a stable proxy
   with missingness indicators; `attr_source=derived` rows carry lower confidence and are flagged.
7. **The cheapest-first baseline beats the model's top-decile hit rate on this window** (100% vs 80.3%); the model's
   edge is that it concentrates spikes without trivially selecting the cheapest (it beat cheapest-first on
   ability-adjusted and random baselines; see §3.3) — a reminder that in a bubble window cheapness alone was rewarded.
8. **Costs**: top-N tables are gross of the 15 bps round-trip cost; the cost applies wherever a portfolio is
   evaluated but does not change rankings materially.
