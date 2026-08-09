# footy-models architecture

## Present objectives

`footy-models` is a Rust-first football recruitment, player-pricing, and squad-capital-allocation system built around historical Football Manager data, Transfermarkt market/transfer data, and FBref performance data.

The current objectives are:

1. **Fair transfer value and fair salary** — estimate what a player should cost and earn from ability, performance, development, age, contract, league, role, and market context.
2. **Undervalued-player discovery** — identify players whose expected acquisition price is materially below the value implied by their football state and future outcomes.
3. **Future-gem detection** — learn the pre-breakout profiles of historical players whose perceived value later spiked, then find current players with comparable state trajectories.
4. **Recruitment compatibility** — estimate tactical, squad, psychological, and environmental fit and the marginal sporting effect of adding a player to a specific squad.
5. **Probabilistic value forecasting** — simulate player development, performance, availability, contracts, transfer activity, and market regimes to produce future value distributions rather than false point precision.
6. **Recruitment portfolio optimization** — rank combinations of acquisitions by expected sporting value, expected resale value, total acquisition cost, fit, and downside under budget and squad constraints.

A central modeling assumption is that **Transfermarkt market values are a useful dense observation of market-perceived player price because, over appropriate historical windows, they are usually close to actual paid transfer amounts**. That relationship is not hard-coded as one-to-one. It is continuously estimated from real transfers so that the system can convert Transfermarkt observations into a calibrated clearing-price surface with explicit residual uncertainty.

---

## 1. System thesis

The project should not be one monolithic valuation model. It should be a set of small, composable primitives whose outputs can be independently validated and reused.

The core chain is:

```text
raw sources
    -> canonical identity
    -> temporal event store
    -> as-of feature state
    -> market-price calibration
    -> player state
    -> fundamental value
    -> transaction/acquisition value
    -> future state/value distribution
    -> recruitment alpha
    -> compatibility
    -> squad simulation/optimization
```

The architecture deliberately separates four concepts that must never be conflated:

1. **Market-perceived value** — what the football market broadly thinks the player is worth.
2. **Fundamental football value** — value implied by ability, production, development, age, scarcity, and level without feeding the current market quote directly into the model.
3. **Expected acquisition price** — the likely clearing price for a specific buyer/seller/contract/window context.
4. **Future value distribution** — the probability distribution of values after future development, performance, contract, and market paths.

The recruitment edge is the discrepancy between these layers, not simply reproducing Transfermarkt values.

---

## 2. Non-negotiable design invariants

### 2.1 Causal/as-of data only

Every historical prediction must answer:

> What was knowable about this player on date `t`?

No feature may use information published, observed, revised, or implied after the prediction timestamp. Random train/test splits are prohibited for valuation and breakout evaluation; use walk-forward historical evaluation.

### 2.2 Source data is never model truth by itself

Transfermarkt, FBref, Football Manager, salary sources, and manual observations are all measurements with different coverage and uncertainty. Every imported record retains provenance.

### 2.3 Missing is not zero

Coverage and missingness are explicit. An absent FBref metric must never be silently interpreted as zero production.

### 2.4 Market values and actual transfers are distinct observations

Actual transfers are sparse, high-confidence clearing-price events. Transfermarkt values are dense market-perception observations calibrated to those events. Both may populate a unified price target, but their observation type and uncertainty remain visible.

### 2.5 Free transfers are not zero-fee paid transfers

Permanent paid transfers, free transfers, loans, options, obligations, swaps, youth movements, and returns from loan are separate event kinds with separate economics.

### 2.6 Fundamental valuation cannot see current market value

Transfermarkt current market value is a target/benchmark for the fundamental model, not a direct feature. Otherwise the model learns consensus and erases the mispricing signal.

### 2.7 Uncertainty is a first-class output

All important price and forecast outputs are distributions or calibrated intervals. Never present precise euro point estimates without confidence/dispersion.

### 2.8 Minimal dependencies

Core domain logic, statistics, temporal logic, features, scoring, and simulation should prefer `std` and small internal implementations. External dependencies belong at I/O and persistence boundaries and must be justified.

---

## 3. Top-level model graph

```text
                         CANONICAL IDENTITY GRAPH
                                   |
          +------------------------+-------------------------+
          |                        |                         |
     FM HISTORY              PERFORMANCE DATA           MARKET DATA
 attributes/personality      FBref/match windows       values/transfers
 CA/PA/roles                 usage/production          contracts/clubs
          |                        |                         |
          +------------------------+-------------------------+
                                   |
                          TEMPORAL FEATURE STORE
                                   |
                 +-----------------+-----------------+
                 |                 |                 |
              FM STATE       PERFORMANCE STATE    MARKET STATE
                 |                 |                 |
                 +-----------------+-----------------+
                                   |
                              PLAYER STATE
                                   |
           +-----------------------+------------------------+
           |                       |                        |
   FUNDAMENTAL VALUE         DEVELOPMENT MODEL       TRANSFER HAZARD
           |                       |                        |
           |                  FUTURE PLAYER STATE          |
           +-----------------------+------------------------+
                                   |
                              FUTURE VALUE
                                   |
                            MONTE CARLO ENGINE
                                   |
          +------------------------+------------------------+
          |                        |                        |
      PRICE ALPHA             SPORTING FIT              WAGE/TAC
          |                        |                        |
          +------------------------+------------------------+
                                   |
                           RECRUITMENT ENGINE
                                   |
                           SQUAD OPTIMIZER
```

