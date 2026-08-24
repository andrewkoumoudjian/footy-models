# Player, team, and specific-match simulation

This extension applies the valuation stack's existing as-of, uncertainty, provenance, and deterministic Monte Carlo principles to football performance and match outcomes.

## Governing graph

The preferred specific-match path is bottom-up. The match result is not an independent model sitting beside the player models.

```text
historical source payloads
  FM / Transfermarkt / FBref / StatsBomb / results APIs / optional Sofascore
                         |
                         v
              canonical sporting state
                         |
       +-----------------+------------------+
       |                                    |
player performance history             lineup / formation history
raw FM attributes                      team fixture history
role / ability state                   availability / fatigue inputs
psychology / morale / form             team mental state
       |                                    |
       +-----------------+------------------+
                         |
                         v
       Monte Carlo path-specific kickoff state
                         |
          stochastic pre-match selection
 availability -> formation -> XI -> slot/role -> starter-minute target
                         |
                         v
                 active player graph
 coarse matchup + raw AttributeId matchup + teammate synergy
                         |
                         v
              segment player events
 role + fatigue + system + environment + score + mental state
                         |
                         v
            causal team-process DAG
 possession/progression/... -> chance creation -> shots/quality/etc.
                         |
                         v
                segment scoring rate
                         |
                         v
              goal / red / injury / sub
                         |
                         v
      recursive player/team mental transition
                         |
                rebuild active graph
                         |
                        ...
                         |
                         v
            scoreline / W-D-L / xPts
```

`forecast` remains the generic distribution/Monte Carlo library. `match` owns fixture-specific composition. `source-*` owns provider payload shapes. No provider HTTP or JSON shape leaks into model scoring.

## A match is `n` complete Monte Carlo paths

All fixture probabilities and distributions are generated from complete match histories. If the caller requests `n` paths, the engine executes exactly `n` independent paths.

A path includes its own sampled kickoff latent mental state, formation/XI/roles/minutes, player↔player matchups, segment player events, team process channels, goals/cards/injuries/substitutions, recursive mental-state transitions, and final score.

No sampled lineup, incident, score state, or psychological state is shared between paths. Fixed seed + identical inputs + identical path count is deterministic.

## Source policy

- Existing Football Manager, Transfermarkt, and FBref inputs remain first-class.
- Transfermarkt remains a market/contract/transfer source rather than a scoring dependency.
- Existing FBref match-level performance is merged into the source-neutral player-performance surface.
- StatsBomb Open Data supplies open match/event/lineup history where covered. `parse_statsbomb_lineups` preserves starter status, position intervals, and minutes rather than collapsing a player to one position label.
- football-data.org and OpenFootball supply broad fixture/result coverage. OpenFootball accepts both common `matches[]` and `rounds[].matches[]` shapes.
- Sofascore can improve current lineup/player/team coverage, but the useful endpoint surface is unofficial. Its adapter is optional and cached; lineup records preserve the provider's `confirmed` flag so a projected lineup is never promoted to known truth.

Remote collection, credentials, retries, rate limits, and cache policy live outside model crates.

## Canonical sporting sidecars

The original canonical valuation artifact is intentionally not rewritten.

`SportingDataset` is additive and binds to the exact base `dataset_version` and `source_manifest_checksum`. It provides provider provenance, canonical player links, provider-scoped club/competition IDs, deterministic provider-namespaced match IDs, player/team stats, results, and a metric registry that preserves existing FBref IDs.

`SportingLineupDataset` is a second additive sidecar for lineup history. It canonicalizes player/club/competition/match identity but deliberately retains provider position tokens and formation tokens. Provider position taxonomies are not assumed to be semantically identical; position-token -> canonical `RoleId` mapping is a versioned modeling decision upstream of match simulation.

Missing or unresolved lineup players become explicit issues rather than guessed identities.

## Historical and inter-match state

Player windows provide total/per-90 production, exponentially weighted recent production, recent-vs-prior trend, match variance, coverage, peer comparison, and competition translation. Team windows provide per-match EWMA/trend/variance plus W/D/L, points, goals for, and goals against.

Selection uses its own primitive. `build_selection_window` is calibrated against **team fixture dates**, not only rows where the player appeared. A club fixture with no player appearance is evidence of non-selection without becoming a fake zero-performance observation. Start/appearance/minute estimates are exponentially weighted and shrunk to explicit priors. Formation history is calibrated similarly.

Form is separate again: it is a causal residual of historical realized player performance versus the expectation that existed before each historical match. It is an inter-match state and is held fixed during one simulated fixture. In-match success/error modifies transient self-belief or morale instead; the realized match can update form for the next fixture.

## Stochastic XI projection

`LineupProjectionInput` contains candidate squad, causal start probability, availability, latent within-squad quality, fatigue, role fits, expected starter minutes/uncertainty, substitute probability, formation priors, and optional confirmed starters/slots.

Every path samples formation, availability, eleven unique starters, roles/slots for unknown assignments, and starter-minute targets.

Known information only collapses uncertainty it resolves:

- no lineup known -> full XI stochastic;
- partial lineup known -> remaining places stochastic;
- player confirmed but position unknown -> start fixed, role stochastic;
- player and slot confirmed -> both fixed;
- full XI/slots confirmed -> lineup uncertainty collapses.

Mental-state effects can adjust start/substitute probabilities in logit space before the path's XI is sampled. Exact probability boundaries remain exact: impossible remains impossible and certain remains certain.

## Player state and exact matchup primitives

`PlayerMatchupState` carries source-neutral dimensions such as technical, ball progression, creativity, finishing, defending, aerial, physical, decision making, mentality, and goalkeeping. `PlayerAttributeState` carries raw canonical FM `AttributeId` values for fine mechanisms such as pace-vs-recovery, dribbling-vs-tackling, strength-vs-strength, or finishing-vs-goalkeeping.

Rules are metric-specific. The repository does not hard-code which numeric `AttributeId` means one football concept; that mapping belongs to the versioned FM schema.

Spatial relevance is determined from sampled tactical slots, so direct opponents emerge from the sampled formation rather than hard-coded position pairs. Teammate synergy is separate from opponent matchup.

## Player event process

A player/metric/segment rate is conditioned on independent primitives:

```text
historical latent rate
  x own raw-attribute effect
  x aggregate opponent matchup
  x raw-attribute opponent matchup
  x teammate support
  x exact sampled role fit
  x fatigue
  x own tactical/system modifier
  x opponent suppression modifier
  x match environment modifier
  x current score-state modifier
  x current path mental-state modifier
```

Mental effects can alter mean and variance separately. Missing psychological information creates latent path uncertainty rather than a zero observation. Count metrics are sampled as counts; non-negative continuous metrics remain continuous.

## Player metrics do not directly equal goals

`CausalChannelGraph` is the result bridge. Metrics load into football-process channels, channels can cause downstream channels, and terminal channels alter attacking/defensive scoring intensity.

A metric with no channel loading can still be forecast but cannot accidentally move W/D/L. For example:

```text
X progressive carries
    -> Progression
    -> ChanceCreation
    -> ShotQuality
    -> team scoring intensity
```

## Recursive game and mental state

`simulate_dynamic_mental_match` is the preferred engine when recursive psychology is enabled.

Each path:

1. samples latent player/team kickoff mental state;
2. adjusts path-specific selection probabilities and samples both starting XIs;
3. builds the active player/matchup graph;
4. samples player events for the current segment from current football + mental state;
5. evaluates the causal team-process graph;
6. samples segment goals;
7. samples red-card and injury hazards and applies tactical/forced substitutions;
8. updates player/team transient mental state from goals, personal residual performance, own/teammate incidents, substitutions, score pressure, leadership/cohesion, innovation, and mean reversion;
9. rebuilds active social/matchup/event state for the next segment.

The recursively evolving player dimensions are primarily morale, self-belief, motivation, and role satisfaction. Team collective morale, belief, cohesion, and active leadership evolve separately. Stable traits and inter-match form are not rewritten by one segment.

Pressure resilience governs response to high-pressure/trailing states; active leadership/cohesion buffer negative shocks; professionalism strengthens reversion toward the path's kickoff equilibrium; consistency dampens psychological innovation variance; discipline/current morale/hostility can affect card risk.

Substitute selection uses only the mental-state **change since kickoff** because the kickoff effect has already entered the path's projected lineup. This prevents double counting.

## Outputs and explanation

The forecast surface includes W/D/L, score distributions, expected points, formation/XI/start probabilities, appearance/minute distributions, player-stat distributions, ordinary player influence factors, player/team causal-channel contributions, incident probabilities, mental-state trajectories, and transition-driver decomposition.

The model can therefore explain a path or an aggregate effect as:

```text
Team A concedes
-> collective belief falls
-> X pressure resilience absorbs part of the shock
-> X self-belief decreases
-> X next-segment progressive-carry distribution shifts
-> Team A progression / chance creation weaken
-> subsequent scoring intensity changes
```

Aggregate explanations are expectations across all `n` paths rather than claims that one representative path literally occurred. Pairwise explainers additionally expose the probability X and Y occupy interacting slots and the exact raw-attribute relationships contributing to the matchup effect.

## Counterfactuals

Player-absence interventions remove a player upstream, reproject the XI, and rerun downstream roles, interactions, player events, channels, and results using common random numbers where supported. Recursive mental-state counterfactuals should follow the same rule: interventions occur upstream and the full path is resimulated rather than subtracting a static player rating.

## Invariants

- no remote fetch during model scoring;
- strict as-of historical state;
- missing != zero;
- unresolved identities are explicit;
- no deterministic guessed XI when uncertainty exists;
- exact selection probability boundaries remain exact;
- stable psychology and inter-match form are distinct from transient in-match state;
- mental shocks are squad-scoped and path-local;
- kickoff selection effects are not double counted during substitutions;
- metrics reach W/D/L only through the causal football-process graph;
- a fixture runs exactly the requested `n` independent complete Monte Carlo paths;
- fixed seed + same artifacts + same `n` reproduces the output.
