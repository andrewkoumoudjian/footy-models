# Historical same-engine backtest and calibration status

## Audit: what existed and what was actually missing

The repository already had, before this work:

- `crates/match/src/second_resolved_mc.rs`: the production one-second player-driven match Monte
  Carlo (`simulate_second_resolved_dynamic_mental_match`) with lineup sampling, player event
  models, score state, incidents, substitutions and psychology.
- `apps/matchforecast/src/driver.rs`: the shared input adapter used by the MLS and Premier League
  binaries (team EWMA windows, squad construction, player event models, neutral mental model).
- `apps/matchforecast/src/backtest.rs`: an MLS walk-forward backtest of that engine (with
  SofaScore-derived inputs).
- `scripts/backtest_pl_event_oos.py` + `artifacts/validation/premier-league-events-2015-16-oos.json`:
  a **separate causal event-rate baseline** on StatsBomb 2015/16, explicitly *not* the Rust engine.
- `scripts/check_forecast_release.py`: fail-closed release gates that require a version-matched
  same-engine OOS report and refuse cross-model substitution.
- `configs/forecast_targets_v1.json` / `docs/forecast-targets.md`: the target contract, including
  the explicit allowance to evaluate an observed-stat-only variant of the same engine when
  time-compatible FM attributes do not exist.

The true gaps this work addresses:

1. **No same-engine historical OOS validation.** The 2015/16 StatsBomb evaluation scored a separate
   rate model, so nobody knew whether the actual Rust engine predicted anything useful.
2. **No joint scoreline distribution export.** The engine exported only marginal goal summaries,
   so joint exact-score log loss and derived totals/BTTS markets were impossible.
3. **No per-player path occurrence export.** `P(player scores at least once)` could not be computed
   from collapsed marginals, blocking player goal/assist occurrence scoring.
4. **No historical input builder.** Nothing constructed strictly causal player pools, per-90 rates,
   selection histories and labels from the 2015/16 event data in the engine's input contract.
5. **A performance blocker**: the matchup interaction graph was rebuilt per player-metric on every
   clock tick, making a 500-path historical walk-forward impractical (measured ≈6.4 s/path before
   batching, ≈1.4 s/path after).
6. **A roster-coverage bug**: the production 45-day registered-squad freshness filter silently
   discarded every player of clubs whose matches were not event-covered recently. Historical runs
   now use a documented no-freshness loader.

FM26 attributes were **not** usable for 2015/16 fixtures: the repository only holds the 2026
database, and using it for a 2015 fixture would be direct leakage. Per the target contract, this
work evaluates an **observed-stat-only version of the same engine and entry point**, and says so
everywhere.

## Data actually used

- StatsBomb open data, English Premier League 2015/16, local cached JSON: 74 event-covered matches
  (38 Manchester United fixtures, 38 AFC Bournemouth fixtures, 2 of them head-to-head, plus 4
  fixtures each for 18 other clubs) out of 380 season fixtures. The other 306 fixtures contribute
  **scores only** (team goal windows) and never player events.
- All features for a fixture use only event-covered matches with `date < fixture date`; same-day
  fixtures never contribute. `data/statsbomb/pl_hist/build_report.json` records the audit
  (`max_feature_date_before_fixture: true`, zero warnings).
- Labels: StatsBomb shot outcomes, `Pass.goal_assist`, own goals, red cards, substitutions and
  lineup minute intervals for the same 74 matches.

Coverage consequences (honest, not papered over):

- Only 56 of the 74 covered fixtures have ≥11 causally listed players for both clubs (both clubs
  need at least one prior covered match). 18 fixtures are skipped and recorded.
- The evaluable sample is **club-skewed** toward Manchester United and AFC Bournemouth. This is a
  property of the open-data subset, not of the engine.
- Actual goals credited to players in the causal candidate pool: see the report coverage block.
  Players who first appear for a club in the target fixture (transfers, academy debuts) cannot be
  in a causal pool and are reported as uncovered.

## Method

