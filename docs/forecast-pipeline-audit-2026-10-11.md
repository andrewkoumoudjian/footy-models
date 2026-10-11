# Forecasting pipeline audit — 2026-10-11

**Scope:** `andrewkoumoudjian/footy-models` at `8ba0c86`, current four-fixture Premier League pipeline, historical Rust-engine evaluator and local data inventory. This is an architecture/data audit, **not** a refit, successful OOS challenge or permission to promote any result. Original forecast and evaluation archives must remain immutable.

## Verdict

The main constraint on forecast reliability is **as-of player data quality, selection-model specification and observed-label breadth**, not the number of simulated seconds or neural-network capacity. The live Premier League driver runs the sophisticated Rust event engine on heuristically generated player rates, absent selection histories, effectively unconstrained formations and an incident prior inconsistent with historical incidence. Team and player data do exist locally but feed different experiments and are **not yet unified in the live as-of feature contract**.

Using all available data means using *all applicable, verified, legally usable, correctly effective-dated information*, with explicit missingness and uncertainty. It does not mean treating the 2026 FM attributes as valid 2015 training inputs, joining unverified names, or injecting observed match labels into their own pre-match features.

## Current execution graph and insertion points

```text
Historical E0/E1 results ----------> scripts/forecast_premier_league.py
                                       -> apps/plforecast -> result-only + FM team benchmark
                                               |
Registered squad article ---> scripts/match_premier_league_fm26.py   [identity/vintage FIX]
Raw FM26 -> name/CA matcher ------> squads_<asof>.json               [source confidence FIX]
Verified team absences -----------> scripts/prepare_pl_player_inputs.py
                                       -> player_fm_join.csv           [selection and rate REPLACE]
                                       -> asof_team_stats.csv          [true xG/missingness FIX]
                                               |
                          apps/matchforecast/src/driver.rs::build_input
                            |                |                 |
                       squad/role XI    player event rates    team-goal anchor
                       [FIX first]       [TRAIN after]       [CALIBRATE after]
                            \________________|_________________/
                                             |
                      crates/match/src/second_resolved_mc.rs
                            path-local state and event clock
                                             |
                  full-score PMF, players, moments, minute windows
                                             |
               same-engine chronological target-specific scorer
                        [EXTERNAL VALIDATION REQUIRED]
                                             |
               configs/forecast_targets_v1.json release gate
```

`run_pl_pipeline.py` orchestrates the first path, but currently does **not** pass observed Premier League player event history, trained selection/participation models or either the experimental Candle model to the live Rust `build_input` (`driver.rs::run_premier_league` calls `build_input(..., None, None)`). The independent team-goal model is used as a **goal-intensity anchor**, not as a replacement result distribution, so improving the player model must be evaluated jointly against that anchor to avoid repeated signals/double counting.

## Local data inventory: actual coverage and use

| Input / provenance | Present, inspected | In live EPL path? | Appropriate use and qualification |
| --- | --- | --- | --- |
| Official 2026–27 registrations + dated October absences | 600 registered/U21 listings across eight clubs; 391 FM UID matches; 323 players after exclusions | **Yes** | Candidate pool, dated club and availability states; registration does not mean selection |
| FM26 raw attributes | 563,382 records across player/staff/derived sources, 55 `attr_` columns in export | **Partially** | Dated quality and traits only for reliably identified records; pass source quality and missingness; don't use future editions for earlier matches |
| EPL/Championship E0/E1 match CSVs | 1,196 dated EPL team inputs, August 2023–October 2026; E1 available for promoted-club priors | **Yes** | Team attack/defence, shots and SOT; true xG for only 50 cached 2026/27 EPL games, not the prior 1,146 rows |
| StatsBomb EPL 2015/16 events and lineups | 74 covered fixtures, 74 v2 player-input files; 25 later OOS fixtures | **Historical evaluation only** | Goal/card/substitution/shot event semantics, causal selection baselines; not a live 2026 attribute validation |
| SofaScore MLS player-match history | 113 local per-match files Feb–Aug 2026 | **Experimental MLS only** | 2026 as-of participation/rate fitting and FM26-linked pilots; not direct EPL league transfer without verification |
| FM26 + SofaScore pre-match shot dataset | 377 train, 155 validation, 163 previously examined test player-match rows | **No** | A rigorously separated but small model comparison; neither Candle MLP nor set-attention beats baseline on clean validation |
| As-of 2026 MLS candidate-set experiment | 39 train / 10 validation / 17 examined-test fixtures | **No** | Supports explicit candidate/selection learning; target-fixture appearances may never become input roster |
| Galsaar EPL historical results | 11,113 fixtures (1993–2022); no player identities | **Not in live path** | Broader team-level prior, regime and incident checks; discount old seasons and maintain chronology |
| FM22/FM23 local binary archives | Present but extracted name mappings inadequate (12 and 0 in stored notes) | **No** | Potential historically valid FM snapshots *after verified extraction*; no join until validated |
| FBref via `probberechts/soccerdata` | Source adapter implemented; local FBref cache has only three HTML pages, no usable player-match archive verified | **No** | Acquire bounded, permitted dates of player match logs/lineups; preserve provider IDs; current name-only adapter is not a safe FM join |
| Hugging Face FM account/archive reference | Prior local access failed; no verified new historical player dataset | **No** | Authenticate/license-check and inspect vintage before considering as a training source |

