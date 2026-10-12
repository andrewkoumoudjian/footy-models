# Pooled EPL+MLS shot-rate model + full-league bundles — 2026-10-12

> **Addendum — complete-everyone FM26 union (same day).** The men-only
> extract is a strict UID subset of the complete v6 frame (0 men UIDs absent
> from complete; 97,720 complete UIDs absent from men; 16 men + 23 complete
> duplicate UIDs quarantined). Neither file alone is complete: complete
> leaves attributes empty where men has values (~61% of intersecting rows),
> and ships no `attr_source` column. New shared loader
> `scripts/fm26_complete.py` unions both frames UID-keyed with no CA, kind,
> or gender pre-filter (staff-kind rows preserved for typed-ID rescue;
> duplicate UIDs quarantined, never last-row-wins; file order never trusted;
> per-field conflicts recorded with men-provenance winning). All consumers
> switched: canonical resolver (`canonical_reep_first_v3`), unified bundle
> `load_fm`, attribute reconstruction, and the MLS (name,DOB) FM index
> (staff-with-db-block rescued, derived/unknown never observed). Complete-only
> rows carry `unknown_no_men_provenance` and stay trait-missing until
> corroborated — identity is solved for everyone, traits never overstated.
> Rerun outputs below are executed and audited; the men-primary baseline it
> replaced is not shown.
>
> **Supersession note (same day, parallel track).** A parallel track has
> since landed a graded-evidence resolver rewrite
> (`docs/EPL_FM26_IDENTITY_RESOLUTION_FIX_2026-10-12.md`, HEAD `96a350f`)
> that absorbs this union work and reaches **1244 attached / 332 unresolved
> / 5 senior** (`epl_canonical_clean_v10.json`: Gabriel, João Pedro,
> Alisson, Allan, Ben Wilson) with **442 native db blocks + 69
> reconstructed** admitted in `unified_epl_canonical_clean_v10.json`
> (20 teams × 1576 players × 248 cols, 0 bad heights). Those are the current
> identity/bundle authorities; the v3-era 952 counts below are retained as
> the audited intermediate that proved discovery is Reep-constrained. The
> pooled v2 model below is roster-independent (ESPN + MLS labels) and stands.
>
> **Retained v3 intermediate (audited, superseded).**
> `epl_canonical_clean_v3.json`: 20 clubs, 1576 rows, **952 FM candidates /
> 624 unresolved / 16 senior unresolved** — identical matches to the men-only
> v2 run. Identity discovery is Reep-constrained, not FM-missing-constrained:
> the union audited 661,364 complete + 563,382 men rows, attached 12,079
> complete-only native UIDs to the wanted universe, quarantined 0 duplicate
> UIDs among wanted (23 globally), and flagged 306 conflicted fields across
> 28 wanted UIDs (men-provenance values used, conflicts recorded). No
> person is missed due to men-only filtering anymore; e.g. Alisson UID
> 19058734 exists (complete-only player row, ca176) but carries no
> name/DOB in either frame and no Reep bridge, so it stays unresolved with
> null traits rather than borrowing another person's attributes.
>
> **Union bundles + refit pooled model (executed).**
> EPL `unified_epl_2026_10_11_canonical_v4_union.json`: 20 teams, 1576
> players, 248 cols — 425 db + 61 reconstructed + 1090 missing, 0 bad
> heights. MLS `unified_mls_2026_10_11_30club_v2_union.json`: 30 teams, 791
> players — 349 db + 11 reconstructed (identity counts unchanged: the union
> adds identity coverage, never overstated traits).
> MLS labels rebuilt on the union index (`fm26-mls-shots-union-v2`,
> `fm26-mls-candidate-sets-union-v2`): train 547 / val 192 / test 222 joined
> (were 377/155/163), **266 staff-kind db rows rescued**, missing/ambiguous
> 1705 → 1439. Pooled refit `pooled_epl_mls_shot_model_v2_union.json`
> (same 1085/276/2181 pooled rows; 266 rows upgraded from fallback zeros to
> real FM attrs, `fm_verified` coeff 0.009 → 0.033): val 1.1638 → 1.1485
> (Δ−0.0153), pooled test 1.2422 → 1.2370 (Δ−0.0051), MLS test Δ−0.0101, EPL
> test Δ−0.0038. Small consistent gains in both leagues; research-only.
>
> **Loader bug caught and fixed during this work.** The men extract's
> provenance column `attr_source` itself starts with `attr_`, leaked into the
> trait union as field `"source"`, and the index builder's `f"attr_{field}"`
> key overwrote real provenance with `''` — zero MLS samples (loud failure,
> no silent corruption). Fixed centrally (provenance excluded from traits,
> explicit keys ordered last) with regression tests; bundle/resolver outputs
> never read the stray key and stand as built.

