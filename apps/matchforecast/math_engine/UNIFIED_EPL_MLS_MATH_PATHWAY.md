# Single mathematical football model — EPL + MLS source and primitive contract

**Status: implementation research, not calibrated/publishable.** This work extends the isolated mathematical forecasting path. It does **not** replace the active Rust match engine, alter pre-existing predictions, or claim every officially registered player can be identified in the FM26 export.

## Objective and unified probabilistic pathway

The required forecast is **one conditional joint distribution**, not independent player-specific classifiers:

    (registered players, available data, team context, competition)
                  |
                  v
      one frozen source-neutral feature contract
        |          |             |          |
      FM26       SofaScore       FBref     earlier team lineups
    mental,      recent and      minutes,  prior chemistry /
    technical,   historical      xG/stats  pair co-starts
    physical     ratings, etc.                |
        |          |             |          |
        +----------+-------------+----------+
                  |
          team mental summaries / lineup-dependent social fit
                  |
          pooled regularized GLM, no player-ID terms
             p(start), shot hazard, conversion
                  |
          exact-cardinality stochastic XI with observed eligibility
                  |
          one Rust marked event simulation / substitutions / psychology
                  |
          coherent goals, shots, scorer occurrence, scoreline, H/D/A

In one shared regression artifact, the player-agnostic conditional law is

- P(start | X) = logistic(X · b_start).
- N_shots | minutes, X ~ Poisson(minutes / 90 × exp(X · b_shot)).
- N_goals | N_shots, X ~ Binomial(N_shots, logistic(X · b_convert)).

The existing Rust mathematical engine conditions the starting XI to exactly one goalkeeper and ten outfield players, simulates completed substitutions, and conserves player/team shots and goals **within the same paths**. **Do not independently multiply start, goal and outcome forecasts together or overwrite path-consistent match probabilities.** The three conditional heads are components of **one coherent statistical model and one shared feature standardizer**, not per-person models. Regularization must be selected on predeclared validation data with later genuinely untouched matches for the final test. The current code is a scientific challenger, not a production predictor.

## Primitive reuse: build on existing code, don't reinvent it

| Existing primitive | Source | Role in the mathematical pathway | Causal boundary |
|---|---|---|---|
| `PlayerState`, `AbilityVector`, `DevelopmentVector`, `RoleVector` | `footy-state` | Player ability and role representation; age, CA/PA realization; long-horizon development as related analysis | Only dated state snapshots, never future progression |
| `PsychologyVector`, `PsychologyMeasure` | `footy-state` | Stable professional/ambition/consistency/pressure/adaptability/leadership/teamwork/discipline with confidence; supplement FM mental dimensions | FM-derived estimates have lower confidence; morale is a latent state, not directly observed from a score |
| `tactical_fit`, `squad_complementarity`, `EnvironmentFit` | `footy-recruitment` | League transition, role/formation fit, language/culture if actually known, environment, available role coverage | Must be computed from pre-match, already-measured inputs; no speculative biographical imputations |
| `historical_analogues` | `footy-recruitment` | Diagnostic nearest analogues for interpretation and cold starts; uncertainty/prior discovery | No individual player model, no future return label from historical analogues |
| `CandidateAnalysis` and `surge::SURGE_FEATURE_NAMES` | `footy-recruitment` | Observe cross-market context and explicit 47-feature value/development schema in companion valuation task | **Do not leak 12/24/36-month future value predictions, realized future transfer fee, or breakout labels into same-date match outcomes** |
| `MarketPerceivedValue`, `PriceDistribution`, `MarketIndex`, `TransferHazard` | `footy-market` | Latest *published* value, dated valuation confidence, transfer exposure; auxiliary valuation projections | Future transfer-market targets are separate forecast **outputs**, not observed match inputs; publication timestamps mandatory |
| `NextMatchPlayerRateModel`, `PlayerRateFeatures` | `footy-forecast` | Baseline shrinkage benchmark and residual rate uncertainty for per-90 metrics | Compare against pooled GLM; no fitted rates from label match |
| `Rng64`, `EwmaVolatility`, `Garch11`, `Mf2Garch` | `footy-forecast` | Seeded simulation and match-to-match heteroskedasticity; MF2 only for genuinely lower-frequency market series | Never invent high-frequency latent precision; train risk/state from earlier residuals |
| `FeatureTransform`, `RidgeOls`, `fit_logistic_irls`, `fit_huber`, `ConformalRegressor` | `footy-models` | Common missingness/standardization logic, regularization, robust calibration and prediction intervals | Fit transform exclusively on training history; certify predictive claims only on independent held-out games |
| `build_team_window`, `TeamPerformanceWindow`, `team_metric_quantile` | `footy-performance` | SofaScore & observed team match rolling means, variances, trends and quantiles | Source event dates AND publisher availability precede fixture |
| `footy-match` exact-cardinality dynamic programming, substitution and second-resolved psychological engine | `footy-match` | Reuse coherent game paths, pair interactions and within-match changing mental states | Must not mistake hypothetical psychology for measured attitudes, or Monte Carlo precision for OOS skill |
| Football entity registry `Reep` + verified Sortitoutsi UID links | `data/identity/reep` and local FM26 data | Typed provider-ID crosswalk for SofaScore/FBref/Transfermarkt and FM26 UIDs, preserving redirects and collision detection | Reep bridge does not itself prove FM mental traits were measured or club membership is current |
| Quant-fund statistical primitives (HMM/GARCH, tree inference) | `docs/QUANT_FUND_PRIMITIVES_REUSE.md` | Optional low-complexity regime/uncertainty comparisons **only after a simple conditional model passes proper-score gates** | Do not train one HMM per player or treat unseen future regime labels as features |

