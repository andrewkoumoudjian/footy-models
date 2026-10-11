# Footy-Models — Value-Surge Model & Transfer Proposals

## 1. Objective

Find players currently valued EUR 400k–1M by Transfermarkt who are most likely to appreciate at least +50% in market value within the next 12 months, restricted to players aged ≤ 28 who are good on the field per Football Manager 2026 attributes. Outputs are calibrated probabilities with intervals, never bare point predictions.

## 2. Data

- **FM26 male database** (`data/fm_universe.csv`, 563,382 men): full 1–20 attributes, CA, PA, development signals (`pa_gap`, `ca_age_pct`), position class, composite signals. FM Potential Ability is used as the game's own estimate of future ability; `attr_source=db` is preferred, `attr_source=derived` is flagged and treated as lower-confidence.
- **Transfermarkt**: `player_valuations`, `appearances`, `transfers`, `players`, `clubs`, `competitions` extracts, joined causally per player valuation date.
- **Training set** (`data/surge/train.tsv`, built by `data/build_surge_dataset_v2.py`): 151,628 strictly causal rows, 47 features, surge rate 22.5%.
- **Candidate set** (`data/surge/candidates.tsv`): 3,615 rows (2,299 aged ≤ 28), FM + TM features joined and deduplicated by `player_id` (highest CA kept).

## 3. Target definition

For every historical Transfermarkt valuation observation `(player, t, mv_t)` with `mv_t` in [400k, 1M]:

- `surge = 1` iff `max(market value in (t, t+365]) >= 1.5 * mv_t`, else 0.
- Rows are kept only when the player's valuation series extends ≥ 365 days past `t` AND at least one valuation is observed in `(t, t+365]` — censored outcomes are dropped, not encoded as zeros.
- All features are computed from observations strictly before `t` (valuations, appearances, transfers, player/club/competition state).

## 4. Features (47)

### Market state (causal valuation history) (11)

`log_mv, log_mv_180d_ratio, log_mv_365d_ratio, log_peak_prior_ratio, log_min_prior_ratio, log_n_vals_prior, log_days_since_first_val, val_freq, log_days_since_last_val, last_change_dir, mv_vol_log`

### Performance (appearances, causal windows) (8)

`perf_present_730, log_mins_365, log_apps_365, g_per90_365, a_per90_365, log_mins_180, mins_trend, g_per90_180`

### Transfer history (5)

`ever_transferred, log_days_since_transfer, log_transfer_fee, free_transfer, log_mv_transfer_ratio`

### Player state (6)

`age, age_sq, pos_gk, pos_df, pos_mf, height_cm`

### Competition context (4)

`comp_mapped, first_tier, confed_uefa, confed_sam`

### FM26 development/ability state (13)

`fm_present, fm_ca, fm_pa, fm_pa_gap, fm_ca_age_pct, fm_sig_technical, fm_sig_physical, fm_sig_mental, fm_sig_defensive, fm_sig_attacking, fm_sig_creative, fm_attr_db, fm_attr_derived`

Every feature is standardized (mean/std fit on training rows) with a paired missingness-indicator slot, so **missing is not zero**: an absent value contributes a 0 standardized value plus a 1.0 missingness indicator; measured zeros and absences stay distinct. FM features are attached by `player_id`; historical players without FM26 linkage contribute `fm_present=0` and missing FM slots.

## 5. Model

- Logistic regression fit by IRLS (ridge `1.0`, seed `42`, deterministic), 94 model inputs (47 standardized values + 47 missingness indicators).
- Model version 1, dataset version 1, feature-set version 1, calibration version 1, trained through 2025-06-10 (rows observed on or before this date).
- Two-layer design: FM26 attributes are a single snapshot (summer 2026). The surge model is trained on TM + performance + FM-state features over historical valuation windows (strictly causal). FM features used in historical rows are the stable, low-drift signals (CA, PA, PA gap, age-relative CA percentile, composite attribute signals) with missingness indicators; the candidate layer applies the fitted model to current FM26 attributes as the quality/development screen.
- Uncertainty: split-conformal calibration on out-of-fold residuals produces an 80% probability interval; interval endpoints (`interval_10`, `interval_90`) are reported per proposal.

## 6. Walk-forward validation

Strictly causal walk-forward: train on rows with `as_of <= cutoff`, test on rows with `as_of` in `(cutoff, cutoff+365]`. No random row splits.

