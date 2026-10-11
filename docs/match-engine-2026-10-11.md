# Match-engine diagnosis and changes, 2026-10-11

## Weekend read (n=3, plus Coventry–Newcastle pending)

Simulator mean LL 0.926 vs baseline 0.954; Brier 0.555 vs 0.569. Standard error of a
3-match mean LL difference is ~0.3–0.5 nats, so the gap is noise. Per the repo acceptance
rule (proper-score improvement supported by uncertainty) nothing here updates the model
or the release gates. The weekend is consistent with the frozen OOS picture: outcome
layer not demonstrably better than causal Poisson, goal mix missed by both (Hull–Everton
4 goals from the lowest-event pricing), draws underpriced by the simulator (18.1% twice).

## Frozen OOS evidence (the part that does justify changes)

`artifacts/validation/pl-hist-same-engine-oos-calibrated.json`, 25 frozen OOS fixtures:

- WDL LL: engine raw 1.037, engine calibrated 1.087, Poisson 0.890. Brier: 0.629 vs 0.519.
- Exact-score LL: engine 3.138 vs Poisson 2.752. Team-goals CRPS/MAE also favor Poisson.
- The frozen calibrator (T=1.7, draw bias +0.15, fit on 30 fixtures) made OOS WDL **worse**
  (1.087 vs 1.037). Two parameters fitted on dozens of fixtures overfit: this is the bug
  fixed below, not a reason to add model complexity.

## Changes in this session

1. **Shared-tempo path-rate correlation** (`SpecificGoalProcess::path_rate_correlation`,
   mirrored in legacy `GoalProcessParameters`). Home/away per-path log goal rates were
   sampled independently, which suppresses joint high/low-tempo games and contributes to
   draw mispricing. Positive correlation models open/closed games moving both attacks
   together. Default 0.0 = bit-identical legacy stream (tested); nonzero is an opt-in
   experiment (`--path-rate-correlation` on `footy-matchforecast-pl-hist`, recorded in the
   artifact `input_contract`) requiring frozen-OOS proper-score evidence before production.
2. **Shrunk outcome calibration** (`OutcomeCalibrator::fit`): penalized objective
   `LL + 2*((T-1)^2 + bias^2)/n` pulls weak evidence toward identity, vanishing at large
   n. Walk-forward minimum raised 6 → 15 prior fixtures (MLS + PL-hist drivers). Tests:
   6 noisy samples stay near identity; 30 systematically draw-starved samples still
   correct.
3. **Market benchmark scorer** (`footy-backtest`: `score_probability_forecast`,
   `compare_forecasts`). External odds enter here only, as a scored comparator.

## External sources assessed

- **Polymarket odds as a forecaster input: declined.** Pre-match odds are market-perceived
  value; feeding them into the engine collapses the architecture's market vs fundamental
  separation (§20) and the `team_goals` no-bookmaker-inputs rule, turning the forecaster
  into an odds-follower. Genuinely timestamped pre-match odds are welcome as a benchmark
  via (3), never as a feature. (Coordination note: the parallel session is designing a
  conservative market adapter — cutoff-time validation, explicit mappings, offline
  fixtures. That belongs in the benchmark harness, not the simulation inputs.)
- **EPL-Pulse_v1 (Poisson GLM, independent Poisson joint): pertinent as confirmation.**
  It is the same model class as our causal-Poisson baseline, leakage-safe features
  included. Its value here is negative evidence: independent Poisson underprices draws
  (Dixon–Coles), which is exactly our standing red flag — addressed structurally by (1)
  plus the scoreline table the engine already exports, not by importing the artifact
  (different feature contract, reimplementation would leak our causal pipeline).
- **galsaar/EPLResults archive: substantive historical match outcomes.** The
  Hugging Face viewer advertises six image records, but the linked results.csv.zip
  holds 11,113 football match rows spanning 1993–2022 and some incident labels.
  Its use is restricted to fixture-level results/priors; it has no dated FM
  player attributes or observed player-event records.
- **openai/math repo: adjacent foundations, no direct forecasting import.** Correction of
  an earlier overstatement: the catalogue does contain mathematics adjacent to our
  machinery — family 104 (turn-based stochastic mean-payoff games, quasipolynomial exact
  algorithms), family 115 (exact/uniform contingency-table sampling), family 139
  (log-concave sampling query complexity), switch-chain mixing-time results, family 227
  (Glauber-dynamics mixing/spectral gaps), and the spin-glass families. None of these
  supplies a calibrated football model: no family found addresses proper scoring rules,
  probability calibration, overdispersion estimation, or point-process goal modeling —
  which is where our binding constraint lives (25 frozen OOS fixtures, engine trailing
  Poisson by ~0.15 nats). The correct relationship: foundational results can sharpen
  the *tools* (MC convergence discipline, sampling theory, game-theoretic tactic
  framing), and future catalogue families on calibration/scoring/point processes are
  worth watching — but no current manuscript changes a parameter, prior, or gate in
  this repo. Adopting one would require the same frozen-OOS evidence as any other
  change.
