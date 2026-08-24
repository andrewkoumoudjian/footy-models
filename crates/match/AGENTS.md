# `match` crate

Own the source-neutral causal graph for one specific fixture.

- Build the match from player primitives upward: selection/availability -> formation/roles -> pairwise interactions -> player event rates -> causal team-process channels -> score state -> result.
- A match forecast is `n` independent **full-match Monte Carlo paths**. Do not sample one lineup/state and reuse it across paths. Fixed seed + identical inputs + identical `n` must reproduce the same distributions.
- The preferred production fixture engine is `simulate_second_resolved_dynamic_mental_match`. Match state is resolved on an integer one-second clock, with a continuous latent ordering inside each second for collisions. Do not use coarse segment boundaries when event order can affect downstream state.
- If two or more state-changing events occur in the same integer second, order them inside that second, apply the first event, invalidate/recompute the remaining hazards, then continue through the unconsumed fraction of that second. Never resolve a goal/card/injury/substitution bundle simultaneously merely because timestamps share a second.
- One-second resolution does **not** mean resampling epistemic player quality 5,400 times. Sample latent player/metric rates once per player/metric/path; one-second or sub-second increments are aleatoric draws conditional on that path state.
- Count metrics are Poisson increments. Continuous non-negative metrics must use an independent-increment process whose variance scales linearly with elapsed time; clock refinement must not collapse or explode full-match variance.
- Per-90 incident probabilities must be transformed to continuous hazard intensities before use on the event clock. An exact probability of 1 is a deterministic/scheduled-event primitive, not an infinite stochastic hazard.
- Time-continuous psychology coefficients must be rescaled when advancing less than their calibration horizon: mean reversion/blending scale exponentially with elapsed time; innovations scale with sqrt(time); score-pressure drift scales linearly with time. Never apply a 15-minute coefficient once per second unchanged.
- Scheduled substitutions receive one stable latent sub-second ordering for that path/second. A prior event in the same second may invalidate the substitution, but must not cause its event time to be redrawn.
- If the starting XI is unknown, it is a Monte Carlo path variable. Never replace lineup uncertainty with one deterministic guessed XI.
- Confirmed information should collapse only the relevant uncertainty: partial lineups remain partially stochastic; confirmed starters with unknown roles still receive stochastic role assignment.
- Historical selection and formation calibration must be strictly as-of. Team fixtures with no player appearance are evidence of non-selection; do not manufacture fake player-stat zeros.
- Keep provider schemas outside this crate. Inputs are canonical player/club/metric/role/attribute IDs and source-neutral state.
- Raw FM `AttributeId` values may drive exact matchup primitives (pace-vs-recovery, dribbling-vs-tackling, etc.). The aggregate ability vector is a fallback/prior, not a replacement for calibrated fine-grained interactions.
- Pairwise matchup rules are metric-specific and composable. Do not hard-code winger-vs-fullback or formation-specific logic into the simulator.
- Player metrics never affect the score merely because they exist. Result influence must flow through the acyclic `CausalChannelGraph`; metrics with no loading can still be forecast without affecting W/D/L.
- Stable psychology traits and dynamic mental state are different primitives. Professionalism/pressure/consistency/leadership/teamwork/discipline must not be overwritten by short-term morale or form.
- Define form as a causal residual versus pre-match expected player performance, with as-of decay and shrinkage. Form is an inter-match state: do not mutate it inside a simulated fixture because of one good/bad second.
- Morale must be evidence-driven with explicit drivers/weights. Do not silently infer morale from a team win/loss.
- Missing psychology is latent uncertainty. In recursive simulation, sample a zero-centered prior for an unknown player rather than treating missing state as neutral with certainty.
- Mental effects are metric-specific and may affect mean, variance, selection, uncertainty, or incident risk independently. Never apply an uncalibrated universal morale multiplier to all statistics.
- Recursive mental state is path-local. Goals, performance residuals, cards, injuries, substitutions, score pressure, leadership/cohesion, innovation, and reversion update the current path state before subsequent event hazards/player processes are evaluated. Never share a mental realization between Monte Carlo paths.
- Team mental shocks are team-scoped. They may affect active and bench players on that squad, but must never leak into the opponent squad.
- Kickoff mental effects on selection are applied once. Later substitution decisions may use the within-path state **change since kickoff**; do not reapply the full kickoff effect and double count psychology.
- Preserve exact selection-probability boundaries. Mental logit adjustments must never turn a true 0 into a tiny possibility or a true 1 into less than certainty.
- Team cohesion/leadership/morale should follow the currently active players. Reproject active social state after red cards and substitutions.
- Low-confidence psychology must widen the path distribution and/or reduce effect size; it must not create false certainty.
- Teammate synergy, own attributes, opponent matchup, role fit, fatigue, morale/form/psychology, tactical/system effects, environment, score state, injuries, red cards, and substitutions remain separate causal factors so forecasts can expose attribution.
- Recompute state-dependent rates after every state-changing event, including another event in the same integer second. Do not compute a full-match player box score first and bolt game state onto the result.
- Match results must emerge after sampled player/team state. Do not short-circuit the graph with a second independent result model.
- Counterfactuals must intervene on upstream primitives and re-run the graph with common random numbers; never subtract a static player rating from a result probability.
- Use the repository's deterministic `Rng64`; fixed seeds must reproduce lineup, player-stat, mental-state, event ordering, and result distributions.
- Missing data is uncertainty or absence of a factor, never an implicit zero observation. Calibrators must supply priors for required player event processes before simulation.