```
scripts/build_pl_hist_inputs.py     causal player pools, team rows, labels, no-lookahead audit
        │
        ▼
footy-matchforecast-pl-hist         runs the actual Rust engine per fixture (observed-stat-only
        │                           contract: no FM, no attributes, no matchup rules; neutral
        │                           psychology; 2015/16 substitution rules), exports raw paths
        │                           plus online walk-forward calibration parameters
        ▼
scripts/score_pl_hist_backtest.py   OOS metrics vs causal baselines, matchday-block bootstrap,
        │                           engine fingerprint, release-gate-compatible report
        ▼
artifacts/validation/pl-hist-same-engine-oos.json
```

- Split: the last 24 simulated fixtures are held out; everything earlier is fit-period only.
  The split is chronological by fixture date; the report records `fit_end_date < oos_start_date`.
- Calibration is frozen at the OOS start: the temperature / draw-bias parameters fitted by the
  engine on all fit-period fixtures are applied unchanged to every OOS fixture. No OOS result
  informs the model or the calibration.
- Baselines: regularized independent Poisson with causal home advantage (prior results only),
  league-average Poisson, previous-only player per-match rates, previous-only 15-minute/one-minute
  team goal hazards, and previous-only state/incident priors. All are reproducible from the same
  local data.
- Metrics follow `configs/forecast_targets_v1.json`: multiclass log loss/Brier, joint scoreline
  log loss, discrete CRPS and count MAE, occurrence Brier for player goals/assists/starts, minute
  and 15-minute goal Brier, score-state Brier, incident Brier. Matchday-block bootstrap intervals
  are paired on the same fixtures.

## Results (200 paths per fixture, seed 42)

Machine-readable report: `artifacts/validation/pl-hist-same-engine-oos.json`. Split: fit 31
fixtures (ends 2016-02-07), held-out OOS 25 fixtures (2016-02-13 → 2016-05-17). All numbers below
are OOS; intervals are 95% matchday-block bootstrap of (engine − causal reference), so an interval
entirely below zero means the engine improved that proper score. Frozen calibration experiments are
scored on the same OOS fixtures in `pl-hist-same-engine-oos-calibrated.json` and
`pl-hist-same-engine-oos-selection.json`; their paired differences are (experiment − baseline), so a
negative difference means the experiment improved.

| Target | Engine | Causal reference | Engine − reference [95% CI] | Verdict |
|---|---:|---:|---|---|
| W/D/L log loss | 0.9768 | 0.8899 (regularized Poisson) | [−0.0253, +0.1923] | **not established** |
| W/D/L Brier | 0.5892 | 0.5186 (Poisson) | — | worse point estimate |
| Exact score joint log loss | 3.0139 | 2.7523 (Poisson) | [+0.0415, +0.4577] | **clearly worse** |
| Team goals discrete CRPS | 0.6105 | 0.5523 (Poisson) | [−0.0081, +0.1227] | not established |
| Team goals MAE | 0.9541 | 0.8506 (Poisson) | — | worse point estimate |
| Player goal occurrence Brier | 0.0369 | 0.0382 (previous-only rate) | [−0.0019, −0.0005] | **improved** |
| Player assist occurrence Brier | 0.0293 | 0.0291 (previous-only rate) | [−0.0004, +0.0010] | not established |
| Player start Brier | 0.1689 | 0.1558 (previous-only rate) | [+0.0085, +0.0185] | **clearly worse** |
| Player appearance Brier | 0.2069 | 0.1761 (previous-only rate) | [+0.0238, +0.0376] | **clearly worse** |
| Player minutes MAE | 25.39 | 24.24 (previous-only mean) | — | worse point estimate |
| Player shots MAE | 0.5116 | 0.6148 (previous-only rate) | [−0.1134, −0.0930] | **improved** |
| 15-minute goal Brier | 0.2174 | 0.2326 (previous-only window hazard) | [−0.0331, +0.0044] | better point, not established |
| 1-minute goal Brier | 0.0277 | 0.0289 (previous-only minute hazard) | [−0.0019, −0.0006] | **improved (small)** |
| Score-state Brier | 0.6072 | 0.6264 (previous-only state prior) | [−0.0504, +0.0094] | better point, not established |
| Red-card Brier | 0.0162 | 0.0137 (previous-only prior) | [+0.0001, +0.0043] | slightly worse |
| Substitution Brier | 0.1816 | 0.4692 (league-average prior) | [−0.3488, −0.2249] | **improved** (weak reference) |

