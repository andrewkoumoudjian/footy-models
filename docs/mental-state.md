# Mental state, morale, form, and psychology

Mental effects are causal state primitives inside the same projected-XI -> player-event -> team-channel -> result graph. There is no separate morale result model and no opaque mentality score.

## State hierarchy

```text
stable traits                         inter-match dynamic state
professionalism                      residual form
ambition                             form trend
consistency                          pre-match morale
pressure resilience                  self-belief
adaptability                         role satisfaction
leadership                           motivation
teamwork
 discipline
      \                                  /
       +---------- kickoff mental state -+
                       |
                       +--> stochastic XI / substitute probabilities
                       +--> player-event mean and variance
                       +--> discipline / red-card risk
                       |
                       v
            one Monte Carlo match path
                       |
            one-second event clock
                       |
       goal / performance / red / injury / sub
                       |
                       v
          recursive transient mental state
        morale / self-belief / motivation /
       role satisfaction / collective belief /
             cohesion / leadership
                       |
                       v
        subsequent player/event hazards
                       |
                      ...
                       v
                 final result
```

Stable psychology and transient state are deliberately separate. A poor spell can move morale without rewriting professionalism, leadership, consistency, or pressure resilience.

## Every fixture is `n` full Monte Carlo paths

`simulate_second_resolved_dynamic_mental_match(input, mental_model, transition, parameters, paths, seed)` is the preferred production fixture engine.

`paths=n` means **exactly n complete independent match histories**, not repeated draws from one lineup, one mental trajectory, or one final score distribution. Every path independently samples kickoff psychology, formation/starting XI/roles/minutes, player-vs-player graph, latent player rates, match-level goal-rate uncertainty, player events, goals/cards/injuries/substitutions, recursively updated mental state, and final score.

No realized lineup, score, event ordering, incident, or mental state from one path is shared with another. Fixed seed + identical inputs + identical path count is deterministic.

## Why the clock is one-second resolved

The old recursive engine operated on coarse segment boundaries. That creates an artificial ambiguity when a goal, dismissal, injury, or substitution occurs inside the same segment.

The production engine now uses an integer match-second clock. Within each integer second it uses a continuous competing-hazard clock:

```text
72:14.000 current state
    |
    +--> calculate current hazards / player rates
    |
72:14.312 event A fires
    |
    +--> mutate score / active XI / mental state
    +--> invalidate old hazards
    +--> rebuild rates for remaining 0.688 s
    |
72:14.771 event B may fire from new state
    |
    +--> mutate again
    |
72:15.000 next integer second
```

The latent sub-second ordering is a simulation device, not a claim that source data observed a real sub-second timestamp. It exists so two events sharing one reported second are not resolved simultaneously.

Scheduled substitutions receive their latent within-second ordering **once** for that path/second. A prior red card, injury, or other event may invalidate the scheduled substitution, but cannot redraw its time.

After every state-changing event, including one earlier in the same second, the engine rebuilds the relevant active lineup, mental state, event rates, football-process channels, goal intensity, card/injury risk, and substitution eligibility before continuing.

## Stable psychology

`footy-state::PsychologyVector` keeps professionalism, ambition, consistency, pressure, adaptability, leadership, teamwork, and discipline as separate confidence-bearing measures.

`PsychologyTraitScale` maps source scales into centered `[-1, 1]` state. The match engine does not hard-code a Football Manager scale or attribute identity.

Stable traits can affect metric-specific event distributions when calibrated and can govern transient-state transitions. Pressure resilience controls response to trailing/high-stakes states; leadership buffers collective shocks; teamwork contributes to active-XI cohesion; consistency dampens mental-state innovation; professionalism strengthens mean reversion toward kickoff equilibrium; discipline contributes to dismissal risk.

## Form is inter-match, not in-match momentum

Form is not last-five W/D/L and is not simply recent goals.

`FormObservation` stores observed player performance versus the **pre-match expectation** that existed before the historical fixture: observed rate, expected rate, residual scale, minutes/exposure, and observation confidence.

`build_form_state` estimates an as-of EWMA residual with exposure weighting, shrinkage, and a separate recent-vs-prior trend.

During one simulated fixture, `form` and `form_trend` remain fixed. A good second, minute, or spell does **not** mutate the player's inter-match form state. Instead realized performance can change transient self-belief/morale. After a real match is observed, the match residual can become a new `FormObservation` for the next fixture.

## Pre-match morale and missing information

`MoraleObservation` can represent explicit drivers such as direct signals, performance feedback, selection/role situation, team environment, career security, and social support. The model does not silently infer morale from a team win/loss.

Every Monte Carlo path samples a **latent kickoff mental state**. Confidence controls how concentrated that path draw is around the measured value.

A player with no mental-state record is not treated as neutral with certainty. The engine samples a zero-centered latent prior using `initial_state_sigma_at_zero_confidence`, after which the player can react to the simulated match like anyone else. This includes bench players who may later enter.

## Stochastic XI and psychology

Before the XI is sampled, each path applies that path's kickoff mental state to selection primitives in logit space.

```text
historical start probability
+ path-specific motivation / morale / role effects
-> path-specific start probability
-> sampled XI
```

Confirmed starters remain confirmed through lineup locks. Exact probability boundaries remain exact: true 0 stays 0 and true 1 stays 1.

Substitute selection is also mental-state aware. The kickoff mental effect is applied once. Later choices use only the **within-path mental-state change since kickoff**, preventing double counting.

## Epistemic uncertainty is path-level

