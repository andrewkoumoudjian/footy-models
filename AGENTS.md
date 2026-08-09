# Repository agent instructions

## Mission

Implement the architecture in `architecture.md` as a Rust-first, dependency-light football valuation and recruitment platform. Optimize for historical correctness, reproducibility, explicit uncertainty, and efficient scoring before convenience.

## Global invariants

- Read `architecture.md` before architectural or cross-crate changes.
- Historical features are strictly causal/as-of. Never use future observations, later revisions, eventual destinations, or post-event information.
- Missing data is not zero. Preserve coverage and observation confidence.
- Keep Transfermarkt market-perceived value, fundamental value, acquisition price, and future value as distinct types/outputs.
- Do not use current Transfermarkt value as a feature in the fundamental-value model.
- Do not encode free transfers as zero-fee permanent transfers.
- Preserve source provenance and external IDs through canonicalization.
- Models must expose uncertainty/calibration, not only point predictions.
- Headline validation uses walk-forward historical splits, never random row splits.
- Prefer deterministic behavior and fixed seeds for model/backtest reproducibility.

## Rust rules

- Prefer `std` in core/hot-path crates.
- New dependencies require a concrete need and should be boundary-scoped or feature-gated where possible.
- Avoid Python runtime dependencies in production model scoring.
- Use strong ID/money/date/version types rather than primitive aliases where accidental mixing is possible.
- Avoid strings in hot structures; use interned IDs or compact enums.
- Treat clippy warnings and compiler warnings as problems to fix, not suppress globally.
- No `unsafe` unless a measured performance requirement exists and the safety invariant is documented with tests.

## Architecture boundaries

- Source crates parse/import and emit canonical records. They do not own modeling logic.
- Generic statistical algorithms in `crates/models` must not depend on FBref/Transfermarkt-specific types.
- Apps are thin orchestration layers; domain logic belongs in crates.
- `backtest` may consume production crates, but production crates must not depend on `backtest`.
- Keep the internal dependency graph acyclic.

## Data sourcing

- Transfermarkt ingestion must consume data the operator is authorized to use. Do not introduce unauthorized scraping or ML-use assumptions as a required implementation path.
- FBref ingestion must respect current source access rules and coverage limitations; cache immutable historical inputs rather than making remote access part of scoring.
- Source-specific HTML/CSV/JSON schemas stop at adapter boundaries.

## Testing expectations

- Unit-test temporal boundary conditions and identity/reconciliation rules.
- Add regression fixtures for every source parser.
- Add leakage tests for as-of feature construction.
- Validate model intervals for empirical coverage.
- Backtests must record dataset/model/feature versions and cutoffs.

## Scope of nested AGENTS.md

The nearest `AGENTS.md` applies additional rules for that subtree. Nested files may narrow responsibilities but must not weaken these global invariants.
