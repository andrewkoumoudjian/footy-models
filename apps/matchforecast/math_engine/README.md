# Mathematical match-engine challenger

Latest isolated participation research: [PARTICIPATION_RESEARCH.md](PARTICIPATION_RESEARCH.md), including the unchanged baseline, all ablations, input limits and reproduction commands.

Research-only implementation isolated inside apps/matchforecast. Its Rust API is in ../src/math_engine/mod.rs and its executable is footy-matchforecast-math. It does not change the production second-resolved engine, archived pre-match predictions, existing backtests, or default matchforecast behavior.

## Mathematical model and invariants

1. Reuse the existing regularized, as-of Rust TeamGoalModel to estimate separate team attacking/defensive goal intensities, including its existing **Championship promotion prior** from 552 historical E1 fixtures. Estimate shot intensities with Gamma-Poisson pseudo-match shrinkage using actual verified shots only. No future target-match data enters fitting.
2. The existing exact-cardinality partition-function dynamic program selects one goalkeeper and ten outfield players from the *pre-match candidate set*. Each group follows a weighted subset law conditioned on its exact size. **Previously recorded positions take precedence** over FM attribute heuristics: two goalkeeper ratings above a threshold demonstrably misclassified some outfielders. Only when position is unknown is a stricter four-attribute goalkeeper profile permitted on real player/database records. Outfield roles are not yet modeled, so XI probabilities are not certified as calibrated.
3. The simulation is a marked competing-intensity shot process: a shot is sampled, an active player is assigned, and a Bernoulli mark determines whether the shot is a goal. A unique conversion intercept solves the player-weighted target conversion rate exactly, preserving the team-goal anchor. Every goal is a shot, team goals equal credited player goals on every path, and the scoreline/WDL/timeline plus direct player goal occurrence are projections of those same paths.
4. Conditional score-state, numerical advantage, and FM finishing parameters are explicit, default-zero research interventions. Red cards can be enabled separately; the historically estimated 0.006857 per player/90 is a candidate, not validated for game results. Completed tactical substitutions and exact selection marginals are **separate opt-in research features**, requiring earlier actual start/bench histories and explicit competition-era substitution caps. Injuries, assists, positional matchups, goalkeeper changes and stoppage time remain unmodeled here; the incumbent second-resolved engine still owns its own mechanisms.

When all research effects are zero, Poisson thinning recovers the causal independent-Poisson goal-count limit. Player priors from the existing dated FM26/official-squad pipeline allocate attempts and scorers, but the conditional conversion mean still matches each team's fitted historical goal rate. This is a coherence guarantee, not an out-of-sample accuracy improvement.

## Relevant openai/math work

- Family 113, [entropy and perfect-matchings](https://github.com/openai/math/blob/main/lean/docs/113.md), motivates treating the joint constrained selection law as the primitive. Its perfect-matching entropy inequalities do not claim football lineup calibration.
- Family 114, [approximate counting of polymatroid/matroid bases](https://github.com/openai/math/blob/main/overview.tex), provides relevant combinatorial foundations: a one-goalkeeper plus ten-outfield selection is a partition-matroid special case. The existing fixed-cardinality DP gives an exact solution for this special weighted law, so importing the paper's general approximate-counting algorithm is unnecessary.
- Family 115, [fixed-margin contingency tables](https://github.com/openai/math/blob/main/lean/docs/115.md), reinforces joint count consistency. Uniform contingency-table sampling would impose the *wrong* football event distribution. We instead satisfy the goal/shot and player/team totals directly by construction.

These papers do not solve the statistical estimation of football shot hazards or calibration. The relevant fitted models are regularized Poisson, conjugate Gamma-Poisson shrinkage, logistic marking and walk-forward proper-score evaluation. No paper is claimed to provide a football-performance theorem.

## Reproduction

First create the usual dated official roster, availability, and FM-UID matched player inputs with scripts/run_pl_pipeline.py, if those cached input files are missing. For the existing October 10 snapshot, the complete **offline** research pipeline is:

    python3 apps/matchforecast/math_engine/run_pipeline.py --output-dir data/ml/math_engine/full_20261010 --paths 2500 --seed 42

Focused checks:

    python3 -m unittest discover -s apps/matchforecast/math_engine -p 'test_*.py'
    RUSTFLAGS='-D warnings' cargo test -p footy-matchforecast --lib math_engine
    RUSTFLAGS='-D warnings' cargo check -p footy-matchforecast --bin footy-matchforecast-math

The builder verifies all 323 prepared PL player UIDs against the raw FM26 file **and the dated registered-squad file**. Raw classifications: 157 genuine player/database, 76 player/reconstructed, 75 staff/database, and 15 staff/staff. Only **157** qualify for direct observed FM attribute effects; the other 166 remain registered candidates with unverified FM measurements. It rejects mismatched identities, unavailable goalkeeper coverage and post-cutoff roster observations, requires a declared FM vintage, checks historical score totals against the raw match source, and hashes all input files. Six October 10 result-only rows contain **artificial 12/11 shots** in the upstream adapter. The conservative dated pipeline now excludes *all six same-day results entirely*, because no verified result-publication clock exists. The 2026-10-10 run consequently uses 1,190 previously dated EPL matches and 552 Championship matches with source-verified observed shot counts. Its output includes an executable SHA256 audit and refuses to overwrite an existing snapshot.

## Historical same-engine diagnostic: observed-only contract

The 2026 FM attributes cannot be backtested on 2015/16 events. The separate historical runner uses only **strictly prior** StatsBomb player/lineup observations and chronological team results. It establishes goalkeeper roles only from earlier lineup events, feeds the *same compiled Rust engine*, saves immutable independent match inputs/outputs, and loads the actual fixture's labels **after** prediction.

    cargo build --release -p footy-matchforecast --bin footy-matchforecast-math
    python3 apps/matchforecast/math_engine/historical_backtest.py --output-dir data/ml/math_engine/historical_new_run --paths 2500

The previously examined 25-match February–May 2016 subset is an **exploratory diagnostic**, not untouched OOS evidence. Match log loss was 0.9045 from 2,500 simulation paths versus 0.9077 for the related independent-Poisson analytic law; the paired matchday-block interval crossed zero. Starting XI Brier (0.1598 vs prior 0.1587) and player shots MAE (0.4763 vs 0.4673) failed to show improvements. **Native path-based goal-occurrence Brier was 0.0365 versus 0.0409 for the prior**, with a matchday bootstrap interval for the paired difference of [-0.0058, -0.0028], but 4/64 credited player goals had no pre-match candidate ID and the repeated-club/previously examined sample invalidates new significance claims. Do not tune coefficients to this already-inspected period.

## Calibration boundary

This challenger is **not validated** against 2026–27 observed Premier League player event labels. FM26 26.2 is an as-of March 2026 snapshot, so it cannot legitimately be pasted into historical 2015/16 fixtures. The 25-match historical same-engine diagnostic now compares the *new* mathematical challenger on an observed-only historical feature contract, but cannot certify it because those fixtures were already inspected, club coverage is narrow, and FM26 is excluded. Future promotion requires at least 200 representative later held-out fixtures with period-correct FM editions or an observed-only feature contract; score WDL and joint exact-score log loss, count CRPS, player goal/shot calibration and timeline reliability against both the unchanged existing engine and causal Poisson, with paired matchday-block intervals. Until then all emitted probabilities are research scenario distributions.