One-second resolution must not convert epistemic uncertainty into white noise.

For every player/metric/path, the engine draws the latent production rate **once** from the pre-match epistemic distribution and holds that latent rate for the path. Match-level goal-rate uncertainty is also sampled once per team/path.

Sub-second and one-second player-stat increments are then aleatoric realizations conditional on the current path state.

This distinction matters:

```text
WRONG
sample player quality again every second
-> epistemic uncertainty averages away / becomes white noise

RIGHT
sample latent quality once per path
-> current role/matchup/mental state modifies that latent rate
-> event increments are sampled conditionally through time
```

## Player-stat increments and clock invariance

Count metrics are modeled as Poisson increments.

Continuous non-negative metrics use a gamma-process increment parameterized so both mean and variance scale linearly with elapsed time. Therefore moving from a 15-minute clock to a one-second clock does not artificially collapse full-match variance or explode it through repeated full-variance draws.

Personal success/error feedback is based on realized-versus-expected increments, standardized by interval variance and then exposure-scaled. An ordinary pass in one second therefore cannot create the same psychological shock as an entire exceptional 15-minute spell.

## Recursive mental transition

Between state-changing events, time-continuous mental mechanisms advance over the actual elapsed fraction of a second.

```text
MentalState_t
  + personal realized-vs-expected performance
  + current score pressure * elapsed time
  + mean reversion over elapsed time
  + stochastic innovation scaled by sqrt(elapsed time)
        |
        v
MentalState_t+dt
```

State-changing events then apply discrete shocks:

```text
+ goal for / against
+ own or teammate red card
+ own or teammate injury
+ substituted on / off
+ active leadership / cohesion change
```

The next hazard/rate calculation uses the updated state immediately.

The recursively evolving player dimensions are primarily morale, self-belief, motivation, and role satisfaction. Stable traits and inter-match form are not rewritten by match events. Team state evolves separately through collective morale, collective belief, cohesion, and leadership actually present on the pitch.

## Time scaling of psychology

The original transition coefficients were interpreted over a reference 15-minute horizon. Applying those coefficients unchanged every second would create catastrophic overreaction.

The one-second engine rescales them by mechanism:

- pressure drift scales linearly with elapsed time;
- mean-reversion and active-lineup blending fractions scale exponentially with elapsed time so the reference-horizon effect is preserved;
- innovation sigma scales with `sqrt(time)`;
- event shocks such as goals/cards/injuries remain discrete event effects and are not divided by 900.

## Incident hazards

Per-90 incident probabilities are converted to continuous hazard intensities:

```text
lambda_second = -ln(1 - p_90) / 5400
```

This preserves the intended 90-minute survival probability on a second-resolution event clock.

An input stochastic incident probability of exactly `1.0` is rejected by the preferred public API because it implies infinite continuous hazard. A truly certain event belongs in a deterministic/scheduled-event primitive instead.

Current morale, discipline, and hostility can modify dismissal intensity. After a dismissal or injury, the active graph changes immediately and subsequent hazards are rebuilt.

## Same-second collisions

A path can contain multiple state-changing events in one integer second. The forecast exposes `same_second_collision_probability`, and state events are applied sequentially according to latent within-second order.

This explicitly handles cases such as:

- goal then red card in the same reported second;
- red card then goal;
- injury followed by forced substitution;
- scheduled substitution invalidated because the outgoing player was dismissed/injured first;
- two incidents affecting the active graph before the next integer second.

The engine has a configurable safety cap on state-changing events per second to prevent pathological calibration from creating an infinite/cascading loop.

## Leadership and cohesion

Team mental state follows the current active XI. When a captain is dismissed or substituted, active leadership/cohesion changes immediately and can alter subsequent team/player response.

This allows a replacement with similar technical ability but different leadership/teamwork state to change the path through social-state channels.

## Relationship to simpler engines

`apply_mental_model` / `simulate_mental_match` remain useful pre-match-only calibration/debugging surfaces.

`simulate_dynamic_mental_match` remains available as a coarse segment-based recursive engine for regression/calibration comparisons.

For production fixture forecasting where event ordering matters, use `simulate_second_resolved_dynamic_mental_match`.

## Output

`SecondResolvedDynamicMentalForecast` contains:

- W/D/L probabilities, goal distributions, expected points, and exact path count;
- stochastic projected starting-XI and formation distributions;
- player-stat distributions from the same second-resolved paths;
- initial/final player morale, self-belief, motivation, and mean absolute morale movement;
- mean state-event counts per path;
- same-second collision probability;
- exact simulated match-clock seconds.

Aggregate outputs are expectations/distributions across all `n` complete paths, not a claim that one representative path occurred.

## Invariants

- no future information enters kickoff state;
- one fixture request with `n` paths produces exactly `n` complete match histories;
- unknown XI/roles/mental state remain stochastic path variables;
- form is updated between matches, not from simulated in-match momentum;
- missing psychology is latent uncertainty, not a zero observation;
- latent player/goal epistemic rates are sampled once per path, not every second;
- continuous-stat mean/variance are invariant to numerical clock refinement;
- state-changing events inside the same second are ordered and applied sequentially;
- hazards are recomputed after every state-changing event;
- mental shocks are team-scoped and cannot leak to the opponent squad;
- kickoff selection effects are not reapplied during substitution decisions;
- exact 0/1 selection boundaries remain exact;
- stable psychology is not overwritten by transient events;
- player metrics still reach W/D/L only through the causal football-process graph;
- fixed seed + identical artifact + identical `n` produces identical output.
