# `recruitment` crate

Own decision-layer primitives: undervaluation, analogues, tactical fit, squad complementarity, environment fit, total acquisition cost composition, candidate analysis, squad state, and optimizer interfaces.

- Do not hide all reasoning behind a single opaque score. Preserve market, fundamental, acquisition, future value, fit, cost, and risk components.
- Define alpha relative to realistic acquisition and carrying costs, not only Transfermarkt value.
- Historical analogues must compare only information available at the historical analogue date.
- Tactical fit, squad complementarity, and environmental fit are separate outputs.
- Squad value is not a sum of FM attributes. Model expected minutes, replacement level, role coverage, availability, and interaction effects.
- Optimizers must respect explicit budget/squad constraints and expose the trade-offs/Pareto frontier where practical.