- **FM attributes + transformer: optional source-neutral upstream experiment.**
  FM attributes already feed the 2026 engine, but historical 2015/16 contracts
  intentionally exclude FM26 values. A small Candle CPU transformer can operate
  in pure Rust and need not replace the causal channel graph or run at every
  simulated second. The isolated Candle shot-rate MLP experiment and masked
  player-set attention module now exist; neither is promoted. A trained
  transformer still needs dated pre-match *candidate-set* contexts (using observed
  final lineups as inputs would leak) and an independent held-out comparison
  with simpler GLM/MLP baselines. There is no proven minimum number of
  sequences; sample-size sufficiency is empirical.
- **Modal/CUDA burst: not spent.** Nothing in this workstream is compute-bound (200–2000
  CPU paths/fixture, std-only Rust). GPU is justified only by a measured bottleneck
  (batch matrix ops at 100k+ fixture scale) with a CPU baseline on record. No such
  measurement exists; spending there now would be waste.

## Red-card prior (verified, left to the parallel session)

Confirmed in this repo: per-player per-90 0.02 applied to every active player
(`second_resolved_mc.rs` hazard list; same 0.02 in segment engines/drivers) implies
~22 × 0.02 = 0.44 expected dismissals/match, P(≥1 red) ≈ 35–36%, vs their measured
13.7% over 8,289 archived matches (≈0.0067/player/90). ~2.6× overstatement, before
mental-state multipliers. Fix (causally fitted incident prior + same-engine OOS check)
is owned by the parallel session to avoid conflicting patches; not touched here.

## Underlying-level findings (added same day, user request)

Searched for foundations that could solve the three standing issues at their root:
draw underpricing, exact-score worse than Poisson, MC error at low path counts. Sources:
full openai/math manuscript catalogue (372 families, grepped), football score-model
literature (Dixon–Coles 1997; Karlis–Ntzoufras 2003; Boshnakov et al. 2017),
variance-reduction literature.

### 1. Dixon–Coles dependence correction — IMPLEMENTED (default off)

The classical low-score adjustment multiplies the 0-0, 1-0, 0-1, 1-1
cells by τ (typical ρ ≈ -0.03). Marginal and total-mass preservation holds
only when the base is independent Poisson. Applying it to an *empirical*
simulation scoreline table and renormalizing can change team-goal marginals,
W/D/L and O/U 2.5. Implemented as a diagnostic post-hoc scoreline-table layer in
`footy-match::calibration` (`dixon_coles_tau`, `apply_dixon_coles`,
`fit_dixon_coles_rho` with the same AIC-style shrinkage toward 0), wired into
`footy-matchforecast-pl-hist` via `--dixon-coles-rho` (default 0.0 = identity) with
per-fixture applicability flags and rho recorded in the artifact. Raw engine tables
stay immutable. The driver now records the maximum changed team-goal
marginal explicitly and flags this as an alternative scoreline-only forecast.
Tests: textbook τ values, mass/marginal conservation on an independent-Poisson grid,
nonconservation on arbitrary path tables,
infeasible-ρ rejection, deterministic shrunk fit that recovers draw-heavy histories
and stays at 0 without signal. Promotion requires frozen-OOS exact-score LL gain.

### 2. Shared-component bivariate Poisson (Karlis–Ntzoufras) — NOT adopted as engine

Positive covariance across the whole matrix via a shared Poisson component. Our new
`path_rate_correlation` is a related structural mechanism inside the simulator (shared
tempo moving both rates together), but is not identical to a common-component
bivariate Poisson. It is a coherent full-path challenger, not yet statistically
proven better. Kept as the candidate to
beat Dixon–Coles in the frozen comparison, not a second implementation.

### 3. Bivariate Weibull + copula (Boshnakov et al. 2017) — diagnostic only

Weibull inter-arrival counts handle non-Poisson marginal dispersion; copula handles
dependence. Pertinent because our production engine is literally an event-time
(second-resolved) process: the transferable insight is to diagnose our simulated goal
inter-arrival times against exponential (Poisson) vs Weibull. No engine change until
that diagnostic shows marginal — not dependence — failure.

### 4. Monte Carlo error — SE REPORTING implemented, antithetic deferred

`footy-forecast::multinomial_outcome_standard_errors` (multinomial SE per W/D/L class;
72 paths ≈ ±6pp at p=0.4) answers the forecast AGENTS.md mandate to report effective
simulation error before spending paths. Antithetic path-rate sampling could
reduce rate-driven variance at low path counts if the paired output covariance
is negative, but the gain must be measured; it is a possible next step if MC error
— not epistemic uncertainty — binds; control variates were rejected (no analytic
quantity with known expectation exists under lognormal mixing).

### 5. openai/math catalogue — no importing family found

The first keyword search of the 372-family catalogue did not find a direct
football calibration theorem. The subsequent manuscript-level audit found
more actionable connections: families 113 (entropy with feasible matching
marginals), 114 (polymatroid/matroid counting), and 115 (fixed-margin
contingency sampling) motivate testing joint XI distributions rather than
sequential role draws. Their mathematical hypotheses are not equivalent to
the actual weighted conditional lineups, so no theorem is imported as an
unearned calibration guarantee. See the companion mathematical audit.

## Acceptance for any future change (unchanged)

Frozen walk-forward OOS proper-score gain (WDL LL/Brier, exact-score LL, CRPS) with
bootstrap/uncertainty, leakage tests, deterministic rerun, no n=3 updates.
Coventry–Newcastle is one more scoring row, not a model input.