The source summary above is limited to the inspected local environment; it does not imply additional remote data are accessible or licensed. FBref's soccerdata interface supports `read_player_match_stats`, `read_lineup` and `read_events`, but successful collection and provider rights have to be checked separately ([soccerdata API](https://soccerdata.readthedocs.io/en/latest/reference/fbref.html)).

### Material source defects

1. **P0 identity/type leak, before fitting.** `scripts/match_premier_league_fm26.py` indexes FM rows with a name/CA filter but **does not require** `kind=player` or retain `kind`/`attr_source` in `squads_<asof>.json`. Of the 323 simulated FM IDs, the raw records classify **157 as genuine DB player records**, **76 as derived player records**, and **90 as `kind=staff`** (75 `attr_source=db`, 15 `attr_source=staff`). A person can have multiple football roles, so staff classification isn't proof of a false name match, but it is unverified input that cannot be granted player-rating certainty. Gate source kind, dated playing eligibility, DOB/provider-ID uniqueness and ambiguity; maintain a manual resolution table, not CA-based tie-breaking. The raw official roster also lists one FM UID under both a parent and loan club; the input adapter excludes the loaned-out copy by name suffix, but team-membership intervals must become explicit data.
2. **P0 selection and role specification.** All **323** live player rows have `team_fixtures=0`, `selection_starts=0`, `selection_listings=0` and no real last-appearance timestamp. `driver.rs::squad` consequently uses CA/old minute proxies and fixed `0.96` availability; it keeps just 18 candidates or pads invented players. `driver.rs::formation(id)` always generates the same 11 synthetic role IDs for both 4-4-2/4-3-3, and `squad` assigns **role_fit=1.0 for every candidate and role**. These are labels on interchangeable slots, not real formations/keepers. Introduce as-of active-player registration/positions, verified GK slots, role eligibility, formation mappings, dated recent starts/minutes, injury state and explicitly unknown candidates; refuse impossible XI states rather than fabricate confidence. Keep the max-entropy role-neutral sampler research-only until its conditional/unconditional availability calibration and goalkeeper constraints are tested.
3. **P0 false measurement certainty.** `scripts/prepare_pl_player_inputs.py::write_results` replaces absent `HxG/AxG` with **realized goals** and absent shots/SOT with fixed numbers, then writes ordinary numeric columns with no provenance/missingness flag. Only **50 of the 1,196 EPL input rows** have measured source xG; **1,146** do not. This is a *valid earlier-result feature proxy* if explicitly labeled, not measured xG. Add `observation_kind`, provider, effective/published timestamp, coverage and uncertainty to typed features. `driver.rs::f()` likewise maps failed/missing numerical parses to **0**, violating the source-independent missing-is-not-zero invariant. Reject corrupt rows and marginalize explicitly missing metrics.
4. **P1 nearly unused fine-grained FM information.** Although the raw FM export contains **55 `attr_` columns**, `prepare_pl_player_inputs.py` collapses most data into heuristic player shots/xG/xA/passes/carries; `driver.rs::attributes_and_states_for` supplies only finishing, technique and a marking/tackling composite to the matchup graph, and manufactures most ability-vector dimensions from CA. Pass vintage-correct observed traits with source-quality/missingness, but fit their target-specific log-rate contributions on historical labeled matches rather than increasing arbitrary coefficients. Fatigue is currently initialized to `0.15` for every candidate; actual rest/travel/workload isn't represented.
5. **P1 unreliable event processes.** `driver.rs` sets `red_card_probability_per90=0.02` *per active player*, and `second_resolved_mc.rs::sample_stochastic_event` independently generates red hazards for the entire XI. The historical incident audit (`artifacts/validation/pl-red-card-prior-research.json`) shows an implied **35.9%** red-match probability versus **10.9%** realized in its 1,270-match later period; a previous-only fit was **14.0%**, with a better Brier/log score as a simplified diagnostic. That study lacks the complete match engine and cannot alone justify a promoted rate. Red cards, injuries and completed substitutions need separately estimated, state-aware hazards and one event definition per label; score after correction on fresh same-engine OOS. Currently the goal point process is not generated conditionally on a modeled shot attempt: goal and player-shot event hazards are separate, and the engine adds an attributed shot upon a scored goal to conserve counts. This preserves aggregate arithmetic but not a calibrated shot→on-target→goal mechanism.
6. **P1 match and temporal calibration.** The actual second-resolved 2015/16 study has only 25 held-out, club-skewed fixtures: W/D/L LL **0.9768 versus 0.8899** for the causal regularized-Poisson reference; exact-score LL **3.0139 versus 2.7523** (worse, with positive bootstrap lower bound). The fixed-goal correction worsened OOS; selection adjustment was research-only; the corrected StatsBomb elapsed-clock goal-time log loss **1.79482 versus 1.79176 uniform** provides no timing victory. Liverpool–City's relative-pressure peak is an already-examined anecdote with mechanism confounding from scheduled substitutions. Do not fit a fatigue/red-card/momentum coefficient to that fixture. Keep arbitrary momentum weighting `shots+3×goals` display-only until an independent possession/territorial-pressure target and valid scoring rule exist.
7. **P2 computation, not the first bottleneck.** The 1-second engine recomputes state-dependent contexts using maps and per-player metric loops; more paths reduce simulation error but don't repair a biased input contract. Profile before changing caches/event scheduling. Optional Modal already ran a **byte-identical** one-fixture local/cloud smoke; a single large eight-CPU replay is expensive. Shard future independent historical fixtures and return deterministic, per-fixture hashed outputs. Do not re-score the same inspected OOS period as new evidence.

