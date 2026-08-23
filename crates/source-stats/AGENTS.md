# `source-stats` crate

Own provider-specific parsing for open/free football statistics and match-result inputs.

- Emit source-neutral records with external IDs, dates, metrics, and provenance; canonical identity belongs in `dataset`.
- Remote collection is outside model scoring. Parse cached/immutable payloads and let orchestration own HTTP, caching, authentication, and rate limits.
- Treat Sofascore endpoints as an optional unofficial input surface, never a required platform dependency.
- Keep provider-specific field names and JSON shapes inside this crate.
- Missing fields remain missing. Do not synthesize zeros for unavailable statistics.
- Add compact regression fixtures for every supported provider shape.
