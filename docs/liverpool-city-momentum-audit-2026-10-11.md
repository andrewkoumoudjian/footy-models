# Liverpool–Manchester City momentum: frozen-forecast audit

**Technical appendix** to the [case-study conclusions](liverpool-city-momentum-case-2026-10-11.md).
This appendix retains the original six-bin shot/goals decomposition
and precise provenance records. It is not a separate confirmation
or independent test.

## Provenance and actual events

This is a **post-result diagnostic, never a retrofitted forecast**.
The original simulated forecast under
[artifacts/premier-league-2026-10-11/player_paths.json](../artifacts/premier-league-2026-10-11/player_paths.json)
was dated **October 10, 2026**, 72 complete Monte Carlo paths, Liverpool
seed 44. Its audited SHA256 is
09a7078b065ac36395954a05f5c136824eda0c0fb123c808850aec99f47cba7d.
After the match Liverpool lost **0–3**: Gravenberch dismissed 62',
Haaland scored 65', Doku 68', Cherki 78'. Sources: the
[Liverpool official report](https://www.liverpoolfc.com/news/match-report-liverpool-man-city-premier-league-anfield)
and [Manchester City report](https://www.mancity.com/news/mens/liverpool-v-city-premier-league-match-report-october-2026-63927325).
The post-match label file is
[artifacts/validation/liverpool-city-actual-2026-10-11.json](../artifacts/validation/liverpool-city-actual-2026-10-11.json).
**Two of three** goals, not all three, came in the 60–75 peak.

The original *display* momentum index is
100 × (home_shots + 3 × home_goals − away_shots − 3 × away_goals) /
(home_shots + 3 × home_goals + away_shots + 3 × away_goals),
using **simulated mean counts per 15-minute interval**. Negative favors
Manchester City. This is not observed possession or a calibrated probability.

| Window | City relative pressure (negative) | City shots | City goals | Liverpool shots | Liverpool goals |
| --- | ---: | ---: | ---: | ---: | ---: |
| 00–15 | +8.19 | 2.069 | 0.167 | 2.403 | 0.208 |
| 15–30 | +7.11 | 1.958 | 0.194 | 2.264 | 0.222 |
| 30–45 | +0.70 | 1.972 | 0.333 | 2.139 | 0.292 |
| 45–60 | −3.04 | 2.167 | **0.375** | 2.431 | 0.222 |
| **60–75** | **−13.15** | **2.333** | 0.278 | **1.806** | 0.208 |
| 75–90 | +3.55 | 1.764 | 0.292 | 2.000 | 0.278 |

The City's-relative 60–75 pressure peak survives goal weights of
0, 1, 3, 5 or 10 in the display formula. Compared with 45–60, the
dominant contribution is the **0.625 expected Liverpool-shot decline**;
City expected shots rise only 0.167 and City expected goals **fall 0.097**.
Absolute City goal expectancy peaks at **45–60**, not 60–75. The original
scenario assigned only **4.17%** probability to **any red card by either
team** in 60–75. It did *not* anticipate a specific Gravenberch dismissal.
Simulated substitution-trigger occurrence was **100%** in that window,
suggesting the engine's scheduled substitution clock is a material
confound worth explicitly ablating. This is a hypothesis, not proven cause.

## How unusual was the timing?

City's predicted share of eventual goals among six windows was
[0.1017, 0.1186, 0.2034, 0.2288, 0.1695, 0.1780].
For the observed goal bins [60–75, 60–75, 75–90], conditional goal
arrival log loss improves on uniform-by-clock by only **0.0992 nats
total**, i.e. **0.0331 nats per goal**. If one *assumes independent*
timing draws after conditioning on exactly three goals, the chance of
at least two in a **predeclared** 60–75 bin was 7.64% (model) versus
7.41% (flat). The real simulator has path dependence, so these simple
multinomial quantities are **diagnostics, not the exact joint-path likelihood**.

Reproducible audit:
[scripts/audit_pl_momentum_timing.py](../scripts/audit_pl_momentum_timing.py) and
[artifacts/validation/liverpool-city-timing-audit.json](../artifacts/validation/liverpool-city-timing-audit.json).

## Historical timing evidence (not FM26 validation)

Using the separately frozen 200-path **observed-stat-only** engine on
StatsBomb 2015/16, 31 earlier fit fixtures and 25 strictly later
held-out fixtures with 68 total recorded goals:

| Metric | Engine | Earlier-only or clock-flat reference |
| --- | ---: | ---: |
| Squared fractional relative-pressure error (148 nonempty windows) | 0.109702 | constant full-match simulated pressure: 0.110122 |
| Direction accuracy (133 non-tied windows) | 54.14% | constant full-match simulated direction: 51.13% |
| Away relative-pressure peak agrees with observed | 12.0% first tie; 14.0% tie-adjusted | uniform peak: 16.67% |
| Conditional goal-window log loss per goal | 1.78517 | flat six-window clock: 1.79176 |
| Same log loss against fit-only league clock | 1.78517 | 1.86524 (fit on only 75 earlier goals) |

Match-cluster bootstrap (2,000 draws, engine minus reference):

- Pressure squared error: **−0.000420**, 95% CI
  [−0.002341, +0.001478].
- Conditional goal timing per goal against flat: **−0.006589**, CI
  [−0.036591, +0.023880].
- Conditional goal timing against fit-only clock prior: **−0.080069**,
  CI [−0.146954, −0.013331]. The fit-only reference has just
  75 goals with a predeclared 3-per-bin symmetric Dirichlet smoothing
  and is a **weak, noisy reference**. Winning against it does not
  establish an advantage over a well-powered league timing prior.

The observable away-pressure peak calculation was corrected to compare
the **same normalized relative pressure definition** on both sides,
not a normalized forecast against a raw observed count difference.
The fit-only clock is constructed exclusively from the 31 earlier
historical fixtures. It cannot read held-out goal labels during fitting.
Sources: [script](../scripts/score_pl_hist_momentum_oos.py) and
[machine report](../artifacts/validation/pl-hist-momentum-oos.json).
This evaluates the historical *mechanism* only: it is not a 2026 FM26
player-model test and its small, repeatedly inspected OOS should not
become a hyperparameter-selection set.

## Same-seed reproduction warning

Using the current locally built executable with the same seed 44 and
72 paths, a replay moved City's relative-pressure peak to **45–60**
instead of the original frozen forecast's 60–75. The original chart
has **not** been replaced. At least the frozen run manifest's
registered-squad fingerprint no longer matches the locally saved
registration artifact; multiple code/build revisions also exist.
The registered-squad JSON mismatch is **not proof of the cause** because
the forecast driver actually reads the dated player CSV. Until the
frozen executable version and its exact input environment are recovered,
do not claim seed-reproducibility across revisions.
The machine report stores the original/replay curves and source hashes.

## Decision

**Preserve the relative-pressure mechanism, not the coincidental
event-specific narrative.** No live model parameter has been changed.
Future experiments should separately validate pre-match relative
pressure, absolute shot/goal timing, and **after-observation**
conditional match state following a real red card. On a new
time-separated dataset, compare these quantities to fit-only clock
priors, team-tempo models, and fixed match-strength controls.
Explicitly test tactical-substitution schedule ablations and red-card
state effects using the same paths/fit protocol; lock all criteria
before evaluating truly new fixtures.