---

## 4. Core scalar primitives

Use strong Rust types so IDs, money, dates, probabilities, and model versions cannot be accidentally mixed.

```rust
pub struct PlayerId(pub u64);
pub struct ClubId(pub u32);
pub struct CompetitionId(pub u32);
pub struct MatchId(pub u64);
pub struct TransferId(pub u64);
pub struct SourcePlayerId(pub u64);
pub struct MetricId(pub u16);
pub struct AttributeId(pub u16);
pub struct RoleId(pub u16);
pub struct FeatureId(pub u32);

/// Days since a fixed epoch. Avoid heavyweight date dependencies in hot paths.
pub struct Date(pub i32);
pub struct SeasonId(pub u16);

pub struct EuroCents(pub i64);
pub struct Probability(pub f32);
pub struct Confidence(pub f32);
pub struct StringId(pub u32);
```

Guidelines:

- use integer cents for money;
- use integer IDs for categorical values;
- use interned strings outside hot structures;
- use `f32` for persisted feature vectors where precision is sufficient;
- use `f64` during fitting, robust aggregation, calibration, and simulation when accumulated numerical error matters.

---

## 5. Provenance and observation quality

Every imported observation must retain source metadata.

```rust
pub enum Source {
    FootballManager,
    Transfermarkt,
    Fbref,
    Salary,
    Manual,
}

pub struct Provenance {
    pub source: Source,
    pub external_id: StringId,
    pub retrieved_at_unix: i64,
    pub source_date: Date,
    pub confidence: f32,
}

pub struct Observed<T> {
    pub value: T,
    pub provenance_id: u64,
}
```

The system should support multiple conflicting observations without destroying the raw record. Reconciliation creates a canonical value while preserving the originals.

---

## 6. Coverage and missingness

```rust
pub enum CoverageStatus {
    Complete,
    Partial,
    Missing,
    Unknown,
}

pub struct Coverage {
    pub source: Source,
    pub competition: CompetitionId,
    pub season: SeasonId,
    pub metric_group: u16,
    pub status: CoverageStatus,
}
```

Each feature pipeline must distinguish:

- measured zero;
- not applicable;
- source unavailable;
- player not observed;
- low-minute/low-sample estimate.

Missingness indicators may themselves be useful model features, but they must not leak future coverage knowledge.

---

## 7. Canonical identity graph

The identity graph maps Football Manager, Transfermarkt, FBref, and salary-source entities to one canonical `PlayerId`.

```rust
pub struct Player {
    pub id: PlayerId,
    pub birth_date: Date,
    pub name: StringId,
    pub height_cm: Option<u16>,
    pub preferred_foot: Option<u8>,
}

pub enum LinkMethod {
    Explicit,
    ExactMetadata,
    WeightedMetadata,
    Manual,
}

pub struct IdentityLink {
    pub player: PlayerId,
    pub source: Source,
    pub source_id: StringId,
    pub score: f32,
    pub method: LinkMethod,
    pub verified: bool,
}
```

Candidate matching should proceed from strongest to weakest evidence:

1. explicit known cross-source mapping;
2. exact normalized name + DOB;
3. DOB + surname + nationality;
4. name + club + age;
5. fuzzy name + club/height/nationality/position metadata.

Never force ambiguous links. Retain candidate scores and a manual-review state.

---

## 8. Temporal primitives

Everything important is an event or a validity interval.

```rust
pub struct Snapshot<T> {
    pub player: PlayerId,
    pub valid_from: Date,
    pub valid_until: Option<Date>,
    pub value: T,
}

pub struct Event<T> {
    pub player: PlayerId,
    pub date: Date,
    pub value: T,
}
```

Core operations:

```rust
pub trait AsOf<T> {
    fn as_of(&self, player: PlayerId, date: Date) -> Option<&T>;
}
```

Prefer bitemporal storage where useful:

- **valid time** — when the fact was true in football reality;
- **observed/ingested time** — when this system learned the fact.

This allows later audits of whether a historical backtest accidentally used data that was known only after the prediction date.

---

## 9. Football Manager source primitives

Football Manager is a historical structured scouting signal, not merely a set of columns.

```rust
pub struct FmAttribute {
    pub id: AttributeId,
    pub value: u8,
}

pub struct PositionRating {
    pub position_id: u16,
    pub value: u8,
}

pub struct FmSnapshot {
    pub player: PlayerId,
    pub date: Date,
    pub current_ability: Option<u16>,
    pub potential_ability: Option<u16>,
    pub attributes: Vec<FmAttribute>,
    pub positions: Vec<PositionRating>,
    pub reputation: Option<u16>,
    pub personality: Option<PsychologyVector>,
}
```

Retain each historical release/snapshot independently. Never overwrite older Football Manager observations with a newer database.

Derived primitives include:

