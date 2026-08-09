# `query` app

Provide human/research access to canonical entities, historical states, predictions, analogues, and backtest artifacts.

- Querying must preserve as-of semantics when a date is supplied.
- Expose provenance/confidence for data inspection rather than presenting reconciled values as unquestionable truth.
- Do not duplicate ranking/model logic that belongs in library crates.
- Prefer stable machine-readable output modes alongside concise human-readable output.
- Treat this app as an inspection/API boundary, not a hidden mutation path.
