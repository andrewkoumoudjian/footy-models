# `performance` crate

Own canonical metrics, match/period aggregation, rolling windows, per-90/EWMA/trend features, coverage-aware normalization, peer groups, and competition-strength translation.

- Every rolling feature is causal and parameterized by an `as_of` date.
- Minutes/sample size and coverage accompany performance estimates.
- Never normalize players across incompatible roles without an explicit peer-group definition.
- Prefer robust statistics (median/MAD/quantiles) for heavy-tailed football metrics.
- League/competition translation factors must be fit only from data available before the target prediction date during backtests.
- Keep raw observations available; derived windows are reproducible artifacts, not destructive replacements.
