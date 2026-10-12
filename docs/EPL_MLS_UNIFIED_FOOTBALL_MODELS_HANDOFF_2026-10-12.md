# Footy Models — Complete Unified EPL/MLS Requirements and Continuation Handoff

**Date:** 2026-10-12 · **Repository:** [`andrewkoumoudjian/footy-models`](https://github.com/andrewkoumoudjian/footy-models) · **Primary branch:** `main` · **Working folder:** `/home/drewclaw/footy-models`

> **TL;DR.** Build **one** rigorous, interpretable, well-calibrated football forecasting and simulation engine using **every legitimate, as-of-available primitive** already developed in the project: match events, players, ability, psychological attributes, tactics, fitness, cohesion, development, recruitment, market value and uncertainty. Cover **all 20 EPL teams and the entire available MLS league**, with the complete official squad candidate population, not the teams selected for one demo. **Reep + typed provider IDs first**; resolve people against the *full native FM26 binary exports* and independently verify DOB/name/club/Sortitoutsi evidence. Never treat an associated FM UID as sufficient proof that a native extracted ability/mental block belongs to the footballer. Reconstruct truly missing ratings **probabilistically with the existing code**, never as fake observed ratings. Train and test chronologically and adopt more complex methods only after independently measured OOS improvement. Complete, integrate and validate the pipeline, not just an audit or an ID patchlist.

#### Session addendum (2026-10-12, pipeline reproduction + production integration)

Full-pipeline reproduction, four rerun-discovered fixes (RFC-4180 CSV
parsing, resolver evidence determinism, FM-union kind preservation,
historical NaN contract), dated ESPN observed Gamma-Poisson shot priors +
provider positions integrated into the production EPL adapter, all
chronological OOS gates re-run, and the Liverpool–City momentum ablation
extended to 72 paths per variant (peaks are Monte Carlo + scheduled-sub
artifacts). Reproduction table, evidence and remaining blockers:
`docs/EPL_MLS_PIPELINE_REPRODUCTION_AND_INTEGRATION_2026-10-12.md`.

## Latest canonical execution update (2026-10-12, identity resolver fixed)

**Clean coverage regression closed.** The canonical, zero-name-guess Reep-first
resolver now attaches **1,244** of the **1,576** official 2026/27 EPL
registrations across **all 20 clubs**, up from the earlier clean-run **952** and
above the older *incremental* research roster (**1,228**) — without copying any
of its guessed UIDs. The gap was **resolver over-strictness**, not absent
players. Four generalizable causes were fixed (unique Reep identity without
Transfermarkt; name-evidence-first disambiguation; family-anchored native-name
grading including 2-token names and spelling-variant middles; loan-duplicate
collision guard). Full write-up:
`docs/EPL_FM26_IDENTITY_RESOLUTION_FIX_2026-10-12.md`.

Verified final artifacts: `data/identity/reep/epl_canonical_clean_v10.json` and
the per-player ledger `data/identity/reep/epl_canonical_clean_v10_evidence.json`.

| Metric | Earlier clean (v3) | Fixed (v10) |
| --- | ---: | ---: |
| Official clubs / registrations | 20 / 1,576 | 20 / 1,576 |
| Attached FM candidate identities | 952 | **1,244** |
| Unresolved registrations | 624 | **332** |
| Unresolved **senior** registrations | 16 | **4** |
| Native player-kind `db` blocks admitted (name+DOB corroborated) | 425 | **442** |
| Individual FM traits quarantined (identity only) | 527 | 802 |

**Unresolved is now classified** (`identity_resolution_state`): **300** are
`absent_from_fm_and_reep_source_coverage` (no person in any of the 16 local FM
exports and no Reep FM bridge — mostly the post-snapshot 2026 youth intake;
spot-verified absent across every local CSV), **11** `ambiguous_reep_identity`,
**8** `ambiguous_native_homonym`, **13** `reep_candidate_not_corroborated`. The
four unresolved seniors are: Loureiro Da Costa (two `Joao Costa` tie), Ben
Wilson (two `Ben Wilson` tie), Alisson Becker (UID 19058734 has no name/DOB in
complete-v6 and is `Orlando Quintana` in `fm26_complete.csv` — a contested UID
is never attached), and Allan Andrade Elias (absent from all exports). Gabriel
Magalhães is deliberately left `ambiguous_reep_identity` rather than attached to
a different namesake.

**Identity is not attribute confidence.** Of 1,244 attached identities only
**442** carry a native player-kind `db` block with name **and** DOB
corroborated; the other **802** hold the identity with all individual FM traits
quarantined (`fm_native_record_trusted_for_this_player = false`). The
regenerated 248-column bundle `data/identity/reep/unified_epl_canonical_clean_v10.json`
has 20 teams / 1,576 players / 248 columns, `database_block` 442,
`reconstructed_distribution` 69, `missing` 1,065. It is a **feature contract,
not a trained or OOS-calibrated model**.

The standard roster CLI is the only production identity builder and reproduces
these results automatically (no hard-coded UID patchlist); `scripts/run_pl_pipeline.py`
invokes it. Reep is pinned at `20261005T180536Z`, SHA256
`f23fbce71e142dc76703a065bd14a820564222ca13a775e4b24e30bb648da8af`. The MLS
30-club research bundle is still `data/identity/reep/unified_mls_2026_10_11_30club_v1.json`
(791 SofaScore-observed players, **not** an official league roster); no official
2026 MLS registration source has been ingested.

## 1. Non-negotiable user requirements (compiled from this conversation)

1. **One unified EPL + MLS system**, not disconnected demos or several models contradicting one another. Reuse all primitives from performance, forecasting, recruitment, value, psychology, match simulation and transfer models. A simpler statistical model should be the baseline and can be the final model if it has stronger OOS evidence.
2. **Complete, up-to-date league rosters.** The EPL has **20 clubs**, not eight. All official senior registrations and U21 registrations must remain in the candidate universe, even if their FM UID or statistics are not found. Refresh rosters after transfer windows; mark registration, loans, club changes, injuries and match availability separately. The current September 3, 2026 EPL registration HTML is an **as-of snapshot**, not a perpetual source of truth. Aim at all **MLS teams and players**, not just recently observed clubs.
3. **The native FM26 dataset is real and available locally and/or on Hugging Face.** Use the complete FM26 records including technical, physical and **mental attributes**, CA/PA, positions, height, DOB, age, playing roles, development, and attribute provenance. FM labels can be name variants, expanded legal names, gender-partitioned export artifacts, or records misclassified as staff. Do not silently disregard those players or borrow a different person's attributes.
4. **Use Reep before FM name matching.** Download and pin the complete DuckDB from <https://reep.football/data/>. Search the whole Reep player identity register, canonical names, alternative names, redirects, and typed `fm:player`, `transfermarkt:spieler`, `sofascore:player`, `fbref:person` bridges. Use reverse provider lookups and full names. No primary exact-name-only matcher; no CA/market-value/fame tie-breaking to identify a person. If the official roster lacks stable person IDs, record that residual uncertainty.
5. **Understand Sortitoutsi versus FM UIDs.** A `sortitoutsi_uid` must be typed as Sortitoutsi, not silently taken as an FM native player UID. Use the explicitly verified `si_id -> binary_uid` table with native name/DOB corroboration. Search both `sortitoutsi_fm26_roster.csv` and `sortitoutsi_uid_map.csv`. Preserve the raw/source-specific ID, the canonical person ID and the selected native FM UID separately.
6. **Inspect and correct *every* remaining unmatched person, not merely summarize counts.** Search all FM source exports (including `fm26_complete_v6.csv` and the gender-partitioned files), Reep entries including those with no FM bridge, aliases, Transfermarkt player IDs, club and DOB, and the Sortitoutsi map. Produce a per-person candidate/evidence ledger, accept only defensible unique identities, and retain unresolved registrations with explicit reasons. Fix the **underlying matcher and end-to-end command**, not hard-coded UID patches. Re-run **all 20 clubs** after any resolver change.
7. **Missing FM mental/technical attributes must be recovered properly.** Prefer an exact native player's true database fields. When the record exists but attributes are missing, use the existing FM26 reconstruction/conditional-prior code to produce distributions, posterior means/intervals, masks and conditioning evidence, with clear distinction from observed DB attributes. If native UID points to the wrong name/DOB, **do not** use its traits, even if Reep connects the UID. If the original footballer is missing from FM26 entirely, keep a position/age/role conditioned prior until authentic evidence is available.
8. **Use all available contemporary and historical match data.** SofaScore player and team histories/ratings, FBref advanced features (access via `probberechts/soccerdata` where appropriate), Transfermarkt values/transfers, StatsBomb events for historical engine calibration, ESPN where useful, actual lineups/substitution logs, injuries, cards and dated team context. Historical publication timestamps must predate the prediction cutoff. Connect local and Hugging Face FM datasets; never assume the private HF repo is publicly accessible if auth fails.
9. **Every previously built model primitive:** forecast goal/shot/event rates and participation; fatigue, tactical switches and substitutions; red card advantage; set pieces; player-to-player interaction and co-start chemistry; team mentality, psychological traits, leadership and tactical fit; environment adaptation; player development; squad state; monetary/player valuation momentum and uncertainty; transfer hazard; historical analogues and 12/24/36-month **value forecasts as separate targets**. Include only demonstrably causal/as-of feature values in pre-match inputs.
10. **Use actual outcomes to establish trustworthiness.** Well-calibrated W/D/L probabilities, correct score distributions, goals, shots, xG, assists, cards, player event participation, substitutions and minute-by-minute pressure/momentum. Report uncertainty, prior strength and source quality. Prove OOS improvement and reliability; do not assume adding a transformer or extra features is automatically better.
11. **Investigate the Liverpool vs Manchester City momentum case.** The simulator appeared to identify City's period of maximum relative pressure, during which goals were subsequently scored. Determine whether this came from true predictive player/team interactions, timing structure, fatigue, tactics, observed substitution information, scheduled substitution artifacts, Monte Carlo coincidence, or hindsight. Quantify prospective predictive skill across many unseen matches and seeds, not that one anecdote.
12. **Technology and operations.** Rust match engine, deterministic simulation, robust numerical foundations, lean compute and good performance. Consider **Candle** for a conditional transformer/sequence model **only if** the data volume and measured OOS gains justify it. Consider **Modal** for reproducible remote parallel training/simulation when local compute becomes limiting; it is optional infrastructure, not a requirement to introduce larger models. Integrate parallel-agent work without overwriting uncommitted modifications.
13. **Deliver usable source and artifacts.** The automatic standard command should build the full official universe, resolve identities, safely prepare model inputs, run validated simulations and output comparable machine-readable results. Include a concise, complete development handoff with exact commands and code paths. Do not claim the feature bundle is already a trained unified model.

## 2. Current evidence and precise limitations

### 2.1 EPL official roster and candidate identities

- Official 2026/27 EPL registration HTML: `data/pl/raw/registered_pl_2026_27.html`; parsed by `scripts/match_premier_league_fm26.py` from [the Premier League 2026/27 lists](https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists). As of October 11, 2026: **20 clubs, 1,576 senior+U21 registrations**. The original **eight-club** file was only the fixture demo and must never stand in for league-wide coverage.
- Original FM **name-first** matcher: **1,051 provisional UIDs** out of 1,576. A Reep audit of those guesses does *not* discover missing names. Avoid using it again as the production algorithm.
- Subsequent multi-step, evidence-audited **Reep + raw-FM + TM** research: **1,162 candidate UIDs**, **414 unresolved** (**46 senior, 368 U21**), before the later recovery pass. From the 1,162 associated UIDs, only **552** were native **player-kind/db-sourced** individual trait blocks; 249 had reconstructed player attrs, 324 were classified staff and 37 lacked a record. *These numbers are a legacy research snapshot, not a guarantee of canonical clean-run coverage or true verified attribute count.*
- A subsequent per-candidate Reep/TM pass over that provisional research snapshot attached **66 additional candidate FM UIDs**, leaving **348 unmatched including seven senior**. Four categories were distinguished: supported identity bridge; native player attributes that pass independent name+DOB evidence; attributes not admissible; and rejected collisions. Stored in `data/identity/reep/epl_remaining_resolved_v1.json` and `.../epl_remaining_resolution_ledger_v1.json`. **These results start from an older provisional roster and MUST NOT be mistaken for independent clean-run coverage.**
- The **clean-from-official** canonical resolver, now invoked by the normal `scripts/match_premier_league_fm26.py`, initially produced **952 FM candidate IDs** (20 clubs); the coverage regression versus the older 1,228 incremental roster has since been **fixed** (see the execution update above and `docs/EPL_FM26_IDENTITY_RESOLUTION_FIX_2026-10-12.md`), reaching **1,255 attached / 321 unresolved** with the four documented causes corrected in generalizable code. Do not append old patched UIDs; every attachment must be reproduced by the canonical CLI.
- *Examples caught by the identity audit:* Cody Gakpo (expanded given names), Martin Ødegaard (Ø normalization), Đorđe Petrović (`Đ` transliteration and DOB), Iliman Ndiaye (`N'Diaye` apostrophe), Phil/Philip Foden, Rayan Cherki, Calvin Bassey, Reece James versus similarly named players, Mathias Jensen with duplicate names, Matheus Nunes with **contradictory native FM name**. The last must keep its identity evidence but **not borrow** another person's native mental attributes.
- The roster article is a **registration snapshot**. `loaned_out` and dated eligibility data must be preserved; don't assume every registered person is eligible, fit, in the country, or in the actual XI.

### 2.2 Reep, FM, MLS and ancillary data

- Full Reep DuckDB release `20261005T180536Z`, ~779 MB, SHA256 `f23fbce71e142dc76703a065bd14a820564222ca13a775e4b24e30bb648da8af`, stored locally under `data/identity/reep/` with `active.json`. `scripts/sync_reep_register.py` verifies a pinned release and SHA256. Reep contains 135,042 typed FM-player bridges, 26,602 SofaScore-player bridges, 15,991 FBref-person bridges and 392,745 Transfermarkt-player bridges. **It does not supply DOB for every Reep entity** and an official name->Reep candidate remains distinct from a source-issued identity match.
- Native FM paths: `/home/drewclaw/fm26-data/fm26_men_full_attrs.csv`, `/home/drewclaw/fm26-data/fm26_complete_v6.csv`, `fm26_correct.csv`, `fm26_final_men.csv`, `fm26_men.csv`, `sortitoutsi_fm26_roster.csv`, `sortitoutsi_uid_map.csv`, `work/ca_curves_smooth.json`. The **full v6** binary can include players absent from the gender-partitioned men export. It may also contain UID rows with empty names/DOB/attribute blocks: no invented certainty.
- Hugging Face data repositories referred to in development: `akks88/football-manager-data` and `akks88/footy-models-demo`; previous direct unauthenticated download got HTTP 401. Verify actual authentication and dataset versions when reconnecting, and never silently substitute a stale local copy.
- Existing 2026 MLS SofaScore sample: **27 observed clubs / 664 recently observed candidates** in the `2026-08-24` research bundle. This is **not** full MLS coverage. Full league rosters/team memberships, more provider histories and fixture chronology remain required. Historical SofaScore IDs: 931 observed; 616 bridged to Reep, 561 bridged to unique FM UID, of which just 370 were native player-kind DB attrs with DOB match in that audit; staff-kind, derived and DOB-conflict cases were explicitly separated.
- Existing unified research schema: **248 numeric/missingness-masked player features** shared by EPL and MLS, with typed auxiliary team graph and provenance. A common *schema* does not imply all 248 values are observed or that a single fitted model has been tested. FBref and market observation rows require independent dated ingestion; missing values stay null.

## 3. Source-of-truth architecture: one identity graph and one forecasting model

```text
Versioned official EPL / MLS registrations + as-of transfer/loan ledger
  -> Reep full DuckDB: labels, aliases, redirects, typed provider IDs
  -> independent native FM exports (men + complete binary), Sortitoutsi SI->FM
  -> person evidence: name variants, DOB, club/date, typed provider links
  -> identity ledger [verified / supported provisional / ambiguous / absent]
  -> unique FM/position/psychology provenance [DB observed / derived / prior / absent]
  -> provider histories: SofaScore, FBref, StatsBomb, ESPN, Transfermarkt
  -> one causal as-of player/team/fixture feature contract + masks/uncertainty
  -> lineup/availability/participation distribution
  -> calibrated time-varying goals/shot/pass/card/substitution hazards
  -> deterministic seeded football-match event simulation
  -> W/D/L + goal counts + correct score + minutes + player events + intervals
  -> independent value/development forecast heads (future targets, NOT inputs)
  -> strict rolling-origin OOS diagnostics and calibration/release gate
```

**Identity is NOT an attribute.** Require explicit typed keys (`fm26_uid`, `sortitoutsi_uid`, `transfermarkt_player_id`, `sofascore_player_id`, `fbref_person_id`, `reep_id`). A player may have a Reep-confirmed FM UID but a native FM row for a different name/DOB. Keep the FM UID as *candidate identity metadata*; set **all extracted FM trait values to missing** until corroborated. Validate native FM kind, DOB, name evidence, complete/partial trait block, vintage, duplicate UID and provenance. Never promote staff psychological ratings merely to raise player coverage.

**Attribute reconstruction:** If the correct FM person exists but ratings are missing, use `scripts/reconstruct_missing_fm26_attributes.py`, anchored to native FM/CA/PA/age/position/role distributions and age curves. Save an uncertainty distribution and `attr_source=conditional_prior`/`reconstructed` masks, not fictitious original database traits; do not reconstruct a *different person's* attributes after a UID collision. Ensure player-specific forecasts propagate missing/uncertain traits instead of treating prior means as certainty.

**Model inputs must be dated and published:** Observation time and `published_at` must be less than the prematch information cutoff. Use only 2026-vintage FM26 features for forecasts made after that FM snapshot; NEVER train historical 2015/16 match outcomes with 2026-version attributes as if they existed then. Older historical football seasons need contemporaneous features or a separate observed-stat model. Split training, validation and held-out evaluation by time and league; prevent identity/teammate leakage.

## 4. Every model and primitive to integrate, without feature inflation

| Primitive / module | Required outputs and legitimate as-of inputs | Status / limitation |
| --- | --- | --- |
| **Team strength and scores** (`footy-forecast`, match engine) | Calibrated attack/defense, home advantage, schedule, team form, bivariate score probabilities, W/D/L, goals, correct score | Rust engine and historical baselines exist; recalibration required across leagues |
| **Player ability & contribution** (`footy-state::AbilityVector`) | FM26 technical/physical/mental CA/PA, role and position strengths, measured minutes, xG/xA/shot creation, passing, defense; model masked players | FM attribute extraction partially unreliable; typed identity/attribute gating essential |
| **Psychological traits** (`footy-state::PsychologyVector`) | Teamwork, leadership, consistency, anticipation, composure, concentration, work rate, decisions, determination, pressure proxies with source tags | Stable FM traits possible; actual morale/pressure resilience and cohesion are NOT automatically measured |
| **Dynamic mental state** (`footy-state`) | Latent confidence, morale, pressure, composure response, motivation, posterior uncertainty; changes due to score, recent mistakes, cards, fatigue | Model as *latent* with conservative regularization; only add if validated |
| **Tactical fit & environment** (`footy-recruitment::EnvironmentFit`) | Role fit to actual dated manager formation, pressing/tempo/possession, league adaption, cultural and language fit where data genuinely exists | Many features missing; do not invent labels; avoid nationality-based identity proxies |
| **Player-partner chemistry** (`footy-match::cohesion`) | Dated historical co-starts, combinations, pass links/lineup familiarity, matchup-dependent interactions | Observational graph exists; quantify incremental OOS benefit versus simpler experience counts |
| **Player development** (`footy-state::DevelopmentVector`) | CA/PA realization, age curves, ability trajectory, role-dependent aging, uncertainties | True velocity requires multiple dated snapshots; do not extrapolate one FM26 rating as observed history |
| **Performance/form** (`footy-performance::PerformanceWindow`) | Exposure-adjusted per90 events, EWMA, form trend and variance, team strength, travel/fixture congestion | Use actual prior matches; missing must not mean 0; shrink small samples |
| **Availability, fatigue & starters** (`footy-state::UsageVector`) | Dated minutes 7/30/90 days, probable XI, minutes likelihood, injury/suspension, fatigue and recovery, bench depth | Registration != fitness != minutes; use probabilistic lineups before confirmed selection |
| **In-match tactics / event hazards** (`footy-match`) | Minute/second resolved possession/pressure, shot/goal/xG, red card, substitution chance, momentum, fatigue, set pieces, game state | Baseline exists; separate live observation from future simulation behavior |
| **Player event counts** (`footy-forecast::player_shot_count`) | Poisson/Gamma-Poisson or negative-binomial shots/assists/goals/card/event counts, exposure and competition priors | Prior research compared baseline and Gamma-Poisson; require larger strict holdouts |
| **Market & player value** (`footy-recruitment::surge`, `footy-market`) | Existing **47 value-feature slots**, dated market-value series, 180/365d momentum, volatility, age, league, transfer history, transfer hazard, price prediction intervals | The market-value **future** value/return/transfer is an auxiliary target; never feed hindsight value into match forecast |
| **Recruitment and squad value** (`footy-recruitment`) | Role scarcity, environment/complementarity, replacement value, 12/24/36m value scenarios, development, salary/fee if observed | Distinguish future-player-value forecasts from present football production |
| **Regime and nonlinear dependencies** | Optional Markov/HMM latent state, risk/volatility, nonlinear team-player interactions, attention/Candle temporal net | Research challengers only. Must beat calibrated parsimonious baseline with uncertainty measured |

No forced use of every primitive's **forecast** as a match input. Reuse the feature-generation contract and temporal inputs where causal and observed. The unified model can have separate *well-defined auxiliary heads* trained jointly only when that improves strict OOS goals/score/participation without future-target leakage.

## 5. Forecasting and simulation: required outputs

**Pre-match:** Match W/D/L probabilities summing to one; goals and correct-score PMF; expected goals/shots/on-target/assists/pass sequences/cards/corners; player shots/goals/assists/minutes; lineup probabilities and absence/formation scenarios; ranges, quality/calibration score, priors, prediction timestamp, source data versions and identity uncertainties. Users must be able to simulate all EPL and MLS fixtures and vary team news without changing recorded historical evidence.

**During-match:** A continuous-time or fine time-bucket conditional event engine. State includes score, minute, possession/phase, field position/pressure, player energy, substitutions, red cards, tactical shape, game incentives and player interactions. Simulate fatigue and tactical substitutions via **hazards conditional on current state**, not foreknowledge of what actually happened. For live updates distinguish observed lineup/cards/substitutions from counterfactual pre-match forecasts. Update W/D/L, total goals, next-goal probabilities, momentum and player event likelihoods as evidence arrives.

**Liverpool–Manchester City hypothesis test:** Archive the exact initial pre-match feature set and the originally simulated pressure path; rerun many independent seeds, remove scheduled hindsight substitutions, ablate fatigue, tactical effects and player-specific interactions, and test an entire chronological holdout of matches. Score **time-localized pressure vs subsequent shots/xG/goals**, proper goal-timing/log scores and reliability; compare against smoothed in-game score/time priors. One striking realization is evidence for investigation, not proof of genuine forecasting skill.

## 6. Critical code paths and current worktree caveats

| Purpose | Source |
| --- | --- |
| **NORMAL full EPL roster builder** | `scripts/match_premier_league_fm26.py` — now calls `resolve_official_roster` instead of legacy `best_match` |
| Canonical Reep-first resolution | `scripts/recover_epl_remaining_identities.py` — full aliases/labels, family/GIVEN variants, FM native and complete-v6 lookup, Sortitoutsi binary UID, typed Transfermarkt+DOB evidence, collision ledger |
| Reep download and cryptographic pinning | `scripts/sync_reep_register.py`, `data/identity/reep/active.json` |
| Alternate prior Reep researches | `scripts/resolve_epl_reep.py`, `scripts/resolve_epl_reep_first.py`, `scripts/apply_reep_first_epl_roster.py`, `scripts/audit_reep_crosswalk.py` — maintain historical audits but do not invoke patch chains as the production source of truth |
| Four-export unmatched audit | `scripts/reconcile_epl_fm_sources.py` and `scripts/promote_epl_fm_tm_evidence.py` |
| Unified 248-column research features | `scripts/build_unified_prematch_bundle.py` |
| FM attribute conditional reconstruction | `scripts/reconstruct_missing_fm26_attributes.py`; inspect source-level admissibility before using generated priors |
| Player participation/shot event studies | `scripts/build_historical_player_panel.py`, `crates/forecast/src/player_shot_count.rs`, `apps/matchforecast/src/main_shot_rate_eval.rs` |
| Gameplay/simulation | `crates/match/`, `crates/state/`, `crates/forecast/`, `apps/matchforecast/` |
| Recruitment/transfer/psychology/value | `crates/recruitment/`, `crates/market/`, `crates/performance/`, `crates/models/` |
| Standard EPL forecast adapter and run manifest | `scripts/prepare_pl_player_inputs.py`, `scripts/run_pl_pipeline.py` |
| Parallel historical/ESPN/HF work | `apps/matchforecast/math_engine/` and corresponding Rust binary files; *may be concurrently modified* |

**Important parallel worktree warning:** Other coding sessions have been actively editing `crates/forecast`, `apps/matchforecast`, the `math_engine` sources and some FM-reconstruction/unified-bundle files. Before editing, inspect `git status --short`, latest commits and uncommitted diffs. Stage **only intended code**, never `git add .`, `git reset --hard`, or overwrite another agent's work. Version local research datasets separately under ignored `data/identity/reep/` and don't delete frozen test or experiment evidence.

## 7. Clean reproduction, verification and release gates

```bash
cd /home/drewclaw/footy-models
git status --short --branch

# Reep release is already downloaded and pinned in active.json.
# Build the entire 20-club 2026/27 roster from official cached HTML,
# *not* an earlier name-matched roster or fixture-only shortlist.
uv run --no-project --with duckdb --with beautifulsoup4 python \
  scripts/match_premier_league_fm26.py --offline \
  --as-of 2026-10-11 \
  --output data/identity/reep/epl_canonical_clean_v2.json \
  --identity-audit data/identity/reep/epl_canonical_clean_v2_evidence.json

# Must satisfy: 20 clubs / 1,576 retained registrations / unique UIDs,
# complete per-person unresolved and rejection ledger. Compare this clean
# result with old incrementally recovered v1, never blindly merge old UIDs.
python3 -m unittest discover -s tests -p test_epl_canonical_identity_pipeline.py
python3 -m unittest discover -s tests -p test_epl_reep_first.py
python3 -m unittest discover -s tests -p test_epl_fm_multisource.py
python3 -m unittest discover -s tests -p test_epl_full_league_roster.py
python3 -m unittest discover -s tests -p test_unified_prematch_bundle.py

# New independent canonical audit and feature artifacts; use names which do
# not collide with previous immutable research outputs.
uv run --no-project --with duckdb python scripts/audit_reep_crosswalk.py \
  --squads data/identity/reep/epl_canonical_clean_v2.json \
  --output data/identity/reep/coverage_canonical_clean_v2.json
python3 scripts/build_unified_prematch_bundle.py \
  --league EPL --cutoff 2026-10-11 \
  --epl-roster data/identity/reep/epl_canonical_clean_v2.json \
  --reep-audit data/identity/reep/coverage_canonical_clean_v2.json \
  --output data/identity/reep/unified_epl_canonical_clean_v2.json
```

If a command fails, **fix the underlying code** and rerun with a new versioned output. Preserve the original snapshots; don't retroactively replace the data used in an old OOS run. If `uv` is unavailable in the execution environment, install the DuckDB and BeautifulSoup4 dependencies deliberately; never silently switch to name-first matching.

**Validation gates before claiming completion:** 20 official clubs; 1,576 roster entries; no silent omission from simulation candidate universe; zero false `Sortitoutsi == native FM` assumptions; zero duplicated person UIDs for unrelated simultaneous EPL registrations; DOB/native-name and kind cross-checks; completely excluded wrong-person FM attributes; all truly measured/derived/conditional attributes correctly masked; no future-derived features; deterministic fixed seeds and source hashes; out-of-sample reliability and calibration improvement over proper statistical baseline; observed and generated distributions explicitly separated.

## 8. Priority implementation roadmap

**P0 — Finish identity foundation (blocking).** *Substantially complete for the EPL.* The canonical Reep-first CLI now reproduces **1,244 attached / 332 unresolved** (4 senior) from raw official registrations, up from 952, with the four over-strictness causes fixed in generalizable code and regression tests added (`docs/EPL_FM26_IDENTITY_RESOLUTION_FIX_2026-10-12.md`). Remaining: 300 are absent from all 16 local FM exports (post-snapshot 2026 youth) and 32 are genuine homonym/ambiguity cases — attach only with a newer FM snapshot or a dated club-corroborated provider ID, never by a name heuristic. Keep the regular EPL CLI the only production identity builder. **MLS is still open**: no official 30-club registration source has been ingested (only the 791-player SofaScore-observed universe).

**P1 — Accurate true-vs-imputed FM primitive source.** Verify native named `db` blocks in men and complete-v6 against DOB and person evidence; detect row kind misclassification without automatically promoting staff attrs; code probabilistic reconstruction for correct-but-missing FM players. Integrate observed FM mental features, role vectors, age, CA/PA, uncertainty, lineup/squad mental aggregates and season membership in EPL and MLS. Reject future-vintage leakage.

**P2 — League-complete live data.** Ingest all 2026/27 EPL/2026 MLS squads and current transfers, official fixtures, match-level and per-player SofaScore, FBref (`soccerdata`), ESPN, accessible StatsBomb, TM market histories and detailed substitution/lineup/availability logs. Resolve typed Reep IDs at ingestion, not via names at feature join. Store event timestamps/publication times. Maintain regular refresh and report team/player coverage; don't infer MLS completeness from the 664-observed-player sample.

**P3 — One calibrated model and event engine.** Make a parsimonious generative/statistical core with exposure-aware player rates, lineup/availability uncertainty, time-varying team/player hazard interactions, home advantage, substitutions/fatigue and red cards. Couple Rust simulator outputs to the single feature schema; remove self-fulfilling controllers and artificial pressure schedules. Conditional team tactical/psychological factors only where measured or modeled with uncertainty.

**P4 — Value, development, fit and recruiting.** Attach all 47 causal valuation primitives; independently train and calibrate market-value development/transfer-risk horizons, tactical and squad fit/chemistry as auxiliary tasks. Only adopt a shared representation when OOS match forecasting improves. Keep future value/return/transfer as distinct targets.

**P5 — Scientific validation / scale.** Rolling-origin held-out proper scores (WDL multiclass log loss/Brier/ECE, score PMF, count likelihood, player event accuracy, goal timing calibration), ablations and bootstrap uncertainty per league/season. Investigate Liverpool-City momentum as a hypothesis across OOS games. Optional Candle/transformer and Modal compute are **challengers** requiring evidence and performance benchmarks. Seed and snapshot reproducibility, Rust engine efficiency, concise documentation and deployment-ready model provenance are required.

## 9. Continuation prompt for a new agent/session

```text
Continue development in GitHub repository andrewkoumoudjian/footy-models on
the latest main, preserving ALL concurrent uncommitted work. Read
docs/EPL_MLS_UNIFIED_FOOTBALL_MODELS_HANDOFF_2026-10-12.md first, along with
docs/reep-identity-register-2026-10-11.md and
docs/unified-epl-mls-forecast-primitive-contract.md.

Implement, do not merely audit. Priority P0: rerun the canonical, Reep-first
official 20-club EPL FM26 identity builder from raw registration HTML;
identify and fix clean-run coverage regression versus earlier incremental
research; include the complete gender-agnostic FM26 binary v6 export and
Sortitoutsi->binary UID mappings. Search every unmatched senior + U21 name
in the entire Reep register (including people without FM bridges), actual
FM exports, aliases, TM typed provider IDs and dated DOB/club evidence.
Never append guessed or hard-coded player UIDs. Keep identity, native player
record and observed/derived psychological attributes as SEPARATE trust gates.
Report all 20 team counts and unresolved names; require no dropped names,
collision-free typed identities and explicit unknowns.

Then regenerate the unified 248-column EPL/MLS features and finish historical
SofaScore/FBref/ESPN/provider data joins. Model all valid technical, physical,
psychological, tactical, chemistry, development, value, market and injury/
participation primitives in one calibrated probabilistic football engine;
auxiliary future player-value forecasts must never leak into match inputs.
Improve minute-level game simulation (fatigue, substitutions, cards, tactical
switches, pressure/momentum) and scientifically test the Liverpool-Man City
momentum coincidence with prospective ablation/OOS evaluation.

Use simpler interpretable statistical baselines before Candle transformers;
Modal compute only if necessary. Preserve fixed snapshots, do not infer true
MLS completeness from the older 27-club sample, and add regression tests for
all recovered examples. Commit focused source/docs/test changes to main,
leaving other agents' uncommitted work intact. Document exact outputs,
reproduction steps, remaining limitations and OOS evidence. Do not call a
feature schema a trained/calibrated model.
```

## 10. Definition of done

**Data/identity:** exact current full EPL and MLS roster universes; all available players searched; each unresolved candidate has auditable evidence; namespaced IDs; true vs derived vs unavailable FM abilities clearly separated; no wrong-person ratings. **Features:** every legitimate primitive available with provenance, cutoff and masks; no unobserved pseudo-psychology. **Simulation:** same validated Rust engine for fixtures and events; calibrated predictive probabilities and correct score/event distributions, dynamic substitutions/fatigue/cards/tactics, tested momentum. **Proof:** repeatable chronological OOS tests against a parsimonious baseline; honest performance/uncertainty. **Operations:** one canonical normal command, reproducible versioned artifacts, protected old baselines, complete written handoff, source-only focused commits.
