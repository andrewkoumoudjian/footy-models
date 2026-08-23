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
       |                                    |
       +-----------------+------------------+
                         |
                         v
          stochastic pre-match selection
 availability -> formation -> XI -> slot/role -> starter-minute target
                         |
                         v
                 active player graph
 coarse matchup + raw AttributeId matchup + teammate synergy
                         |
                         v
              segment player events
 role + fatigue + system + environment + current score state
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
                rebuild active graph
                         |
                        ...
                         |
                         v
            scoreline / W-D-L / xPts
```

`forecast` remains the generic distribution/Monte Carlo library. `match` owns fixture-specific composition. `source-*` owns provider payload shapes. No provider HTTP or JSON shape leaks into model scoring.

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

## Causal historical state

Player windows already provide:

- total and per-90 production;
- exponentially weighted recent per-90 production;
- recent-vs-prior trend;
- match-level variance;
- coverage / low-sample state;
- robust peer comparison;
- causal competition translation.

Team windows provide per-match EWMA/trend/variance plus W/D/L, points, goals for, and goals against.

Selection uses its own primitive. `build_selection_window` is calibrated against **team fixture dates**, not only rows where the player appeared. A club fixture with no player appearance is therefore evidence of non-selection without becoming a fake zero-performance observation. Start/appearance/minute estimates are exponentially weighted and shrunk to explicit priors.

Formation history is calibrated the same way with `calibrate_formation_priors`.

## Stochastic XI projection

`LineupProjectionInput` contains:

- candidate squad;
- causal start probability;
- availability probability;
- latent within-squad quality;
- fatigue;
- role fits;
- expected starter minutes and uncertainty;
- substitute probability;
- formation priors;
- optional confirmed starters/slots.

Every path samples formation, availability, eleven unique starters, roles/slots for unknown assignments, and starter-minute targets. Scarce positions are assigned before flexible positions.

Known information only collapses the uncertainty it actually resolves:

- no lineup known -> full XI is stochastic;
- partial lineup known -> the remaining places are stochastic;
- player confirmed but position unknown -> start is fixed, role remains stochastic;
- player and slot confirmed -> both are fixed;
- full XI/slots confirmed -> lineup uncertainty collapses.

`project_lineup` exposes marginal start probabilities, expected minutes, role distributions, and formation probabilities from the same sampling primitive used inside the match simulation.

## Player state and exact matchup primitives

There are two complementary player-state surfaces.

### Coarse state

`PlayerMatchupState` carries the source-neutral ability vector: technical, ball progression, creativity, finishing, defending, aerial, physical, decision making, mentality, and goalkeeping. Metric-specific `MatchupRule`s can compare different dimensions.

### Fine FM attributes

`PlayerAttributeState` carries raw canonical `AttributeId` values. `AttributeMatchupRule` can express exact mechanisms such as:

```text
pace          vs recovery pace
acceleration  vs agility
 dribbling    vs tackling
crossing      vs positioning / blocking
strength      vs strength
jumping/aerial vs aerial defending
finishing     vs goalkeeper/defensive attributes
```

Rules are metric-specific. The repository does not hard-code which numeric `AttributeId` means pace, tackling, etc.; that mapping comes from the versioned FM schema already used by ingestion.

`AttributeEventLoading` also allows an own attribute to modify an event process without pretending a specific defender is its counterpart, e.g. finishing affecting shot conversion.

Spatial relevance is determined from the sampled tactical slots. Opponent coordinates are mirrored into the attacking frame, so the direct-opponent graph emerges from the formation rather than a hard-coded "LW always plays RB" assumption.

Teammate synergy is a separate primitive. Co-minutes, pass-network links, progressive-pass/carry relationships, overlap/underlap effects, or learned pair interactions can therefore enter without being conflated with opponent matchup.

## Player event process

`SpecificPlayerEventModel` is source-neutral and can represent any canonical metric: touches, passes, progressive passes, key passes, crosses, carries, progressive carries, dribbles, shots, shots on target, xG, xA, box touches, pressures, tackles, interceptions, blocks, clearances, duels, aerials, fouls, turnovers, recoveries, goals, assists, cards, or future metrics.

For a player/metric/segment, the rate is conditioned on independent primitives:

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
```