**Status: research-only, executed locally, not calibrated for publishing.**
Mathematical pathway only; no deep-ML training. All counts below were
computed by running the listed commands on the development machine. No
invented rosters, traits, or scores.

## 1. What was actually executed

```bash
# Canonical clean resolver (zero name-guessed UIDs, Reep-first + TM/SI/DOB)
uv run --no-project --with duckdb --with beautifulsoup4 python \
  scripts/match_premier_league_fm26.py --offline --as-of 2026-10-11 \
  --html-cache data/pl/raw/registered_pl_2026_27.html \
  --output data/identity/reep/epl_canonical_clean_v2.json \
  --identity-audit data/identity/reep/epl_canonical_clean_v2_evidence.json

# EPL 20-club bundle, strict native-attribute gate, no old priors
python3 scripts/build_unified_prematch_bundle.py --league EPL --cutoff 2026-10-11 \
  --epl-roster data/identity/reep/epl_canonical_clean_v2.json \
  --reep-audit data/identity/reep/coverage_full_all_sources_v1.json \
  --output data/identity/reep/unified_epl_2026_10_11_canonical_v3.json

# EPL provisional comparison (1228 patched IDs, same strict gate)
python3 scripts/build_unified_prematch_bundle.py --league EPL --cutoff 2026-10-11 \
  --epl-roster data/identity/reep/epl_remaining_resolved_v1.json \
  --reep-audit data/identity/reep/coverage_full_all_sources_v1.json \
  --output data/identity/reep/unified_epl_2026_10_11_provisional_1228_v1.json

# MLS 30-club bundle (recency filter removed; all most-recent-club kept)
python3 scripts/build_unified_prematch_bundle.py --league MLS --cutoff 2026-10-11 \
  --output data/identity/reep/unified_mls_2026_10_11_30club_v1.json

# One pooled EPL+MLS ridge-Poisson shot model (player-agnostic, league flag allowed)
python3 scripts/fit_pooled_epl_mls_shot_model.py \
  --output data/ml/pooled_epl_mls_shot_model_v1.json

# Tests
python3 -m unittest discover -s tests -p 'test_unified_prematch_bundle.py' -v
python3 -m unittest discover -s tests -p 'test_pooled_epl_mls_shot_model.py' -v
python3 -m unittest discover -s apps/matchforecast/math_engine -p 'test_*.py' -v
cargo test -p footy-forecast --lib
cargo test -p footy-matchforecast --lib math_engine
cargo test -p footy-match --lib
```

## 2. EPL identity and bundle coverage (real, not claimed)

| Roster | Total rows | FM UID candidates | Unresolved | Senior unresolved |
|---|---|---|---|---|
| `epl_canonical_clean_v2.json` (clean, zero guesses) | 1576 | 952 | 624 | 16 |
| `epl_remaining_resolved_v1.json` (patched provisional) | 1576 | 1228 | 348 | 7 |

Canonical bundle `unified_epl_2026_10_11_canonical_v3.json`:
20 teams, 1576 players, 248 columns. Identity quality:
`reep_tm_provisional` 855, `native_exact_legal_provisional` 97,
`unresolved_no_provider_id` 624. FM admission:
`database_block` 425 (name+DOB corroborated),
`reconstructed_distribution` 61 (existing source-derived blocks),
`missing` 1090 (all UIDs held separately, traits quarantined on mismatch).
Zero implausible heights after fix (65354 cm sentinel now missing).

Provisional comparison `unified_epl_2026_10_11_provisional_1228_v1.json`:
same 1576 rows, 248 columns, but admits 562 db + 250 reconstructed
(812) because the legacy roster lacks explicit `fm_native_record_trusted`
flags. The extra ~326 traits are name-compatible but not DOB-verified and
must NOT be treated as verified. This confirms the handoff warning: do not
copy 1228-ID counts as verified attributes without rerunning the gate.

SofaScore/FBref/market in EPL bundle: 0 rows prior (honest gaps — no local
EPL SofaScore archive, FBref season aggregates are sidecar-only per
`historical_fbref_crosswalk`, TM snapshot has no dated series).

## 3. Seven senior unresolved (patched ledger, retained with nulls)