Coverage: 64 actual non-own goals, 60 credited to causal pool players (0.938); 50 registered
assists, 47 in pool (0.940); 1,445 player-fixture rows scored. OOS clubs: Manchester United 13,
AFC Bournemouth 13, then one or two fixtures each for 18 other clubs.

Diagnostics:

- The engine **over-predicted goals** on the fit period (88.7 predicted vs 75 observed) and its
  modeled home/away goal ratio (1.178) exceeded the fit-period observed ratio (1.083). The
  fit-period-only correction (`--goal-rate-scale 0.8452`, `--home-advantage-multiplier 1.0022`)
  is recorded in `pl-hist-goal-contract-fit.json` and was evaluated as a frozen OOS experiment
  (**rejected**; see the experiment section below).
- The frozen fit-period outcome calibrator degenerated to its grid edge (temperature 3.0) and made
  OOS W/D/L log loss worse (1.0786). It is **not** promoted.
- **Selection is the largest miscalibration.** The lineup sampler compresses start probabilities
  toward the middle relative to the input priors (prior 0.9+ → realized 0.81, prior <0.1 →
  realized 0.002), so the engine's start/appearance Brier is clearly worse than the same prior
  without the sampler. This is the top roadmap item below.

### Frozen calibration experiments (fit on the 31 fit fixtures, evaluated on the same 25 OOS fixtures)

Both experiments reran the identical frozen engine with the same seeds and differ only in one
recorded input parameter, so a paired comparison against the baseline is exact per fixture. Paired
differences are (experiment − baseline); negative means the experiment improved that score.

**1. Global goal-level + home-advantage rescaling — rejected.** This was the direct correction
implied by the fit-period overprediction, and it makes the outcomes it was meant to fix worse. The
frozen baseline was in fact already correctly levelled on OOS: 70.7 expected goals (H 38.5 / A 32.2)
against 68 actual (H 37 / A 31), while the rescaled run produced only 56.5 expected goals against
the same 68. The fit-period overprediction did not persist into the evaluation window — exactly the
failure mode a frozen OOS window exists to catch.

| Paired OOS metric (experiment − baseline) | Δ | 95% CI | Verdict |
|---|---:|---|---|
| W/D/L log loss | +0.0604 | [+0.0081, +0.1209] | **worse** |
| 15-minute goal Brier | +0.0097 | [+0.0017, +0.0180] | **worse** |
| Exact-score log loss | +0.1240 | [−0.0624, +0.3264] | worse point |
| Team-goals CRPS | +0.0272 | [−0.0138, +0.0713] | worse point |
| Player appearance Brier | +0.0014 | [+0.0000, +0.0027] | slightly worse |
| Player goal-occurrence Brier | −0.0008 | [−0.0012, −0.0003] | **improved** |

**2. Selection-marginal correction — accepted as a mechanism fix, target not promoted.** The
fit-period isotonic table (`pl-hist-selection-correction.json`, 1,166 player points from the 31 fit
fixtures) rewrites the sampler input so realized start marginals land on the input priors. On OOS it
does what it was fitted to do: the sum of absolute prior−realized gaps over ten prior buckets falls
from 0.678 to 0.455 and *every* bucket moves toward the diagonal (prior 0.89: realized 0.800 →
0.836; prior 0.53: 0.611 → 0.560; prior 0.20: 0.077 → 0.094). Player minutes MAE improves by an
established margin; the start/appearance Brier gains are directionally positive but not individually
established on 25 fixtures; outcome-level point estimates lean slightly worse but are not
established. No established deterioration anywhere, so the `--selection-correction` flag stays
available for same-engine evaluations and is still not wired into any default path.

| Paired OOS metric (experiment − baseline) | Δ | 95% CI | Verdict |
|---|---:|---|---|
| Player minutes MAE | −0.2579 | [−0.4252, −0.1027] | **improved** |
| Player start Brier | −0.0016 | [−0.0040, +0.0007] | improved point |
| Player appearance Brier | −0.0007 | [−0.0024, +0.0010] | improved point |
| Player goal-occurrence Brier | −0.0004 | [−0.0008, +0.0000] | improved point |
| W/D/L log loss | +0.0458 | [−0.0010, +0.0966] | worse point |
| Team-goals CRPS | +0.0175 | [−0.0019, +0.0394] | worse point |