## Where improvements should sit, with an evidence-driven build order

| Phase | Owner / boundary | Concrete build | Gate to next stage |
| --- | --- | --- | --- |
| **A: trustworthy pre-match records** | Source adapters, entity identity, temporal feature store (`crates/source-*`, `entities`, `temporal`, `dataset`; `scripts/match_premier_league_fm26.py`) | Canonical dated club/player IDs; track FM `kind`/`attr_source`; roster eligibility and position; actual-source vs imputed xG; data availability timestamp; reject invalid joins | All candidate rows have justified identities or explicit unknown states; all features satisfy `published_at <= forecast_asof`; no silent zeros/fabricated lineup roles |
| **B: broaden observed labels** | StatsBomb + FBref/Soccerdata + SofaScore import adapters; schema-normalized event tables | Acquire licensed PL seasons/other league player minutes, official starts, events, goals, assists, cards and substitutions, with provider IDs and source-clock labels; restore historically dated FM archives where possible | At least 200 independent chronologically held-out fixtures with usable player/lineup targets across diverse clubs, no overlap or same-day leak; meaningful coverage audits |
| **C: participation/lineup** | `crates/forecast` selection; `crates/match/lineup.rs`; live `driver.rs::build_input` | Empirical-Bayes or regularized logistic player-start/appearance model, player minutes/survival model, explicit keeper/role/formation constraints; conditional availability, locks and tactical bench | Start/appearance Brier + minutes MAE improve against causal earlier-only priors on untouched dates, while team-goal and score forecasts don't significantly worsen |
| **D: conditional player events** | New source-neutral learned-priors adapter upstream of `crates/match` | Exposure-aware Poisson/NB GLM with role and calibrated FM features for shots; independent empirical scorer/assist models; only after baseline win, a tiny MLP and permutation-invariant attention challenger | Player shot-count log score, goal/assist Brier, reliability and coverage improve; zero target-minute leakage; accurate full-match path outputs; validation decides hyperparameters, final test stays untouched |
| **E: game-state/goal hazards** | `crates/match/second_resolved_mc.rs` and team-goals calibration | Match-level attack/defence prior as anchor, conditional shot→quality→goal process as a coherent challenger, fit red/injury/successful-sub hazards and 15-minute score-state/timing effects; ablate scheduled subs and numerical advantage with common random numbers | Exact-score/WDL log score and team-goal CRPS improve over regularized-Poisson and unchanged engine, plus calibrated conditional incidents on fresh matchdays |
| **F: statistical certification** | `crates/backtest`, chronological runner and `check_forecast_release.py` | Nested rolling-origin fit/validation/test, club/season clustered uncertainty, prediction intervals, score-calibration curve & PIT/rank histograms, distinct per-target release gates and immutable manifests | At least 200 geographically/club-diverse held-out fixtures and valid label coverage per target; paired interval of proper-score difference excludes zero in favorable direction; negative-control leakage tests pass |
| **G: scalable execution** | Optional `scripts/modal_pl_hist.py` and orchestration | Fixture-level deterministic batch sharding, immutable input mounts, per-run binary+dataset fingerprints, bounded retry/logging and identical local/cloud outputs | Matches local scores/outputs and reduces wall time/cost per valid fixture; runtime may never change the training cutoff or stochastic path specification |