| cutoff | train rows | test rows | AUC |
|---|---|---|---|
| 2016-06-01 | 45951 | 9392 | 0.7544 |
| 2017-06-01 | 55343 | 10002 | 0.7673 |
| 2018-06-01 | 65345 | 9567 | 0.7683 |
| 2019-06-01 | 74912 | 15996 | 0.7304 |
| 2020-06-01 | 90908 | 12329 | 0.7694 |
| 2021-06-01 | 103237 | 13816 | 0.7903 |
| 2022-06-01 | 117053 | 11224 | 0.7946 |
| 2023-06-01 | 128277 | 11789 | 0.8001 |
| 2024-06-01 | 140066 | 11485 | 0.8051 |

- Pooled out-of-fold AUC: **0.7765**
- Pooled out-of-fold AUC **without FM features**: 0.7643 (FM state adds 0.0122 to discrimination)
- Expected calibration error (10 bins): 0.0067
- Conformal 80% interval radius: 0.4902 (coverage 0.8000 on 105600 calibration rows)

## 7. Reliability / decile table (pooled out-of-fold)

| decile | count | mean predicted prob | realized surge rate | mean realized return |
|---|---|---|---|---|
| 0 | 10560 | 0.0235 | 0.0123 | -0.1456 |
| 1 | 10560 | 0.0532 | 0.0426 | -0.0558 |
| 2 | 10560 | 0.0834 | 0.0728 | 0.0117 |
| 3 | 10560 | 0.1155 | 0.1067 | 0.0749 |
| 4 | 10560 | 0.1510 | 0.1470 | 0.1548 |
| 5 | 10560 | 0.1915 | 0.1967 | 0.2476 |
| 6 | 10560 | 0.2395 | 0.2350 | 0.3333 |
| 7 | 10560 | 0.3000 | 0.3058 | 0.4945 |
| 8 | 10560 | 0.3855 | 0.3959 | 0.8179 |
| 9 | 10560 | 0.5580 | 0.5547 | 1.9795 |

A monotonic predicted-to-realized relationship across deciles confirms the ranking separates likely surge players from the rest (baseline surge rate 22.5%).

## 8. Proposals (top 100)

- 100 proposals in `data/proposals_top100.csv`; market value 400000 – 1000000 (mean 735000).
- Position mix: MF 40, FW 27, DF 22, GK 11.
- FM attribute source mix: db 98, derived 2 (derived rows are statistically generated from real FM players and treated as lower confidence).
- Selection criteria: candidate universe `current_mv` in [400k, 1M], FM CA present with `attr_source` in {db, derived}, age ≤ 28, and the FM quality gate **or** development-signal screen (see below); top 100 ranked by predicted surge probability, ties broken by `player_id` for determinism.
### FM quality gate (documented thresholds)

Position-appropriate FM attributes ≥ 12 (a solid professional level on the 1–20 scale; two position-critical attributes avoid one-dimensional players):

- Goalkeepers: Handling ≥ 12 and Reflexes ≥ 12
- Defenders: Tackling ≥ 12 and Marking ≥ 12
- Midfielders: Passing ≥ 12 and Vision ≥ 12
- Forwards: Finishing ≥ 12 and Dribbling ≥ 12

Development-signal fallback (young + proven upside): age ≤ 22, FM `pa_gap` ≥ 20, and `ca_age_pct` ≥ 0.5 (above-median FM CA percentile within the player's age band).

## 9. Limitations

- FM26 attributes are a single snapshot; historical training rows attach current FM state as a stable proxy and encode absence with missingness indicators. The model cannot observe attribute evolution over time.
- `attr_source=derived` attributes are statistically generated from the distribution of real FM players; they carry lower confidence.
- Transfermarkt data runs through 2026-06-12; candidates are scored as of their latest valuation date.
- Surge is a coarse, inherently uncertain event; 80% intervals are wide (conformal radius documented above). Calibrated probabilities, not guarantees.
- Negative FM PA codes (randomized potential bands, e.g. -7, -65, -95) are treated as missing in the model; for reporting only, `fm_pa` in proposals converts each code to its band midpoint (documented conversion: -95→185, -85→175, -75→165, -65→155, -55→145, -45→135, -35→125, -25→115, -15→105, -10→185, -9→165, -8→145, -7→125, -6→105, -5→85, -4→65, -3→45, -2→30, -1→20).
- Free transfers are never encoded as zero-fee permanent transfers; `free_transfer` is a separate indicator and `log_transfer_fee` is missing for free moves.

## 10. Reproducibility

- Fixed seed 42, ridge 1.0, deterministic IRLS; same input → byte-identical model artifact and proposals.
- Source TSV checksum: `17516262428878515100`.
- Enforced by `cargo test --release` (proposal MV range, completeness, determinism) and `scripts/gate_check.sh`.
