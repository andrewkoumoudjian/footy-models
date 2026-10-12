# Mathematical participation and substitution research — 2026-10-11

Status: **research only**. No previous production inputs, defaults or frozen forecasts were modified. The 25 held-out 2015/16 fixtures were **already inspected** in preceding experiments. This retrospective ablation does not provide independent model-selection evidence.

## Mathematical and data contract

The dated previous-only player input supplies a Beta(0.5,0.5) posterior mean for starting and for appearances conditional on not starting. The Rust research engine uses the existing exact-cardinality, maximum-entropy dual and DP sampler separately for the goalkeeper and 10-outfield partition. The prior probabilities are projected onto feasible roster marginals; they must not be misreported as automatically calibrated independent Bernoulli forecasts. This is a tractable constrained combinatorial special case of the matching/matroid work discussed in the openai/math survey; no cited theorem asserts football prediction accuracy.

Starting minutes are pooled toward the historical previous-only cohort mean using four pseudo-starts. A second-half survival model calibrates a per-minute substitution hazard lambda by solving E[minutes given start] = 45 + (1 - exp(-45*lambda))/lambda. The model then samples a distinct earlier-observed bench player for each completed replacement, capped at the applicable rule-era allowance (three per team in 2015/16). Incoming players participate in the same shot/goal event stream. Minutes, appearances, player shots and player goals are conserved on each simulation path. No goal can be scored without a sampled shot. Injury and red-card censoring, goalkeeper changes, role-specific replacements and stoppage time are not yet calibrated.

These shrinkage and hazard assumptions are **research hypotheses**, not proven estimates of causal substitution decisions. With a substitution cap, pre-cap fitted expected minutes need not equal final simulated minutes.

## Retrospective 25-fixture diagnostic (2,500 paths per fixture)

| Metric (lower better) | Frozen default | Subs alone | Exact lineup + subs |
|---|---:|---:|---:|
| Starting XI Brier | 0.15983 | 0.16017 | 0.15907 |
| Appearance Brier | 0.17524 | 0.16948 | 0.17000 |
| Minutes MAE | 27.16 | 27.36 | 24.32 |
| Player shots MAE | 0.47630 | 0.47630 | 0.46939 |
| Player-goal occurrence Brier | 0.03655 | 0.03666 | 0.03655 |
| Match 1X2 log loss | 0.90453 | 0.89740 | 0.91363 |
| Generated substitutions per match | 0 | 4.40 | 4.21 |

Observed lineup exposures imply 5.52 completed bench appearances per match, versus 5.72 event-labelled substitutions; these different definitions must not be conflated. The challenger still **underpredicts substitutions**, misses four out of 64 historical credited player goals because the corresponding players were outside the prior pool, and has not shown a reliable benefit to match 1X2 or player-goal probabilities.

Joint exact-lineup/substitution mode reduces minute MAE by 2.84 and shot MAE by 0.0069 on this **reused historical sample**. The paired matchday bootstrap interval crosses zero for the start Brier and WDL changes. Descriptive intervals excluding zero for other metrics are not independent statistical significance evidence, because this sample has been inspected and is heavily concentrated in two clubs; the changed random-number consumption is not paired common-random-number event matching.

The aggregated, immutable inputs-to-result fingerprint and complete paired descriptive intervals are in participation_retrospective_25.json; compare_participation.py rejects mismatched fixtures, seeds, path counts and missing retrospective warnings.

## Reproduce

From the repository root, build the research binary and run three immutable, separate experiment directories:

    cargo build --release -p footy-matchforecast --bin footy-matchforecast-math
    python3 apps/matchforecast/math_engine/historical_backtest.py --output-dir data/ml/math_engine/replay_base --paths 2500
    python3 apps/matchforecast/math_engine/historical_backtest.py --output-dir data/ml/math_engine/replay_subs --paths 2500 --max-subs 3
    python3 apps/matchforecast/math_engine/historical_backtest.py --output-dir data/ml/math_engine/replay_joint --paths 2500 --max-subs 3 --selection-marginals

    python3 apps/matchforecast/math_engine/compare_participation.py --baseline data/ml/math_engine/replay_base/report.json --substitutions data/ml/math_engine/replay_subs/report.json --joint data/ml/math_engine/replay_joint/report.json --output data/ml/math_engine/replay_comparison.json

## Live data boundary and next research

The dated 2026-10-10 PL fixture snapshot has 323 roster players, including only 157 whose FM raw rows are classed both player and database-sourced. **No reliable player start / substitute histories are present**, so optional fitted selection and substitutions fail closed. The unchanged default uses no substitutions. Historical team inputs are now restricted to *dates before* as_of: 1,190 EPL results and 552 Championship results. Six 2026-10-10 result-only rows, with synthetic shot placeholders, are excluded altogether because source publication times are unverified.

The next gate is a licensed, entity-linked, dated EPL appearance/lineup/event source and subsequent independent historical seasons (200+ representative held-out fixtures), proper scoring and competing-risk survival estimation under contemporaneous competition rules. Until that exists, do not promote this mathematical challenger or fit a transformer to the same inspected fixtures.
