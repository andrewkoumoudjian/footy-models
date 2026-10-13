# FM24-informed full match engine — architecture decision and implementation plan

**Status:** Approved direction (2026-10-13); specification only, not implemented.
**Target:** `footy-models`, Rust `crates/match`. **Priority:** reconstruct decision and event-generation mechanics, then Monte Carlo tail paths.
**Decision:** Build an **FM-style full-match simulator as the primary generative engine**. Match results, probabilities, and player statistics emerge from complete simulated player actions and state transitions. The separately fitted team-goals/Poisson models remain independent benchmarks and diagnostics; they **must not constrain, substitute for, or overwrite** generative simulation outcomes.

## 1. Objective and boundary

For a specific fixture, sample a full match from pre-kickoff player attributes, availability, starting-XI uncertainty, formations, tactics, environment, and the current world state. Simulate player decisions, action outcomes, ball movement, possession, set pieces, goals, bookings, injuries, substitutions, tactical adjustments, fatigue, and stoppage time. Independent trajectories form one coherent distribution for W/D/L, scoreline, team/player events, time-window probabilities, and extreme match scenarios.

FM24 is a **research reference for reconstructing football mechanics**. It is not a required runtime dependency. Do not embed, redistribute, or depend on the FM24 executable or copyrighted assets. FM24 is native C++ era; FM26 is Unity. The FM24 **Full Match Engine (FME)** and **Quick Match Engine (QME)** are separate research targets; do not assume quick-match weights and distributions drive the detailed FME.

This decision is intentionally different from using the existing Poisson/team-goal model as a binding goal anchor or assigning finished match results to paths. Those older approaches remain benchmarks, not the target architecture.

## 2. Reconstruction evidence program (FM24 first)

Start with authorized copies/exports and documented community research, then recover the most consequential mechanics before attempting any visual, animation, or high-fidelity 3D physics work.

| Priority | Extract / infer | Why it matters | Evidence needed |
|---|---|---|---|
| P0 | FM24 player attributes, roles, positional ability, traits, hidden attributes when lawfully accessible | State and decision inputs | Official editor/export schema, edition/version, field definitions |
| P0 | FME action-choice logic: pass/carry/dribble/shoot/press/tackle/clear; action targets and movement | Determines **which** events occur | Recovered code/data and/or controlled behavioral experiments |
| P0 | FME action-resolution logic: pass success, interceptions, shot location/quality, shot outcome, fouls | Determines **what happens** conditional on each choice | Formula/lookup provenance and falsification tests |
| P0 | FME tactical shape: role duties, marking, pressing, defensive blocks, space exploitation, substitutions | Couples all agents through the same evolving state | Parameter tracing and scenario interventions |
| P1 | `simatch.fmf` / `physical_constraints.jsb` and related resources | Movement speeds, acceleration, turning, ball/player constraints | Decoded data type, offsets, units, version, validated runtime effect |
| P1 | QME `qme_stat_relevant_attribute_data` and `qme_distribution_data` | Potential compact statistical model and attribute dependencies | Decode with QME-only provenance; never claim FME equivalence |
| P1 | `momentum_weightings_data` and event-code dictionaries | Possible clues for event bookkeeping/momentum | Trace consumers before attributing causal effects |
| P2 | Animation/presentation/commentary resource tables | Mostly observational/debugging value | Exclude from action generator unless linkage is proved |

