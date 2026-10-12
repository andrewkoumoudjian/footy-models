# Unified EPL / MLS feature contract — league-wide 2026/27 roster correction

**Implemented:** `scripts/build_unified_prematch_bundle.py`,
`scripts/match_premier_league_fm26.py`, `scripts/audit_reep_crosswalk.py`.
The objective is **one model of football outcomes** fed player-, team-,
environment- and value-related primitives. This commit builds its typed input
contract, **not** fitted or validated new match/valuation model weights.

## Full roster, never a fixture shortlist

On **2026-10-11**, the authoritative post-window official Premier League
registration article (published **2026-09-03**) enumerates all **20 clubs**:
Arsenal, Aston Villa, Bournemouth, Brentford, Brighton, Chelsea, Coventry,
Crystal Palace, Everton, Fulham, Hull, Ipswich, Leeds, Liverpool,
Manchester City, Manchester United, Newcastle, Nottingham Forest, Sunderland,
Tottenham. Senior registrations and U21 lists are both preserved. The earlier
four-fixture roster of eight clubs was a limited experiment, never a full EPL
training population.

Local current full-registration audit:

| Evidence | Number |
| --- | ---: |
| Official clubs | 20 |
| Registered + U21 entries | 1,576 |
| Legacy FM display/legal-name associations | 1,051 |
| Reep-first, native-name-guarded FM UIDs | **1,156** |
| Remaining without attached FM UIDs | **420** |
| **After FM/Sortitoutsi/Transfermarkt full identity recovery** | **1,162 attached** |
| Still unmatched senior registrations / U21 | **46 / 368** |
| Native player-kind `db` individual FM attributes | **552** |
| Input numeric columns shared with MLS | 248 |

These 1,576 are **registration candidates**, not a pre-match selected/fit
matchday squad. Some U21 listings are explicitly on loan. The original 8-team
demo is kept as historical evidence. Full EPL bundle creation now **fails**
if passed an eight-team or otherwise incomplete roster. The corrected Reep-first
resolver `scripts/resolve_epl_reep.py` searches canonical labels and aliases
*before* FM attribute matching, checks native names, and quarantines false
bridges. Its follow-up `scripts/reconcile_epl_fm_sources.py` scans all four FM
exports and Sortitoutsi UID records for the remaining names, while
`scripts/promote_epl_fm_tm_evidence.py` checks Reep's FM-to-Transfermarkt
provider IDs against birthdate, native name and prior club. Six more names
were recovered. A Reep link does not by itself prove an official registration's
identity: the public register excludes DOB and the official roster has names
but no provider IDs. The current
2026/27 registration source is a September snapshot; it should be refreshed
whenever the league posts a replacement (particularly the winter window).

## Primitives exposed to the ONE shared EPL/MLS model

| Domain code (reused semantics) | Input representation | Missing-data and causality contract |
| --- | --- | --- |
| `footy-state::AbilityVector` | FM CA, PA, technical/physical/creative/defensive attributes | `db` and reconstructed `derived` marked separately; name-assigned EPL FM remains tentative |
| `footy-state::PsychologyVector` | FM teamwork, leadership, consistency and raw mental traits | Pressure, professionalism, ambition, adaptability, morale and motivation not invented |
| `footy-performance::PerformanceWindow` | Minutes-exposure per90, EWMA, trend, variance of player shots/xG/xA | Match days strictly older than forecast cutoff; missing does not mean zero |
| `footy-performance::TeamPerformanceWindow` | Team xG, shots, goals, possession, passing, fouls/cards | Actual prior matches only; do not convert goals into unobserved xG |
| `footy-state::UsageVector` and forecast participation priors | Past starts, minutes, appearances and recency | Not proof of medical availability; lineup predictions remain probabilistic |
| `footy-state::RoleVector` / recruitment tactical fit | Prior position shares with role evidence count | Manager's current tactical profile unavailable until dated evidence is ingested |
| `footy-match::cohesion` | History of teammate co-starts as sparse pair graph | Observed previous combinations, not claimed innate personality compatibility |
| `footy-state::DevelopmentVector` | CA/PA realization; velocity if *multiple* dated ability snapshots | No fake year-over-year development based on one FM snapshot |
| `footy-recruitment::surge::FORECAST_FEATURE_NAMES` | **All 47** valuation primitive slots: value momentum/volatility, minutes, age, FM quality, transfer context | Raw **dated past** values only; missing slots kept null; predictions of future value excluded |
| `footy-market::TransferHazard` / `MarketCalibration` | Reserved probability and price-distribution slots | Unfitted/unobserved = null, not surrogate player performance labels |
| `footy-recruitment::EnvironmentFit` | Adaptation/playing-time/culture/tactical fit fields | Playing-time prior can be observed; language/culture and morale require independent evidence |
| Factor primitives (HMM/GARCH) | Reserved latent-form and risk-volatility features | Never declare an inferred regime as measured or promote without chronological OOS gain |

