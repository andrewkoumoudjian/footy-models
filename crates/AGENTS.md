# Library crate instructions

These crates implement reusable domain and modeling primitives.

- Keep public APIs small, typed, documented, and deterministic.
- Crates should have one clear responsibility and an acyclic dependency direction.
- Do not add CLI, environment-variable, logging-framework, or network concerns to domain crates.
- Prefer iterators/slices and streaming interfaces over loading duplicate full datasets into memory.
- Persisted structures require explicit versioning or migration strategy before release use.
- Source-specific schemas must not leak into downstream market/state/model crates.
- Add benchmarks only after correctness tests; optimize measured bottlenecks, not guesses.
- Dependencies require justification. Prefer internal small algorithms for robust statistics, rolling windows, calibration, PRNG, and simple linear algebra.
