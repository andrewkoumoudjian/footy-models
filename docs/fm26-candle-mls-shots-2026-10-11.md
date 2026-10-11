# FM26 × MLS shots: first causal Candle experiment (2026-10-11)

## What actually exists locally

The previous research note correctly required dated FM attributes, but was too
categorical about availability. These local sources exist and were inspected:

- FM26 version 26.2, March 2026, full men's attribute export under
  /home/drewclaw/fm26-data/fm26_men_full_attrs.csv. It includes 54 named
  attributes, current ability, DOB, UID, and extraction provenance. Some rows
  contain **synthetic reconstructed** attributes (attr_source=derived). Only
  genuine attr_source=db values enter this experiment.
- SofaScore historical MLS fixture manifests and per-player match statistics
  under data/sofascore_cfm/: 113 locally cached matches, dated February–August
  2026, with observed player minutes and shot counts. This is a limited,
  selected fixture sample, **not the complete MLS season**.
- A cached Hugging Face dataset reference for akks88/football-manager-data;
  the machine's stored Hugging Face token is invalid, and anonymous access was
  unavailable. This experiment uses verified local files instead.

## Reproducible implementation

- [As-of materializer](../scripts/build_fm_mls_shot_dataset.py) builds JSONL
  with exact normalized name + DOB joining; ambiguities are excluded, not
  resolved by maximizing CA. Strictly prior events supply the individual
  Gamma-Poisson rate. Data before April 1 may contribute to historical priors
  but cannot be FM26+event supervised labels. Its 40 features include role,
  age, CA, 16 FM attributes, per-attribute missingness and historical shot
  rate/exposure. **Realized minutes** appear only as the Poisson offset and
  scoring exposure, never as model features.
- [Candle 0.11 CPU pilot](../experiments/candle-mls-shots/) is a separate Rust
  workspace pinned by its own Cargo.lock, invoked under Rust 1.97.1 without
  changing production's pinned 1.75 toolchain. It fits a 16-hidden-unit MLP
  with bounded log-rate residual, shrinkage and deterministic initialization.
  Validation loss alone chooses the best epoch. Training and inference execute
  in Rust/Candle without Python/CUDA.
- [Uncertainty scorer](../scripts/score_fm_mls_candle.py) runs a 3,000-draw
  paired match-cluster bootstrap on held-out count log-loss differences.
  Six focused Python tests and the whole suite (43 tests) pass at this
  revision. Weights, predictions and reports remain in gitignored data/ml/.

## Chronological splits and outcomes

Only observed FM attributes matched unambiguously to player events:

| Split | Dates (UTC) | Player-match rows | Distinct matches |
| --- | --- | ---: | ---: |
| Train | Apr–May 2026 | 377 | 46 |
| Validation | Jul 2026 | 155 | 14 |
| Test | Aug 2026 | 163 | 19 |

The full usable match count is 79; this table is conditional on matched
players, not all squad members.

| Proper score (lower better) | Baseline prior | Candle MLP | Candle minus baseline |
| --- | ---: | ---: | ---: |
| Validation shot-count Poisson LL | 0.8882493 | 0.8887181 | +0.0004688 |
| Test shot-count Poisson LL | 0.9966251 | 0.9945638 | -0.0020613 |
| Validation shot-occurrence Brier | 0.1780889 | 0.1783208 | +0.0002319 |
| Test shot-occurrence Brier | 0.1741533 | 0.1737059 | -0.0004474 |

Paired match-cluster bootstrap 95% confidence intervals for Poisson
log-loss differences:

- Validation (14 matches): **[-0.0008691, +0.0018294]**
- Test (19 matches): **[-0.0036209, -0.0005738]**

Best checkpoint selected by validation was **epoch 1**, with no improvement
over the baseline on validation. The test gain is tiny despite its conditional
match-bootstrap interval, and **must not be interpreted as a model-winning
result**. The bootstrap ignores correlations across fixtures for recurring
clubs and players, attribute matching selects a nonrandom subset, and match
minutes are observed exposures. These numbers were **not** scored in the
second-resolved match simulator.

Input fingerprint of the initial experiment:

- local FM26 CSV SHA256:
  b91eaccd9be04278d863a80ef26473455932acdaf8f2b321196c50ada08b74cb
- dated JSONL SHA256:
  02d6ee54757e740c0db0b696534914c30a096737f35033104304573e0bed7553

## Decision and next work

**No production promotion.** Do not use the test split for hyperparameter
selection or claim W/D/L improvement from conditional shot-count scores.
The next properly powered experiment needs further **as-of FM edition
snapshots joined to event and lineup records over multiple seasons**.

1. Establish a regularized Poisson/negative-binomial GLM with the same
   dated features, exposure and role offsets before claiming a neural win.
2. Ablate source families: true FM attributes versus role/age/CA and past
   performance. Never treat reconstructed attributes as observed.
3. Only once validation wins, wire an optional, versioned per-player event
   forecast to the existing causal match simulator, and retest match W/D/L,
   exact score, player shots, starting XI and minutes on larger OOS windows.

This experiment does not touch the match-simulator calibration, path engine
or any parallel Dixon–Coles/shared-tempo work.
