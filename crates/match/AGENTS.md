# `match` crate

Own the source-neutral causal graph for one specific fixture.

- Build the match from player primitives upward: selection/availability -> formation/roles -> pairwise interactions -> player event rates -> team attack/defence -> score state -> result.
- If the starting XI is unknown, it is a Monte Carlo path variable. Never replace lineup uncertainty with one deterministic guessed XI.
- Confirmed information should collapse only the relevant uncertainty: partial lineups remain partially stochastic; confirmed starters with unknown roles still receive stochastic role assignment.
- Historical selection and formation calibration must be strictly as-of. Team fixtures with no player appearance are evidence of non-selection; do not manufacture fake player-stat zeros.
- Keep provider schemas outside this crate. Inputs are canonical player/club/metric/role IDs and source-neutral state.
- Pairwise matchup rules are metric-specific and composable. Do not hard-code winger-vs-fullback or formation-specific logic into the simulator.
- Teammate synergy, opponent matchup, role fit, fatigue, tactical/system effects, environment, and latent player quality remain separate causal factors so forecasts can expose attribution.
- Match results must emerge after sampled player/team state. Do not short-circuit the graph with a second independent result model.
- Use the repository's deterministic `Rng64`; fixed seeds must reproduce lineup, player-stat, and result distributions.
- Missing data is uncertainty or absence of a factor, never an implicit zero observation.
