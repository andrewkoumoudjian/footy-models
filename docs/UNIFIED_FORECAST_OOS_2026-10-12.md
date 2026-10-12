# Unified EPL+MLS forecasting engine: OOS validation + integrated forecasts — 2026-10-12

**Status: implemented, executed, research-only.** Mathematical pathway only; no
deep-ML training. One pooled, player-agnostic residual model family (shots,
conversion, participation) over shared as-of empirical-Bayes baselines, plus
rolling-origin validation of the production Rust team-goal model and
end-to-end corrected forecasts from the coherent Rust match simulator.
Identity-recovery work was deliberately not extended; genuine blockers are
recorded as data gaps, not fixed with guesses.

## 1. What was implemented (source only; data artifacts stay gitignored)

| Change | File |
|---|---|
| Pooled 3-head fit with real EPL FM linkage, temperature calibration, matchday-cluster bootstrap CIs | `scripts/fit_unified_forecast_heads.py` |
| Adapter writing pre-match `corrected_shots_per90` into engine inputs | `scripts/apply_pooled_correction.py` |
| Rust pooled-correction primitive (mirrors the Python clip/predict math) + golden tests | `crates/forecast/src/pooled_correction.rs` |
| Rolling-origin OOS scoring for `TeamGoalModel` (proper losses, ECE, bootstrap) | `crates/match/src/oos.rs` |
| Full 13x13 scoreline PMF consistent with `score_distribution` | `crates/match/src/team_goals.rs` |
| Thin rolling-origin CLI over dated results CSV/TSV | `apps/matchforecast/src/main_team_goals_oos.rs` (`footy-team-goals-oos`) |
| Engine consumes optional per-player corrected rates with provenance | `apps/matchforecast/src/math_engine_main.rs` |
| Regression tests (causality, gates, binomial math, artifact) | `tests/test_unified_forecast_heads.py` |

## 2. Key modeling fix: EPL rows carry real FM traits now

