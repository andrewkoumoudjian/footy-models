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