The experiment path exports are deterministic but large and are not stored in the repository;
regenerate them with the flags recorded in each report's `input_contract` and the commands in the
Reproduce section.

## What is and is not validated

- **Validated on this sample:** player shot volume, player goal occurrence, one-minute goal
  occurrence and substitution incidence beat their causal references with bootstrap intervals
  below zero. The engine's player-production layer is genuinely informative.
- **Not validated:** match outcome (W/D/L), exact score and team-goal distributions do not beat the
  causal regularized-Poisson baseline; exact score is clearly worse. The goal level was already
  close to correct on OOS (70.7 expected vs 68 actual), the larger fit-period gap did not generalize,
  and a global fit-period rescale made losses significantly worse (see the experiment section).
- **Not validated:** the 2026 FM26-enhanced pipeline. The FM attributes, registered-squad priors
  and results-model anchor change the feature contract; this report does not transfer to them.
- **Not validated:** any league-wide claim. The sample is 25 held-out fixtures from a club-skewed
  subset, far below the 200-fixture release-policy threshold. The release gate therefore keeps
  every target `research_only`.
- **Not validated:** one-minute event timing beyond goal occurrence; corners/yellow cards (not in
  the engine); momentum (still a display-only proxy).

## Accepted / rejected calibration changes

Every calibration experiment is recorded here with the acceptance criteria from
`docs/forecast-targets.md` (proper-score improvement on unseen fixtures, supported by uncertainty,
no unacceptable deterioration elsewhere).

| Change | Status | Evidence |
|---|---|---|
| Batched matchup interaction graph (performance, distribution-preserving) | **Accepted** | Unit test asserts batched contexts equal the per-player path; distributions unchanged; ≈4.5× faster wall clock |
| Joint scoreline + player occurrence exports (new outputs, no distribution change) | **Accepted** | Coherence tests: scoreline sums to 1, W/D/L marginals agree, attributed + unattributed goals conserve team goals |
| Historical no-freshness player loader (coverage fix) | **Accepted** | Recovers 29 fixtures that the 45-day production rule wrongly discarded; causality unchanged |
| Outcome temperature/draw-bias calibration | **Rejected, not promoted** | Frozen fit-period calibrator degenerated to temperature 3.0 and made OOS W/D/L log loss worse (1.0786 vs 0.9768 raw) |
| Assist prior 0.68 → 0.61 (previous session) | **Rejected, retained** | Failed the paired loss gate; see `primary-assist-prior-2015-16-oos.json` |
| Goal-level + home-advantage contract correction | **Rejected, not promoted** | Frozen OOS paired deltas (`pl-hist-same-engine-oos-calibrated.json`): W/D/L log loss +0.0604 [+0.0081, +0.1209] worse; 15-minute goal Brier +0.0097 [+0.0017, +0.0180] worse; only player goal occurrence improved (−0.0008 [−0.0012, −0.0003]). The fit-period overprediction did not persist: baseline OOS 70.7 expected goals vs 68 actual, rescaled 56.5. |
| Lineup-sampler marginal compression (isotonic selection correction) | **Accepted as mechanism fix (research flag); target not promoted** | Frozen OOS (`pl-hist-same-engine-oos-selection.json`): every prior bucket moves toward the diagonal (sum of absolute gaps 0.678 → 0.455); minutes MAE −0.2579 [−0.4252, −0.1027] established; start Brier −0.0016 and appearance Brier −0.0007 directional; W/D/L +0.0458 [−0.0010, +0.0966] and team-goals CRPS +0.0175 [−0.0019, +0.0394] lean slightly worse but are not established. |

## Development roadmap, ordered by expected predictive benefit

1. **Goal-process calibration, conditional on match state (global rescaling rejected).** A single
   global goal scale fitted on the fit period made W/D/L and 15-minute losses worse OOS because the
   fit-period overprediction did not persist into the evaluation window. Any further level
   calibration must be conditioned on match state/team strength and validated with the same frozen
   protocol instead of applied as a global constant.
