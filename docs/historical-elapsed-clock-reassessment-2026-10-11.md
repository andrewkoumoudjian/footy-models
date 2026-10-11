# Corrected StatsBomb elapsed clock: historical timing and substitution reassessment

**Research-only correction of the evaluator**, not a new forecast and not
a refit. The 200-path historical Rust simulation was left untouched.
Its previously protected 25-match OOS outcomes were not used to tune
parameters. This reassessment must not itself become a model-selection set.

## A documented one-minute label-alignment error

The original historical label builder computes the event minute as
max(0, source minute - 1), capped at regulation minute 89. But raw
StatsBomb event timestamps use elapsed-clock minutes: an event with
minute=45 occurs at the start of 45–60, **not** in 30–45.
The original v1 scorer projected source shots and the derived
minute-indexed goal labels into the earlier interval. Half-time
substitutions in particular were incorrectly allocated to 30–45.

The new [v2 source-clock mode](../scripts/score_pl_hist_momentum_oos.py)
uses each source Shot, goal-scored Shot, own goal and Substitution event
at its original elapsed minute. Stoppage-time events remain clamped
to 75–90 because the current simulator predicts 90 regulation minutes.
Recorded official label totals are independently cross-checked.

This deliberately saves a **new** report rather than changing the
frozen [v1 historical evidence](../artifacts/validation/pl-hist-momentum-oos.json).

### Versioned v2 label materialization is implemented

The [historical data builder](../scripts/build_pl_hist_inputs.py)
now supports `--clock-contract elapsed_v2`. It creates an **entirely
separate** `data/statsbomb/pl_hist_elapsed_v2` directory (the legacy
v1 directory is left unchanged). The v2 fixture and label files
declare a schema version and the precise label clock contract.
Every goal, card, assist and substitution event retains the
original source minute, second and period alongside its corrected
regulation-minute index. Bad or missing source timestamps fail closed.

The [cross-version audit](../scripts/audit_pl_hist_label_clock.py)
checks all 74 matches and all 75 causal team/player input files.
**Every pre-match input file is byte-identical between v1 and v2.**
Across the 74 fixtures, the event-clock corrections were:

| Event class | Total events | Changed minute index | Changed 15-minute bin |
| --- | ---: | ---: | ---: |
| Goals (including own goals) | 189 | 178 | 13 |
| Completed substitutions | 417 | 395 | 64 |
| Registered assists | 123 | 116 | 10 |
| Red cards | 9 | 8 | 0 |

Reproduction:

    python3 scripts/build_pl_hist_inputs.py --clock-contract elapsed_v2
    python3 scripts/audit_pl_hist_label_clock.py

The [audit manifest](../artifacts/validation/pl-hist-label-clock-v2-audit.json)
includes exact source-file fingerprints, counts, bin corrections
and an explicit no-lookahead assertion.

## Same forecasts, different clock interpretation

On 25 chronological OOS fixtures (68 recorded goals), the score changes:

| Measure (lower is better) | v1 label minute-1 | v2 raw elapsed clock |
| --- | ---: | ---: |
| Engine conditional goal-time NLL per goal | 1.78517 | **1.79482** |
| Flat six-bin timing NLL per goal | 1.79176 | 1.79176 |
| Engine minus flat (negative means better) | -0.00659 | **+0.00306** |
| Relative pressure fraction MSE | 0.10970 | 0.11649 |
| Constant full-match pressure MSE | 0.11012 | 0.11712 |
| Away-advantaged peak tie-adjusted hit rate | 14% | 12% |

Paired fixture-bootstrap 95% CI for v2 engine minus uniform
goal time is **[-0.02278,+0.02970] nats/goal**.
The earlier weak goal-clock prior also changes with the correct
source time: model 1.79482 versus prior 1.82741, delta -0.03259,
CI [-0.08677,+0.02345]. **Neither timing comparison passes the
proper-score improvement gate** under the corrected clock.

## Substitutions: plausible 60–75 prevalence, weak complete timing

The archive contains 74 event-covered matches with just nine
red cards, too few for a trustworthy fitted 11-v-10 hazard model.
The corrected OOS subset has completed substitutions recorded in
almost every 60–75 and 75–90 period. Thus the model's very high
60–75 substitution-trigger probability is **not automatically
implausible**, though its hard scheduling remains worth investigating.

The v2 diagnostic scores the simulator's any-team *substitution
trigger* probability against actual *completed substitution*
occurrence and compares an earlier-fit Beta(2,2)-smoothed per-bin
empirical reference:

| 25 OOS fixtures, 150 bins | Brier versus completed substitutions |
| --- | ---: |
| Simulated trigger occurrence | 0.12710 |
| Fit-only completed-substitution reference | **0.08419** |
| Engine minus reference | +0.04291 |

Paired fixture bootstrap interval for the difference:
**[+0.01711,+0.06641]**. The proxy performs worse against this
target, but **a trigger is not necessarily a completed substitution**.
Do not call the numbers directly calibrated substitution
probabilities; first align the engine to a completed-substitution
output definition.

## Run and provenance

    python3 scripts/score_pl_hist_momentum_oos.py \
      --source-elapsed-clock \
      --output artifacts/validation/pl-hist-momentum-elapsed-clock-v2.json

The [machine report](
../artifacts/validation/pl-hist-momentum-elapsed-clock-v2.json)
records the corrected target clock, 31 earlier fit fixtures,
25 later scored fixtures, fitted substitution and goal clocks,
fixture-level errors, CIs, and the unchanged simulator source.

**Future historical label formats must use a new explicitly versioned
source-clock contract.** Do not silently rewrite old event-labeled
training datasets or retroactively relabel archived v1 results.
Upcoming player-event classifiers should use raw source elapsed
time, a consistent convention for period starts, and exact
stoppage-time handling wherever the forecast horizon supports it.

### Engine event semantics fixed for subsequent experiments

The Rust simulator now exports **both** `p_substitution`
(the original backwards-compatible attempted substitution/injury
event) and `p_completed_substitution` (actual successful tactical
or injury-forced replacement, counted only after the lineup state
changes). Failed attempts no longer inflate the latter.
This is a **new field**, not a change to event generation, RNG,
goal rates or the already frozen historical prediction file.
The historical scorer prefers the completed field when present,
but the old 200-path forecast contains only attempted events.
Thus the v2 Brier table above must still be treated as a
**mismatched proxy comparison** until a newly frozen, properly
versioned simulation exports successful substitutions throughout
its original out-of-sample period.