- attribute velocity;
- attribute acceleration;
- development breadth;
- development consistency across releases;
- CA/PA realization where the data legitimately contains those values;
- age-relative development percentiles;
- role-specific latent ability vectors.

Example latent vector:

```rust
pub struct AbilityVector {
    pub technical: f32,
    pub ball_progression: f32,
    pub creativity: f32,
    pub finishing: f32,
    pub defending: f32,
    pub aerial: f32,
    pub physical: f32,
    pub decision_making: f32,
    pub mentality: f32,
    pub goalkeeping: f32,
}
```

Keep the raw attributes alongside all derived dimensions.

---

## 10. Role primitives

Position labels are too coarse. Build role vectors from FM attributes and observed performance.

```rust
pub struct RoleScore {
    pub role: RoleId,
    pub score: f32,
}

pub struct RoleVector {
    pub scores: Vec<RoleScore>,
}
```

Examples include:

- ball-playing CB;
- stopper CB;
- overlapping fullback;
- inverted fullback;
- holding 6;
- progressive 6;
- box-to-box 8;
- advanced 8;
- creator 10;
- touchline winger;
- inside forward;
- channel-running 9;
- target 9.

Role definitions must be versioned because their feature weights will evolve.

---

## 11. Transfermarkt sourcing and source adapter

The Transfermarkt layer needs, at minimum:

- player IDs and metadata;
- club history;
- transfer history and exact transfer dates;
- actual transfer fee/loan fee where available;
- historical market-value observations and dates;
- contract information where available;
- clubs, leagues, and seasonal transfer activity.

The core system must not depend on HTML selectors or scraping details. The source boundary is a trait:

```rust
pub trait TransfermarktSource {
    fn players(&mut self, sink: &mut dyn RecordSink<PlayerSourceRecord>) -> Result<(), SourceError>;
    fn transfers(&mut self, sink: &mut dyn RecordSink<TransferSourceRecord>) -> Result<(), SourceError>;
    fn market_values(&mut self, sink: &mut dyn RecordSink<MarketValueSourceRecord>) -> Result<(), SourceError>;
    fn contracts(&mut self, sink: &mut dyn RecordSink<ContractSourceRecord>) -> Result<(), SourceError>;
}
```

Supported implementations should be data-rights-safe inputs such as:

```text
TmCsvImport
TmJsonImport
TmDatabaseImport
TmLicensedApi
TmAuthorizedExport
```

**Important sourcing constraint:** do not make unauthorized automated scraping or unauthorized ML use of Transfermarkt content an architectural dependency. Current Transfermarkt terms restrict automated extraction and ML use. The ingestion crate therefore consumes data the operator is authorized to use and keeps the rest of the platform independent of collection mechanics.

### Transfermarkt records

```rust
pub struct MarketValueObservation {
    pub player: PlayerId,
    pub date: Date,
    pub value: EuroCents,
}

pub enum TransferKind {
    Permanent,
    Loan,
    LoanWithOption,
    LoanWithObligation,
    Free,
    Youth,
    ReturnFromLoan,
    Internal,
    Unknown,
}

pub struct TransferEvent {
    pub id: TransferId,
    pub player: PlayerId,
    pub date: Date,
    pub from: ClubId,
    pub to: ClubId,
    pub kind: TransferKind,
    pub fee: Option<EuroCents>,
    pub loan_fee: Option<EuroCents>,
    pub market_value_before: Option<EuroCents>,
}

pub struct ContractSnapshot {
    pub player: PlayerId,
    pub club: ClubId,
    pub date: Date,
    pub expiry: Option<Date>,
    pub option_years: Option<u8>,
}
```

Do not backfill market values using future observations. For as-of features, use the latest observation known at or before the requested date or a strictly causal smoother.

---

## 12. Transfermarkt-to-clearing-price calibration

This is a central primitive.

For each legitimate paid transfer `j`, align the paid fee to the latest Transfermarkt market value available strictly before the transfer event:

```text
x_j = log(TMValue_j)
y_j = log(Fee_j)
```

Fit a local robust relationship:

```text
y_j = alpha_t + beta_t * x_j + epsilon_j
```

Weight historical observations by proximity to the calibration date:

```text
w_j = exp(-lambda * |t - t_j|)
```

Store:

```rust
pub struct MarketCalibration {
    pub as_of: Date,
    pub alpha: f64,
    pub beta: f64,
    pub residual_sigma: f64,
    pub observations: u32,
    pub effective_samples: f64,
}
```

Then transform a Transfermarkt market value into a calibrated market-perceived clearing price:

```text
P_perceived(i,t) = exp(alpha_t + beta_t * log(TMValue(i,t)))
```

Do not assume `alpha = 0` and `beta = 1`; estimate whether that is approximately true in each market regime.

### Hierarchical calibration

Calibration should progressively specialize by:

1. global market;
2. rolling time window;
3. position/role group;
4. age bucket;
5. selling-market tier;
6. buying-market tier;
7. transfer route.

Use parent shrinkage when a narrow bucket has too little data:

```text
theta_shrunk = n/(n+k) * theta_local + k/(n+k) * theta_parent
```

This is easy to implement and avoids fragile tiny-sample calibrations.

### Robust fitting

