# `core` crate

Own only lowest-level primitives: typed IDs, money, date representation, string IDs/interning interfaces, provenance, confidence, version IDs, and common errors.

- `core` must not depend on any other workspace crate.
- Prefer `std` only.
- Keep types compact and suitable for large arrays.
- Money is integer cents; do not store canonical money as floating point.
- Date semantics must be explicit and stable; do not smuggle timezones into integer-day types.
- Avoid football-source-specific concepts here unless they are truly universal domain primitives.
- Any serialization layout intended for persistence must be versioned and tested for round trips.