Count metrics are sampled as counts; continuous non-negative metrics remain continuous. Missing required player-event models are not interpreted as zero: the preferred simulator validates that every selectable player has a calibrated event-model surface, which should be supplied from observed state plus explicit priors when samples are sparse.

## Player metrics do not directly equal goals

`CausalChannelGraph` is the result bridge. It is deliberately an acyclic graph:

```text
player metric deviation
        |
        v
metric -> process-channel loading
        |
        v
possession / retention / progression / width / central access /
transition attack / chance creation / box entries / shot volume /
shot quality / finishing / pressing / counterpress / transition defence /
defensive suppression / aerial control / set pieces / goalkeeping
        |
        v
channel -> channel causal edges
        |
        v
terminal channel -> attack/defence scoring log-intensity
```

A metric with no channel loading can still be forecast and explained, but it cannot accidentally move W/D/L. The graph rejects cycles so causal order is explicit.

Example:

```text
X progressive carries
    -> Progression
    -> ChanceCreation
    -> ShotQuality
    -> team scoring intensity
```

This is preferred over assigning a direct arbitrary "progressive carries -> goal rate" coefficient.

## Dynamic in-match state

`simulate_primitive_match` is the preferred fixture engine. Each Monte Carlo path:

1. samples the two pre-match lineups;
2. builds the active player graphs;
3. recomputes matchups for the current active players;
4. samples player events for the current time segment;
5. evaluates the causal team-channel graph;
6. samples segment goals;
7. samples red-card and injury hazards;
8. applies tactical substitutions from sampled starter-minute targets;
9. chooses replacements by substitute probability, availability, role fit, and fatigue;
10. rebuilds the active matchup/event graph for the next segment.

Score state is not a pre-match feature. It is endogenous. Per-metric trailing/leading modifiers change subsequent player behavior, so a 1-0 state can change progression, pressing, shot volume, etc. before the next scoring intensity is produced.

Red cards remove a player without replacement. Injuries can force a substitution subject to remaining substitutions/available replacements. Tactical substitutions preserve the outgoing tactical slot and re-evaluate replacement role fit; future work can add explicit score-triggered formation-template changes without changing the selection/matchup/event/channel boundaries.

## Outputs and explanation

`PrimitiveMatchForecast` contains:

- W/D/L probabilities, score distributions, and expected points;
- projected formation/XI/start probabilities;
- player appearance probabilities and expected minutes;
- unconditional player-stat distributions;
- conditional-on-appearance player-stat distributions;
- separate influence summaries for own attributes, aggregate matchup, raw-attribute matchup, teammate support, role fit, fatigue, system/environment, and score state;
- player channel contributions;
- team channel contributions;
- red-card and injury-exit probabilities.

This lets an explanation distinguish, for example:

```text
X's dribble rate moved because:
  + raw pace-vs-recovery matchup
  + teammate overlap support
  - role-fit penalty
  - opponent low-block suppression

Those dribbles/progressive carries moved:
  Progression -> ChanceCreation -> ShotQuality

which changed Team A's scoring intensity and therefore W/D/L.
```

`project_metric_matchups` separately reports the probability that X and Y actually occupy interacting starting slots across projected-XI paths, so an opponent is not described as "the matchup" when that pairing only exists on a minority of lineup scenarios.

## Counterfactuals

`simulate_primitive_player_absence` uses common random numbers. It removes a player upstream, reprojects the starting XI, and reruns substitutions, roles, direct matchups, player events, team channels, and the result.

Therefore:

```text
with X     -> projected XI A -> matchup graph A -> event graph A -> W/D/L A
without X  -> projected XI B -> matchup graph B -> event graph B -> W/D/L B
```

The delta includes replacement quality, role reshuffling, teammate effects, changed opponent matchups, player-event changes, and result changes. It is not a static player-rating subtraction.

## Extension rule

New providers stop at source-neutral source records and canonical sidecars. Do not add provider-specific schemas to `performance`, `forecast`, or `match`. New football mechanisms should normally become small primitives: a new state observation, a new rate modifier, a new matchup rule, a new process channel/edge, or a new game-state transition.