Implement Huber-loss IRLS internally for transfer calibration and other small regressions rather than introducing a large ML dependency.

---

## 13. Unified historical price target

A unified target provides a dense historical price surface while preserving observation quality.

```rust
pub enum PriceTargetKind {
    ActualTransfer,
    CalibratedMarketValue,
}

pub struct PriceTarget {
    pub player: PlayerId,
    pub date: Date,
    pub price: EuroCents,
    pub variance_log: f32,
    pub kind: PriceTargetKind,
}
```

Rules:

- paid transfer -> target is the actual fee with transaction-specific uncertainty;
- non-transfer market-value observation -> target is the calibrated Transfermarkt clearing-price equivalent;
- actual transfers receive materially higher observation confidence;
- free/loan/swap-like events do not enter the permanent-paid-fee target without a dedicated event model;
- observation weights must be stored or reproducibly derived.

---

## 14. FBref sourcing and performance adapter

FBref is a performance-data source, not a schema dependency. The architecture should support the current and future availability of standard, shooting, passing, possession, defending, playing-time, miscellaneous, goalkeeping, and match-log statistics without assuming every metric is present historically.

```rust
pub trait PerformanceSource {
    fn coverage(&mut self, sink: &mut dyn RecordSink<CoverageRecord>) -> Result<(), SourceError>;
    fn players(&mut self, sink: &mut dyn RecordSink<PlayerSourceRecord>) -> Result<(), SourceError>;
    fn performances(&mut self, sink: &mut dyn RecordSink<PerformanceSourceRecord>) -> Result<(), SourceError>;
}
```

The source adapter should use permitted/authorized access, cache immutable historical inputs locally, respect Sports Reference access rules/rate limits, and never make repeated remote access part of model scoring.

FBref has changed advanced-data availability over time, so the canonical layer must use a generic metric registry rather than one field per current webpage column.

```rust
pub struct MetricValue {
    pub metric: MetricId,
    pub value: f32,
}

pub struct PerformanceObservation {
    pub player: PlayerId,
    pub club: ClubId,
    pub competition: CompetitionId,
    pub start: Date,
    pub end: Date,
    pub minutes: u32,
    pub metrics: Vec<MetricValue>,
}
```

Prefer match-level observations when available:

```rust
pub struct PlayerMatch {
    pub player: PlayerId,
    pub match_id: MatchId,
    pub minutes: u16,
    pub metrics: Vec<MetricValue>,
}
```

The canonical data store, not FBref season boundaries, should produce rolling historical windows.

---

## 15. Performance windows

For each valuation date, support causal windows such as:

- 30 days;
- 90 days;
- 180 days;
- 365 days;
- previous 365 days;
- career to date.

```rust
pub struct PerformanceWindow {
    pub player: PlayerId,
    pub as_of: Date,
    pub horizon_days: u16,
    pub minutes: u32,
    pub values: Vec<MetricValue>,
}
```

Derived statistics include:

- totals;
- per-90 values;
- exponentially weighted recent values;
- recent-vs-prior trends;
- match-to-match dispersion;
- minutes/usage state;
- role-conditional percentiles;
- sample-size confidence.

---

## 16. Competition and transfer normalization

Player production and player prices need environment context.

```rust
pub struct CompetitionStrength {
    pub competition: CompetitionId,
    pub as_of: Date,
    pub attacking_factor: f32,
    pub defensive_factor: f32,
    pub overall_factor: f32,
}
```

Estimate league translation factors from players moving across competitions. A first primitive can compare role-normalized production before and after a move, then later control for age, minutes, team strength, and tactical role.

Peer normalization should use robust statistics such as median/MAD rather than means where outliers dominate.

```text
z_robust = (x - median) / (1.4826 * MAD)
```

Peer groups may include age bucket, role, competition tier, and season/window.

---

## 17. Player state

`PlayerState` is the canonical feature object consumed by downstream models.

```rust
pub struct PlayerState {
    pub player: PlayerId,
    pub as_of: Date,
    pub age_years: f32,
    pub ability: AbilityVector,
    pub development: DevelopmentVector,
    pub performance: PerformanceVector,
    pub usage: UsageVector,
    pub competition_strength: f32,
    pub role: RoleVector,
    pub psychology: Option<PsychologyVector>,
}
```

The state should include enough information to model:

- latent ability;
- realized production;
- age/development stage;
- development momentum;
- current opportunity/minutes;
- performance momentum;
- league difficulty;
- role and positional scarcity;
- uncertainty/coverage.

Current Transfermarkt market value must not be embedded inside the fundamental player state.

Useful derived primitives include:

```text
CapabilityGap = PredictedPerformanceFromAbility - ObservedPerformance
DevelopmentMomentum = PlayerState_t - PlayerState_t-12m
OpportunitySuppression = ExpectedMinutesGivenState - ActualMinutes
```

These are candidate alpha signals for pre-breakout players.

---

## 18. Feature registry and DAG

Feature order must never be implicit.

```rust
pub struct FeatureDefinition {
    pub id: FeatureId,
    pub name: StringId,
    pub kind: FeatureKind,
}
```

Examples:

