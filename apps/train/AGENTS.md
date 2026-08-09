# `train` app

Orchestrate historical feature materialization, model fitting, calibration, and artifact emission.

- Require an explicit training cutoff and dataset/feature versions.
- Refuse features with timestamps beyond the cutoff.
- Persist full model metadata: cutoff, feature ordering, parameters, source dataset hashes/versions, random seed, and calibration artifact.
- Training may be compute-heavy; scoring dependencies must remain lightweight.
- A successful run should be reproducible from recorded inputs and configuration.
