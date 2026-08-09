# `source-fbref` crate

Own permitted/authorized FBref/Sports Reference import and normalization only.

- Treat source metric availability as time/competition dependent. Missing metrics are not zero.
- Prefer generic `MetricId -> value` records over baking the current webpage schema into downstream structs.
- Preserve competition, club, season/match, minutes, date range, and provenance.
- Prefer match-level data when available; downstream `performance` builds windows.
- Keep remote retrieval optional and outside model scoring. Cache immutable historical inputs.
- Respect source access/rate rules; no high-frequency fetch loop should be required for normal operation.
- Parser changes need fixtures covering schema/header changes, missing tables, multi-club seasons, and partial coverage.
