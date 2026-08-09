# `state` crate

Own the source-neutral football state consumed by models: ability vectors, development, role representation, usage, psychology, and `PlayerState` assembly.

- `PlayerState` is an as-of object. Construction must never request future source data.
- Do not include current Transfermarkt market value in the fundamental player state.
- Retain raw FM/performance references or lineage so derived states are explainable.
- Psychology is a vector with confidence, not one pseudo-scientific scalar.
- Role vectors may evolve but require explicit versioning.
- Derived signals such as capability gap, opportunity suppression, and development momentum need definitions/tests and must not encode future outcomes.
