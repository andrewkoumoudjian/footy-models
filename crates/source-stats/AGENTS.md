# `source-stats` crate

Own provider-specific parsing for open/free football statistics, lineup history, and match-result inputs.

- Emit source-neutral records with external IDs, dates, metrics/position intervals, and provenance; canonical identity belongs in `dataset`.
- Preserve lineup uncertainty supplied by the provider. A projected/unconfirmed lineup must not be promoted to a confirmed starting XI.
- Preserve position intervals and substitution/role changes when a provider supplies them; do not collapse a multi-role match to the first label.
- Provider position and formation tokens remain provider tokens here. Mapping them to canonical tactical roles/formations is a versioned modeling decision upstream of `match`.
- Remote collection is outside model scoring. Parse cached/immutable payloads and let orchestration own HTTP, caching, authentication, and rate limits.
- Treat Sofascore endpoints as an optional unofficial input surface, never a required platform dependency.
- Keep provider-specific field names and JSON shapes inside this crate.
- Missing fields remain missing. Do not synthesize zeros for unavailable statistics or minutes.
- Normalize metric names consistently, including camel-case boundaries, before they enter the append-only metric registry.
- Add compact regression fixtures for every supported provider shape.