Provider blocks retain explicit IDs and provenance. **Reep** bridges FM,
SofaScore, FBref and Transfermarkt in separate typed ID namespaces, but a
Reep FM bridge does *not* validate that an EPL registration name is the same
person, nor that a reconstructed FM mental trait is observed. MLS had **664
previously observed active local candidates** in the August 24 research
sample, not an asserted complete 30-team league roster.

The numeric model row contains 248 ordered columns plus observed/missing masks,
the league categorical indicator, attribute source, identity evidence, and an
optional previous-co-start graph. Same-day matches and unavailable/unpublished
observations cannot enter a prior feature. FBref/Transfermarkt histories are
currently absent from the canonical new bundle until identity-verified,
date-published provider rows are ingested. No fabricated FPL/mental ratings.

## Reproduce without rewriting frozen data

```bash
python3 scripts/match_premier_league_fm26.py --offline \
  --as-of 2026-10-11 --output data/pl/squads_2026-10-11_full.json

uv run --no-project --with duckdb python scripts/resolve_epl_reep.py \
  --roster data/pl/squads_2026-10-11_full.json \
  --output data/identity/reep/epl_full_reep_first_v3.json

python3 scripts/reconcile_epl_fm_sources.py \
  --roster data/identity/reep/epl_full_reep_first_v3.json \
  --report data/identity/reep/epl_unmatched_all_sources_v1.json \
  --output data/identity/reep/epl_all_sources_review_v1.json

uv run --no-project --with duckdb python scripts/promote_epl_fm_tm_evidence.py \
  --roster data/identity/reep/epl_full_reep_first_v3.json \
  --audit data/identity/reep/epl_unmatched_all_sources_v1.json \
  --output data/identity/reep/epl_full_all_sources_v1.json \
  --review data/identity/reep/epl_tm_recovery_review_v1.json

uv run --no-project --with duckdb python scripts/audit_reep_crosswalk.py \
  --squads data/identity/reep/epl_full_all_sources_v1.json \
  --output data/identity/reep/coverage_full_all_sources_v1.json

python3 scripts/build_unified_prematch_bundle.py \
  --league EPL --cutoff 2026-10-11 \
  --output data/identity/reep/unified_epl_all_sources_v1.json

python3 scripts/build_unified_prematch_bundle.py \
  --league MLS --cutoff 2026-08-24 \
  --output data/identity/reep/unified_mls_v2.json
```

Choose fresh output filenames on rerun: the identity audit and model-bundle
builder refuse overwrites of previous research snapshots. Local provider
raw data, Reep DuckDB, roster JSON and aggregate bundles stay untracked.

## Release criteria

The data bundle is not proof that a 248-feature model improves predictive
accuracy. Need EPL and MLS historical player provider streams, verified
published timestamps, complete historical lineups, and independent strict
chronological OOS tests. Compare one regularized multitask learner against
the causal Gamma–Poisson player-count, participation, and team-goal baselines;
validate same-engine W/D/L, scoreline, substitutions, cards, and goal timing.
Use value forecasting as an auxiliary *target* or as-of value primitives, not
as a future-derived input. Do not deploy extra HMM, transformer, psychological
controllers, or incident adjustments without reproducible OOS improvement.
