# Footy Models — Unified EPL/MLS Forecasting: Complete Conversation Handoff
**Version:** 2026-10-12 • **Repository:** https://github.com/andrewkoumoudjian/footy-models • **Owner's request:** execute full data/model pathway, preserve parallel agent work, and continue in another session without repeating this discussion.

> **First thing for the next agent:** THIS IS A FULL IMPLEMENTATION HANDOFF, NOT A CLAIM THAT THE LATEST PIPELINE HAS RUN. Read this document, inspect actual HEAD and uncommitted work, unblock remote terminal, execute full 20-club EPL + 30-club MLS collection and feature builds, verify native attributes, and proceed into properly labeled modeling and OOS evaluation. Do not reuse old eight-club inputs as a full EPL league roster or misrepresent name-based matches as verified identities.

## TL;DR — what the user wants

Build **one coherent, player-agnostic, team- and match-contextual probabilistic football forecasting engine** focusing initially on the **Premier League and MLS**. It must learn transferable relationships between **all available** SofaScore ratings/events/team stats, FBref player statistics, FM26 individual technical/physical/**mental** attributes, live as-of availability, tactical roles, development, psychological fit, historical co-playing chemistry, market/player value forecasting primitives, and recent historical player/team form. Predictions must include match outcome, goals/scoreline, shots, player events, playing minutes/lineups, and **time-evolving in-match momentum**, with fatigue, red cards, substitutions, tactics and within-match interactions. Use the existing Rust probabilistic event simulator for internally consistent joint outcomes. The **mathematical pathway** is owned by this agent: interpretable statistical/physics-inspired models, tractable inference, properly regularized GLMs, shrinkage, quantile/conformal calibration, state-space methods and efficient zero-shot/pretrained methods where justified; **no deep-ML training in this math pathway**. Another independent neural/Candle/transformer track may exist, but must be benchmarked honestly rather than silently mixed into the math challenger.

Crucial user requests, repeated many times:
1. **Do the work, do not only write plans.** Fetch/update data, investigate every unmatched player, run the pipeline, measure actual coverage and output evidence.
2. **All EPL and MLS players:** full official EPL universe across **20 clubs** (currently 1,576 official registration rows, including U21), not the old 600-player eight-club fixture subset; MLS has **30 clubs in 2026** (existing 664-person SofaScore archive spans only 27 historically observed clubs). Build all current squads first; fixture-specific candidates must be a separate subset.
3. **Use Reep FIRST for identities.** Reep v1 DuckDB, typed provider namespace bridges, redirects, aliases, overlays, Transfermarkt, Sortitoutsi binary UID map, and all FM source exports; name/DOB are corroboration or fallback, not the primary cross-provider key. Do not assume two different provider IDs share a namespace.
4. **Find as many people as possible in FM26's 500k+ rows**, remove CA and youth filters for identity discovery; investigate difficult aliases, expanded legal names, nicknames, transliterations, FM UID collisions, staff-kind misclassifications and records missing from the men-only export.
5. **Recover the real FM attributes first**, including mental/hidden traits where stored. Where attributes truly are missing, use the **existing empirical FM26 conditional-generation code** and confidence distributions; never label a generated value as an observed personal trait or impute someone else's traits.
6. **All available primitives from other models** should be considered: player market-value forecasting, psychology/mental-state and professional fit, tactical and environmental fit, team chemistry, formation and playing-time paths, development, risk/uncertainty, form/momentum regimes, player-event rates. Strict pre-match timestamps, permissions, and missingness masks.
7. **One shared player-agnostic model** across EPL and MLS, not one separately fitted model per athlete, with rigorous proper-scoring out-of-sample evaluation. Model semantics should be causally meaningful, computationally efficient, robust, deterministic and explainable.

## Status by evidence level (do not conflate)

| Item | Grounded latest status | What it does NOT prove |
|---|---|---|
| 2026/27 official EPL registrations | 20 clubs, **1,576 registration entries** including U21, September 3 source, snapshot dated October 11 | Not necessarily 1,576 unique currently active footballers; loan/parent duplicate listings need eligibility gate |
| Old fixture sample | 8 EPL teams, 600 entries | NOT a full EPL universe |
| Reep-first EPL search v4/v5 | 1,576 processed; previous strict pass 835 FM identities; subsequent v5 returned **868** candidate FM UIDs before other parallel reconciliation | Not a final exhaustive resolution of all players |
| Separate latest multi-source EPL reconciliation | **1,228 provisional FM UID assignments, 348 unresolved registrations, 7 unresolved senior registrations** in local epl_remaining_resolution_ledger_v1.json | Not 1,228 native FM attribute profiles, not full official person-ID verification |
| Latest all-source direct Reep audit | Local epl_remaining_resolved_v1.json, epl_remaining_resolution_ledger_v1.json and coverage_full_all_sources_v1.json | Some tentative FM UIDs can have wrong native names/DOB: absolutely quarantine associated traits |
| Older FM-generation/full-bundle path | Full 1,576-player 248-column EPL bundles v4 through v7 were built; v6 provisional split: 709 observed/stored attributes, 100 existing derived, 26 newly generated, 741 missing IDs | These are results of **older 835-ID** identity inputs. DO NOT copy the counts to the newer 1,228-ID roster without rerunning native identity/attribute checks |
| Earlier MLS player-set bundle | 664 candidates across 27 historically observed clubs, including 296 DOB-verified FM DB attribute records plus 7 reconstructed in that historical version | NOT all 30 MLS club rosters nor proven current active team assignments |
| Shared feature contract | Same **248 numeric/missingness columns** in EPL/MLS, with stable feature metadata, team context and as-of provenance | Not a trained or calibrated model |
| Shared math GLM challenger | Start Bernoulli, on-field shot Poisson, conversion binomial, pooled across persons/leagues, no ID coefficients | No valid new 2,240-row multi-league training panel; static roster matrices have no labels |
| Rust match mathematical path | Exact 1GK + 10 outfield stochastic XI, substitution and path-consistent shot-to-goal event process, earlier historical ablations | Not validated as a full live EPL/MLS forecasting product |
| Running requested final pipeline in THIS handoff turn | **BLOCKED:** development terminal returns "too many retained terminal sessions (limit 64)" on new exec; read-only files still accessible | Do NOT claim a completed latest-run verification or tests |

Reep is a **cross-provider identity index**, not a complete roster or FM attributes service. Official data: https://reep.football/data/ ; handbook: https://reep.football/handbook/ ; v1 current release 20261005T180536Z with approximately 2.04m total entities, 10.04m bridges, 135,042 published FM player bridges (local full-audit count). Reep's own public player identities and aliases are incomplete and do not supply complete public club-career histories. Typed IDs and redirects are mandatory; do not mix the v0 reep_p IDs with v1 rp IDs.

## Data sources — requested explicitly, with mandatory provenance

### Identity and roster
- **Official EPL:** https://www.premierleague.com/en/news/4706139/see-all-the-202627-premier-league-squad-lists ; all 20 clubs, registered/U21 rows, September 3, 2026 publication. Capture current transfer, injury, departure and loan status separately. Do not treat registration as automatically matchday eligible.
- **Official MLS:** 2026 competition has **30 clubs**; https://www.mlssoccer.com/league-reports/competition-guidelines/ . Collect date-stamped current player/club registrations and player-history records for ALL 30; existing 27-team history is not full-roster coverage.
- **Reep v1:** local read-only DuckDB data/identity/reep/reep-20261005T180536Z.duckdb (publisher SHA256 f23fbce71e142dc76703a065bd14a820564222ca13a775e4b24e30bb648da8af), metadata data/identity/reep/active.json . Tables: entities, players, aliases, redirects, bridges, overlay links/names, observed clubs. Typed bridge key is provider + namespace + external_id. Reep itself does not license downstream commercial data.
- **FM26:** full binary-derived /home/drewclaw/fm26-data/fm26_men_full_attrs.csv (~563,382 rows), PLUS /home/drewclaw/fm26-data/fm26_complete_v6.csv and other exports; https://huggingface.co/datasets/akks88/football-manager-data/blob/main/data/fm26_men.csv .
- **Sortitoutsi:** https://huggingface.co/datasets/akks88/football-manager-data/blob/main/data/sortitoutsi_fm26_roster.csv and https://huggingface.co/datasets/akks88/football-manager-data/blob/main/data/sortitoutsi_uid_map.csv . Confirm binary_uid vs SI UID, proper name/DOB and explicit edge provenance; do NOT equate raw integer IDs across providers.
- **Transfermarkt:** https://huggingface.co/datasets/akks88/football-manager-data/blob/main/transfermarkt/transfermarkt-datasets.duckdb ; local dated Transfermarkt joins and Reep Transfermarkt spieler bridges. Use date-constrained club history/DOB/transfer evidence to distinguish possible names; market values must have snapshot timestamps.
- **Other identity helpers:** Reep's Soccerway/Opta/Wikidata/SofaScore/FBref aliases, current roster official IDs from authorized sources. One canonical player can legitimately have multiple club registration entries due to loans. A Reep collision can ALSO merge two different people: review separately.

### Football observation / model inputs
- **SofaScore:** advanced recent and historical **player and team** ratings, player event xG/xA, shots, passing, creating/finishing, defenders, goalkeeper, possession, tackles, duel wins, game-by-game ratings, minutes, substitutions, confirmed starting XI and lineup, match and team trends, dated historical form. Must match via native SofaScore ID → Reep canonical player and reject after-target observations. MLS local event archive already exists but needs refresh; EPL has been 0 recent events in the final 20-club bundle.
- **FBref:** player match and historical advanced/basic stats (shots, SOT, npxG, xAG, progressive actions, defense, minutes), preferably through user-mentioned https://github.com/probberechts/soccerdata/ and rights-cleared data. Historical user-provided https://huggingface.co/datasets/roirani80/fbref-xg-analysis-2024-2025/blob/main/fbref_cleaned_with_features.csv and https://huggingface.co/datasets/aloobun/fbref_understat_combined provide season-level historical observations. **A prior full-season aggregate is NOT 10 previous matches.** Existing sidecar matched 254 current EPL Reep ID candidates to older FBref seasons; 229 joined to Understat xG. These are unverified current official-roster ID candidates, and those sources contain no current MLS match observations. FBref advanced data availability and training rights must be reviewed.
- **Other public source:** ESPN for dated team schedules, rosters, results, lineup metadata where its terms permit; football provider stats must not be fabricated.
- **FM mental:** Anticipation, Decisions, Composure, Concentration, Determination, Consistency, Important Matches, Bravery, Aggression/Dirtiness, Teamwork, Work Rate, Leadership, plus psychological professionalism/adaptability/ambition/pressure/discipline where genuinely present. Include technical and physical traits, CA/PA, height/age/role, level, development. These cannot be replaced by one rating sum; input individual attributes and lineup/team aggregates, number observed, uncertainty.
- **FM empirical reconstruction:** https://huggingface.co/datasets/akks88/football-manager-data/blob/main/data/attr_generation_curves.json ; local /home/drewclaw/fm26-data/hf_upload/data/attr_generation_curves.json , original /home/drewclaw/fm26-data/work/generate_attrs.py and positional priors /home/drewclaw/fm26-data/work/pos_pop.json . Position+CA conditional distribution trained on actual FM observations. For a missing trait, use conditional mean/variance/p10/p90 with generated flags; for unknown role use a marginal mixture; with missing identity/CA do NOT manufacture a personal 1–20 value. Keep dynamic morale distinct from fixed traits.
- **Value and psychological fit:** measured as-of Transfermarkt value, value-history changes, volatility, previous transfers, contract where sourced; player-value forecast for 12/24/36 months, transfer hazard and scouting fit as **auxiliary predictions**. Tactical environment, leadership, complementarity and chemistry require measured positions, expected XI and observed co-start patterns. Future value or post-match target columns cannot be slipped in as current-match features.
- **User Hugging Face demo:** https://huggingface.co/datasets/akks88/footy-models-demo . Maintain exact snapshot provenance.

## Identity algorithm and critical findings

### Why hundreds were initially missed
- Code wrongly restricted FM search by CA >=50 and only a few name tokens; excluded low-CA youth and nonstandard names.
- Old workflow matched names first then attached Reep as an audit; user explicitly rejected that. **Invert it:** Reep provider-ID discovery first, use aliases, then FM UID, then independent name/DOB/club checks.
- Official legal name variants matter: Phil Foden / Philip Walter Foden; Rayan Cherki / Mathis Rayan Cherki; Calvin Bassey / Calvin Chinedu Bassey; Mykhailo Mudryk transliteration; Bruno Guimarães, Alisson Becker, Evanilson, Igor Thiago, Amad Diallo.
- FM source partition and quality: at least 27 unique Reep-linked FM IDs absent from the men attribute export are present in the additional fm26_complete_v6.csv; *do not automatically attach traits*, since some local UID→native name conflicts are observed.
- 16 native UID values were duplicated in the original 563k-row export, sometimes over unrelated people; quarantine collisions, never last-row-wins.
- **FM kind=staff is NOT conclusive absence of a footballer identity.** Of 281 staff-classified records, many contain real binary DB attributes; however the official-player-to-native-record relationship still requires independent corroboration. Previous v6 conditional output counted 454 player-kind DB blocks, 221 complete staff-kind DB blocks, 34 partial staff-kind DB blocks, 100 existing reconstructed, 26 newly generated and 741 missing; these numbers are superseded by the newest identity ledger and must be recomputed.
- Some Reep entity names and Reep→FM UID associations match an entirely **different native FM person**; UID can be retained as identity candidate while ALL mental/technical/CA trait inputs remain null. Need native legal name + corroborated DOB/TM/SI and competition context when appropriate. No false hardcoded name acceptance to reach 100%.

### Identity acceptance levels (explicit separate counts)
1. OFFICIAL_REGISTRATION_ONLY — known on roster; canonical official ID; provider IDs unknown.
2. REEP_PROVIDER_PERSON_CANDIDATE — Reep entity found by nickname/name/alias; no issuer verified person ID.
3. REEP_TYPED_FM_UID_CANDIDATE — exact Reep fm/player namespace bridge found; may not match native FM row.
4. NATIVE_IDENTITY_CORROBORATED — native name and DOB independently consistent with TM or SI and official legal identity; no duplicate UID collisions.
5. FM_ATTRIBUTE_BLOCK_OBSERVED — native individual FM DB attributes admitted only after proper native-person check; zero/unassigned traits missing, partial block per-field observed masks.
6. FM_ATTRIBUTE_BLOCK_DERIVED — existing source-generated realization or newly generated conditional distribution with **both confidence/uncertainty and source generation method**.
7. NO_PERSONAL_FM_PROFILE — player retained, missing FM attributes; team context and other observation feeds still available.

Ledger must include: official club and registration type, source URL/date, canonical Reep person and all typed provider links, alias evidence, SI/FM UID, TM ID, native FM row kind/name/DOB, collisions, whether source traits admitted, generation provenance, reason for rejection, and next source candidates.

## Entire existing model primitive inventory (reuse before inventing)

| Module / code | What to consume / benchmark | Boundaries |
|---|---|---|
| crates/state, PlayerState, PsychologyVector, AbilityVector, DevelopmentVector, RoleVector, UsageVector | player abilities, stable mental, form, development and usage with confidence | no future morale or invented hidden psychology |
| crates/performance, PerformanceWindow, TeamPerformanceWindow | player/team rolling as-of means/variance/trend, observed ratings | no same-match ratings or post-cutoff aggregates |
| crates/recruitment, tactical_fit, squad_complementarity, EnvironmentFit, historical_analogues, surge features | role fit, manager requirements, transfer/adaptation, psychological fit and complementarity; valuation features | fit only when dated context exists; future return labels are not predictors |
| crates/market, MarketPerceivedValue, PriceDistribution, MarketIndex, TransferHazard | dated market history and auxiliary player-value / transfer forecasts | do not sneak future player price into present match X |
| crates/forecast, NextMatchPlayerRateModel, EwmaVolatility, Garch11, Mf2Garch, Rng64 | player shot/usage rate baseline, regime variance and seeded simulation | no unsupported high-frequency precision |
| crates/models, FeatureTransform, RidgeOls, fit_logistic_irls, fit_huber, ConformalRegressor | interpretability, missingness, regularization and predictive intervals | standardization only on training period |
| crates/match, exact lineup DP and Rust marked event engine | valid XI, substitutions, event rates, shots, goals, cards, moments, proper joint scoreline | no independent inconsistent forecasts |
| apps/matchforecast/math_engine/shared_math_glm.py | one pooled EPL/MLS start–shot–conversion statistical challenger | untrained on full latest labeled panel |
| scripts/build_unified_prematch_bundle.py and 248 numeric columns | canonical player and team pre-match feature contract | one date per fixture; missingness vs zero |
| scripts/fit_fm_mls_candidate_glm.py, scripts/score_fm_mls_candle.py | other branches/agents' candidate models and comparisons | no uncontrolled promotion, compare matched as-of OOS |
| docs/QUANT_FUND_PRIMITIVES_REUSE.md | optional efficient covariance/state/HMM/GARCH ideas inspired by other math primitives | consider only if justified by OOS improvements |

Team psychology and chemistry: do not merely SUM observed mental traits uncritically. Engineer lineup-weighted means, variances, leadership/teamwork extremes and observed-counts, pair co-start network, stability, previous-match interaction, position/formation complementarity; propagate lineup probability and missing-attribute uncertainty. Team average/sum **can be a covariate** as the user requested, but individual and pair interaction information should remain available.

## The SINGLE probabilistic mathematical forecasting architecture

**Unit of training:** strictly as-of candidate-player × upcoming fixture, with team-level match and opponent context. **Shared pooled parameters across all players** and both competitions; league flag and latent scale/context effects allowed, but **NO identity embedding, per-player optimized intercept, per-player fitted GLM**.

One structured conditional model parameter vector (can have multiple linked heads under shared statistical framework):
1. Candidate availability, start probability; exact-cardinality 1 GK + 10 fielders via maximum entropy joint selection, conditioned on independently dated roster/fit/position and capped by projected active squad.
2. Conditional playing time and substitution appearance hazards, fatigue load and role-aware tactical substitution, preserving player event counts and rule-era caps.
3. Player shot-attempt marked point process conditioned on on-field minutes, teammates/opponents, tactical role, SofaScore/FBref performance and stable FM skills; use Poisson/negative-binomial if dispersion demands.
4. Shot quality/xG mark distribution, then conditional goal conversion with Beta-binomial/logit calibration, keeper/defender effects; resolve goals via one path, no ad hoc independent per-player goal probability distribution.
5. Cards/fouls/discipline, red-card manpower, substitution, possession/field tilt, fatigue and tactical change as explicit state transitions; live in-match updating using only realized prior events.
6. Team/lineup-level chemistry and psychological fitness: learned interaction terms, nonlinear but interpretable partial pooling (e.g., shape restrictions, GAM/spline, low-rank interactions, cross terms) when supported; volatility/uncertainty propagation. Never equate SofaScore rating with private morale.
7. Match/scoreline/WDL and player shot/goals/assist/etc outputs **jointly derived from the same stochastic paths**; uncertainty bands, probability calibration and deterministic reproducibility.
8. Player market value forecast (12/24/36 months), performance potential, transfer risk and recruitment/psychological fit are **auxiliary separate target heads** with shared information and strict time labels, not contemporaneous observed inputs. User explicitly asked that existing player-value forecasting and psychological-fit primitives be included.

Mathematical methods encouraged: Bayesian hierarchical shrinkage, regularized GLM/GAM, survival models, maximum entropy exact selection, explicit causal hazard updates, continuous-time marked point processes, Poisson/negative-binomial mixtures, state-space/HMM only if data-supported, variance/volatility and low-rank pair interaction models, reliable zeroshot inference libraries if suitable. No unnecessarily deep ML in this pathway. Optionally evaluate pre-trained tabular models (e.g. TabPFN-style zero-shot) where legally/licensing/performance appropriate, without redefining math-path no-deep-training requirements; label them distinct challengers.

The user previously highlighted a Liverpool–Manchester City simulation anticipating a City momentum period in which goals subsequently arrived. **Investigate whether it came from real predictive event intensities, scheduled substitution artifacts, simulated noise, overfitting, or hindsight.** Run alternative seeds, distribution of momentum peaks, timestamp-aligned future-event labels and out-of-sample calibration. Do not cite one coincident successful game as validation.

## Scoring, leakage rules and benchmark gates

- Features are only admissible if observation EVENT timestamp and PUBLICATION/availability timestamp precede forecast cutoff. Freeze source vintage, identity graph, data hashes and published labels. Strictly avoid same-day or future season totals, retroactively collected player metrics in historical as-of forecasts.
- Model fitting uses full dated EPL/MLS player-match examples, not a static 2,240-row as-of roster feature snapshot. Fit transform ONLY on training data. Fixed chronological splits with real untouched later seasons/matches; avoid 2015/16 inspected 25-game selection as final test.
- Score start Brier/log loss; appearance and minutes calibration/MAE; shots log-likelihood/count calibration; shot-to-goal log loss; xG, goalkeeper/discipline where available; goals/scores via joint path; WDL log loss and Brier; reliability diagrams; calibration slope/intercept, uncertainty, matchday-clustered bootstrap; coverage/bias by team/league/position/youth/missing FM source.
- Baselines: basic Poisson / Dixon-Coles team goal model, prior player-rate/shrinkage model, existing Rust baseline, market odds (when available and licensing permitted), unchanged model versus added FM traits and chemistry (ablation). Compare at matched dates/data coverage/paths/seed and do not claim a boost without OOS proof.
- Track latency, memory, energy, reproducibility and efficient in-process Rust paths; no inventing 2026 results or scores.
- Earlier 25-fixture StatsBomb 2015/16 research (2,500 paths each) saw minutes MAE improve from about 27.16 to 24.32 with selection + substitutions, player appearance improve modestly, but WDL log loss worsened. Retrospective, inspected and nonrepresentative: not a promotion basis.
- Training and public reuse of FBref/SofaScore/ESPN/Transfermarkt content requires provider-specific permission review. Respect rate limits and terms. Reep's CC0 crosswalk does NOT override originating provider rights.

## Critical FULL DATA pipeline procedure for next agent

### 0. Recover the development terminal and protect concurrent agents' files
The primary tool mcp__Drewclaw__exec_command currently fails with: "too many retained terminal sessions (limit 64); drain a returned session with write_stdin before starting another." A separate Drew terminal bridge was unavailable because its tunnel-client had disconnected. This blocker prevented the current user-requested full execution, tests and commits. First recover session/terminal state **without killing other agents' live jobs**, or use an explicitly authorized replacement compute session. Refrain from claiming results not executed. Check:

~~~bash
git status --short --branch
git log -8 --oneline
git fetch origin main
git diff --check
~~~

Do not reset, clean, hard checkout, overwrite ignored data, or indiscriminately git add -A. Main may have advanced due to parallel agents. The handoff was committed remotely separately while local uncommitted work was preserved.

### 1. Identify the most current full EPL evidence, not the older v4/v7 snapshot
Read:
- data/identity/reep/epl_remaining_resolution_ledger_v1.json (summary: 1,228 matches / 348 unresolved / 7 seniors)
- data/identity/reep/epl_remaining_resolved_v1.json (20-club full roster)
- data/identity/reep/epl_full_all_sources_v1.json
- data/identity/reep/coverage_full_all_sources_v1.json (Reep / typed FM candidate audit)
- scripts/recover_epl_remaining_identities.py ; scripts/resolve_epl_reep_first.py ; scripts/reconcile_epl_fm_sources.py ; scripts/promote_epl_fm_tm_evidence.py
- scripts/build_unified_prematch_bundle.py (contains latest UNCOMMITTED native-person/attribute trust checks; inspect before editing).

Investigate and resolve the last 7 seniors explicitly using typed Reep, Transfermarkt, SI and official IDs; report row-level reason and avoid false-attribute matching. Keep 348 unresolved registrations, mostly younger, in the numeric feature set with null masks. Look for other Reep release versions or trustworthy IDs rather than forcing 100% through fuzzy names.

### 2. Actual complete EPL pre-match feature rebuild
The bundle CLI uses flags --league, --cutoff, --epl-roster, --reep-audit, --fm-file, --fm-complete-file, --fm-attr-priors, --output. The **latest all-sources roster is NOT necessarily tagged identity_rebuild=reep_first_all_20_clubs_v1**, and the generator priors sidecar may be keyed to older v5 Reep result; **do not force-feed older priors into new identities**. First run WITHOUT incompatible old prior file; recompute generated priors based on the new identity ledger only after fixing the provenance schema.

~~~bash
python3 scripts/build_unified_prematch_bundle.py \
  --league EPL --cutoff 2026-10-11 \
  --epl-roster data/identity/reep/epl_remaining_resolved_v1.json \
  --reep-audit data/identity/reep/coverage_full_all_sources_v1.json \
  --output data/identity/reep/unified_epl_2026_10_11_latest_identity_gated_NEW.json
~~~

The filename NEW is a placeholder: choose a fresh output path, confirm no overwrite. Compute and SAVE actual count: 20 teams, 1,576 official rows, exactly 248 stable feature names, unique/colliding Reep persons, 1,228 provisional FM UIDs (or new verified improvements), observed complete and partial DB profiles, reconstructed, missing FM, native conflicts rejected, missing SofaScore/FBref/market values. **Cross-check that no native FM trait from a known different person is used.** The current experimental attribute-admission guard has not been tested on this latest roster.

### 3. Expand/refresh MLS to ALL 30 clubs
The existing observed MLS bundle data/identity/reep/unified_mls_v1.json is 664 player candidates/27 clubs, and a short-horizon rebuild produced only 483 players from 22 clubs due to staleness. These are not adequate official 2026 squad universes. Collect current 30-team official club rosters and dated provider IDs. Preserve old history separately and join through Reep; never drop injured or seldom-used officially registered candidates because they lack recent SofaScore activity. Include all known registered candidates, even when no FM or match stats. **Do not invent an MLS roster total**; compute it after fetch. Add missing-three-club coverage diagnostics. Match FM26 identity+traits with the same safety gate used for EPL.

### 4. Ingest SofaScore, FBref, markets into both leagues
Connect/match dated IDs, collect team and player recent/historical events and ratings, lineups, usage, start/minutes, tactical role, errors and nulls. Full-season historical HF FBref is separate from live recent-ten. Source rights and timestamp/replay safety required; use every available legitimate recent and historical primitive. Persist per-provider completeness, source hash, ingest time and available-as-of date, with full per-player missingness.

### 5. Generate labeled snapshots and train ONE shared math model
Rebuild the fixed 248-column input **per historical fixture** before kickoff, with actual outcomes separately joined after. One shared pooled EPL+MLS GLM/GAM/marked-hazard model; player agnostic, no identity lookup. Add FM mental, lineup-weighted psychological/team chemistry and value primitives where valid. Score and compare proper likelihood/calibration against same-engine baseline, including momentum alignment. Interface predicted conditional rates/selection marginals into the existing Rust mathematically consistent simulator. No uncontrolled neural training, protect untouched OOS.

### 6. Tests, hashes, branch/commit and handoff report
~~~bash
python3 -m unittest discover -s tests -p 'test_reep_epl_full_recovery.py' -v
python3 -m unittest discover -s tests -p 'test_unified_prematch_bundle.py' -v
python3 -m unittest discover -s tests -p 'test_reconstruct_missing_fm26_attributes.py' -v
python3 -m unittest discover -s apps/matchforecast/math_engine -p 'test_*.py' -v
python3 -m py_compile scripts/build_unified_prematch_bundle.py \
  scripts/resolve_epl_reep_first.py \
  scripts/recover_epl_remaining_identities.py \
  scripts/reconstruct_missing_fm26_attributes.py
git diff --check
~~~

Additionally run available strict Rust compilation/test suites only for files touched and compare full features. Assert no dropped registrations, no duplicate unique canonical persons in same active roster (loan duplicates marked separately), observed-trait masks consistent, frozen provenance and timestamps, no future data, no invalid count models, and unchanged baseline snapshots. Document failure evidence. Commit ONLY reviewed source/docs/tests and hash manifests, **never raw 779MB DuckDB or untracked parallel agent outputs**, to main once safe and integrated.

## Prior artifacts, links and local source references

- GitHub https://github.com/andrewkoumoudjian/footy-models .
- Previous math pathway code: apps/matchforecast/math_engine/unified_features.py , shared_math_glm.py , export_unified_248.py , historical_fbref_crosswalk.py , historical_backtest.py , participation_retrospective_25.json , PARTICIPATION_RESEARCH.md , UNIFIED_EPL_MLS_MATH_PATHWAY.md .
- Documentation: apps/matchforecast/math_engine/HISTORICAL_SOURCES_AND_248_AUDIT.md , docs/FM26_REEP_ATTRIBUTE_RECOVERY_2026_10_12.md .
- Latest identity ledger: data/identity/reep/epl_remaining_resolution_ledger_v1.json (ignored local evidence; not auto-mounted in another session unless it uses the same approved machine).
- Latest latest full roster: data/identity/reep/epl_remaining_resolved_v1.json .
- Earlier crosswalk and FM generated priors: data/identity/reep/epl_reep_first_full_discovery_20261012_v5.json , data/identity/reep/epl_fm26_conditional_missing_traits_20261012_v3.json .
- Older 1,576 × 248 EPL bundle: data/identity/reep/unified_epl_2026_10_11_reep_first_fm_recovered_v7.json (not based on 1,228 roster; do not treat as fresh).
- MLS history: data/identity/reep/unified_mls_v1.json .
- Reep DuckDB: data/identity/reep/reep-20261005T180536Z.duckdb (read-only; do not commit binaries).
- Source Hugging Face account: https://huggingface.co/akks88/football-manager-data/tree/main .
- Prior committed math work: https://github.com/andrewkoumoudjian/footy-models/commit/bf71f53 , https://github.com/andrewkoumoudjian/footy-models/commit/abf0f65 . The conversation also referenced main at e281d9d before these commits.
- Reep publication: https://reep.football/data/ ; https://reep.football/coverage/ .
- FBref historical data: https://huggingface.co/datasets/aloobun/fbref_understat_combined ; https://huggingface.co/datasets/roirani80/fbref-xg-analysis-2024-2025/blob/main/fbref_cleaned_with_features.csv .
- FBref adapter: https://github.com/probberechts/soccerdata .

## Suggested explicit acceptance criteria for a future completion claim

1. **Full data:** complete 20-club EPL official roster, complete 30-club MLS official roster, no player removed due to missing identity/statistics; cross-provider IDs with confidence, conflicts and unsupported traits recorded.
2. **One source-neutral feature contract:** every eligible player has the **same 248-column** nullable model input at a clearly labeled as-of time. A missing attribute is null, a reconstructed attribute retains uncertainty, not silent zeros. If you deliberately evolve features, version schema and recertify all participants.
3. **SofaScore/FBref/FM:** genuine dated inputs or explicit coverage gaps for each player; actual mental traits where valid, generated conditional priors where eligible, chemistry and psychological fit with uncertainty, auxiliary value forecasting with label hygiene.
4. **Model performance:** actually trained common parameters with held-out EPL+MLS proper scores and successful Rust path consistency; coherent probabilities; no unearned improvement claim.
5. **Delivery:** reproducible pipeline command and executed log, count matrix, missing-identity ledger, full docs, source-hash evidence, tests and committed code, new actual commit link, preserved concurrent agents' work.

## Copy-and-send continuation prompt

> Continue the unified EPL/MLS mathematical football forecasting and full data pipeline in andrewkoumoudjian/footy-models from the latest main, using docs/FOOTY_MODELS_UNIFIED_EPL_MLS_CHAT_HANDOFF_2026-10-12.md as the authoritative task specification. First audit current git status, parallel/uncommitted work and the latest Reep-first identity ledgers (1,228 provisional EPL FM IDs, 348 unresolved, 7 senior). Restore the local development terminal if it is still at its 64-session limit without disrupting active workers. Actually execute and validate the latest 20-club/1,576-entry EPL bundle using the strict native FM attribute identity gate, then refresh ALL 30 MLS clubs (the old 664/27 dataset is incomplete) and run full dated SofaScore, FBref and FM26 attribute ingestion. Use Reep provider IDs and Sortitoutsi/TM reconciliation, recover real FM mental traits and generate missing attributes using the existing source-fitted conditional distributions with explicit confidence. Retain all unmatched players. Build historical as-of player-match datasets and train one player-agnostic pooled mathematical model using all validated FM mental/technical/physical, SofaScore player/team ratings/advanced stats, FBref, tactical and psychological fit, team chemistry, player-value primitives and participation. Interface a coherent conditional start–shots–goals/event model into the existing Rust simulator. Evaluate match outcomes, scoreline, player events and Liverpool–Man City-like momentum on independent future OOS proper scores. No deep-ML training in the math pathway. Implement, test, document and commit only reviewed files; do not overwrite parallel work or claim unrun/untested results.

**End of handoff. Keep it updated with exact next executed run commands, data hashes and verified results.**
