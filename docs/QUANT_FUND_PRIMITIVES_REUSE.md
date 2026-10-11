# Quant-fund primitives reusable in footy-models

## What exists (~/quant-fund/packages)

### factor-primitives (quarcc-factor-primitives)
- `regime/hmm.rs` — full Gaussian HMM: `forward_scaled`, `forward_backward`,
  `observation_sequence_log_likelihood`, `posterior_mode_state`, `state_probability_entropy`,
  `regime_switch_probability`. **Directly usable for per-player form regimes** (hot/cold
  scoring states from rolling per-90 output).
- `regime/garch.rs` — ARMA(1,1) mean update + GARCH(1,1)/GJR-GARCH variance updates.
  **Usable for player/team rate volatility**: heteroskedastic per-match variance instead of
  the current static `variance_factor`.
- `regime/markov.rs` — transition-matrix validation; `baum_welch.rs` for fitting.

### ml-primitives
- `model.rs` — fast serialized tree-model inference (`predict_rows_blocked`). Usable if we
  train a gradient-boosted outcome model on match features (the "GBM" idea).
- `features.rs`, `rank.rs`, `csvio.rs` — feature building + ranking helpers.

## Integration plan

1. **Form regime layer (HMM)** — for each squad player build a per-90 output series from
   cached lineups (shots+xG+xA blend). Fit a 2-state Gaussian HMM (hot/normal) as-of each
   fixture. Use P(hot) to modulate `mean_per90` (e.g. ±15%) and shrink epistemic_sigma when
   posterior is confident. This is genuine player-performance-based prediction, causal by
   construction (fit only on pre-cutoff games).
2. **Variance layer (GARCH-style)** — replace static small-sample inflation with an EWMA/GARCH
   variance update over the player's match-to-match output series.
3. **GBM outcome model** — train quarcc Model on walk-forward features (team windows +
   aggregated squad form features) to predict H/D/A; use as a calibration ensemble member
   against the simulation. Only after steps 1–2 move Brier below uniform.

## Dependency hygiene
factor-primitives is std-only Rust; add as path dependency scoped to the matchforecast app:
```toml
quarcc-factor-primitives = { path = "../../../quant-fund/packages/factor-primitives" }
```
