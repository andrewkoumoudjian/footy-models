# `artifacts` crate

Own immutable/versioned persistence contracts shared by orchestration apps.

- Keep the format dependency-light and deterministic.
- Every artifact must validate its schema, versions, feature ordering, payload length, and checksum before use.
- Writes must be atomic and must not replace an existing immutable artifact.
- Do not put source scraping, football feature engineering, model fitting, or ranking policy here.
- Persist enough lineage to reproduce scoring: dataset/feature/model/calibration versions, cutoffs, source-manifest checksum, and model parameters.
