# EPL/MLS pipeline reproduction, production integration, and momentum validation — 2026-10-12 (session 2)

**Status: executed.** This session re-ran the complete EPL + MLS data pipeline
end to end, reproduced every verified identity/bundle contract, fixed four
genuine defects found by the rerun, integrated the validated observed-shot
prior into the production EPL adapter, re-ran all chronological OOS gates, and
extended the Liverpool–Manchester City momentum mechanism ablation to 72
paths per variant. Identity resolution was NOT reopened (1,244/1,576 stands;
do not restart that investigation).

Artifacts from the parallel session same day: `docs/UNIFIED_FORECAST_OOS_2026-10-12.md`,
`docs/MATCHDAY_FORECAST_2026-10-17_19.md` (single entry point
`scripts/forecast_matchday.py`, universal 20-club bundle, in-engine pooled
head). This session's adapter-level integration is complementary and both paths
remain reproducible.

## 1. Full-pipeline reproduction (all verified)

| Stage | Command | Verified result |
| --- | --- | --- |
| EPL canonical identity (all 20 clubs) | `uv run --no-project --with duckdb --with beautifulsoup4 python scripts/match_premier_league_fm26.py --offline --as-of 2026-10-11 --output data/identity/reep/epl_canonical_clean_v12.json --identity-audit ..._v12_evidence.json` | **1,244 attached / 332 unresolved** (300 absent-from-source, 13 not-corroborated, 11 ambiguous Reep, 8 native homonyms); **442 verified native `db` blocks + 70 source-`derived` (marked unverified)** — byte-equal counts to v10/v11 |
| Reep crosswalk audit | `scripts/audit_reep_crosswalk.py --squads ...v12.json` | coverage ledger regenerated |
| Unified EPL bundle | `scripts/build_unified_prematch_bundle.py --league EPL --cutoff 2026-10-11 --epl-roster ...v12.json --reep-audit coverage_..._v12.json` | 20 teams / 1,576 players / 248 cols; **442 database_block / 69 reconstructed_distribution / 1,065 missing**; 1,244 UIDs held separately; 0 implausible heights |
| FM26 conditional priors | `scripts/reconstruct_missing_fm26_attributes.py --identity ..._v12_evidence.json --output ..._v4.json` | 593 observed-db + 281 staff-kind-rescued db + 258 existing-derived (no redraw) + 56 CA-only conditional priors; totals sum to 1,576 |
| MLS 30-club bundle | `scripts/build_unified_prematch_bundle.py --league MLS --cutoff 2026-10-11` | 30 teams / 791 players; **349 db + 11 reconstructed** — equals `30club_v2_union` exactly |
| MLS shot labels (union index) | `scripts/build_fm_mls_shot_dataset.py` | train 547 / val 192 / test 222; **266 staff-kind db rescued**; missing/ambiguous 1,439 — equals union-v2 manifest |
| MLS candidate sets | `scripts/build_fm_mls_pre_match_sets.py` | 39/414 train, 10/119 val, 17/186 test — equals union-v2 |
| ESPN source refresh | `apps/matchforecast/math_engine/espn_feed.py --since 2026-08-01 --as-of 2026-10-12` | **59 verified completed matches** (was 56), immutable archive + raw SHA-256 |
| Historical panel v2 | `scripts/build_historical_player_panel.py --out data/forecast/player_panel_v2` | with new shot-exposure prior columns (`prior_observed_shots`, `prior_shot_exposure_minutes`, sub-appearance priors) |
| Pooled EPL+MLS shot model refit | `scripts/fit_pooled_epl_mls_shot_model.py --espn data/ml/math_engine/espn_archive_20261012.json ...` | val 1.1638→1.1485 (Δ−0.0153); test pooled 1.2382→1.2330; MLS Δ−0.0101; EPL Δ−0.0039; `fm_verified` coef 0.033; λ=0.1 — reproduces the union-v2 run |
| Full EPL production pipeline | `python3 scripts/run_pl_pipeline.py --offline --fm-file /home/drewclaw/fm26-data/fm26_men_full_attrs.csv --paths 72 --seed 42 --observed-shot-priors data/pl/espn_observed_shot_priors_2026-10-10_v1.json --historical-event-oos --historical-same-engine-oos` | completed through snapshot/report; team benchmark refit; **154/401 fixture-scoped player rows carry observed ESPN Gamma-Poisson shot rates** |
| Historical event OOS (StatsBomb 2015/16) | `--historical-event-oos` | minute-assist Brier 0.011449; 15-min goal Brier 0.161833; team-goal MAE 0.8875; paired deltas vs reference: no clear improvement at 95% on goal timing (honest); assist-prior promotion **rejected** by gate (as designed) |
| Same-engine historical walk-forward | `build_pl_hist_inputs.py` → `footy-matchforecast-pl-hist --paths 200` → `score_pl_hist_backtest.py` | rerun after the NaN contract fix (see §2.5); 56 evaluable fixtures, no-lookahead audit True |

