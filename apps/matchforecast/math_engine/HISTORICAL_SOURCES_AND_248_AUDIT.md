# EPL / MLS unified 248-feature pathway: historical-source audit

Implementation audit of October 12, 2026, following main commit e281d9d.
This is NOT a trained, validated, or publishable match-forecast model.

## Shared two-league candidate input

- EPL: 20 clubs and 1,576 official registered/U21 candidates.
- MLS: 27 historically observed clubs and 664 player candidates.
- All 2,240 candidates export to 248 identically named numerical features,
  each nullable with a strict missingness mask. No player-ID column enters X.
- The EPL and MLS bundle snapshots have different historical cutoffs.
  Their original dates remain in the manifest. The merged scoring artifact
  is NOT one fictional simultaneous fixture or a historical training panel.
- The EPL's FM26 source attributes are provisional, often not independent
  current-registration identity verification. Missing FM fields remain null.

The export command:

    python3 apps/matchforecast/math_engine/export_unified_248.py \
      --epl data/identity/reep/unified_epl_full_2026_10_11.json \
      --mls data/identity/reep/unified_mls_v1.json \
      --manifest data/ml/math_engine/unified_shared_248_manifest.json \
      --rows data/ml/math_engine/unified_shared_248_rows.jsonl \
      --feature-schema data/ml/math_engine/unified_shared_248_schema.json

The source bundles are preserved. Files in data/ml are ignored research
outputs; every new command refuses overwriting any existing output.

## Historical FBref / Understat evidence

The user-supplied Hugging Face data consists of full historical seasons.
It does not contain EPL recent-ten-match logs, and it does not contain
MLS player seasons. The Reep ID-based crosswalk finds:

| Identity evidence | EPL candidates |
|---|---:|
| Registered universe | 1,576 |
| Reep candidate FBref ID with an older finished season | 254 |
| Of those, a jointly matched historical Understat xG record | 229 |
| Candidate FBref ID with no earlier source season | 148 |
| No Reep candidate FBref ID | 1,174 |

The historical sidecar preserves all older season-club records individually,
without double-counting summary club rows. Full 2026/27 season totals
are blocked at the October 2026 cutoff. Provider-ID associations to
current official registrations are still candidate-only and do not
automatically authorize use as measured EPL player training features.

    python3 apps/matchforecast/math_engine/historical_fbref_crosswalk.py \
      --bundle data/identity/reep/unified_epl_full_2026_10_11.json \
      --fbref data/ml/math_engine/huggingface_sources/aloobun_player_stats.csv \
      --understat-joined data/ml/math_engine/huggingface_sources/joined_season_20261012.json \
      --cutoff-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      --output data/ml/math_engine/huggingface_sources/epl_full_fbref_historical_sidecar_v1.json

This requires an observation date after the original bundle/source archives
were saved. It is NOT a retroactively available October 11 feature.
The old 248 columns remain unchanged; in particular, a previous full-season
stat must not be mislabeled as recent ten match observations.

**Licensing gate:** Sports Reference announced its FBref advanced-data removal
in January 2026 and its published terms restrict some model-training uses.
The permissions for third-party historical Hugging Face data also need review.
Do not bypass official site access restrictions. This sidecar is an
identity/provenance audit, not a cleared training dataset.

## Mathematical model and safety gate

The pooled shared_math_glm.py implementation now accepts the official
248-feature ordered schema. It learns the same regularized feature
coefficients for every EPL and MLS player, with no per-player ID terms:

- Bernoulli logistic probability of starting;
- Poisson shot attempt intensity per actual on-field 90-minute exposure;
- Binomial conversion probability conditional on a shot.

The heads share training-set normalization and missingness indicators.
Chronologically separated training/validation/test splits, known
pre-match feature cutoffs, and count-event conservation are required.
Actual starting XI constraints and joint match outcomes belong to the
existing Rust mathematical simulator.

The 2,240-person matrix contains no event labels and cannot honestly
train, validate, or backtest the model as currently archived.

The shared feature loader now quarantines duplicate raw FM26 numerical
UIDs rather than accidentally taking the final duplicate row, preserving
an explicit unknown FM mental-trait state.

## Next evidence gate

Acquire a properly authorized, timestamped EPL SofaScore player-match
history and current official roster-to-Reep identity crosswalk; for MLS,
use the existing as-of SofaScore event feed. Recreate one player candidate
matrix for each historical pre-match instant, then add the observed event
labels only after the forecast snapshot is frozen. Track exact match IDs,
provider IDs, access rights, and data publication times. Require separate
out-of-sample tests before any production forecast.
