# Liverpool–City: exact-binary reproducibility and mechanism ablations

**Status: retrospective research diagnostics, not OOS forecasting or model selection.**
The frozen October 10 source forecast and its timeline images were not altered.

## Reproducibility contract

The new [research runner](../scripts/run_frozen_pl_research.py) stores the exact
executable bytes, dated team input CSV, player-model CSV and team forecast JSON.
It fingerprints these and every output with SHA256, records CLI flags, rejects
nonchronological feature cutoffs, and refuses to overwrite an existing archive.
A repeated identical simulation must produce byte-identical output.

All three runs used the **same executable and three identical input files**:

| Artifact | SHA256 |
| --- | --- |
| Compiled executable | 2916eeee1735c5d6f5e5a8b9ef1050e2fe48a191607eef21015b045ce3e23321 |
| Historical team features | 205a39774fcc46d5f8093643aef5c9c4d0472640b12fc32d91785109de289a96 |
| FM-matched player inputs | 1da0330903e33a0fc7dca0a4696d9e55cff4844202d6912407fa8b85cc7edfd3 |
| Dated match-result baseline | c4c67b014b510cc0d00814f597fa77c9d4af90ca1f06ee7d480aeea2716809ec |

The 12-path baseline was run twice on exactly this frozen environment,
seed 42 (Liverpool fixture seed 44). Both outputs had SHA256
7fcc4bb26c926c82c78b937167e199b0f27353432824df18b3edb6282384a196.
It is **reproducible for this exact binary/inputs/seed**, unlike the earlier
replay whose binary version and audit inputs did not match its archived run.

## Default-off research interventions

The EPL adapter has two new, explicitly named, opt-in flags:

- --research-no-tactical-subs: set the earliest tactical substitution
  to minute 90, suppressing *scheduled* substitutions during regulation.
  Injury-forced replacements remain enabled.
- --research-red-probability-per90 0: disable red-card event hazards
  for active players, keeping other event types intact.

The baseline defaults and production pipeline do not supply these flags.
Each changed model writes a separate, labeled research snapshot.

| 12-path variant | Most City-favoured *relative-pressure* bin | Highest City *goal intensity* bin | Expected City goals for full match |
| --- | --- | --- | ---: |
| Default | 60–75 | 30–45 | 1.50 |
| No scheduled tactical substitutions | 30–45 | 45–60 | 1.75 |
| No red cards | 00–15 | 00–15 | 1.75 |

The original 72-path October 10 forecast had its strongest City relative
pressure in 60–75 and City absolute goal-rate peak in 45–60. The 12-path
experiment is a separate code/build/input environment, **not a replacement**
for the pre-match snapshot.

For Monte Carlo sensitivity, a further frozen run of **72 paths**
with the new, exact-binary baseline shifts City's strongest
relative-pressure interval to **45–60**. Its pressure index by
successive 15-minute interval was approximately
[-12.07,-6.33,+6.70,-12.42,-4.70,+13.35].
The old original 72-path pre-match snapshot peaked at 60–75.
The same-path-count difference across *different builds* does not
prove stochastic instability, but the 12-to-72 difference in the
current build demonstrates that interpreting a 12-path peak is
particularly unreliable. The 72-path report is preserved under
the gitignored frozen archive directory on the machine.

The [historical raw-clock reassessment](
historical-elapsed-clock-reassessment-2026-10-11.md)
also invalidates the earlier tiny goal-timing improvement over
a uniform clock: v2 NLL is 1.79482 versus 1.79176 (worse,
confidence interval includes zero). Source-clock alignment and
substitution event semantics must be addressed before fitting a
numerical-advantage or late-momentum model.

**These shifts are NOT demonstrated causal effects.** Only 12 independent
paths per variant produce enormous sampling error (worst-case binary
proportion standard error approximately 14.4 percentage points). Changing
which stochastic events are generated also changes RNG consumption.
Thus the shared seed does **not** make the variant paths paired
counterfactuals. These results only establish mechanism sensitivity.

The EPL channel graph has no explicitly calibrated defensive
numerical-advantage term: all current goal loadings have
defense_log_weight = 0, while active-player metrics are aggregated
to cross-sectional means. Removing a player and updating mental
state can indirectly alter player rates, but a dismissal is **not
guaranteed to cause an immediate opposing-team scoring increase**
in the current model. This gap merits direct historical evaluation.
The original October forecast also assigned 100% probability to
a scheduled substitution trigger in 60–75, which may generate
mechanical late-match changes.

## Reproduction

Use a **new output directory**; existing archives are never overwritten.
Licensed input rows and binary copies remain in gitignored data/ml:

    python3 scripts/run_frozen_pl_research.py \
      --output-dir data/ml/liv-example-baseline --only Liverpool \
      --paths 12 --seed 42 --repeat 2

    python3 scripts/run_frozen_pl_research.py \
      --output-dir data/ml/liv-example-no-tactics --only Liverpool \
      --paths 12 --seed 42 --repeat 1 --no-tactical-subs

    python3 scripts/run_frozen_pl_research.py \
      --output-dir data/ml/liv-example-no-red --only Liverpool \
      --paths 12 --seed 42 --repeat 1 --red-probability 0

    python3 scripts/run_frozen_pl_research.py \
      --verify data/ml/liv-example-baseline

    python3 scripts/summarize_frozen_pl_ablation.py \
      data/ml/liv-example-baseline \
      data/ml/liv-example-no-tactics \
      data/ml/liv-example-no-red

The [machine-readable comparison](
../artifacts/validation/liverpool-mechanism-ablation.json) includes
the common executable/input fingerprints, every 15-minute pressure
component, incident probabilities and explicit limitations.

## Evidence gate before model changes

Next: collect a larger, dated historical event dataset, using
verified FBref/soccerdata and StatsBomb sources. Evaluate the
side-specific dismissal hazard and **conditional 11-v-10 versus 11-v-11
shot/goal intensity** after the event, controlling for score and
team strength. Separately assess substitution timing against
actual substitution events. Compare models on protected later
matches with proper scores and uncertainty, and run substantially
more paths with a frozen binary/inputs before interpreting peaks.
Do not fit anything to this already-known Liverpool result.
