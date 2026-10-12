# Dated player-event/lineup panel and first participation evaluation

## Scope and outcome

The input audit's phase B and initial phase C are implemented as **research-only**.
`scripts/build_historical_player_panel.py` creates one normalized historical
player-match observation table and one causal training table. The observed
target match never supplies its own player IDs, positions, events or minutes
as input features. Dates are handled in closed matchday blocks to exclude
same-day source leakage. All source IDs are provider-qualified; a name is
never treated as evidence of common identity across sources.

The first `footy-participation-eval` run fits the existing source-neutral Rust
`NextMatchParticipationModel` from strictly older training dates and scores
start probability, appearance probability and conditional starter minutes
against an unlearned previous-only empirical-Bayes prior. Different leagues
are evaluated **independently**, without transferring the 2015/16 EPL model
to 2026 teams or silently using future FM26 attributes.

## Data contract

| Source | Covered fixtures | Observed player-match records | Causal candidate rows |
| --- | ---: | ---: | ---: |
| StatsBomb 2015/16 EPL event+lineup snapshots | 74 | 2,663 | 2,919 |
| SofaScore 2026 confirmed match sheets (MLS and other competitions) | 113 | 4,550 | 3,680 |
| **Combined, in separate namespaces** | **187** | **7,213** | **6,599** |

The `player_matches.csv` file is the observed-label store: stable provider
player ID, match date, team, lineup, start, minutes, optional shots, goals,
assists, position and explicit shot coverage. The `participation.tsv` rows
contain exclusively prior lineup history, elapsed days and the held-out
target's labels. StatsBomb position labels come from the separately cached
original lineup JSON; SofaScore position labels use the provider player
record. Historical positions are carried forward only after their fixture
date, never read from the predicted fixture. Only previously observed candidates can be scored. Bench
listings count as listings, zero-minute non-appearances count as zero, and
players with unknown shots do **not** receive invented zero-shot labels.

Candidate windows are 120 days for sparse StatsBomb coverage and 45 days for
SofaScore, with the historical club membership inferred only from the most
recent earlier provider listing. Because source sheets and fixture clocks have
no independently verified publication timestamps in the local archive, the
contract conservatively uses `prior_match_date < forecast_match_date` rather
than asserting precise known pre-kickoff availability. A missing or
unconfirmed lineup is excluded, never interpreted as a negative selection.

FBref/Soccerdata's existing adapter supplies name-indexed player-match rows
without stable provider person IDs or fully verified bench/lineup coverage.
It is **not** combined with the learned participation population until a
verified persistent-ID and observed-selection contract is available. This is
intentional, not missing-data zero imputation.

## First fixed-model chronological experiment

The Rust evaluator partitions *distinct match dates* within each source:
first 60% fit, next 20% validation, final 20% test. The ridge/logistic
regularization is fixed beforehand (`logistic_l2=2`, `minutes_ridge=8`). A
paired 1,000-draw bootstrap resamples entire matchdays, rather than treating
the 22+ player targets per fixture as independent evidence. Lower Brier is
better. Reports are saved under an untracked local data directory.

| Source | Validation start Brier: prior → learned | Test start Brier: prior → learned | Test appearance Brier: prior → learned | Test starter-minutes MAE: prior → learned |
| --- | --- | --- | --- | --- |
| 2026 SofaScore | 0.16628 → 0.15541 | 0.17195 → 0.16393 | 0.16308 → 0.15438 | 9.44 → 9.24 min |
| 2015/16 EPL | 0.16319 → 0.15940 | **0.17539 → 0.18695 (worse)** | 0.19283 → 0.19080 | 7.58 → 6.98 min |

The 95% matchday-bootstrap interval of *learned minus prior start Brier* is
`[-0.01251, +0.00058]` for SofaScore's 780-row/11-matchday final test and
`[+0.00184, +0.02188]` for EPL's 559-row/11-matchday final test. The
EPL result is evidence of degraded out-of-sample start calibration in this
limited archive. Neither independent test has 200 full fixtures; it does not
meet the release policy.

**Decision:** don't promote the learned prior into live Premier League
forecasts. Preserve the source-neutral Rust implementation and this negative
result; do not retune against the inspected final test dates. Revisit the
model only after acquiring additional EPL lineup histories and a new frozen
chronological test window, preferably with actual roster/absence inputs at
each as-of date.

## Reproduce without altering published artifacts

```bash
python3 scripts/build_historical_player_panel.py \
  --out data/forecast/player_panel_v1

cargo run -p footy-matchforecast --bin footy-participation-eval -- \
  --input data/forecast/player_panel_v1/participation.tsv \
  --out data/forecast/player_panel_v1/participation_eval_v1.json

python3 -m unittest discover -s tests -p 'test_historical_player_panel.py'
cargo test -p footy-matchforecast --bin footy-participation-eval
```

Choose a new output path for repeated evaluation because the Rust tool
refuses to overwrite an existing report. The build manifest includes source
and output hashes to make the local panel inspectable and reproducible. Raw
licensed/provider data remain untracked; no present-day FM attributes, future
target labels, current lineups, or fixture results enter pre-match training
features. The shared Rust match engine and production forecasts are unchanged.

## Remaining path to production

Expand per-fixture identities and actual lineup coverage to at least 200
independent held-out EPL matches. Then train the start/appearance/minutes
model on prior club participation, dated roles, injuries and likely rotation,
evaluate conditional shot counts with genuine playing-time exposure, and
integrate only a model that beats the source-matched, same-engine baseline
under the existing target-specific forecast release gates. Candle/attention
models and new stochastic state mechanics remain downstream options, not
data-quality fixes.
