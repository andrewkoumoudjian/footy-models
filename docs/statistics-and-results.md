# Player, team, and match statistics

This extension applies the repository's existing causal/as-of and Monte Carlo methodology to sporting outputs as well as player prices.

## Model graph

```text
FM / Transfermarkt / FBref
StatsBomb Open Data / football-data.org / OpenFootball / optional Sofascore
        |
        v
provider adapters (`source-*`)
        |
        v
FM-anchored player links + provider-scoped club/competition/match IDs
        |
        +-----------------------------+
        |                             |
player match observations        team match observations + results
        |                             |
rolling per-90 state             rolling per-match state + form
        |                             |
        +-------------+---------------+
                      |
             generic `forecast`
          rate/exposure/result primitives
                      |
                      v
              specific `match` graph

availability + selection history + formation history
                      |
                      v
       stochastic XI / formation / role assignment
                      |
          +-----------+-----------+
          |                       |
   latent player state      exact sampled opponent XI
          |                       |
          +---- metric-specific player↔player edges
          |         + teammate synergy
          |         + tactical/system/environment factors
          v
        player event-rate distributions
          |
          v
        realized player events
          |
          v
        team attack / defence state
          |
          v
        segmented score-state process
          |
          v
       scorelines / W-D-L / expected points
```

`forecast` remains the generic deterministic Monte Carlo library. `match` is the higher-level causal graph for one fixture. It consumes canonical/source-neutral primitives and uses the same `Rng64`; it does not know provider JSON shapes or perform remote collection.

## Data-source policy

- **Existing Football Manager, Transfermarkt, and FBref inputs stay first-class.** Transfermarkt continues to own market/transfer/contract observations and valuation targets; existing FBref performance observations are merged into the player-match surface rather than replaced.
- **StatsBomb Open Data** is the preferred genuinely open event-level source where its competition coverage exists. Matches and events can produce results, team event totals, player event totals, and xG. Its lineup files can also supply historical starters/positions to upstream selection calibration.
- **football-data.org** is useful for broad fixtures/results/table coverage. The adapter parses cached v4 `/matches` responses; authentication and throttling stay in orchestration.
- **OpenFootball** is useful as a CC0 results fallback. Its team-name identifiers remain provider-scoped rather than being silently merged with clubs from another source.
- **Sofascore** can materially improve current player/team/lineup coverage, but the useful API surface is unofficial. It is therefore an optional cached-payload adapter, never an architectural dependency and never called from model scoring.

Remote HTTP collection, retries, credentials, caching, and provider-specific access policy belong outside the source-neutral model crates. Immutable historical payloads should be staged before canonicalization so a model run can always be reproduced.

## Canonical sporting sidecar

The base canonical dataset format is deliberately unchanged so existing valuation artifacts remain readable. `SportingDataset` binds itself to the exact base `dataset_version` and `source_manifest_checksum`, then extends it additively:

- provider player IDs map to the same canonical `PlayerId` through the existing weighted metadata resolver; ambiguous links become explicit issues rather than guessed matches;
- provider clubs and competitions receive deterministic IDs above the base ID ranges but remain provider-scoped, so a matching club name never implies cross-source identity;
- the metric registry starts with all existing FBref metric IDs and only appends new names, preserving old feature semantics;
- match IDs are deterministic hashes namespaced by provider;
- provider provenance is explicit and checked against every source record.

`SportingDataset::player_matches` combines existing match-level FBref observations with new provider observations. Rows with unknown minutes remain available in the sidecar but are excluded from per-90 materialization instead of being coerced to zero. `SportingDataset::team_matches` joins team metrics to canonical results so team form and team statistics use the same match identity.

## Generic sporting Monte Carlo

### Player statistics

For each player/metric, the generic forecast separates:

1. the estimated latent per-90 production rate;
2. match-to-match production variance;
3. expected minutes/exposure;
4. uncertainty in minutes;
5. epistemic uncertainty in the true rate.

A path samples the latent rate, realized rate, and exposure independently, then produces either a non-negative continuous total or a count total. Outputs are distributions (`p10/p25/p50/p75/p90`, mean, standard error), not point estimates.

### Team statistics

Team forecasts use the same structure at per-match resolution. Rolling team state stores totals, per-match rates, exponentially weighted recent form, recent-vs-prior trend, dispersion, W/D/L, points, and goals for/against.

## Specific-match graph

### 1. Selection is stochastic when the XI is unknown

`LineupProjectionInput` accepts formation priors, a squad of candidates, optional confirmed starters, and explicit selection parameters. Every Monte Carlo path samples:

- formation;
- player availability;
- eleven unique starters;
- exact tactical slot/role for starters whose role is not confirmed;
- starter minutes.

A partial confirmed lineup collapses only the confirmed dimensions. If all eleven players and slots are known, start/role uncertainty collapses completely. If no XI is known, the complete XI is a path variable.

`build_selection_window` calibrates start/appearance probabilities against **team fixture dates**, not only a player's appearances. A fixture with no appearance row therefore counts as evidence of non-selection without manufacturing a player-stat zero. It uses causal exponential decay plus an explicit empirical-Bayes prior. `calibrate_formation_priors` applies the same as-of/decay discipline to formation history.

`lineup_candidate_from_state` combines this selection state with the canonical player's role vector and latent ability. Availability and fatigue remain separate inputs so an injury probability does not get confused with tactical selection probability.

### 2. Exact X↔Y interactions

The match graph uses normalized pitch coordinates from the sampled tactical slots. Opponent coordinates are mirrored into the attacking team's frame, producing an explicit spatial graph for all relevant player pairs.

`MatchupRule` is metric-specific and compares source-neutral ability dimensions. For example, a dribble model can compare ball progression/technical ability against defending/physical ability, while an aerial model can use aerial/physical dimensions. There is no hard-coded winger-vs-full-back rule: the sampled formation and spatial overlap determine which opponents are relevant.

`PairwiseSynergy` separately represents teammate relationships such as co-minutes/pass-network/overlap effects. The player metric process receives two distinct terms: opponent advantage and teammate support.

`project_metric_matchups` repeatedly samples both XIs and reports each attacker/defender pair's interaction probability, mean spatial weight, mean ability advantage, and unconditional expected weighted advantage. Explanations can therefore say that Y is a major driver for X only on the paths where X and Y actually occupy interacting roles.

### 3. Player event processes

`PlayerMetricImpactModel` starts from the empirically calibrated per-90 distribution and applies separate multiplicative/log effects for:

- direct opponent matchup;
- teammate support;
- role fit;
- fatigue;
- own tactical/system production;
- opponent tactical suppression;
- match environment.

The model then samples the realized player metric on that path. Count metrics are sampled as counts; continuous non-negative metrics retain continuous totals. Each metric also has an explicit peer/league reference and calibrated attack/defence log-weight so the realized performance can affect team state without silently double-counting the team prior.

`PlayerTeamImpactProfile` is the sparse-data backstop from the latent player state. `latent_team_impact_from_state` turns the FM-derived `AbilityVector` into attack/defence strength through versioned caller-supplied weights rather than hard-coded football assumptions. `fatigue_from_load` is a transparent load/rest primitive that can be replaced by a learned calibration later.

### 4. Player state produces the result

For each path the simulator performs, in order:

```text
sample home XI
sample away XI
    -> build metric-specific matchup graph
    -> sample every modeled starter metric
    -> aggregate latent + realized player contributions
    -> construct home/away attack and defence log state
    -> apply team/environment residual context
    -> construct scoring intensities
    -> simulate score in equal time segments with score-state feedback
    -> record score/W-D-L/expected points
```

There is no second independent result model that bypasses player performance. The generic team-form goal model remains useful as a baseline/calibration source, but the specific-match graph is the final path when lineup/player context is available.

The output includes unconditional player-stat distributions (including the probability of not starting), conditional-on-start distributions, start probabilities, minutes, and separate influence summaries for attack/defence contribution, matchup, teammate support, role fit, fatigue, and system/environment effects.

### 5. Counterfactuals

`simulate_player_absence` forces one player unavailable, removes any confirmed-starter lock, then re-runs the **entire** graph with the same seed. The replacement XI, roles, teammate links, direct matchups, player events, team state, and result are all allowed to change. This supports questions such as "what does X add to this exact fixture?" without subtracting a static player rating from the original result.

## Remaining game-state extension boundary

The score process is already segmented and score-dependent. Red cards, injuries, tactical shifts, and substitutions should enter as additional state transitions inside those paths, not as pre-match features. The current selection object retains substitution probability for that extension; it is not yet used to create substitute minutes, so current player-stat/result simulation is starter-centric after the stochastic XI is chosen.

## Extension rule

New providers should stop at generic source records and enter through the sporting sidecar. Do not add provider-specific schemas to `performance`, `forecast`, or `match`, and do not rewrite the base canonical artifact merely to add a data source. Cross-provider club identity should become a separate verified graph if the product needs it; provider-name similarity is not sufficient evidence.
