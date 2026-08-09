# `source-fm` crate

Own Football Manager import/parsing only.

- Preserve each historical FM release/snapshot; never overwrite old states with newer values.
- Emit canonical source records with provenance, raw attributes, positions, CA/PA when legitimately present, and personality/hidden attributes only when the supplied dataset contains them.
- Do not derive market value, role scores, or development labels here; downstream crates own derived features.
- Keep parser mappings versioned because FM schemas/releases can change.
- Add compact fixtures for every supported source format/release family.
- Unknown columns should be observable in validation output rather than silently discarded when they may contain relevant metadata.
