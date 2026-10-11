# Out-of-sample Premier League player-event calibration audit

## Audit conclusion

**Current 2026 FM26 minute/player simulator has NOT been statistically validated here.** This run evaluates a simple date-causal player/team event-rate baseline against actual goals, registered assists, shots, and goal/assist timing in a historical Premier League season. The held-out evaluation is genuine, but a different model and time period. Avoid transferring its metrics to the Rust FM26 match simulator.

**Data:** StatsBomb open-data cached English PL 2015/16 events; 74 event-covered matches of 380 season fixtures. Other 306 metadata fixtures lacked event files and were not evaluated.
**Holdout:** 24 chronologically last covered fixtures, from 2016-02-13 to 2016-05-17. For each prediction use only event-covered fixtures with `match_date < predicted_match_date`. No same-day events, future roster, FM26, or test-match starting XI may enter feature calculation.

## Real out-of-sample performance

| Held-out metric (lower = better) | Causal historical-rate model | Simple previous-only reference | Model minus reference |
|---|---:|---:|---:|
| Player shot occurrence Brier | 0.2162 | 0.2347 | -0.01849 |
| Player goal occurrence Brier | 0.0598 | 0.0577 | +0.00214 |
| Player assist occurrence Brier | 0.0425 | 0.0431 | -0.00057 |
| At-least-one goal per 1-minute bin Brier | 0.0142 | 0.0141 | +0.00003 |
| At-least-one assist per 1-minute bin Brier | 0.0115 | 0.0114 | +0.00004 |
| At-least-one goal per 15-minute bin Brier | 0.1618 | 0.1613 | +0.00052 |
| Team shot count MAE | 3.7267 | 3.8525 | -0.12582 |

### Match-paired sampling uncertainty

| Metric | Model − baseline (negative favors model) | 95% fixture bootstrap interval | Improvement established at 95%? |
|---|---:|---|---|
| minute goal | +0.000029 | [-0.000023, +0.000082] | no |
| 15minute goal | +0.000518 | [-0.003283, +0.004635] | no |
| minute assist | +0.000039 | [+0.000015, +0.000063] | no |
| team shots | -0.125816 | [-0.393460, +0.121234] | no |

**Interpretation:** Shots modestly improve as a point estimate, but the match bootstrap overlaps zero. Goal timing is marginally worse than uniform historical timing; player goal Brier is worse than the simple reference. More Monte Carlo paths would not fix those features. The minute-level assist baseline also performs worse. No prospective reliability claim is warranted. These intervals do not fully correct team/fixture dependence in an incomplete open-data subset.

## Coverage / actual events

Across the 24 held-out matches, 7 actual goals and 9 registered assists came from players absent from the causal pre-match player universe. Known-player coverage averages 35.0 candidates across both clubs, **not** confirmed lineup participants.

| Date | Actual fixture | Real final score | Pre-match event-rate expected goals | Actual recorded goal minutes (0-indexed regulation bins) |
|---|---|---|---|---|
| 2016-04-16 | Manchester United – Aston Villa | 1–0 | 1.16–0.99 | Manchester 32′ |
| 2016-04-17 | AFC Bournemouth – Liverpool | 1–2 | 1.19–0.98 | Liverpool 41′, Liverpool 47′, AFC 90′ |
| 2016-04-20 | Manchester United – Crystal Palace | 2–0 | 1.16–0.84 | Manchester 55′ |
| 2016-04-23 | AFC Bournemouth – Chelsea | 1–4 | 1.18–0.83 | Chelsea 5′, Chelsea 34′, AFC 36′, Chelsea 71′, Chelsea 90′ |
| 2016-04-30 | Everton – AFC Bournemouth | 2–1 | 1.13–1.18 | Everton 7′, AFC 9′, Everton 64′ |
| 2016-05-01 | Manchester United – Leicester City | 1–1 | 1.16–0.99 | Manchester 8′, Leicester 17′ |
| 2016-05-07 | Norwich City – Manchester United | 0–1 | 1.42–1.15 | Manchester 72′ |
| 2016-05-07 | AFC Bournemouth – West Bromwich Albion | 1–1 | 1.17–0.99 | West 15′, AFC 82′ |
| 2016-05-10 | West Ham United – Manchester United | 3–2 | 1.55–1.15 | West 10′, Manchester 51′, Manchester 72′, West 76′, West 80′ |
| 2016-05-17 | Manchester United – AFC Bournemouth | 3–1 | 1.17–1.17 | Manchester 43′, Manchester 74′, Manchester 87′ |

## Event attribution and limitations

Actual shot records are StatsBomb `Shot` objects; actual goals are `Shot` with outcome `Goal`, plus own-goal accounting where relevant. Actual assists are `Pass.goal_assist=true` (this is **not** `xA` and not necessarily every informal secondary assist). Timestamps in the data include clock minute and second; after 90′, added-time observations are assigned to minute 90 for 90-bin conservation, never treated as an exactly timed 90th-minute event.

The benchmark predicts player probabilities of at least one goal/assist/shot from strictly earlier observed appearances, team rates, and historical event timing. It **does not** predict goalscorer / assist identities from FM attributes, expected lineups, or dynamic psychology. The **current Rust simulator** now outputs 90 one-minute marginal event bins and player-scorer/provisional-assist allocations, but its scorer weights and fixed assist probability have not yet been backtested on historical as-of FM/team inputs.

Required for a credible prospective test: dated squads/injuries, contemporaneous FM snapshots or non-leaky historic player signals, confirmed postgame scorers/assists/shots, a pre-frozen calibration protocol, and reporting of 1-minute and player-event Brier/MAE/log loss against both uniform and historical-rate baselines with matchday-block uncertainty. Hold the chronological test portion out of tuning.

## Reproducible scripts

```bash
python3 scripts/backtest_pl_event_oos.py --holdout-matches 24
python3 scripts/write_pl_oos_report.py
python3 -m unittest discover -s tests -p test_pl_event_oos.py
```

All raw StatsBomb event files remain local; scored aggregate, source metadata, actual observed event timestamps, and per-match predictions are saved in [`artifacts/validation/premier-league-events-2015-16-oos.json`](../artifacts/validation/premier-league-events-2015-16-oos.json).
