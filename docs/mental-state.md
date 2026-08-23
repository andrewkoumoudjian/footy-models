# Mental state, morale, form, and psychology

Mental effects are modeled as causal state primitives upstream of the same projected-XI -> player-event -> team-channel -> result graph. There is no separate "morale result model" and no generic hidden mentality score.

## State hierarchy

```text
stable traits                         dynamic evidence
professionalism                      residual player form
ambition                             morale signals
consistency                          self-belief/confidence
pressure resilience                  role satisfaction
adaptability                         motivation
leadership
teamwork
 discipline
      \                                  /
       +---------- player mental state -+
                       |
                       +--> stochastic selection / substitute probability
                       +--> metric-specific event-rate mean
                       +--> metric-specific event variance
                       +--> epistemic uncertainty
                       +--> discipline / red-card risk
                       |
projected XI ---------+--> projected team morale / belief / cohesion / leadership
                       |
match pressure / novelty / hostility
                       |
                       v
             same specific-match graph
```

Stable psychology and dynamic state are deliberately separate. A poor month can reduce morale or self-belief without rewriting professionalism, leadership, or pressure handling.

## Stable psychology

The existing `footy-state::PsychologyVector` already keeps professionalism, ambition, consistency, pressure, adaptability, leadership, teamwork, and discipline as separate confidence-bearing measures.

`PsychologyTraitScale` maps the source scale into centered `[-1, 1]` state. The match crate does not hard-code Football Manager's numeric range; the caller supplies the center and half-range belonging to the versioned source schema.

Low-confidence hidden/personality information has proportionally less forecast effect.

## Form

Form is not last-five W/D/L and is not simply recent goals.

`FormObservation` stores:

```text
observed player rate
expected pre-match player rate
residual scale
minutes/exposure
observation confidence
```

`build_form_state` uses the causal residual

```text
(observed - expected) / residual_scale
```

with an exponentially weighted as-of window, exposure weighting, explicit shrinkage toward neutral, and a separate recent-vs-prior trend.

This is designed to avoid double counting fixture strength, team quality, finishing luck, and the same performance history already used to estimate the player's latent rate.

## Morale

Morale is evidence driven. `MoraleObservation` can represent explicitly identified drivers:

- direct morale signal;
- performance feedback;
- selection / role situation;
- team environment;
- career / contract security;
- social / leadership support.

The model does **not** silently infer morale from a win or loss. If team results are used as a morale signal, that assumption enters explicitly through a driver observation and a calibrated weight.

`build_morale_state` applies causal time decay, source confidence, driver weights, and prior shrinkage.

## Motivation and self-belief

Motivation remains distinct from morale. `derive_motivation` combines explicit ambition, morale, role satisfaction, and residual form using caller-supplied weights. Self-belief is a separate dynamic measure so confidence effects can be calibrated independently from happiness/morale.

## Team social state

`TeamMentalState` contains:

- collective morale;
- collective belief;
- cohesion;
- leadership.

`project_team_mental_state` weights player state by projected start probability. A direct cohesion estimate can replace the default teamwork + role-satisfaction proxy when stronger data exists.

This keeps social effects connected to the players expected to be on the pitch instead of attaching one static club-level morale number to every lineup.

## Match context

`MatchMentalContext` separates:

- pressure intensity / stakes;
- environmental novelty, which scales adaptability effects;
- hostility / emotional temperature, which can enter discipline risk.

Pressure resilience therefore matters more in a high-pressure match only when the metric-specific loading says it should.

## Metric-specific effects

`MentalMetricLoading` maps one player mental dimension into one canonical event metric with two distinct coefficients:

```text
mean_log_weight
log_variance_weight
```

For example, a calibrated consistency signal can reduce event variance without being forced to increase the mean. Pressure handling can affect decision-heavy metrics while having no direct loading on unrelated metrics.

`TeamMentalMetricLoading` does the same for collective morale, belief, cohesion, and leadership.

Every loading remains metric specific and versioned/calibrated by the caller; the engine contains no universal claim such as "high morale adds 5% to all stats."

## Selection

`MentalSelectionLoading` adjusts start and substitute probabilities in logit space before the XI is sampled.

Thus mental state can influence the projected XI without bypassing the stochastic lineup process:

```text
causal historical selection probability
      + explicit mental selection effect
      -> adjusted start probability
      -> Monte Carlo XI sampling
```

Impossible/known probabilities remain fixed: a probability of exactly 0 remains 0 and exactly 1 remains 1.

## Uncertainty

Mental-state confidence is part of the model. A loading is multiplied by the confidence of the underlying measure. The unobserved portion of a loaded mental signal increases player epistemic rate uncertainty.

If an entire player mental state is unavailable, `missing_state_relative_sigma` adds uncertainty rather than synthesizing average morale/psychology.

This preserves the repository invariant: missing is not zero.

## Discipline

Red-card risk can be adjusted through explicit, separately calibrated log-risk terms for discipline, low morale, and match hostility. The signs are not hard-coded because the calibration layer owns empirical direction and magnitude.

## Integration

`apply_mental_model` clones a `PrimitiveMatchSimulationInput` and modifies only upstream primitives:

- lineup start/substitute probabilities;
- player event-rate means;
- player event-rate variances;
- player epistemic uncertainty;
- red-card incident multipliers.

It returns the transformed input plus per-player/per-metric attribution records.

`simulate_mental_match` then calls the existing `simulate_primitive_match`. Therefore morale/form/psychology alter the same lineup, matchup, player-event, causal-channel, substitution, incident, and goal process rather than creating an independent result forecast.

Current score-state behavior remains endogenous through the specific-match engine's trailing/leading metric modifiers. A future refinement can make those modifiers player/team-mental-state specific inside each segment; that should extend the existing game-state primitive rather than introduce a second psychology simulator.
