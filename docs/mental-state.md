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
       goal / performance / red / injury / sub
                       |
                       v
          recursive transient mental state
        morale / self-belief / motivation /
       role satisfaction / collective belief /
             cohesion / leadership
                       |
                       v
           next segment event processes
                       |
                      ...
                       v
                 final result
```

Stable psychology and transient state are deliberately separate. A poor spell can move morale without rewriting professionalism, leadership, consistency, or pressure resilience.

## Every fixture is `n` full Monte Carlo paths

`simulate_dynamic_mental_match(input, mental_model, transition, paths, seed)` is the preferred mental-state fixture engine.

`paths` means **complete independent match histories**, not repeated draws from a final score distribution. For every path the engine samples its own:

```text
latent kickoff psychology
-> formation / starting XI / roles / minutes
-> player-vs-player graph
-> segment player events
-> team process channels
-> goals / red cards / injuries / substitutions
-> updated player and team mental state
-> rebuilt next-segment graph
-> ...
-> final score
```

No realized lineup, score, incident, or mental state from one path is shared with another. Fixed seed + identical inputs + identical path count is deterministic.

## Stable psychology

`footy-state::PsychologyVector` keeps professionalism, ambition, consistency, pressure, adaptability, leadership, teamwork, and discipline as separate confidence-bearing measures.

`PsychologyTraitScale` maps source scales into centered `[-1, 1]` state. The match engine does not hard-code a Football Manager scale or attribute identity.

Stable traits have two jobs:

1. they can affect metric-specific event distributions directly when calibrated;
2. they govern how transient mental state evolves.

Examples of the second role:

```text
pressure resilience -> response to trailing/high-stakes states
leadership          -> buffers negative collective shocks
teamwork            -> contributes to active-XI cohesion
consistency         -> dampens segment mental-state innovation
professionalism     -> strengthens reversion toward kickoff equilibrium
discipline          -> card-risk mechanism
```

## Form is inter-match, not in-match momentum

Form is not last-five W/D/L and is not simply recent goals.

`FormObservation` stores observed player performance versus the **pre-match expectation** that existed before the historical fixture:

```text
observed player rate
expected pre-match player rate
residual scale
minutes / exposure
observation confidence
```

`build_form_state` estimates an as-of EWMA residual with exposure weighting, shrinkage, and a separate recent-vs-prior trend.

During one simulated fixture, `form` and `form_trend` remain fixed. A good 15-minute spell does **not** mutate the player's inter-match form state. Instead it can change transient self-belief/morale. After the real match is observed, the realized residual can become a new `FormObservation` for the next fixture.

This prevents double counting ability, fixture strength, team quality, and random finishing.

## Pre-match morale and missing information

`MoraleObservation` can represent explicit drivers such as direct signals, performance feedback, selection/role situation, team environment, career security, and social support.

The model does not silently infer morale from a team win/loss. If results are used as morale evidence, that assumption is explicit and calibrated.

For the recursive simulator, every Monte Carlo path samples a **latent kickoff mental state**. Confidence determines how concentrated that draw is around the measured value.

A player with no mental-state record is not treated as neutral with certainty. The path engine samples a zero-centered latent prior using `initial_state_sigma_at_zero_confidence`, after which the player can react to the simulated match like anyone else. This matters for uncertain bench players who may enter later.

## Stochastic XI and psychology

Before the XI is sampled, each path applies that path's sampled mental state to the existing selection primitives in logit space.

```text
historical start probability
+ path-specific motivation / morale / role effects
-> path-specific start probability
-> sampled XI
```

Confirmed starters remain confirmed because confirmed lineup information is represented by lineup locks rather than guessed probabilities.

Substitute selection is also mental-state aware. The kickoff mental effect is applied once; later substitution choices use only the **within-path change since kickoff**, preventing double counting.

## Recursive transition boundary

A segment first generates football outcomes from the current state. At the segment boundary, realized events update mental state before the next segment is simulated.

Conceptually:

```text
MentalState_t
  + goals for / against
  + personal performance residual
  + own or teammate red card
  + own or teammate injury
  + substituted on / off
  + current score pressure
  + active leadership / cohesion
  + stochastic innovation
  + mean reversion
        |
        v
MentalState_t+1
        |
        v
