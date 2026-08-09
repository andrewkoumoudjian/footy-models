# `entities` crate

Own canonical football entities and cross-source identity resolution: players, clubs, competitions, external IDs, identity links, and match candidates.

- Canonical IDs are source-neutral.
- Preserve all source IDs and link confidence; never silently overwrite ambiguous links.
- Matching must progress from strongest deterministic evidence to weaker scored evidence.
- Fuzzy matching is a candidate generator, not automatic truth when ambiguity remains.
- Keep normalization deterministic and locale-aware enough for football names without pulling large text libraries unless proven necessary.
- Identity resolution must be auditable: retain method, score, and manual verification state.