`fit_pooled_epl_mls_shot_model.py` gave every EPL row all-zero FM features by
construction. The new fit joins the 129 verified ESPN↔FM crosswalk identities
(`espn_pipeline_final_oct12/crosswalk.json`, exact-name or legal token-pair
only) to usable complete-everyone FM26 union rows (same kind/provenance gate
as the MLS labels), with a closed exact club-alias map
(Coventry City→Coventry, Hull City→Hull, Newcastle United→Newcastle,
Manchester City→Man City, Nottingham Forest→Nott'm Forest). Result: 523/2240
EPL rows verified (was 0); the rest keep zeros with `fm_verified=0`. EPL test
Poisson loss delta improved from −0.0038 to **−0.0068** with no train-side
change (coefficients identical — the gain is purely better EPL features).

## 3. Out-of-sample results (all chronological; ESPN/MLS labels are retrospective)

```bash
python3 scripts/fit_unified_forecast_heads.py --output data/ml/unified_forecast_heads_v1.json
./target/release/footy-team-goals-oos --asof-csv data/pl/asof_team_stats.csv \
  --results-tsv data/pl/results.tsv --out data/ml/team_goals_rolling_oos_v1.json
```

**Shots head** (pooled; train 1085 MLS / val 276 MLS / test 2181 MLS+EPL):
val 1.1638→1.1485 (Δ−0.0153); test pooled Δ**−0.0075**, 95% matchday-cluster
CI **[−0.0121, −0.0028]** (73 matchdays, excludes 0); MLS test Δ−0.0101, EPL
test Δ−0.0068. Consistent sign in both leagues → **wired into the engine**.

**Conversion head** (ESPN EPL only, 406/176/249 — MLS sets have no goal
labels): validation selects the **baseline** (0.34612 vs 0.34617 best ridge);
test identical; occurrence ECE 0.020, temperature 1.0. Null result → **gated
challenger, not wired in**. Zero-shot test rows (237) scored at rate 0.0 (no
own-goal noise in window).

**Participation head** (ESPN EPL 40-man listings, 1120/480/640 — MLS sets have
no start labels): val 0.5125→0.5101 (L2=0.1); test NLL 0.509→**0.451**, Brier
0.1632→**0.1443**, ECE 0.136→**0.049** (temperature 0.5, validation-selected),
CI [−0.085, −0.030]. Strong but EPL-only and retrospective with an
early-rotation vs settled-lineup regime pattern → **gated challenger pending
prospective validation**, not wired into lineup sampling.

**Team goals** (production `TeamGoalModel`, monthly rolling origin on
1,196 EPL matches 2023–2026, 816 fixtures scored 2024-08→2026-10):
WDL log-loss **1.0115 vs 1.0839** home-advantage-only (Δ−0.072, CI
[−0.093, −0.052]); WDL Brier 0.6055 vs 0.6567; scoreline 2.9395 vs 3.0270;
every season favors the fitted model. This is the engine's actual goal anchor
and it is now OOS-validated.

## 4. Integrated forecasts (Rust engine, 2500 paths, seed 42, as_of 2026-10-10)

```bash
python3 scripts/apply_pooled_correction.py \
  --input data/ml/math_engine/unified_forecast_oct11/inputs_base.json \
  --output data/ml/math_engine/unified_forecast_oct11/inputs_corrected.json
./target/release/footy-matchforecast-math .../inputs_corrected.json .../predictions_corrected.json \
  --paths 2500 --seed 42 --selection-marginals false --max-subs 0
```

129/323 players corrected (all verified crosswalk identities with pre-cutoff
ESPN history); 194 keep engine priors with explicit missing coverage.
Corrections redistribute shots across players (mean |Δ| expected shots 0.095,
top movers up to 1.1) while preserving team means by construction — team-level
WDL/xG move ≤0.01, as expected. MC standard error ≈0.01 on WDL. All coherence
checks pass (probabilities sum to 1, player↔team conservation, analytic
Poisson consistency at zero effects).

| Fixture | Home | Draw | Away | xG | Top P(goal) players (id, P, xG, xS) |
|---|---|---|---|---|---|
| Crystal Palace v Nott'm Forest | 0.373 | 0.264 | 0.362 | 1.26–1.26 | 48042766 (0.130, 0.15, 0.94), 49041006 (0.118, 0.13, 0.92) |
| Hull v Everton | 0.425 | 0.320 | 0.255 | 1.08–0.75 | 45109174 (0.114, 0.12, 1.01), 2000196762 (0.110, 0.12, 1.20) |
| Liverpool v Man City | 0.338 | 0.244 | 0.418 | 1.44–1.64 | 14229525 (0.235, 0.29, 1.24), 29179241 (0.225, 0.27, 2.25) |
| Coventry v Newcastle | 0.290 | 0.248 | 0.463 | 1.20–1.57 | 29193659 (0.152, 0.18, 1.00), 28100207 (0.140, 0.16, 1.09) |

Measured accuracy behind these numbers: team-goal WDL log-loss 1.0115
(rolling OOS) and player-shot Poisson Δ−0.0075 (pooled OOS). The
Liverpool–City window/pressure path in these runs is one Monte Carlo
realization, not evidence of momentum skill — the prespecified test is a
200+ fixture prospective ablation of the (zero-by-default) trailing-score and
man-advantage effects, not run here.

## 5. Tests run

`cargo test -p footy-forecast --lib` (22 ok), `cargo test -p footy-match
--lib` (74 incl. new oos/pmf tests, ok), `cargo test -p footy-matchforecast
--lib math_engine` (6 ok), `python3 -m unittest discover -s tests -p
test_unified_forecast_heads.py` (12 ok), existing pooled/shot-dataset suites
ok. Pre-existing failures unrelated to this change (parallel work's untracked
`apps/matchforecast/src/csv_fields.rs`: 2 CRLF tests + `-D warnings`
dead-code) were left untouched.

## 6. Limitations and next steps

- All ESPN/MLS label OOS is **retrospective-chronological** (archives collected
  post-hoc; splits strictly causal but not prospective). Promotion requires
  prospective validation on future fixtures.
- MLS has no goal/start labels and only ~100 dated scored fixtures: conversion,
  participation and MLS team-strengths are prior-driven until provider coverage
  improves. No zeros were imputed anywhere.
- `build_inputs.py` currently fails on fresh runs: 15 `player_fm_join.csv`
  UIDs are absent from the men extract (7 absent from complete-v6 too — likely
  stale join UIDs, an identity-side issue). Forecasts here reuse the frozen
  `full_run_20261010/inputs.json` plus E1 priors; do not treat that as a
  license to re-guess identities.
- Next, in order: prospective protocol for the participation/conversion
  challengers; MLS score ingestion for team-strength OOS; prespecified
  momentum ablation; interval (conformal) outputs on match probabilities.
