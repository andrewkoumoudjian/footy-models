# Candle FM26 player-shot-rate pilot (research-only)

**Purpose:** Test whether observed, dated FM26 player attributes improve a
player-level shot-count intensity forecast beyond a strictly causal historical
Gamma-Poisson rate. Never automatically feed a model into production.

## Local assets

The machine has FM26 men full attributes CSV under /home/drewclaw/fm26-data/
extracted from Football Manager 26 version 26.2 (March 2026). The extraction
marks each attribute vector as db, derived or none. This experiment deliberately
**excludes all derived profiles** (which are statistically synthesized, not
actual observed attributes). The opponent/player event labels come from local
data/sofascore_cfm/player_matches/ JSON files; match dates come from the
associated original SofaScore event manifests. No Hugging Face token is needed
to rerun with local assets.

## Reproduce

From repository root:

    python3 scripts/build_fm_mls_shot_dataset.py \
      --fm-file /home/drewclaw/fm26-data/fm26_men_full_attrs.csv

    rustup run 1.97.1 cargo run --release \
      --manifest-path experiments/candle-mls-shots/Cargo.toml \
      -- data/ml/fm26-mls-shots.jsonl data/ml/candle-mls-shot-results

    python3 scripts/score_fm_mls_candle.py \
      data/ml/candle-mls-shot-results/predictions.json

The generated records and checkpoint stay under ignored data/ml/. This
experiment has an independent Cargo.lock; its dependency on candle-core and
candle-nn is not added to the production Rust 1.75 workspace.

## Causal contract

- March 2026 FM snapshot is conservatively treated as available starting
  April 1, 2026. Only April-August 2026 matches become training or evaluation
  labels; February/March match events, already historical by April, may
  contribute to earlier-player-event rate priors.
- Split train before June 1, validation through July 31, and **held-out test
  August 1-31** (UTC); no random split across player-matches.
- The join is exact normalized-name + DOB; duplicates and absent exact
  matches are rejected, not linked to a different namesake by higher CA.
- Features are pre-match FM26 on-disk attributes, role, age and a strictly
  earlier-player-event rate. Played minutes is **observed conditioning exposure**
  in the loss, *not* an input to the model. This estimates player shooting
  conditional on minutes, not an unconditional pre-match goal forecast.
- Baseline: Gamma-Poisson individual rate with five role-prior exposures.
  Candle: 40 features to 16 ReLU hidden units to one bounded log-rate residual.
  Full-batch AdamW and a residual-shrinkage penalty. The validation split
  selects one checkpoint by minimum proper Poisson negative log-likelihood;
  test labels never select hyperparameters or weights.
- Matched samples consist only of players with unambiguous DOB and true
  on-disk FM attributes, which can cause selection bias. Event coverage is
  small, and repeated players/teams introduce dependence across fixtures.

## Outcomes and limitations

See data/ml/candle-mls-shot-results/report.json for baseline vs Candle
validation/test Poisson log loss and shot-occurrence Brier. Inspect
uncertainty.json for paired-match bootstrap intervals. A neural winner on
shots would **not** establish an improvement in W/D/L, exact-score loss,
expected minutes, or event-time simulation: those need their own full-engine
walk-forward OOS validation.