The model can **consume outputs of the pre-match primitives**, but must not blindly consume future outcomes of recruitment/value models, since that would violate the causal forecasting contract. Psychological fit is an input *only when supported by actual priors*, and economic value is a separate estimation target sharing player representation.

## Data organization: one player person, disjoint provider-ID namespaces

Source registries: FM26 `uid`, Sortitoutsi UID, Reep canonical v1, Transfermarkt, FBref person ID, SofaScore player ID, ESPN, Understat. Reep merges/resolves cross-provider ids; the FM attributes remain keyed to the original FM26 UID. Treat Reep redirects and collisions as first-class identity events, never as arbitrary string matches.

For **EPL and MLS**, each fixture input contains the full **known as-of eligible registered-player universe** plus missingness masks; a missing FM or SofaScore observation does not drop the player. Distinguish official registered candidates from confirmed matchday eligibility and from actual starts. An official EPL name list without provider IDs cannot be promoted to a verified Reep↔FM edge solely because someone has a plausible name.

Known 2026-10-10 eight-club EPL cohort: 600 official registrations; an earlier FM name-reconciler found 323, a newer roster produced 391 raw FM UID assignments. That does not mean 391 confirmed player attribute records. The supplied 2026 FM raw export has 16 duplicated UIDs (some staff records); these are quarantined until independently disambiguated. The March 2026 FM26 database need not contain **all** later registered EPL youths and transfers. A true 600/600 verified mapping must be **demonstrated**, not demanded of a database without source evidence.

### Source families

- **SofaScore:** player and team recent/historical *ratings*, observed event metrics, xG/xA when published, role and actual availability. A team rating is not a latent moral/psychological state. Use time-stamped match logs with provider ID.
- **FBref:** dated per-90 playing time, shooting, passing, progression, defending and exposure. Only contemporaneously available seasons/matches. Use source IDs and Reep bridges, not guesses.
- **FM26:** full technical, physical, mental and hidden traits (when genuinely present in the released source); CA/PA, positions and development. Each value carries `kind`, `attr_source`, vintage and confidence. **Synthetic attribute-generation curves are probabilistic priors, not observed player mental trait values.**
- **Team and chemistry:** previous *confirmed* co-start networks and per-player familiarity; lineup-conditioned mean, variance, leadership presence, teamwork complementarity and cohesion. Registered-squad mental averages are only population summaries, not observed future starting-XI averages.
- **Value forecasts:** dated market value level/velocity/volatility, observed transfers/contract and market context are optional pre-match covariates. Predicted future value/breakout/transfer fee are auxiliary **targets**, not contemporaneous observations.

## Existing new research modules

- `apps/matchforecast/math_engine/unified_features.py`: strictly causal source-neutral features, provenance masking, FM mental team mean/variance and earlier co-start familiarity. Retains all registered candidates, even with missing FM and provider observations.
- `apps/matchforecast/math_engine/shared_math_glm.py`: pooled EPL+MLS conditional selection/shot/conversion joint likelihood, fit on chronological match labels, deterministic missingness-aware normalization and regularization. No player ID in X and no per-person parameter.
- `apps/matchforecast/math_engine/identity_bridge.py`: independent ID namespace audit with Sortitoutsi mappings and known raw-UID collision quarantine.
- `scripts/build_unified_prematch_bundle.py`: previous source import/helpers, including sofa rolling observations, separate measured/reconstructed FM mental masks, dated fbref crosswalk, and roster chemistry.
- `scripts/audit_reep_crosswalk.py`: existing Reep ID bridge, DOB conflict, staff classification and observed versus reconstructed attribute checks.

The last two are **existing in-progress companion work and are not replaced**. The new universal contract cannot yet consume every live source end-to-end: some EPL and MLS player event and injury histories remain unmapped, and complete player-match training labels do not yet cover representative prospective fixtures. **Do not publish or claim forecast gains before the full training/validation gate passes.**

## Training and promotion

1. Audit all 600 registrations against all FM26 identity tables and report counts by no-export / ambiguous / staff / reconstructed / measured, with explicit provider-ID provenance; replicate for MLS rosters.
2. Build dated player-match observations for SofaScore, FBref and ESPN/MLS only when their actual publication time precedes the forecast. Freeze Reep version, provider mapping and official roster vintage.
3. Compute one source-neutral matrix for every pre-match roster candidate, observed-value masks, skill domains, psychology with confidence, lineup chemistry, and team context. No ID columns enter numerical X; league flag is allowed.
4. Train **one** pooled regularized mathematical conditional model on earlier EPL+MLS fixture labels. Compare with player-rate and team-rate baselines. Fix model hyperparameters on predeclared past validation, not 2015/16 or the already inspected 25-match study.
5. Feed the conditional predictions to **one** Rust stochastic path engine; generate exact-cardinality XI, substitutions, marked shots, goals, scorer events and match outcomes consistently. Backtest W/D/L log loss, selection Brier, shots Poisson log loss, conversion log loss, goal binary Brier, calibration and coverage.
6. Promote no target without 200+ independent out-of-sample fixtures, source coverage, time-gated identities and a competitive proper-score improvement with matchday clustered intervals.

The ultimate goal is **accuracy and calibration under observed information**, not “maximum number of primitives loaded” or 600 forced identities.
