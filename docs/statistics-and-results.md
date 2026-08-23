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
canonical player / source-scoped club / competition / match identity
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

The simulation layer deliberately does not learn directly from provider schemas. Providers emit generic external-ID records; canonicalization resolves identity; `performance` builds causal state; `forecast` samples future rates/exposure/results using the same deterministic RNG used by valuation.

## Data-source policy

- **Existing Football Manager, Transfermarkt, and FBref inputs stay first-class.** Transfermarkt continues to own market/transfer/contract observations; FBref continues to supply permitted/local performance imports.
- **StatsBomb Open Data** is the preferred genuinely open event-level source where its competition coverage exists. Matches and events can produce results, team event totals, player event totals, and xG.
- **football-data.org** is useful for broad fixtures/results/table coverage. The adapter parses cached v4 `/matches` responses; authentication and throttling stay in orchestration.
- **OpenFootball** is useful as a CC0 results fallback. Its team-name identifiers remain source-scoped rather than being silently merged with clubs from another provider.
- **Sofascore** can materially improve current player/team coverage, but the useful API surface is unofficial. It is therefore an optional cached-payload adapter, never an architectural dependency and never called from model scoring.

Remote HTTP collection, retries, credentials, caching, and provider-specific access policy belong outside the source-neutral model crates. Immutable historical payloads should be staged before canonicalization so a model run can always be reproduced.

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

The first result primitive accepts context-adjusted home and away goal intensities supplied by the team-strength layer. Each Monte Carlo path samples uncertainty in those intensities and then conditional goal counts. The result is a calibrated surface for home/draw/away probability, goal distributions, and expected points.

The goal-rate producer is intentionally separate from the simulator. It can start with robust team attack/defence strength plus home advantage, then later consume availability, lineups, player-state simulations, opponent interaction, competition translation, and tactical compatibility without replacing the Monte Carlo engine.

## Next integration boundary

The provider adapters now emit generic external-ID records and the source-neutral performance/forecast primitives are implemented. Canonical dataset ingestion should map these provider records through the existing FM-anchored player identity graph and source-scoped club/competition mappings before persistence. Do not invent cross-provider club equivalence merely because names look similar; explicit club identity links should be added as their own verified graph.