next segment player-event distributions
```

The recursively evolving player dimensions are primarily morale, self-belief, motivation, and role satisfaction. Stable traits and inter-match form are not rewritten by one segment.

Team state evolves separately through collective morale, collective belief, cohesion, and the leadership actually present on the pitch.

## Event shocks are primitives

`DynamicMentalTransitionParameters` exposes separate transition coefficients rather than one momentum number:

- goal-for and goal-against morale/belief shocks;
- personal success/error feedback;
- own and teammate red-card shocks;
- own and teammate injury shocks;
- substituted-off role-satisfaction effects;
- substituted-on activation/motivation;
- trailing-pressure and leading-belief effects;
- leadership and cohesion buffering;
- motivation coupling;
- professionalism-linked mean reversion;
- consistency-linked innovation variance.

These defaults are transparent placeholders for calibration, not claims of universal football effect sizes.

## Personal success/error

The segment engine already samples player metrics. Their reference-normalized deviations are aggregated into a bounded personal performance signal.

Positive residual performance can raise transient self-belief/morale; negative residual performance can lower them. This is deliberately distinct from updating `form`.

When richer event attribution is available, this primitive can be specialized into explicit goal, assist, missed-chance, error-leading-to-shot, penalty, or duel outcomes without changing the state-transition boundary.

## Leadership and cohesion

Negative collective shocks are buffered by leadership and cohesion from the current active lineup. After a red card or substitution, the active XI changes and team leadership/cohesion is reprojected before the next segment.

Thus replacing a high-leadership captain can influence teammates even when the replacement has similar technical ability.

## Pressure and psychology

`MatchMentalContext` separates pressure/stakes, environmental novelty, and hostility.

Pressure resilience therefore changes the response to simulated score pressure rather than becoming a permanent generic performance multiplier. Adaptability remains linked to novelty. Discipline/hostility can affect dismissal risk.

## Metric-specific mental effects

`MentalMetricLoading` and `TeamMentalMetricLoading` map mental dimensions into canonical player-event processes with independent mean and variance coefficients.

For example:

```text
self-belief -> shot/carry mean
consistency -> event variance
collective cohesion -> pressing/progression reliability
pressure resilience x current pressure -> decision-heavy metrics
```

The recursive simulator recomputes these effects from `MentalState_t` before every segment. Mental mean/variance contributions are reported separately from system, fatigue, role, and matchup effects.

## Discipline and incidents

Red-card probability can depend on stable discipline, current morale, and match hostility. Because current morale changes within the path, incident risk can also change between segments.

A simulated dismissal then changes the active lineup, teammate mental state, team cohesion/leadership, player-event processes, causal football channels, and therefore the eventual result.

## Explainability

`DynamicMentalMatchForecast` exposes:

- the normal `PrimitiveMatchForecast` sporting outputs;
- initial and final player morale/self-belief/motivation distributions;
- final team collective state;
- probability a player finishes above kickoff morale;
- mean absolute within-path morale change;
- mean mental event-rate and event-variance adjustments;
- transition-driver decomposition;
- exact Monte Carlo path count.

`DynamicMentalDriverContribution` reports upstream effects per path for drivers such as goals, personal success/error, cards, injuries, substitutions, score pressure, mean reversion, and innovation.

This allows explanations such as:

```text
Path 184:
Team A concedes at 31'
-> collective belief falls
-> X's pressure resilience absorbs part of the shock
-> X self-belief -0.04
-> progressive-carry rate falls in the next segment
-> Team A progression/chance-creation channels weaken

X is substituted at 64'
-> replacement Z enters
-> active leadership changes
-> teammate mental state and matchup graph are recomputed
-> remaining path develops differently
```

Aggregate forecast explanations are expectations over all `n` paths, not a claim that one representative path literally occurred.

## Relationship to the static mental transform

`apply_mental_model` / `simulate_mental_match` remain useful as simpler pre-match-only primitives and calibration/debugging surfaces.

For a match where recursive psychology matters, use `simulate_dynamic_mental_match`. It does **not** first collapse mental uncertainty into the static adjusted input. It validates the same model, then samples mental state inside each Monte Carlo path and carries that state recursively through the fixture.

## Invariants

- no future information enters kickoff state;
- form is updated between matches, not from simulated in-match momentum;
- missing psychology is latent uncertainty, not a zero observation;
- mental shocks are team-scoped and cannot leak to the opponent squad;
- kickoff selection effects are not reapplied during substitution decisions;
- stable psychology is not overwritten by transient events;
- player metrics still reach W/D/L only through the causal football-process graph;
- each fixture uses exactly the requested `n` independent full-match paths;
- fixed seed + identical artifact + identical `n` produces identical output.