Blocked/honest gaps: FBref remote fetch refused connection (`soccerdata` →
fbref.com blocked); the dated HF FBref/Understat sidecar
(`data/ml/math_engine/huggingface_sources/joined_season_20261012.json`, 4,625
dated season aggregates with `available_from` gates) remains the FBref leg.
MLS still has no official league registration source (791 SofaScore-observed
players only). 300 EPL 2026 youth registrations remain absent from every local
FM export.

## 2. Defects found by the rerun and fixed (generalizable code, with tests)

1. **RFC-4180 CSV parsing (production, loud failure).** Registered names are
   written `"Surname, Given Middle"`; the Rust reader split naively on `,`,
   mis-aligning every later column for quoted names (15/401 rows) and
   panicking with `invalid FM attribute trust flag` in the production driver.
   New `apps/matchforecast/src/csv_fields.rs` (quote-aware parser, loud
   column-count mismatch failure, unit tests) is used by `driver.rs`,
   `backtest.rs`, `form.rs` (both readers) and `main_team_goals_oos.rs`.
   Also fixed the pre-existing float-in-pattern warning in
   `main_participation_eval.rs`; the package is clean under
   `RUSTFLAGS='-D warnings' cargo check --all-targets`.
2. **Resolver evidence determinism.** `reep_name` was picked as `[0]` of a
   stable sort over a **set** populated by unordered DuckDB queries; alias
   case flipped between runs (`KAMADA`/`Kamada`), breaking byte-identical
   reruns and the immutable-snapshot guard. Fixed with a final name
   tie-break in `scripts/recover_epl_remaining_identities.py`. Identity,
   attributes and UIDs were never affected (display field only).
3. **FM union loses `kind` for rows missing from the primary frame.**
   `scripts/fm26_complete.py` merged `kind=kind_primary`, demoting any row
   present only in the secondary frame to `unknown` and wrongly discarding
   its verified db block (zero verified labels in the MLS label tests). The
   initial men DB was not complete — with men as primary **97,717** UIDs
   exist only in complete v6 — so the secondary frame's explicit kind now
   fills the gap. Union counts for the default pairing are unchanged
   (0 men-only UIDs; 97,717 complete-only; 23 duplicates quarantined).
4. **Attribute reconstruction required the retired audit schema.**
   `scripts/reconstruct_missing_fm26_attributes.py` accepted only the older
   `reep_first_full_epl_identity_audit_v1` shape; the canonical CLI is the
   production identity source, so `canonical_audit()` now adapts the
   canonical per-player evidence ledger (reshaping only — attachment
   criteria unchanged).
5. **Historical measurement contract (regression from e1e3bd4, exposed by the
   first rerun of the hist backtest since that commit).** `team_rows.csv`
   wrote literal `NaN` for unobserved shots/xG; the Rust observation reader
   (correctly) rejects non-finite measurements loudly. Missing measurements
   are now blank cells (never NaN), matching the production convention;
   `scripts/build_pl_hist_inputs.py` fixed with rationale.
6. **Position evidence regression (fail-closed by design).** The canonical
   roster dropped `historical_position`, so every player was UNKNOWN and the
   driver's documented-goalkeeper assert failed ("dated positional rosters
   must fail closed"). The resolver now carries dated provider position
   evidence with the accepted identity (`sortitoutsi_position` from the
   verified SI→binary path, `transfermarkt_position` from the corroborated
   TM row — never an attribute-trust signal);
   `scripts/prepare_pl_player_inputs.py` maps them
   (historical → SI → TM priority, verified-FM GK attributes as final
   fallback only). Result: 156/401 fixture rows documented
   (27 GK / 71 DEF / 58 FWD); both Palace keepers documented; the boundary
   false positive (attacker with Reflexes 12/Handling 11) is neutralized by
   the observed-rate override and left in the conservative heuristic branch.

## 3. Observed shot-rate integration into the production EPL adapter

New `scripts/build_espn_observed_priors.py` (with `tests/test_espn_observed_priors.py`):
resolves every registration to a uniquely verified ESPN athlete ID with the
same strict reconciliation as the research crosswalk (unique same-club exact
full name, or unique same-club legal given+family token pair — no fuzzy
matching), aggregates exposure **only from completed matches strictly before
the prediction cutoff**, and emits the Gamma-Poisson posterior
`(shots + 5·role_prior)/(minutes/90 + 5)` with the same role intensities
validated in Rust. Immutable SHA-pinned artifact:
`data/pl/espn_observed_shot_priors_2026-10-10_v1.json` — 416 verified people
across all 20 clubs at the production cutoff (258 with observed shots; 19 GKs
all detected via verified FM attributes; 1,007 strictly-unresolved, 149
loaned-out).

