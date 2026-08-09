# `temporal` crate

Own events, validity intervals, snapshots, as-of lookup, and bitemporal utilities.

- This crate is the primary leakage barrier.
- Define interval inclusivity/exclusivity once and test all boundaries.
- An as-of query may only return information valid/known at or before the requested cutoff according to the selected time semantics.
- Never interpolate using a future observation unless the API explicitly represents a non-causal research operation; such APIs must not be usable by production feature builders.
- Prefer sorted compact vectors plus binary search before adding complex indexing dependencies.
- Add property/regression tests around equal timestamps, missing intervals, overlapping observations, and revisions.
