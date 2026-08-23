# `match` crate

Own the source-neutral causal graph for one specific fixture.

- Build the match from player primitives upward: selection/availability -> formation/roles -> pairwise interactions -> player event rates -> causal team-process channels -> score state -> result.
- If the starting XI is unknown, it is a Monte Carlo path variable. Never replace lineup uncertainty with one deterministic guessed XI.
- Confirmed information should collapse only the relevant uncertainty: partial lineups remain partially stochastic; confirmed starters with unknown roles still receive stochastic role assignment.
- Historical selection and formation calibration must be strictly as-of. Team fixtures with no player appearance are evidence of non-selection; do not manufacture fake player-stat zeros.
- Keep provider schemas outside this crate. Inputs are canonical player/club/metric/role/attribute IDs and source-neutral state.
- Raw FM `AttributeId` values may drive exact matchup primitives (pace-vs-recovery, dribbling-vs-tackling, etc.). The aggregate ability vector is a fallback/prior, not a replacement for calibrated fine-grained interactions.
- Pairwise matchup rules are metric-specific and composable. Do not hard-code winger-vs-fullback or formation-specific logic into the simulator.
- Player metrics never affect the score merely because they exist. Result influence must flow through the acyclic `CausalChannelGraph`; metrics with no loading can still be forecast without affecting W/D/L.
- Stable psychology traits and dynamic mental state are different primitives. Professionalism/pressure/consistency/leadership/teamwork/discipline must not be overwritten by short-term morale or form.
- Define form as a causal residual versus pre-match expected player performance, with as-of decay and shrinkage. Do not use raw recent goals, ratings, or team W/D/L as an uncorrected form factor.
- Morale must be evidence-driven with explicit drivers/weights. Do not silently infer morale from a team win/loss or turn missing morale into neutral morale.
- Mental effects are metric-specific and may affect mean, variance, selection, uncertainty, or incident risk independently. Never apply an uncalibrated universal "morale multiplier" to all statistics.
- Team cohesion/leadership/morale should be tied to the projected players expected to participate, rather than one static club number detached from lineup uncertainty.
- Low-confidence or missing psychological state must reduce effect size and/or increase epistemic uncertainty; it must not create false certainty.
- Teammate synergy, own attributes, opponent matchup, role fit, fatigue, morale/form/psychology, tactical/system effects, environment, score state, injuries, red cards, and substitutions remain separate causal factors so forecasts can expose attribution.
- Recompute active lineups, player interactions, event rates, channels, and scoring intensity after game-state transitions. Do not compute a full-match player box score first and then bolt game state onto the result.
- Match results must emerge after sampled player/team state. Do not short-circuit the graph with a second independent result model.
- Counterfactuals must intervene on upstream primitives and re-run the graph with common random numbers; never subtract a static player rating from a result probability.
- Use the repository's deterministic `Rng64`; fixed seeds must reproduce lineup, player-stat, and result distributions.
- Missing data is uncertainty or absence of a factor, never an implicit zero observation. Calibrators must supply priors for required player event processes before simulation.
