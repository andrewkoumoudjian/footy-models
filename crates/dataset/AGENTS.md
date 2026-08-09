# `dataset` crate

Own cross-source canonicalization and causal feature materialization between source adapters and model artifacts.

- Football Manager is the canonical player anchor unless the architecture explicitly changes.
- Never force ambiguous identities; persist unresolved candidates and support explicit/manual overrides.
- Source-scoped club/competition IDs must not imply cross-source club identity.
- Canonical persistence is immutable, checksummed, schema-versioned, and bound to the exact source manifest.
- Training features use information strictly before their target date when only day precision is available.
- Missing values require explicit observed/missing indicator features; zero must remain distinguishable from missing.
- Transfermarkt values remain targets/benchmarks for fundamental valuation, never current-price features.
- Performance feature IDs must remain semantically stable across dataset versions.
- Do not fit production models in this crate; emit deterministic feature rows for model crates/apps.