```text
fm_pace
fm_composure
fm_decisions
fm_ca
fm_pa
fm_ca_velocity_12m
perf_goals_p90_365
perf_shots_p90_365
perf_minutes_365
age_years
competition_strength
role_creator_10
```

Derived features declare dependencies:

```rust
pub trait FeatureNode {
    fn dependencies(&self) -> &[FeatureId];
    fn compute(&self, context: &FeatureContext) -> FeatureValue;
}
```

The feature graph should be deterministic, versioned, cacheable, and testable. A model artifact stores the exact `FeatureSetVersion` it expects.

---

## 19. Fundamental-value model

Train the football state against the unified historical price target:

```text
PlayerState_t -> PriceTarget_t
```

Use log price as the initial response:

```text
y = log(price)
```

Start with models whose failure modes are easy to understand:

### Model 1 — robust log-linear

```text
log(P) = beta_0 + beta'X
```

### Model 2 — explicit nonlinear basis terms

Examples:

```text
age^2
ability * age
performance * age
ability * performance
potential * age
minutes * competition_strength
```

### Model 3 — small internal tree ensemble

Only add histogram trees/boosting after the data, leakage controls, and backtests are stable. Avoid bringing an external ML runtime into production simply for convenience.

Output:

```rust
pub struct PriceDistribution {
    pub p10: EuroCents,
    pub p25: EuroCents,
    pub p50: EuroCents,
    pub p75: EuroCents,
    pub p90: EuroCents,
    pub confidence: f32,
}
```

---

## 20. Market-perceived value vs fundamental value

For each player/date maintain separately:

```text
P_market      = calibrated Transfermarkt market-perceived clearing price
P_fundamental = value implied by FM + performance + context, without current market quote
```

Current mispricing:

```text
Mispricing = (P_fundamental - P_market) / P_market
```

This is one signal, not the final recruitment decision.

---

## 21. Acquisition-price model

The acquisition-price layer reintroduces the transaction context intentionally.

```rust
pub struct AcquisitionContext {
    pub contract_remaining_days: Option<i32>,
    pub buyer: ClubId,
    pub seller: ClubId,
    pub window_id: u16,
    pub route_id: Option<u32>,
}
```

Model:

```text
P_acquire = g(P_fundamental, contract, buyer, seller, route, market, timing)
```

This allows a player to have a fundamental value of EUR 25m but an expected acquisition price of EUR 17m or EUR 31m depending on bargaining context.

Keep buyer/seller effects interpretable and shrink sparse club estimates toward league/market parents.

---

## 22. Transfer hazard

Only transferred players reveal clearing prices. Model transfer probability separately.

```rust
pub struct TransferHazard {
    pub p_6m: f32,
    pub p_12m: f32,
    pub p_24m: f32,
}
```

Features may include:

- contract remaining;
- age;
- squad minutes;
- player quality relative to club level;
- recent performance and momentum;
- club buying/selling history;
- league;
- transfer window;
- prior transfer history.

This helps correct transfer-sample selection and distinguishes valuable-but-unavailable players from likely transaction candidates.

---

## 23. Gem detection and future outcomes

For every historical player/date create horizon outcomes.

```rust
pub struct FutureOutcome {
    pub horizon_days: u16,
    pub future_price: EuroCents,
    pub maximum_price: EuroCents,
    pub appreciation_log: f32,
    pub annualized_return: f32,
    pub transferred: bool,
}
```

Targets include:

- +25% within 12 months;
- +50% within 24 months;
- +100% within 36 months;
- top-decile appreciation within peer group;
- future actual paid transfer above a threshold;
- sustained performance improvement, not only market rerating.

The key model is:

```text
PlayerState_t -> P(ValueSpike within horizon)
```

This implements the research question:

> What did future stars look like before the market rerated them?

---

## 24. Historical analogues

Similarity is a first-class explanation primitive.

For a current player, find historical player states similar in:

- age;
- FM ability vector;
- development trajectory;
- role;
- minutes/opportunity;
- production;
- competition strength.

A simple weighted standardized distance is sufficient initially:

```text
D(i,j) = sum_k w_k * (x_ik - x_jk)^2
```

Return the realized 12/24/36-month value and performance outcomes of the nearest historical states. This is useful for explanation, model diagnostics, and scout-facing output without adding SHAP or another heavy dependency.

---

## 25. Salary and total acquisition cost

Salary is a separate labor-market target, not a fixed percentage of player price.

```rust
pub struct WageObservation {
    pub player: PlayerId,
    pub club: ClubId,
    pub as_of: Date,
    pub annual_gross: EuroCents,
    pub confidence: f32,
}
```

Model:

```text
log(wage) = f(PlayerState, league, club payroll tier, age, role, contract leverage)
```

Use an independent salary adapter so FBref wage displays or any other single site are not treated as complete truth.

Total acquisition cost:

```rust
pub struct TotalAcquisitionCost {
    pub transfer_fee: EuroCents,
    pub wages_npv: EuroCents,
    pub agent_fee: Option<EuroCents>,
    pub signing_fee: Option<EuroCents>,
    pub expected_bonuses: Option<EuroCents>,
}
```

Discount future cash flows using an explicit configurable rate and preserve assumptions in the output artifact.

