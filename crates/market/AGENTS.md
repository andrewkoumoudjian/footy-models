# `market` crate

Own market-price primitives: Transfermarkt-to-paid-fee calibration, unified price targets, market indices, transaction/acquisition context, club/route effects, and transfer hazard interfaces.

- Transfermarkt market value and actual fee remain distinct observation kinds.
- Calibrate using the latest market value strictly before each paid-transfer event.
- Use robust fitting and explicit residual uncertainty.
- Hierarchical/local estimates must shrink toward better-supported parents when sample sizes are weak.
- Never encode free transfers as EUR 0 permanent-fee targets.
- Current Transfermarkt value must not leak into the fundamental-value feature vector.
- Buyer/seller/contract effects belong in acquisition-price logic, not silently inside player ability.
- Market indices must be quality-adjusted enough to avoid simply rediscovering changing player composition.
- Transfer-hazard training is time-aware and treats non-transfers as censored/negative opportunity observations according to the chosen formulation.
