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
        +----------------------------+
        |                            |
player match observations       team match observations + results
        |                            |
rolling per-90 state            rolling per-match state + form
        |                            |
        +-------------+--------------+
                      |
                Monte Carlo
                      |
        +-------------+------------------+
        |             |                  |
player stat dist  team stat dist   match outcome dist
```

The simulation layer deliberately does not learn directly from provider schemas. Providers emit generic external-ID records; the sporting dataset sidecar resolves players against the existing FM-anchored universe and allocates provider-scoped club, competition, metric, and match IDs; `performance` builds causal state; `forecast` samples future rates, exposure, and results using the same deterministic RNG used by valuation.

## Data-source policy

- **Existing Football Manager, Transfermarkt, and FBref inputs stay first-class.** Transfermarkt continues to own market/transfer/contract observations and valuation targets; existing FBref performance observations are merged into the player-match surface rather than replaced.
- **StatsBomb Open Data** is the preferred genuinely open event-level source where its competition coverage exists. Matches and events can produce results, team event totals, player event totals, and xG.
- **football-data.org** is useful for broad fixtures/results/table coverage. The adapter parses cached v4 `/matches` responses; authentication and throttling stay in orchestration.
- **OpenFootball** is useful as a CC0 results fallback. Its team-name identifiers remain provider-scoped rather than being silently merged with clubs from another source.
- **Sofascore** can materially improve current player/team coverage, but the useful API surface is unofficial. It is therefore an optional cached-payload adapter, never an architectural dependency and never called from model scoring.

Remote HTTP collection, retries, credentials, caching, and provider-specific access policy belong outside the source-neutral model crates. Immutable historical payloads should be staged before canonicalization so a model run can always be reproduced.

## Canonical sporting sidecar

The base canonical dataset format is deliberately unchanged so existing valuation artifacts remain readable. `SportingDataset` binds itself to the exact base `dataset_version` and `source_manifest_checksum`, then extends it additively:

- provider player IDs map to the same canonical `PlayerId` through the existing weighted metadata resolver; ambiguous links become explicit issues rather than guessed matches;
- provider clubs and competitions receive deterministic IDs above the base ID ranges but remain provider-scoped, so a matching club name never implies cross-source identity;
- the metric registry starts with all existing FBref metric IDs and only appends new names, preserving old feature semantics;
- match IDs are deterministic hashes namespaced by provider;
- provider provenance is explicit and checked against every source record.

`SportingDataset::player_matches` combines existing match-level FBref observations with new provider observations. Rows with unknown minutes remain available in the sidecar but are excluded from per-90 materialization instead of being coerced to zero. `SportingDataset::team_matches` joins team metrics to canonical results so team form and team statistics use the same match identity.

## Sporting Monte Carlo

### Player statistics

For each player/metric, the forecast separates:

1. the estimated latent per-90 production rate;
2. match-to-match production variance;
3. expected minutes/exposure;
4. uncertainty in minutes;
5. epistemic uncertainty in the true rate.

A path samples the latent rate, realized rate, and exposure independently, then produces either a non-negative continuous total or a count total. Outputs are distributions (`p10/p25/p50/p75/p90`, mean, standard error), not point estimates.

### Team statistics

Team forecasts use the same structure at per-match resolution. Rolling team state stores totals, per-match rates, exponentially weighted recent form, recent-vs-prior trend, dispersion, W/D/L, points, and goals for/against.

### Match results

The result primitive accepts context-adjusted home and away goal intensities supplied by the team-strength layer. Each Monte Carlo path samples uncertainty in those intensities and then conditional goal counts. The output is a distribution over home/draw/away outcomes, home and away goals, and expected points.

The goal-rate producer is intentionally separate from the simulator. It can start with robust team attack/defence strength plus home advantage, then consume availability, lineups, player-state simulations, opponent interaction, competition translation, and tactical compatibility later without replacing the Monte Carlo engine.

## Extension rule

New providers should stop at generic source records and enter through the sporting sidecar. Do not add provider-specific schemas to `performance` or `forecast`, and do not rewrite the base canonical artifact merely to add a data source. Cross-provider club identity should become a separate verified graph if the product needs it; provider-name similarity is not sufficient evidence.