---

## 26. Psychology/off-field state

Do not collapse psychology into one arbitrary score.

```rust
pub struct PsychologyVector {
    pub professionalism: f32,
    pub ambition: f32,
    pub consistency: f32,
    pub pressure: f32,
    pub adaptability: f32,
    pub leadership: f32,
    pub teamwork: f32,
    pub discipline: f32,
}
```

Each dimension should have observation confidence. Treat Football Manager personality/mental data as noisy scouting signals, not objective psychological facts.

Use the vector to estimate mechanisms such as:

- development persistence;
- adaptation after league/country changes;
- performance volatility;
- role acceptance;
- leadership/squad effects.

Do not directly convert a psychology score into arbitrary transfer euros.

---

## 27. Compatibility primitives

Compatibility has at least three independent components.

### Tactical fit

```text
Fit_tactical(player, club/tactic)
```

Compare the player's role/capability vector with the team's tactical demand vector.

### Squad complementarity

```text
Fit_squad(player, squad)
```

Estimate whether the player adds capabilities, role coverage, or quality where the existing squad is weak.

### Environmental fit

```text
Fit_environment(player, destination)
```

Use historical transitions to estimate adaptation by league change, playing-time opportunity, age, role, team level, and psychological signals where legitimate.

These outputs stay separate so a strong tactical fit cannot hide a poor availability/cost profile.

---

## 28. Squad state and marginal sporting value

```rust
pub struct SquadState {
    pub as_of: Date,
    pub players: Vec<SquadMember>,
    pub role_coverage: RoleVector,
    pub ability_moments: AbilityMoments,
    pub age_structure: AgeStructure,
    pub psychology: PsychologyAggregate,
}
```

Do not sum raw player attributes. Estimate expected team strength from:

- likely lineup and expected minutes;
- role coverage;
- quality over replacement;
- bench/depth effects;
- availability;
- tactical complementarity;
- interaction terms that have historical support.

Marginal sporting value:

```text
DeltaSportingValue(player) = E[TeamOutcome | Squad + player] - E[TeamOutcome | Squad]
```

Team outcome should ultimately be expressed in football quantities such as expected goal difference, expected points, qualification/relegation probabilities, or competition advancement—not an arbitrary aggregate attribute sum.

---

## 29. Recruitment alpha and ROI

Keep several alpha components separate.

### Current mispricing

```text
alpha_current = P_fundamental(t) - P_acquire(t)
```

### Development alpha

```text
alpha_development = E[P_fundamental(t+h)] - P_fundamental(t)
```

### Total financial alpha

```text
alpha_total = E[future sale value] - acquisition cost - carrying cost
```

### Club-specific recruitment value

```text
ExpectedRecruitmentValue = MonetaryValue(DeltaSportingValue)
                         + ExpectedResaleValue
                         - TotalAcquisitionCost
```

The final screen should support Pareto ranking rather than force all decisions into one opaque scalar.

---

## 30. Market index

Build a quality-adjusted transfer-market index to separate player-specific changes from football-market inflation/scarcity.

```rust
pub struct MarketIndexKey {
    pub period_id: u32,
    pub age_bucket: Option<u8>,
    pub role: Option<RoleId>,
    pub league: Option<CompetitionId>,
}
```

A first estimator can use the median ratio of observed price to constant-quality predicted price:

```text
Index_t = median(ObservedPrice_t / ConstantQualityPrice_t)
```

Use this to estimate:

- global market inflation;
- position scarcity premiums;
- U21/prospect premiums;
- league/buyer premiums;
- route-specific cycles.

---

## 31. Monte Carlo primitives

Use one small generic simulation interface.

```rust
pub trait StochasticProcess {
    type State;

    fn step(&self, state: &Self::State, rng: &mut Rng) -> Self::State;
}
```

Processes:

```text
DevelopmentProcess
PerformanceProcess
AvailabilityProcess
MarketProcess
ContractProcess
TransferProcess
```

A player path should simulate drivers before price:

```text
PlayerState_t
  -> Development
  -> Minutes/Availability
  -> Performance
  -> ContractState
  -> MarketState
  -> TransferEvent/NoTransfer
  -> FuturePrice
```

Suggested path counts are policy, not hard-coded rules:

- screening: ~1,000;
- shortlisted analysis: ~10,000;
- high-stakes committee analysis: more only when convergence diagnostics justify it.

Use a compact deterministic PRNG such as an internal PCG/xoshiro implementation and fixed seeds for reproducible research.

---

## 32. Volatility and MF2-GARCH placement

Do not begin by applying GARCH directly to individual-player Transfermarkt values. Individual players are sparsely and irregularly repriced and exhibit event jumps rather than continuous traded-price dynamics.

Keep a generic volatility trait:

```rust
pub trait VolatilityModel {
    fn variance(&self, t: Date) -> f64;
    fn update(&mut self, observation: f64);
}
```

Possible implementations:

```text
ConstantVolatility
EWMA
GARCH(1,1)
MF2-GARCH
```

Use MF2-GARCH first on sufficiently dense **market-factor/index** series if diagnostics show short/long-run volatility structure. Feed simulated market regimes into individual-player value paths.

