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

## Set-attention transformer prototype

The experimental module at src/set_transformer.rs implements one trainable,
permutation-equivariant self-attention block over masked player feature sets
on Candle CPU. It has no arbitrary player-order positional embedding.
Unit tests assert equivariance when players are reordered, invariance of
valid-player predictions under masked padding (even with extreme padded
values), finite gradients through all parameters and nonzero gradient flow.

Run the tests using:

    rustup run 1.97.1 cargo test --manifest-path experiments/candle-mls-shots/Cargo.toml

This attention block is **not trained on the existing player-shot JSONL**.
Those examples contain only players who actually appeared: gathering them as
a whole-match input would introduce true lineup knowledge unavailable before
kickoff. The missing next asset is a dated match-level candidate roster for
both teams, built solely from information available before each fixture.
Once such datasets exist, train GLM, MLP, Deep Sets and attention under the
same cutoff and score proper out-of-sample losses. The attention implementation
is deliberately test-only so it cannot silently alter the original MLP.

FM22 and FM23 archives exist locally as decompressed binary *blocks*, but
their current extracted_names manifests contain just 12 and 0 name entries,
respectively. Neither is yet a usable source of validated, dated FM player
attribute vectors. A working FM26 extractor does not certify those old
formats or allow historical attributes to be inferred from FM26 ratings.

## Dated two-team candidate sets and actual attention training

Use only players observed in *earlier* matches for each team's pool.
The current match's list of actual appearances is target information, never
the input roster. To reproduce:

    python3 scripts/build_fm_mls_pre_match_sets.py
    rustup run 1.97.1 cargo run --release \
      --manifest-path experiments/candle-mls-shots/Cargo.toml \
      --bin set_attention_pilot -- \
      data/ml/fm26-mls-candidate-sets.jsonl \
      data/ml/candle-mls-attention-results
    python3 scripts/fit_fm_mls_candidate_glm.py \
      data/ml/fm26-mls-candidate-sets.jsonl
    python3 scripts/score_fm_mls_candle.py \
      data/ml/candle-mls-attention-results/predictions.json

The as-of builder yields 66 fixtures with 285 training, 95 July validation,
and 139 August exploratory-test labels matched to genuine FM-db attributes.
Its mean two-team context contains about 42 previously seen players.

The one-head, 16-wide Candle transformer selected epoch 1. July Poisson
log loss is 0.895644 versus baseline 0.894978 (worse). The August period,
which had already been inspected in earlier research, is 1.032342 versus
1.034034 (slightly better but NOT an independent new held-out result).
The ridge-Poisson GLM challenger on exactly the same candidate cohort
selects identity on July validation. All corrections remain default-off.

## FBref expansion using probberechts/soccerdata

The importer at scripts/import_fbref_soccerdata.py implements an optional
FBref data path through https://github.com/probberechts/soccerdata .
Run it against explicit local soccerdata CSV exports:

    python3 scripts/import_fbref_soccerdata.py \
      --league 'ENG-Premier League' --season 2020-2021 \
      --schedule-csv /path/to/schedule.csv \
      --players-csv /path/to/player_match_summary.csv

Or, with soccerdata installed in an isolated environment, explicitly request
at most five historical FBref matches and their per-player summary stats:

    python3 scripts/import_fbref_soccerdata.py \
      --download --league 'ENG-Premier League' --season 2020-2021 \
      --max-matches 5 --from-date 2020-09-01 --until-date 2020-10-01

The importer aligns by FBref report IDs and exact match date/team/season,
captures source hashes, and rejects missing shot labels instead of imputing
zero. FBref player names do NOT establish FM player IDs: a confirmed
identity crosswalk with dated birth/ID evidence must be built first.
Retroactively joining FM26 ratings to 2015-2025 outcomes is prohibited.
The live FBref dataset depends on the provider's current access rules;
historical season-level totals cannot be used as as-of inputs inside
that same season.
