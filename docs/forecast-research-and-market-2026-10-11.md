# Football forecasting research: causal goals, players, markets, mathematical methods

Research audit 2026-10-11. This is an implementation decision record, **not** a claim
that FM26 predictions beat causal Poisson on a sealed test set.

## Actual evidence, in order of usefulness

- Same-engine 2015/16 historical observed-stat evaluation: 25 held-out fixtures;
  W/D/L log loss **0.9768** vs causal Poisson **0.8899**, joint-score loss **3.0139**
  vs **2.7523**. Player shots, goal occurrence and minute goal occurrence are
  comparatively promising. The sample is club-skewed; it is **not** an OOS test
  of the FM26 attribute-enhanced predictor. See `historical-same-engine-backtest.md`.
- User-supplied 2026-10-11 completed fixture results (Palace 3-1 Forest,
  Hull 2-2 Everton, Liverpool 0-3 Man City) are a forward monitoring log, **n=3**,
  not model selection data. The existing 72-path simulator has approximately
  +/-12 percentage points worst-case 95% Monte Carlo sampling uncertainty.
  Do not change goal rate or draw pricing based on these matches. The described
  red card at Liverpool--City suggests a useful *conditional event diagnostic*,
  not evidence that red-card event rates are calibrated.
- The 2012-01-01 to 2016-02-07 fit on the imported historical results archive
  estimates per-player 90-minute red-card incidence **0.006857**, approximating
  22 equally exposed player slots. On **1,270** games strictly after that fit
  through 2019-05-31, any-red-card Brier drops from **0.15966** under the
  existing 0.02 per-player default to **0.09843** under the frozen fitted prior;
  paired matchday-bootstrap fitted-minus-default 95% CI **[-0.06891,-0.05376]**.
  Source: `artifacts/validation/pl-red-card-prior-research.json`. This is
  incident-only, across an external provider/era; **not** proof that changing
  the engine's red-card rate improves goals/WDL, so the default is unchanged.

## What we integrated as research interfaces

1. `import_pl_historical_results.py` ingests the **actual** CSV inside
   `galsaar/EnglishPremierLeagueEPLResults/results.csv.zip`. The HF Hub viewer
   currently indexes six **images** rather than the CSV. The archive contains
   **11,113** matches from **1993-08-14** through **2022-04-10**, with FTHG,
   FTAG, results and partially covered shots/cards. It is not a 2026 training
   feed and contains **no FM player attributes**. Null is retained as missing,
   not a zero. Data and SHA-256 manifest are local/untracked; no historical
   label is permitted into pre-match features. The `HR` and `AR` fields have
   8,289 jointly labeled matches, of which ~13.7% have any recorded dismissal.
2. `polymarket_prematch.py` is a strict 1X2 **benchmark adapter**, not a
   silent odds blend: supply manually verified fixture ID, event slug, three
   distinct YES tokens and the settlement rule **90 minutes plus stoppage**,
   including the draw. Fetch CLOB `/prices-history` (or use a frozen history
   JSON), select the last tick no later than a precise pre-kickoff cutoff,
   reject stale, in-play, unrelated, incomplete and implausibly normalized
   markets. Preserve individual quote times, raw price sum and normalized
   indicative probabilities. Last-traded prices are **not** executable odds:
   liquidity, bid/ask spread, stale ticks, fees and settlement definitions
   matter. No archived valid pre-match EPL Polymarket series was available to
   train or backtest a market-informed model in this run.
3. The Rust historical driver supports `--team-strength-anchor` as an
   **opt-in ablation**. It reassigns the historical goal anchor using strictly
   pre-kickoff team GF/GA and shots-on-target history while holding its
   unweighted mean fixed. Tests check exchange symmetry and normalization.
   This preserves historical parameter defaults, frozen forecasts and the
   broader path coherence of the Rust engine; **no predictive promotion**
   without the same-engine held-out proper-score comparison.

## Model proposals: high priority first

**A. Calibrate two team goal rates and their dependence, rather than totals.**
Today the common `blended_rate` in both `build_input` and
`build_historical_input` averages both teams' GF/GA and SOT before the two
path-specific goal channels. This loses some difference in causal team strength;
the shared global goal rescaling was already rejected OOS. Compare the opt-in
side-specific anchor with the frozen same-engine baseline. Then test
joint/shared match-tempo shocks, red-card/score-state conditional hazards and
low-score association (Dixon-Coles) as separate controlled experiments.
The 1997 Dixon--Coles low-score correction can be benchmarked against Poisson,
but do not paste adjusted categorical probabilities over inconsistent Rust
player/team paths. Any accepted change belongs in the underlying path generator.

**B. Treat FM attributes as *regularized player priors*, with effective dates.**
The existing Rust code uses FM finishing/technique/defending, matchup states,
role fit and other attributes to perturb shot/xG/xA channels. They are not
validated 2026 goal production coefficients, and FM26 ratings in a 2015/16
fixture would be direct lookahead. With dated historical FM editions and
true as-of registrations, train an exposure-offset hierarchical event model:

`shots_{i,m} ~ NegBin(exposure_minutes/90 * exp(team+role+opponent+FM_beta), dispersion)`

`goals_{i,m} | attempts ~ Binomial(attempts, sigmoid(shot_context + player/role_effect))`

Shrink learned FM slopes and player effects toward a zero impact when coverage
is weak, keep missing attributes separate from zero ratings, and propagate
lineup/minutes uncertainty through every match path. Bayes-xG finds persistent
player-specific conversion effects, but it does not validate FM attributes or
suggest training a large net on 74 covered fixtures.