---

## 33. Uncertainty and calibration

Distinguish:

1. **epistemic uncertainty** — insufficient information/model knowledge;
2. **aleatoric uncertainty** — genuine variation in player outcomes;
3. **market uncertainty** — dispersion in comparable transaction prices.

Start with split conformal calibration around model residuals. It is simple, dependency-light, and testable.

For residuals `e_i = |y_i - y_hat_i|`, choose the desired residual quantile `q` and form an interval around the prediction. Perform calibration on strictly later held-out historical periods than model training, never on the same rows used for fitting.

Coverage of P50/P80/P90 intervals should be monitored by age, role, league, price bucket, and sample density.

---

## 34. Backtesting

All principal models implement historical cutoffs.

```rust
pub trait HistoricalModel {
    type Prediction;

    fn train_before(&mut self, cutoff: Date) -> Result<(), ModelError>;
    fn predict_as_of(&self, player: PlayerId, date: Date) -> Result<Self::Prediction, ModelError>;
}
```

Walk-forward pattern:

```text
train <= 2016-06-30 -> predict 2016/17
train <= 2017-06-30 -> predict 2017/18
train <= 2018-06-30 -> predict 2018/19
...
```

Random row splitting is prohibited for headline evidence.

### Pricing metrics

Track at least:

- EUR MAE;
- log RMSE;
- median absolute percentage error;
- log R² where useful;
- interval coverage;
- pinball/quantile loss when quantile models exist;
- calibration slope/intercept.

Slice every metric by:

- age;
- role/position;
- league/tier;
- price bucket;
- minutes/data density;
- contract remaining;
- transfer route;
- youth/prospect subgroup.

### Gem/alpha backtest

For every historical window:

1. freeze all data at date `t`;
2. generate market-perceived, fundamental, and acquisition values;
3. rank eligible players by the intended alpha criterion;
4. select top `N` or a budget-constrained portfolio;
5. observe 12/24/36-month price, transfer, and sporting outcomes;
6. compare against baselines.

Baselines should include:

- Transfermarkt value alone;
- FM CA;
- FM PA;
- age + minutes;
- performance-only model;
- youngest-player heuristic;
- random players from the same starting-price bucket.

Do not call a signal alpha unless it beats these out of sample after costs and realistic acquisition constraints.

---

## 35. Candidate analysis output

```rust
pub struct CandidateAnalysis {
    pub player: PlayerId,
    pub fundamental_value: PriceDistribution,
    pub perceived_value: PriceDistribution,
    pub acquisition_price: PriceDistribution,
    pub mispricing: f32,
    pub future_12m: PriceDistribution,
    pub future_24m: PriceDistribution,
    pub future_36m: PriceDistribution,
    pub transfer_probability: TransferHazard,
    pub tactical_fit: f32,
    pub squad_fit: f32,
    pub fair_wage: PriceDistribution,
    pub downside_probability: f32,
    pub breakout_probability: f32,
    pub data_confidence: f32,
}
```

The API/UI should expose components rather than only one composite score.

---

## 36. Storage architecture

Separate research storage from production scoring storage.

### Research/event store

Normalized records such as:

```text
players
external_ids
clubs
competitions
fm_snapshots
market_values
transfers
contracts
performance_matches
performance_windows
salary_observations
coverage
provenance
```

An optional embedded SQL boundary may use `rusqlite` behind a feature flag when operational convenience justifies it.

### Model dataset

Materialize contiguous numeric matrices and target vectors for fitting. Do not make the model trainer repeatedly execute source-level joins.

### Production scoring snapshot

Use compact, immutable, versioned binary artifacts suitable for memory mapping or sequential reads. The hot scoring path does not need raw source strings, HTML, or verbose provenance payloads.

---

## 37. Versioning and reproducibility

```rust
pub struct ModelVersion(pub u64);
pub struct DatasetVersion(pub u64);
pub struct FeatureSetVersion(pub u64);

pub struct PredictionRecord {
    pub player: PlayerId,
    pub as_of: Date,
    pub model: ModelVersion,
    pub dataset: DatasetVersion,
    pub feature_set: FeatureSetVersion,
    pub prediction: PriceDistribution,
}
```

Every persisted model artifact must carry:

- feature registry/version;
- training cutoff;
- source dataset version/hashes;
- model hyperparameters;
- random seed where applicable;
- calibration artifact/version;
- code/schema version when possible.

The system must be able to explain why a player was ranked on a historical date using the exact artifact versions that produced the ranking.

---

## 38. Minimal-dependency Rust strategy

The target is not zero dependencies at any cost; it is **minimal dependencies in the correctness-critical and hot-path layers**.

### Prefer `std` for

- IDs and domain types;
- dates represented as integer days;
- temporal interval logic;
- string interning tables;
- rolling aggregators;
- robust statistics;
- small dense linear algebra needed by internal regressions;
- Huber IRLS;
- quantiles/MAD;
- calibration;
- deterministic PRNG;
- Monte Carlo;
- ranking and backtest metrics;
- model scoring.

### Boundary-only optional dependencies

Reasonable feature-gated dependencies may include:

```text
serde
serde_json
csv
rusqlite
zstd
```