- Arsenal `Dos Santos Magalhaes, Gabriel` — 6 low-score alternatives (tier 1), no TM; ambiguous common name.
- Bournemouth `Petrovic, Djorde` — 0 candidates (transliteration Đorđe; no Reep FM bridge found).
- Bournemouth `Sanchez Velasco, Juan Luis` — 26 candidates; top `Juan Luis Sánchez Velasco` score 16 but competing identities prevent auto-accept.
- Brighton `Loureiro Da Costa, Joao Pedro` — 23 candidates; two `Joao Costa` score 15 tie; quarantined.
- Man City `Andrade Elias, Allan` — 0 candidates.
- Man City `Ndiaye, Iliman Cheikh Baroy*` — 0 candidates (N'Diaye apostrophe handled, still no bridge).
- Coventry `Wilson, Ben*` — 2 candidates (`Ben Wilson` 2000362675 vs 28028347) score 16 tie; collision quarantined.

Clean v2 additionally quarantines 9 more seniors as insufficient
(Goretzka, Jensen, Reece James, Berge, Browne, Hughes, Mendy, Barnsley,
Barry, Alisson Becker, Avenell) — all retained, traits missing, never
another person's attributes.

## 4. MLS 30-club refresh (real)

`unified_mls_2026_10_11_30club_v1.json`: **30 teams, 791 players**,
248 columns. Previous bundles were 664/27 and 483/22 due to a 120-day
recency deletion rule; the rule is removed (recency is now a dated
feature, not a deletion). All 798 SofaScore player-histories across 98 MLS
fixtures are joined; 7 transferred duplicates resolve to most-recent club.
Per-team counts 20–36. Missing-three clubs restored: Real Salt Lake 26,
Sporting Kansas City 20, St.Louis City 20. FM admission: 349 db + 11
reconstructed (DOB-matched), 145 staff-kind, 262 no link, 20 not in export,
4 DOB conflict. FBref/market 0 rows (same honest gaps as EPL). No official
2026 MLS registration source was available; this is the full
SofaScore-observed universe, not a claim of official roster completeness.

## 5. One pooled EPL+MLS shot model (trained + OOS scored)

`scripts/fit_pooled_epl_mls_shot_model.py`: pooled ridge-Poisson residual
over a shared as-of empirical-Bayes baseline
`(past_shots + 5·role_prior)/(past_minutes/90 + 5)`, role priors
G0.02/D0.40/M0.90/F2.20 (EPL ESPN role-neutral M0.90, documented).
Features (11, fixed before labels): league_mls, age, CA, Finishing,
OffTheBall, Dribbling, Composure, Anticipation, Passing, last-seen
recency, fm_verified. No player-ID coefficients. Chronological splits:
train <2026-06-01 (1085 MLS), validation 06-01–08-01 (276 MLS),
test ≥08-01 (471 MLS + 1710 EPL ESPN). DNP rows skipped (not zero shots).
FM26 vintage 2026-03-01 precedes all labels (2026-04 onward).

| Split | Baseline Poisson LL | Selected (L2=0.1) | Δ |
|---|---|---|---|
| Validation (MLS) | 1.1638 | 1.1463 | −0.0176 |
| Test pooled (MLS+EPL) | 1.2422 | 1.2367 | −0.0055 |
| Test MLS only | 1.1182 | 1.1083 | −0.0099 |
| Test EPL only | 1.2763 | 1.2720 | −0.0043 |

Coefficients (league_mls first): `[0.143, 0.031, 0.008, 0.006, 0.014,
0.030, 0.002, −0.013, −0.0003, 0.040, 0.009]`. Small, consistent gains in
both leagues; no promotion claim — research-only, needs larger future
validation. Reuses `footy-forecast::ExposurePoissonShotModel` role priors
and empirical-Bayes structure; psychological-fit, chemistry, value, and
volatility primitives remain eligible inputs (currently missing→masked)
rather than invented features.

## 6. Rust simulator contract

Corrected per-90 fed to Rust: `shots_per90 = asof_baseline_per90 ·
exp(X·β)`, consumed as `footy_matchforecast::math_engine::Candidate.shots_per90`
(floored at 0.005 in-engine, path-consistent XI + shot-to-goal process).
Start/conversion heads remain baselines pending larger validation; no
independent per-player goal distribution is multiplied outside the engine.
Rust suites pass: `footy-forecast` 18, `math_engine` 6, `footy-match` 70.

## 7. Fixes in this change

- `plausible_height` (140–220 cm) + `plausible_ability` (CA/PA 1–200):
  raw 65354 cm sentinel and out-of-range abilities are now missing, with
  regression test `test_implausible_height_and_ability_are_missing_not_features`.
- MLS recency deletion removed: all 30 most-recent-club assignments kept.
- New pooled model script + 5 chronological/leakage tests.

## 8. What is NOT claimed

- No 1228 verified FM attribute profiles (only 425 canonical db blocks).
- No full EPL SofaScore/FBref/market ingestion (0 rows; gaps documented).
- No official 30-club MLS registration completeness (SofaScore-observed universe).
- No calibrated WDL/scoreline/momentum validation (shot-rate only; Liverpool–City
  momentum requires 200+ fixture prospective study, not run here).
- No Candle/transformer training in the math pathway.

Local artifact hashes (ignored by git, reproducible via commands above):
pooled model `97cd86ce…8745`; EPL canonical v3 `99e1edf9…`;
MLS 30-club `77566feb…`.