**C. Model selection/lineup quality before architecture changes.**
The existing sampler compresses high prior start probabilities. The fit-only
isotonic correction improves historical minutes MAE, but is not approved for
W/D/L. Try a calibrated exact-11 constrained sampler with locked starter and
availability boundaries, then evaluate appearance Brier, not only fit curves.

**D. A small set encoder, not a sequence Transformer, is the first neural test.**
Lineups are unordered collections of players. The first neural challenger
should be shared 2-layer player MLP + masked mean/sum aggregation (Deep Sets),
conditioning on role, minutes/availability, as-of event history, FM attributes
and confidence. Output **residual log shot and goal-hazard adjustments** against
the strong causal Poisson model; do not duplicate W/D/L categorical outputs.
If and only if this clears chronological proper-score gates, evaluate a
2-layer Set Transformer with role/opponent interactions. Standard sequence
attention assigns arbitrary order to an XI and adds parameters without more
labels. Test a strict equal-data ablation of `Poisson -> hierarchical priors ->
Deep Sets -> Set Transformer`, with training-era FM editions and sealed OOS.

**E. Market odds complement, but cannot prove, a player model.**
For each fixture, compare historical as-of 1X2 Polymarket quote distributions
to both a causal Poisson and the simulator on identical matchdays. A learned
market feature requires sufficient timestamped, liquid pre-match samples and
a fit/validation/test split. Reject final settlement/post-kickoff quotes.
Only after the feature beats the same-day reference with uncertainty analysis
consider *upstream* goal-rate conditioning or **joint path reweighting** of all
match, player and minute outputs. Never replace only W/D/L after simulation.

## External material reviewed (exact scopes)

- [EPL-Pulse_v1](https://huggingface.co/Chaste20/EPL-Pulse_v1): model card
  describes two Poisson GLMs with Elo and rolling GF/GA and a precomputed team
  state; a reproducible benchmark design. The posted pickle files must not be
  unpickled from an untrusted source. No published sealed same-engine test or
  transfer proof for the FM26 simulator.
- [Galsaar EPL Results](https://huggingface.co/datasets/galsaar/EnglishPremierLeagueEPLResults):
  archived match score/event fields, no player ratings; ZIP source above.
- [Dixon & Coles (1997)](https://doi.org/10.1111/1467-9876.00065):
  goals as count data, dependence for low-score outcomes and time decay.
- [Bayes-xG (2024)](https://doi.org/10.3389/fspor.2024.1348983):
  Bayesian hierarchical conversion effects with player/position information.
- [Deep Sets (2017)](https://arxiv.org/abs/1703.06114) and
  [Set Transformer (ICML 2019)](https://proceedings.mlr.press/v97/lee19d.html):
  permutation-invariant squad encoders; no direct accuracy result on FM data.
- [SCORE (2025)](https://doi.org/10.1016/j.ijforecast.2025.02.004):
  past-event-conditioned next-event prediction; useful for **in-play** tests,
  not a reason to leak the actual match state into a pre-kickoff model.
- [Polymarket Institute market-data documentation](https://institute.polymarket.com/data):
  Gamma identifies market contracts; CLOB exposes token price history.

### `openai/math` audit

Reviewed the October 2026 [catalogue](https://github.com/openai/math),
especially `overview.tex`: it is mathematical research manuscripts, not a
football model/training dataset, and the README warns some results are not
fully formalized. Potentially adjacent entries include **115** (sampling
contingency tables with prescribed margins), **139** (log-concave sampling),
**104** (stochastic mean-payoff games) and **374** (optimal transport map
stability). They concern **different mathematical settings**: uniform fixed-
margin matrices, special strongly log-concave potentials, infinite-horizon
reward games, and regular OT stability. None proves exact constrained *weighted*
lineup sampling, W/D/L calibration, football event likelihoods, or variance
reduction for this Rust engine. No algorithm from these papers was incorporated
as a predictive improvement without those assumptions; standard paired
bootstrap/proper scoring already supply the validation framework we need.

## Reproduction and gates

```bash
python3 scripts/import_pl_historical_results.py
python3 scripts/fit_pl_red_card_prior.py
python3 -m unittest tests.test_new_forecast_sources
RUSTFLAGS='-D warnings' cargo test -p footy-matchforecast --lib historical::tests

# Full same-engine *research ablation*, expensive; save to new files and
# compare against the already frozen baseline without overwriting it:
./target/release/footy-matchforecast-pl-hist --paths 200 --jobs 4 \
  --team-strength-anchor --output data/statsbomb/pl_hist/strength-paths.json
python3 scripts/score_pl_hist_backtest.py \
  --predictions data/statsbomb/pl_hist/strength-paths.json \
  --compare-predictions artifacts/validation/pl-hist-same-engine-paths.json \
  --experiment 'causal side-specific goal anchor' \
  --output data/statsbomb/pl_hist/strength-oos-report.json

# Red-card prior is separately fitted from training seasons. Test via
# --red-card-probability-per90 0.006857 in a new *same-engine* OOS run.
# Values in this example are a research candidate, not published defaults.
```

Keep the engine-default parameters and frozen historic predictions unchanged.
Require no-lookahead, >200 representative OOS fixtures, proper scores,
matchday bootstrap improvements with compatible label definitions, and no
important cross-target degradation before a promoted production change.