Do not add by default:

```text
Polars
DataFusion
PyO3
PyTorch
TensorFlow
Tokio
large ORMs
large general ML frameworks
```

unless a measured requirement cannot be satisfied by the existing primitives.

Keep source adapters and persistence optional so the core scorer can compile with very few or potentially no non-std dependencies.

---

## 39. Repository layout

```text
footy-models/
|
|-- architecture.md
|-- AGENTS.md
|-- README.md
|
|-- crates/
|   |-- AGENTS.md
|   |
|   |-- core/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- entities/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- temporal/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- source-fm/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- source-transfermarkt/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- source-fbref/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- source-salary/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- performance/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- market/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- state/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- models/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- forecast/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   |-- recruitment/
|   |   |-- AGENTS.md
|   |   `-- src/
|   |
|   `-- backtest/
|       |-- AGENTS.md
|       `-- src/
|
`-- apps/
    |-- AGENTS.md
    |
    |-- ingest/
    |   |-- AGENTS.md
    |   `-- src/
    |
    |-- train/
    |   |-- AGENTS.md
    |   `-- src/
    |
    |-- score/
    |   |-- AGENTS.md
    |   `-- src/
    |
    `-- query/
        |-- AGENTS.md
        `-- src/
```

Expected module responsibilities once implementation begins:

```text
core       ids, money, date, strings, provenance, errors
entities   player, club, competition, identity graph
temporal   event, snapshot, interval, as-of operations
source-*   source parsing/import only; emit canonical source records
performance metrics, windows, normalization, league strength
market     TM calibration, price targets, indices, acquisition, hazard
state      ability, development, psychology, player state, role state
models     linear, robust, quantile, calibration, small tree learner
forecast   development, market paths, Monte Carlo, future value
recruitment undervaluation, analogues, compatibility, squad, optimizer
backtest   walk-forward runner, metrics, portfolio/gem evaluation
apps       thin orchestration binaries only
```

---

## 40. Crate dependency direction

Keep dependencies acyclic and domain-oriented.

A target dependency direction is:

```text
core
  ^
  |
entities       temporal
  ^              ^
  |              |
source-* --------+
  |
performance ----+
  |
state ----------+
  |
market ---------+
  |
models ---------+
  |
forecast -------+
  |
recruitment ----+
  |
backtest
```

More concretely:

- `core` depends on nothing internal;
- `entities` and `temporal` depend on `core`;
- source crates depend on `core`, `entities`, and narrow canonical record interfaces;
- `performance` depends on core/entities/temporal, not on source adapters;
- `state` depends on core/entities/temporal/performance;
- `market` depends on core/entities/temporal and canonical state/features, never on HTML/source parsers;
- `models` contains generic statistical algorithms and should not depend on Transfermarkt or FBref types;
- `forecast` composes state/market/model primitives;
- `recruitment` consumes stable outputs and adds club/squad decision logic;
- `backtest` may depend on most model crates but production crates must not depend on `backtest`;
- apps orchestrate crates but contain minimal domain logic.

Avoid circular dependencies by moving genuinely shared primitives downward rather than introducing cross-crate callbacks everywhere.

---

## 41. First vertical slice

Do not begin with psychology, squad optimization, or GARCH. Validate the economic thesis first.

Build in this order:

```text
1. historical FM import
2. Transfermarkt market-value + actual-transfer import
3. FBref performance import
4. canonical player identity map
5. temporal/as-of event store
6. causal performance windows
7. TM-to-actual-fee rolling calibration
8. dense PriceTarget construction
9. baseline FM + performance fundamental-value model
10. conformal/calibrated price intervals
11. walk-forward historical valuation backtest
12. 12/24/36m undervaluation/gem portfolio backtest
```

The first success criterion is:

> Given all information knowable at a historical date, the system produces calibrated perceived and fundamental player-price distributions, identifies valuation gaps, and demonstrates out-of-sample future value/performance improvement relative to simple FM, Transfermarkt, age, and performance baselines.

Only after this is stable should the project add:

```text
13. transaction/acquisition model
14. transfer hazard
15. salary/TAC
16. explicit development forecasting
17. historical analogues
18. tactical/environment compatibility
19. psychology-derived mechanisms
20. Monte Carlo player paths
21. market volatility/MF2-GARCH where diagnostics justify it
22. squad Monte Carlo and recruitment portfolio optimizer
```

---

## 42. Definition of an undervalued player

The final system should not define a gem as merely `fundamental value > Transfermarkt value`.

The stronger definition is:

> A player whose probability-weighted club-specific sporting value plus expected resale value materially exceeds expected acquisition and carrying cost, after accounting for transfer probability, fit, uncertainty, and downside.

A useful decomposition is:

```text
Market-perceived value        calibrated consensus
Fundamental value             football-state implied value
Expected acquisition price    actual buyer/seller clearing expectation
Future value distribution     development + performance + market paths
Marginal sporting value       team impact for this specific club
Total acquisition cost        fee + wages + agent/signing/bonuses
Recruitment alpha             sporting + resale value minus total cost
```

That decomposition is the core product architecture and should remain visible in code, model artifacts, backtests, and user-facing analysis.
