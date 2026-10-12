# FM26 attribute recovery after Reep-first EPL identity discovery

Research implementation: October 12, 2026. All original binary FM26, Reep, Sortitoutsi and officially registered player data remain unchanged.

## Validated result for the full league

| Source classification in final EPL feature bundle | Players |
|---|---:|
| Native player-kind directly stored FM26 DB trait block | 454 |
| Staff-kind FM26 record whose real DB trait block was recovered through a typed Reep player-UID bridge | 221 |
| Staff-kind FM26 record with sparse, partially populated DB attributes recovered field by field | 34 |
| Existing FM-derived / source-generated attributes | 100 |
| Newly generated probabilistic CA-conditioned attribute priors | 26 |
| Official registration with no accepted unique FM26 ID (attributes remain null) | 741 |
| **All official EPL registrations retained** | **1,576** |

Thus 709 individuals have at least part of a genuine stored FM26 player-trait block (454 + 221 + 34). Among those, 34 blocks are sparse; individual mental fields equal to zero are explicitly treated as missing, never as observed zero on the 1-20 FM scale. Of the 281 staff-classified Reep-linked records, 255 usable DB blocks were recovered, and two nominal DB blocks had no reliable measured attributes and were handled with the 24 staff-without-DB records using probabilistic generation.

Per-trait example coverage in final data: observed Composure for 647 players; reconstructed or generated Composure for 126; no individual Composure information for the other registrations.

## The actual FM26 reconstruction method

The source of the generative prior is akks88/football-manager-data, locally cached at /home/drewclaw/fm26-data/hf_upload/data/attr_generation_curves.json, and the original generator /home/drewclaw/fm26-data/work/generate_attrs.py.

Empirical attribute distributions were fitted from approximately 196,014 actual FM26 player attribute records. The proper model conditions each trait on FM CA and position class (GK, DF, MF, FW) and carries the distribution's mean and standard deviation. The new pipeline emits the conditional expectation, standard deviation and estimated p10/p90 ranges, not random exact 1-20 values masquerading as the player's original mental rating. The original source research explicitly rejected forcing generated sets to match a quadratic CA cost regression, so that rejected tuning step is not used here.

When Sortitoutsi gives an independently verified role for the FM UID, the position-specific curves are selected. When only CA is known, predictions are marginalized over the published empirical population distribution of position classes, with wide uncertainty. When neither the player's identity nor CA is verified, the pipeline leaves individual FM traits missing. It never fills the remaining 741 records with made-up personal psychology.

Staff classification correction is independent of attribute generation: Reep typed fm:player ID, unique raw FM UID, and actual populated attr_source=db blocks permit recovering observed stored traits. The original raw kind=staff and Reep-to-official-registration name-evidence state are preserved. A sparse block is not treated as a complete 54-trait observation; every unassigned mental value remains null. Dynamic morale, psychological fitness and truly hidden unmeasured personality traits remain missing.

## Versioned files on the development machine

- Reep-first full-league identity audit: data/identity/reep/epl_reep_first_full_discovery_20261012_v4.json
- All-20-club roster: data/pl/squads_2026-10-11_full_reep_first_v4.json
- FM conditional attribute distributions: data/identity/reep/epl_fm26_conditional_missing_traits_20261012_v2.json
- Final regenerated unified 248-column player-set: data/identity/reep/unified_epl_2026_10_11_reep_first_fm_recovered_v6.json

The feature bundle retains existing mental observed/reconstructed masks, source and registered-player identities, psychology, value and other primitives. Existing MLS observations are left unchanged. The regenerated research bundle was created October 12 from static FM26 sources available before its target October 11 feature-vintage date: it is not evidence of a prospectively released October 11 forecast.

## Reproduce

    python3 scripts/reconstruct_missing_fm26_attributes.py --identity data/identity/reep/epl_reep_first_full_discovery_20261012_v4.json --output data/identity/reep/epl_fm26_conditional_missing_traits_new.json

    python3 scripts/apply_reep_first_epl_roster.py --reep-audit data/identity/reep/epl_reep_first_full_discovery_20261012_v4.json --output data/pl/squads_2026-10-11_full_reep_first_new.json

    python3 scripts/build_unified_prematch_bundle.py --league EPL --cutoff 2026-10-11 --epl-roster data/pl/squads_2026-10-11_full_reep_first_new.json --reep-audit data/identity/reep/coverage_full_2026_10_11.json --fm-attr-priors data/identity/reep/epl_fm26_conditional_missing_traits_new.json --output data/identity/reep/unified_epl_reep_first_fm_recovered_new.json

All snapshots are immutable and cannot be overwritten. Tests: python3 -m unittest discover -s tests -p 'test_unified_prematch_bundle.py' ; python3 -m unittest discover -s tests -p 'test_reconstruct_missing_fm26_attributes.py' ; python3 -m unittest discover -s apps/matchforecast/math_engine -p 'test_*.py'.

## Remaining identity and data gates

Full 20-club EPL registrations are present, but the 741 without accepted FM26 IDs cannot receive specific FM mental ratings. Their names must be further resolved through verified provider IDs, Reep merges, and independently dated source evidence. The SofaScore EPL player-event feed and training-eligible FBref/market observations are not yet populated in this full-league bundle. This is a feature preparation result, not a trained or independently scored forecasting model.