`scripts/prepare_pl_player_inputs.py --observed-shot-priors` (wired through
`scripts/run_pl_pipeline.py`) overrides only `shots_p90` for observed players
(`event_rate_kind=espn_observed_gamma_poisson_5_exposure_role_prior` + per-row
ESPN provenance columns); xG/xA stay FM-derived proxies; unobserved players
keep the prior heuristic; the engine's shrinkage (`weight = minutes/(minutes+180)`)
propagates the posterior at 85–93% weight. All 401 rows carry
`position_group` with `position_source` provenance.

**Shot-rate model selection evidence** (new `footy-shot-rate-eval` binary on
panel v2, source-separated chronological 60/20/20, matchday bootstrap):

| Source | EB vs role-only (test) | Fitted GLM vs EB (test) | Verdict |
| --- | --- | --- | --- |
| MLS SofaScore 2026 | **−0.0937 nats** [−0.151,−0.050] | −0.0080 [−0.026,+0.007] | EB adopted; extra GLM complexity not justified |
| StatsBomb EPL 2015/16 | **−0.0443 nats** [−0.076,−0.020] | −0.0021 [−0.014,+0.009] | EB adopted |

The empirical Gamma-Poisson posterior is the validated primitive; the fitted
exposure-offset Poisson does **not** significantly beat it on either untouched
test. The Python pooled ridge-Poisson refit (§1) shows the same pattern:
small consistent gains, research-only, no promotion without larger
validation.

## 4. Liverpool–Manchester City momentum: 72-path mechanism ablation

All three variants use the **exact frozen executable** (SHA 2916eee…), frozen
inputs and seed 42 (`data/ml/frozen-liv-baseline-72paths-20261011-r1` +
`data/ml/liv-72p-no-tactics-20261012` + `data/ml/liv-72p-no-red-20261012`;
machine report `artifacts/validation/liverpool-mechanism-ablation-72p-20261012.json`):

| Variant | City-favoured relative-pressure peak | City expected goals | Liverpool expected goals |
| --- | --- | ---: | ---: |
| Default (72 paths) | **45–60** (+12.42 index) | 1.639 | 1.278 |
| No scheduled tactical subs | 45–60 (+5.99, halved magnitude; whole profile flattens) | 1.486 | 1.097 |
| No red cards | 60–75 (+6.37) | 1.431 | 1.167 |

Together with the historical OOS momentum audit (engine ≈ flat-clock prior;
all CIs include zero except the weak 75-goal reference), the conclusion is
unchanged and now better powered: **the striking "momentum" peaks are Monte
Carlo + scheduled-substitution-clock artifacts, not demonstrated predictive
skill.** The engine's scheduled substitution timing materially shapes
simulated pressure paths; removing it halves the peak. No live parameter was
changed. Remaining: the 11-v-10 dismissal-conditioned intensity study needs a
verified dismissal source (open-data 2015/16 `Player Off` events carry no
reason in this cache; classifying injury vs red requires substitution-pair
inference — queued, not fabricated).

## 5. Release gates and reproducibility

- Rerun the production command in §1 with the same seed: roster step now
  prints `UNCHANGED identical canonical roster and identity audit` on the
  second run (determinism fix verified end-to-end).
- `python3 -m unittest discover -s tests` — all suites green, including
  identity (18+6+7+13+6+3+8), MLS label/set chronology, unified bundle,
  observed priors (5), unified forecast heads and universal bundle (parallel
  track), panel, momentum audits.
- `RUSTFLAGS='-D warnings' cargo check -p footy-matchforecast --all-targets` clean;
  `cargo test -p footy-forecast --lib` 18/18; `cargo test -p footy-matchforecast --lib` 22/22.
- Snapshot `artifacts/premier-league-2026-10-11/` regenerated (72 paths,
  seed 42, observed priors, model fingerprint `d4cb48eaf9fe…`).

## 6. Remaining blockers / next steps

1. **FBref remote access refused** (connection blocked); refresh the dated HF
   FBref/Understat sidecar only when auth/network allow; never scrape harder.
2. **MLS official registration source** still absent — the 791-player
   SofaScore-observed universe is not league completeness.
3. **300 EPL youth registrations** absent from all 16 local FM exports — need
   a newer FM snapshot; 8+7 homonyms need a dated club-corroborated provider ID.
4. **Prospective scoring** of the Oct 17–19 matchday against actuals (parallel
   track's next required evidence step), and 200+ fixture prospective gates
   for the observed-prior adapter before any promotion beyond research.
5. Dismissal-conditioned 11-v-10 intensity study needs verified red-card
   labels (see §4); do not infer from reason-less `Player Off` events.
6. `downloaded_files/` holds only foreign lock files; leave untracked.