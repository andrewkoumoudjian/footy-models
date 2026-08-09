# `forecast` crate

Own future player-state/value evolution, stochastic processes, market paths, availability/contract/transfer simulation, and Monte Carlo composition.

- Simulate causal drivers before price: development -> usage/availability -> performance -> contract/market -> transfer -> price.
- Use deterministic reproducible PRNG seeds in tests and stored research runs.
- Report convergence diagnostics or effective simulation error before increasing path counts.
- Keep epistemic, aleatoric, and market uncertainty conceptually distinct where the model supports it.
- Do not apply GARCH/MF2-GARCH directly to individual player values by default. Introduce volatility models first on sufficiently dense market-factor/index series only when diagnostics justify them.
- Stochastic components should implement small traits and be swappable for backtests.
