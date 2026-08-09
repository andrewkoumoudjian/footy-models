# `models` crate

Own generic dependency-light statistical algorithms, not football-source business logic.

Initial priorities:

1. robust log-linear regression / Huber IRLS;
2. small dense linear algebra needed by fitting;
3. quantiles and robust residual statistics;
4. conformal/interval calibration;
5. optional compact nonlinear basis support;
6. only later, a small internal histogram/tree learner if backtests justify it.

Rules:

- No dependency on Transfermarkt/FBref parser types.
- Fitting uses `f64` where accumulated precision matters; scoring may use `f32` artifacts when validated.
- Deterministic seeds and stable feature ordering are mandatory.
- Every model artifact records training cutoff, feature-set version, parameters, and calibration version.
- Do not optimize headline R² at the expense of calibration and subgroup failure analysis.
- Avoid heavyweight ML frameworks unless a measured, documented requirement cannot be met internally.
