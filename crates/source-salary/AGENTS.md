# `source-salary` crate

Own salary/wage-source import and source confidence.

- Salary estimates are observations with provenance and confidence, not ground truth.
- Normalize currency, gross/net basis when known, periodicity, and effective date explicitly.
- Do not infer a missing wage as zero.
- Keep salary source mechanics isolated from the wage model in `market`/modeling layers.
- Preserve ranges or uncertainty when a source provides estimates rather than exact figures.
- Do not collapse bonuses, signing fees, agent fees, and base wage into one number at ingestion time.