### Minimal model interface

Rather than wire datasets directly into the 5,400-second event loop, define a single **versioned, pre-match feature and prior bundle** assembled once per fixture:

```
FixtureAsOf {kickoff_utc, cutoff_utc, feature_contract_hash,
  teams: [AsOfTeam{attack_defence_prior, variance, recent_context, source_coverage}; 2],
  roster: [AsOfPlayer{canonical_id, dated_club, eligibility, roles, traits, source_quality,
                   p_available, p_start, minutes_distribution,
                   event_rate_distributions, missingness}; N],
  state_hazards: {red, injury, completed_substitution, tempo, score_context},
  calibration_id, model_version}
```

The data adapters own source identity and publication-time semantics; the forecast layer owns probabilistic estimates and their uncertainty; the simulator takes only source-neutral distributions and path-local events; the backtest/release layer owns scoring. Expensive neural inference happens once on a candidate player/team set, never every simulated second. Every incremental predictor must demonstrate improvement over the same input/engine baseline before promotion.

### What to implement immediately versus defer

**Start with phase A plus phase C's structural invariants.** They repair measurable errors that propagate to every target: 90 staff-classified FM records in live inputs, 76 derived player records, 323 missing selection histories, indistinguishable formations and 1,146 proxy-xG rows. Then build a dated player-event store from available usable sources, which unlocks genuine phase C/D fitting. Run a simple participation/shot GLM before any new transformer work; neither Candle challenger has won clean validation yet.

**Leave alone for now:** pre-kickoff original forecasts, the 2015/16 inspected holdout as a selection set, the default exact-cardinality research flag, the production result calibrator, arbitrary momentum retuning, transformer promotion and blanket red-card-rate replacement. Each potentially changes the joint distribution and requires a new same-engine test. Preserve already-published historical artifacts verbatim.

## Reproduce the counts and existing checks

```
python3 scripts/audit_forecast_input_coverage.py \
  --fm /home/drewclaw/fm26-data/fm26_men_full_attrs.csv
python3 scripts/audit_pl_hist_label_clock.py
python3 scripts/check_forecast_release.py
cargo test -p footy-match -p footy-matchforecast
```

The coverage audit prints only aggregate classifications, data timestamps, counts and fingerprints; it must not expose the proprietary FM attributes or detailed player records. For a different fixture week use its new as-of roster, feature file and FM vintage, not the October 10 data by default. Use distinct artifact paths for each evaluation. Existing strict-Clippy warnings in unrelated crates are not evidence of changed forecast correctness.

## Bottom line

The best immediate improvement is to fix who can play, where they can play, what we actually know about them, and which prior observations exist. The stable team-goals reference is the anchor to beat. Calibrate player participation, then conditional event rates, then state transitions against a **new, properly sized** historical holdout before increasing model complexity. Modal can accelerate that evaluation, but cannot substitute for adequate as-of training data.