Research leads to inspect and reproduce (not pre-validated recovered algorithms): [FM24-matchengine-maker](https://github.com/wonss737/FM24-matchengine-maker), [PlayGM resource-format thread](https://www.playgm.cc/forum.php?mod=viewthread&page=1&tid=972305), [FM Match Lab](https://fmmatchlab.co.uk/fm-match-lab-mods/), and [FM-Arena attribute interventions](https://fm-arena.com/table/26-player-attributes-testing/). Record source URLs, retrieval dates, legal basis, and actual verification results. Do **not** silently promote community claims to known engine behavior.

Store **our own** parsers, schemas, observations, hashes, tests, and mechanism specifications in the repo; keep original proprietary binaries and licensed player exports outside Git. Reverse engineering and native tracing are restricted to legally authorized work. Do not design the workflow around bypassing anti-tamper protections.

For every recovered candidate rule, record:
- Origin: resource offset/field + version, lawful native-code observation, FM24 experiment, published community hypothesis, or independent football-data fit.
- Semantics: input units/ranges, formula or decision table, stochastic vs deterministic behavior, and whether it applies to FME, QME, or unknown.
- Confidence: `verified`, `experimental`, or `hypothesis`; supporting reproduction and negative evidence.
- Mapping into canonical `AttributeId`, `PlayerId`, `RoleId` and source-neutral primitives.
- Counterexample tests: parameter interventions and matched-seed/repeated-match experiments before integration.

**Extraction milestone:** publish a machine-readable mechanism inventory, a versioned FM24-to-canonical attribute map, and a minimal executable decision/outcome model for one football action chain. A list of binary offsets alone is not enough.

## 3. Generative match model

At time `t`, maintain the full causal state:

    S_t = (clock, score, ball position/velocity, player positions/velocities,
           on-pitch personnel, roles, formation, possession, fatigue, fitness,
           cards, tactical instructions, mental state, referee, environment).

For active player `i`, attributes `A_i`, and global tactics `T`:

    a_t ~ pi(a | S_t, A_i, T)                # player/team action selection
    o_t ~ q(o | a_t, S_t, A_i, A_opponents)  # contested physical/action outcome
    S_(t+) = F(S_t, a_t, o_t)                # state transition + resulting events

The distributions `pi` and `q` should use reconstructed FM24 rules when genuinely recovered, and explicitly versioned, fitted alternatives where recovery is incomplete. Do not mistake a proposed mathematical form for an FM24-discovered formula. An event may itself cause another decision: interception -> counterattack -> defensive recovery -> foul -> red card -> tactical restructuring.

**A goal is terminal output of a legal attacking action chain**, not an independently sampled goal hazard with a scorer added after the fact. A scored shot must have a shot, shooter, chance context, defending/goalkeeping resolution, and an updated score state. Goal counts, xG, assists, shots, goalkeeper actions, and score probabilities must be aggregated from these *same* event records.

Suggested minimal action vocabulary: move/off-ball run, choose target, pass, receive, carry/dribble, contest/tackle, intercept, shoot, save, foul/card, restart, substitute, change tactical instruction. Use a compact 2D pitch and velocity/acceleration constraints initially; 3D graphics, animation, and detailed collision meshes are not needed to generate football decisions. Distinguish intention, attempt, success, and officially credited match events.

Implement a hybrid event-driven clock: advance dynamics between decision points, sample competing action/incident hazards only where appropriate, preserve within-second ordering, and recompute downstream decisions/transition intensities after every state-changing event. Scheduled interventions have stable timing in a trajectory. Keep event-generation semantics explicit: not every player action is memoryless/Poisson.

## 4. Rare events and black-swan paths

Rare outcomes must arise through **dependent state transitions**, not independent, post-hoc tail multipliers.

Examples to make representable: early red card and subsequent tactical collapse; goalkeeper injury with no capable substitute; missed penalty -> counterattack; multiple goals in a short interval; deflection/own goal; stoppage-time equalizer or winner; one team losing its ball progression after a press change; injury clusters subject to shared exposure/weather; extreme foul accumulation; unusually one-sided scorelines.

Mechanics:
1. Give each incident a conditional trigger model (player discipline, contacts, pressure, fatigue, referee, pitch, and history). Calibrate baseline rarity on matching historical definitions; unknown effects carry uncertainty.
2. Apply the actual consequences to personnel, shape, tactical plans, stamina, space, ball ownership, subsequent action choices, and conversion opportunities. Never merely add a red-card goal-rate multiplier without a causal pathway.
3. Model shared shocks at the match/path level where supported (weather, tempo, refereeing strictness, tactical instability). Shared shocks produce correlation without inventing independent coincidences.
4. Allow tactical reactions to feedback: substitutions and formation changes depend on personnel and state, not preassigned scripts alone.
5. Track exposure, causal parent events, and why a scenario became extreme so that rare paths remain interpretable.

**Sampling:** First implement independent, correctly weighted full-match trajectories. For events too rare to estimate efficiently, add a separate importance-sampling mode using an explicit proposal `q(path)` and original model `p(path)`:

    w(path) = p(path) / q(path)
    E_p[f(path)] = E_q[w(path) * f(path)]

Track complete path log-likelihood ratios across **all** changed stochastic decisions and times, including incident transitions. Require support (`q > 0` wherever `p > 0`), finite-weight diagnostics, effective sample size `ESS=(sum w)^2/sum(w^2)`, and uncertainty intervals. **Never report raw oversampled frequencies as probabilities.** For path-conditioned rare-event questions, distinguish `P(result | red card)` from the unconditional `P(result)` and from forced what-if interventions.

## 5. Rust integration: build on existing primitives

Existing relevant code:
- `crates/match/src/second_resolved_api.rs` + `second_resolved_mc.rs`: production-facing second-resolved Monte Carlo, competing event hazards, within-second ordering, path summary outputs.
- `specific.rs`, `matchup.rs`, `lineup.rs`, `psychology.rs`, `second_clock.rs`: canonical player/tactical/mental/clock primitives reusable by an FM-style engine.
- `counterfactual.rs`: intervention semantics; use common random numbers carefully.
- `team_goals.rs` and `apps/plforecast`: independent benchmarks, **not** constraints on the new generative engine.
- `docs/forecast-targets.md`, `docs/historical-same-engine-backtest.md`: forecast targets, frozen validation and provenance contracts.

Proposed isolated, source-neutral module boundary (names are suggestions, not files already present):

    crates/match/src/engine/
      world.rs         # positions, ball, possession, clock, state invariants
      tactics.rs       # roles, formations, collective decisions and adaptation
      decisions.rs     # candidate actions, utility/policy, target choice
      resolution.rs    # action outcomes, contested duels and set pieces
      transitions.rs   # event ordering, state mutation and consequence chains
      incidents.rs     # fouls, cards, injuries, penalties and rare triggers
      paths.rs         # seeded complete-trajectory execution and recording
      rare_events.rs   # weighted tail exploration, path likelihood accounting

Keep simulation mechanics in `crates/match` and input/output orchestration in `apps/matchforecast`. No provider-specific schemas in the hot path, no Python scoring dependency, minimal Rust dependencies, deterministic RNG, typed IDs, reusable buffers where profiling justifies them. Preserve the existing public simulator while an explicit challenger engine is developed. Migration happens after it can generate full self-consistent paths; no premature replacement of published forecasts.

**Minimal candidate interface** (design illustration):

    fn simulate_match(
        prematch: &MatchSetup,
        rules: &EngineRules,
        rng: &mut impl RngCore,
    ) -> MatchTrajectory;

The actual implementation should integrate the repository's existing `Rng64` and typed IDs rather than introduce a second random-number system gratuitously. A `MatchTrajectory` holds the ordered event log and final state. Summaries are derived strictly from complete trajectories. Path-level attributes and uncertainty are drawn once where appropriate; conditional aleatoric decisions occur as the game evolves.

## 6. Output contract

For each trajectory retain: fixture and engine/version hashes; input snapshot/version; seed or reproducible random stream ID; sampled lineup/tactics/latent strengths; chronologically ordered event IDs/timestamps/types; actors, opponents, locations, action intentions and outcomes; causal parent/reason; pre/post state summaries; incident metadata; final score and per-player counts. Use compact event records and optional full replays to control memory.

Aggregate across the same trajectories:
- Joint scoreline, W/D/L, goal totals, player scorer/assister distributions, minutes, shots, passes, duels, fouls, cards, substitutions, shots saved, and other events that the mechanism actually generates.
- Conditional forecasts: `P(next goal | state)`, `P(win | score, time, red)`, time-to-event distributions, tactical-policy alternatives, player absence/attribute interventions.
- Tail measures: `P(>=5 goal margin)`, `P(>=3 goals in 15 min)`, `P(GK injury & goal conceded thereafter)`, extreme-comeback probabilities, and distributions of impact conditional on red cards, with definitions and uncertainty.
- Attribution: report which *modeled* transitions contributed to an extreme path; never claim real-world causal identification from simulator internals alone.

Different simulation paths must not be combined into inconsistent post-hoc box scores or patched with independently generated exact scores. Every published probability retains its source model/engine/version and coverage state.

## 7. Calibration and acceptance

FM24 is the *mechanics reference*, not ground truth for real football frequencies. Validate separately:
1. **Rule fidelity:** lawful FM24 resource decoding and repeatable controlled interventions show the proposed decision/outcome mechanics reproduce the referenced behavior.
2. **Mechanistic validity:** conservation laws and football rules hold path-by-path (11 starters/valid substitutions, possession ownership, legal score transitions, goals imply shots except own goals, coherent event ordering and score accounting).
3. **Real-football calibration:** strictly as-of EPL/MLS samples, chronological walk-forward proper scores for W/D/L, exact-score likelihood, team/player event counts and timing, cards, substitutions, and separately diagnosed tail frequencies. Rare events require much larger samples and interval estimates; absence from a small holdout is not evidence of impossible outcomes.
4. **Simulation precision:** deterministic fixed-seed reruns; Monte Carlo standard errors; path-count convergence; independent-vs-importance-sampled agreement for events that are common enough to cross-check.
5. **Counterfactuals:** compare shared random-stream matched paths while respecting intervention-changed event sequences; do not derive a player's impact by subtracting a constant rating.

Maintain the existing independently evaluated goal-rate, participation and player-event models as **scored reference baselines**; never force their final-score draws into FM-style paths. Report when the new engine is less accurate. Existing release gates remain in force until a new engine/version has its own evidence; simulation realism does not imply predictive calibration.

## 8. Implementation sequence and stopping points

1. **Evidence inventory:** reproducible FM24 resource exports/decoders, verified vs hypothetical mechanism matrix, attributes-to-decision map; no proprietary assets committed.
2. **One complete possession:** field/world state, tactics, move/pass/intercept/carry/shoot/save/restart with reconciled events. Golden deterministic traces, rule invariants and event schema.
3. **Full match:** possession cycles, fatigue, tactics/adaptation, substitutions, fouls, cards, injuries, penalties, stoppage time, stochastic but ordered transitions. The result must emerge exclusively from simulated actions.
4. **Monte Carlo:** full-path IID execution using existing RNG and canonical IDs, trajectory aggregation, coherent joint probabilities, deterministic seeded tests, CPU profiling.
5. **Extreme-path mode:** correlated incident/tactic cascades and optional rigorously weighted importance sampling, causal trace inspection, ESS and tail uncertainty.
6. **Evaluation and rollout:** FM24 behavior replication tests, historical as-of holdouts and baselines, independent FM26 player-attribute mapping, explicit challenger selection only when ready.

**Definition of done for the initial engine:** Given canonical team rosters, attributes, tactics and a seed, it can produce a legally and physically coherent complete match event trace in Rust, with the score and all statistics derived from that trace; thousands of paths generate consistent distributions, and a chosen rare-event path can be replayed and explained. It does not require beating existing predictors to be an experimental engine, but it must not be advertised as accurate without out-of-sample evidence.

## 9. Non-goals and protected boundaries

- No FM24 executable integration, game UI, 3D rendering, animation, or Unity port.
- No release of proprietary FM24 code/resources/attributes in the public repo; only independently authored implementation, mappings/documentation permitted for distribution, and licensed/authorized data.
- No claiming a rule is "FM24's algorithm" without evidence; unknowns are explicit hypotheses.
- No independent direct goal draw or post-hoc score patch in the new engine.
- No discarding existing predictors, past results, frozen baselines or local concurrent work.
- No training on future-sourced player traits, final starting XIs or postmatch observations for a nominal prematch forecast.

**Architecture precedence:** This document defines the **target** FM-style generative engine. Existing `AGENTS.md` and historical forecast documentation describe the **current** engine's operating invariants; both remain applicable to that engine until code is deliberately migrated. Where they prescribe direct goal/shot hazards as implementation details, those mechanisms are **not requirements for the new engine**. Keep the existing implementation frozen/comparable and update its applicable contract explicitly when the challenger is promoted.