2. **Selection-sampler marginal calibration (correction accepted as a research flag).** The
   isotonic input-side correction verifiably reduces the sampler's marginal compression OOS and
   improves player minutes MAE; the start/appearance Brier gains are directionally right but need
   more held-out fixtures before the target itself can be promoted. Next: use the correction in
   future same-engine evaluations and revisit the target gate with a larger OOS sample.
3. **More same-engine historical data.** The single biggest evidence blocker: 25 held-out fixtures
   from two dominant clubs cannot clear the 200-fixture release policy or support league-wide
   claims. Additional dated event sources with lineups (more StatsBomb seasons/competitions,
   licensed historical feeds) would multiply the evidence base.
4. **Scorer/assist attribution calibration.** Model `P(assist | goal)` and the assister identity
   from earlier events (the existing 0.68 prior is explicitly uncalibrated). Player goal occurrence
   already beats the previous-only rate; attribution identity is the next step once occurrence
   labels grow.
5. **Timing and score-state calibration.** 15-minute goal occurrence and one-minute goal occurrence
   already beat their causal references in point estimates (one-minute is established). Recalibrate
   hazards by elapsed time and score state only after the selection correction is in use and a
   conditional level calibration lands, because both change the goal mix.
6. **Team-process calibration.** Overdispersion, home advantage and match-state feedback estimated
   on large team-level result sets (e.g. the cached football-data seasons), then transferred to the
   engine's `goal_process` parameters as versioned inputs.
7. **Time-compatible FM editions.** Acquiring FM snapshots with original effective dates would
   upgrade the historical contract from observed-stat-only to FM-enhanced and let the same-engine
   backtest evaluate the real 2026 feature contract on past seasons.
8. **Path precision policy.** Once structural calibration passes, raise historical evaluation
   paths to the 2,500-path operator policy for publication-grade Monte Carlo precision.

## Reproduce

```bash
python3 scripts/fetch_statsbomb_history.py                  # once, network, local raw cache
python3 scripts/build_pl_hist_inputs.py
cargo build --release -p footy-matchforecast --bin footy-matchforecast-pl-hist
./target/release/footy-matchforecast-pl-hist --paths 200 --jobs 4 \
  --output artifacts/validation/pl-hist-same-engine-paths.json
python3 scripts/score_pl_hist_backtest.py
python3 -m unittest tests.test_pl_hist_backtest -v
```

Calibration experiments are fitted on the fit period only and evaluated frozen on the same OOS
window, with paired matchday-block bootstrap against the baseline run:

```bash
python3 scripts/fit_pl_hist_goal_contract.py --output artifacts/validation/pl-hist-goal-contract-fit.json
./target/release/footy-matchforecast-pl-hist --paths 200 --jobs 4 \
  --goal-rate-scale <fit> --home-advantage-multiplier <fit> \
  --output artifacts/validation/pl-hist-same-engine-paths-calibrated.json
python3 scripts/score_pl_hist_backtest.py \
  --predictions artifacts/validation/pl-hist-same-engine-paths-calibrated.json \
  --compare-predictions artifacts/validation/pl-hist-same-engine-paths.json \
  --experiment "fit-period goal scale + home advantage" \
  --output artifacts/validation/pl-hist-same-engine-oos-calibrated.json

python3 scripts/fit_pl_hist_selection_correction.py \
  --output artifacts/validation/pl-hist-selection-correction.json
./target/release/footy-matchforecast-pl-hist --paths 200 --jobs 4 \
  --selection-correction artifacts/validation/pl-hist-selection-correction.json \
  --output artifacts/validation/pl-hist-same-engine-paths-selection.json
python3 scripts/score_pl_hist_backtest.py \
  --predictions artifacts/validation/pl-hist-same-engine-paths-selection.json \
  --compare-predictions artifacts/validation/pl-hist-same-engine-paths.json \
  --experiment "fit-period selection marginal correction" \
  --output artifacts/validation/pl-hist-same-engine-oos-selection.json
```

Comparison blocks report paired differences as (experiment − baseline), so a negative difference
means the experiment improved that score.

Or as part of the integrated pipeline:

```bash
python3 scripts/run_pl_pipeline.py --render-only --historical-same-engine-oos
```
