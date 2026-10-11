# Liverpool–Manchester City 0–3: pre-match momentum audit

Research date: 2026-10-11. **Conclusion:** an interesting pre-match
*relative-pressure* signal coincided with the decisive sending-off and two
City goals, but the model did not predict that red card specifically,
the absolute goal-rate peak was elsewhere, and a historical OOS audit
has not established reliable time-of-goal calibration.

## Pre-match snapshot and verified match

The saved forecast in
artifacts/premier-league-2026-10-11/player_paths.json has
as_of=2026-10-10, 72 paths and fixture seed 44. This preceded the
October 11 result. The pressure figure was fixed as:

    team_pressure = expected shots + 3 * expected goals
    signed_momentum = 100 * (home_pressure - away_pressure) /
                       (home_pressure + away_pressure)

Negative means City had more relative simulated pressure. This is
not observed possession, tracking data, or a calibrated probability
of scoring the next goal.

The [official Manchester City match report](https://www.mancity.com/news/mens/liverpool-v-city-premier-league-match-report-october-2026-63927325)
records goals at 65' (Haaland), 68' (Doku) and 78' (Cherki).
[Reuters](https://www.reuters.com/sports/soccer/man-city-silence-hostile-anfield-with-3-0-win-over-liverpool-2026-10-11/)
reports Gravenberch's sending-off at 62'. These are post-match
evaluation labels, preserved separately from pre-match inputs in
artifacts/validation/liverpool-city-actual-2026-10-11.json.

| Minute bin | Liverpool-minus-City relative momentum | Simulated City goals | Realized key event |
| --- | ---: | ---: | --- |
| 00–15 | +8.19 | 0.167 | — |
| 15–30 | +7.11 | 0.194 | — |
| 30–45 | +0.70 | 0.333 | — |
| 45–60 | -3.04 | **0.375** | — |
| 60–75 | **-13.15** | 0.278 | red 62', City goals 65' and 68' |
| 75–90 | +3.55 | 0.292 | City goal 78' |

City's peak **relative pressure** was 60–75, and this remained the
strongest relative-pressure bin when the arbitrary goal multiplier
was changed to 0, 1, 3, 5 or 10. City's greatest *absolute* predicted
goal intensity was instead **45–60**. The strongest relative-pressure
window was driven primarily by the shot-volume swing: comparing 45–60
with 60–75, simulated Liverpool shots fall **2.431 to 1.806** (−0.625),
while City shots rise **2.167 to 2.333** (+0.167). The expected City
goals simultaneously fall **0.375 to 0.278**. This should not be read
as the highest probability of a City goal.

The 45–60 to 60–75 shift is mainly a simulated decline in
**Liverpool's** shot pressure: Liverpool expected shots drop from
2.431 to 1.806 (-0.625), while City's rise only 2.167 to 2.333
(+0.167). City's expected goals actually fall from 0.375 to
0.278 (-0.097). The ratio peaks because Liverpool weakens
relative to City, not because City's goal rate suddenly spikes.

In the pre-match six-bin distribution, 60–75 accounted for **16.95%**
of City's expected goals, barely above a 16.67% uniform clock.
Conditioned on City scoring three, at least two falling in that
pre-selected window had probability 7.64% versus 7.41% for uniform.
The full observed 65', 68', 78' goal-time log likelihood gains just
0.0992 nats in total, or **0.0331 nats per goal**, versus uniform timing.
These conditional multinomial arrival scores are diagnostics, not
the exact joint likelihood from state-dependent simulated paths.

The engine reported only **4.17% any-team red-card incidence**
for 60–75, not a specific Liverpool or Gravenberch red-card
forecast. By contrast its tactical substitution-trigger incidence
was 100% in that bin. The one-second simulator does remove a
sent-off player and recompute the available lineup and mental
hazards, creating a plausible causal explanation of a *conditional*
collapse, but it cannot know which real-life card will happen
before kickoff.

## Source and replay reproducibility

A replay with the same seed and 72 paths using the currently available
release binary instead peaked in City's favour at 45–60.
This does not prove that a deterministic RNG failed:
the saved archive was generated around 00:46 local time, whereas the
available release executable was rebuilt around 01:15, and the original
snapshot does not embed an exact executable SHA256.
The registered-squad input fingerprint in its contemporaneous run
manifest also **differs from the currently on-disk audit file**,
although the dated-team results, player inputs, team baseline and
raw FM26 source hashes still agree.
The registered-squad audit mismatch does not itself establish
the cause of the replay difference: the match driver actually
reads the dated player CSV. An unverified code/build revision
remains a separate explanation. Do not attribute the drift to
one specific mismatched file.

This is a **provenance and input/build-matching failure**. It would
be scientifically incorrect to call it pure Monte Carlo seed
instability. Future releases must freeze the binary hash and *every*
input byte along with the exact parameter configuration before
seed-stability analyses.

As a separate reproducibility sanity check, **two current-binary
eight-path runs**, with identical input files, fixture filter and seed
44, produced **byte-identical JSON output** (SHA256
435d70890e8656da5cdf3137e8ca726238879d25c727937bd9768c5282a0028b).
Those eight-path runs are *not* estimates of timing precision or an
equivalent reproduction of the historical 72-path build; they show
only that the current executable behaves deterministically at that
small, matched setting.

Run the locked audit, which also stores source fingerprints:

    python3 scripts/audit_pl_momentum_timing.py

To include the non-identical-build local replay:

    python3 scripts/audit_pl_momentum_timing.py --reproduction data/ml/liverpool-repro-seed42.json

Its recorded evidence is in
artifacts/validation/liverpool-city-timing-audit.json.

## Separate 2015–16 historical validation

A new same-engine chronological audit uses the *already frozen*
200-path, observed-stat-only 2015/16 StatsBomb forecasts. Its
31 earlier fixtures are isolated from the **25 OOS fixtures
starting 2016-02-13**, containing 148 scoreable windows and
68 observed goals. StatsBomb observed shot/goal events are used for
evaluation only.

| Measure (lower loss is better) | Frozen engine | Reference |
| --- | ---: | ---: |
| Window pressure-fraction MSE | 0.109702 | 0.110122, constant full-match forecast share |
| Window stronger attacking side correct | 54.1% | 51.1%, constant full-match side |
| Goal-time NLL per observed goal | 1.78517 | 1.79176, uniform six-bin clock |
| Goal-time NLL per observed goal | 1.78517 | 1.86524, prior-fit goal-clock distribution |
| Correct away-pressure peak, fractionally adjusting ties | 14.0% | 16.7%, random window |

Paired match-bootstrap 95% intervals for engine-minus-comparator:

- Window pressure MSE versus constant same-match strength:
  -0.000420; CI **[-0.002341, +0.001478]**.
- Goal-time negative log loss versus uniform clock, per goal:
  -0.006589; CI **[-0.036591, +0.023880]**.
- Goal-time negative log loss versus prior-fit clock, per goal:
  -0.080069; CI **[-0.146954, -0.013331]**.

The prior-fit clock was built from 75 goals across just 31 *earlier*
fixtures, with a fixed symmetric Dirichlet pseudocount of three
per bin. It happens to generalize worse than a uniform clock.
Beating it significantly does **not** establish an improvement
over the simpler, stronger uniform benchmark. The engine's tiny
pressure and uniform-timing gains are within error.

Reproduction:

    python3 scripts/score_pl_hist_momentum_oos.py

The full audit, bootstrap results and OOS fixture-level evidence
are at artifacts/validation/pl-hist-momentum-oos.json.

## What to do next

1. Preserve the binary SHA256, model code SHA256, all input hashes,
   versioned features and RNG seed with each immutable pre-match run.
   Do not select future model parameters from the already revealed
   Liverpool fixture.
2. Use earlier-season matched event data to separately evaluate the
   timing of shots, goals, red cards, substitutions and shifts in
   *relative* pressure. Compare with constant-rate and causally
   fit-only temporal baselines on the exact same held-out fixtures.
3. Study numerical advantage conditional on simulated events,
   keeping future incident outcomes out of kickoff features.
   Test whether removal of a player and recalculation of in-match
   strengths correctly modifies subsequent scoring hazards.
4. Expand historical samples through verified FBref/soccerdata
   player-match shots and minutes plus old FM editions. Require
   trustworthy historical player identity joins.
5. Only train Candle set-attention models on genuinely as-of
   *candidate* rosters, after lower-capacity GLM/MLP baselines.
   Demand proper-score improvement with uncertainty on fully
   unrevealed later matches before changing production priors.

This work introduces audit harnesses and no automatic simulator
parameter changes or post-hoc refits.
