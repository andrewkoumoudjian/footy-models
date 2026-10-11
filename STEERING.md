# STEERING — Footy-Models Value-Surge Model + Transfer Proposals

You are the implementation agent for the `footy-models` Rust workspace. Read
`architecture.md` and the repo `AGENTS.md` FIRST. Your job: implement the
**future-gem detection / price-alpha** objective — find players currently
valued €400k–€1M by Transfermarkt who are most likely to appreciate over the
next 12 months, restricted to players who are good on the field per Football
Manager 2026 attributes — then produce transfer proposals.

## THE FOOTBALL MANAGER DATA IS THE CORE ASSET (mandatory)

We extracted and published the complete FM26 male database (563,382 men with
full 1-20 attributes, CA, PA). This is a first-class model input — not an
afterthought. Use it three ways:

1. **Player universe.** `data/fm_universe.csv` — ALL FM26 men (563k) with:
   `uid, name, birth_year, age, ca, pa, pa_gap, ca_age_pct` (FM CA percentile
   within the player's age band — the development signal), `pos_cls`
   (GK/DF/MF/FW inferred from attributes), `height`, `home_rep/cur_rep/
   world_rep`, `attr_source` (`db` = real stored attributes, `derived` =
   statistically generated, `staff`/`none` = no attributes), and composite
   signals `sig_technical, sig_physical, sig_mental, sig_defensive,
   sig_attacking, sig_creative` (means of position-relevant attribute sets).
2. **Model features.** FM CA/PA, `pa_gap` (development upside), `ca_age_pct`
   (age-relative talent), position class, and the composite signals are
   primary features. FM's own Potential Ability is the game's estimate of
   future ability — use it.
3. **Quality gate.** "Good on the field" is defined from FM attributes
   (position-appropriate minimums using `pos_cls`/TM position), not from
   market data. Honor `attr_source`: prefer `db` attrs; treat `derived`
   attrs as lower-confidence (they are statistically generated from the
   distribution of real players — flag them in outputs and consider a
   confidence discount).

## Data (already prepared in `data/`)

### ⚠️ CRITICAL — THE MERGING IS DONE. DO NOT RE-MERGE.
A previous run wasted 30+ minutes on a pathological pandas merge (Cartesian
blowup). Everything is already joined:

- `candidates_ready.csv` — **THE candidate set for proposals**: 3,615 rows
  (2,299 with age <= 28), deduplicated by `player_id` (highest CA kept).
  FM attrs (`attr_*`, `ca`, `pa`, `pa_gap`, `ca_age_pct`, `pos_cls`, `sig_*`,
  `attr_source`) and TM features (`current_mv`, `mv_6m`, `mv_12m`, `peak_mv`,
  `n_valuations`, appearances/goals/assists/minutes per season, transfer,
  club, competition, country, position, foot, date_of_birth) are ALL in one
  file. Load it directly — no merging needed.
- `surge/train.tsv` + `surge/train_rows.csv` — **already-built causal
  training set**: 151,628 rows, 47 features, target `surge` (1 = market
  value +50% within 12 months), 22.5% surge rate. Built by
  `data/build_surge_dataset.py` (inspect that script for the exact column
  schema). Reuse these files; do NOT rebuild them unless you have a
  documented reason.
- `fm_universe.csv` — all 563k FM26 men + development features (use for
  scouting/quality-gate context and reference distributions).
- `tm_active_features.csv` — all 22k active TM players with TM features.
- `tm_valuations_summary.csv`, `tm_appearances.csv`, `tm_transfers_all.csv`,
  `tm_players.csv`, `tm_clubs.csv`, `tm_competitions.csv` — raw extracts.

If you EVER need to join anything yourself: LEFT JOIN on `player_id` only,
and assert row counts before/after. NEVER cross-join. If a pandas op takes
> 5 minutes, stop and reconsider the approach.

## Candidate universe for proposals

Rows of `fm_tm_joined.csv` with:
- `current_mv >= 400000` AND `current_mv <= 1000000`
- `ca` present AND `attr_source` in (`db`, `derived`)
- age <= 28 (justify if you use a different cap)
- "Good on the field": position-appropriate FM attributes above minimums
  (define per position using `pos_cls`; e.g., forwards need Finishing >= 12
  and Dribbling >= 12; defenders need Tackling/Marking >= 12; GKs need
  Handling/Reflexes >= 12 — justify exact thresholds in the report) OR a
  strong development signal (young + high `pa_gap` + high `ca_age_pct`).
  Document exactly what you require and why.

## Model requirements (from architecture.md / AGENTS.md — non-negotiable)

1. **Causal / as-of.** Features used at prediction time must be knowable at
   that time. Never use future observations, later revisions, or post-event
   information. No random splits: **walk-forward validation**.
2. **Two-layer design (document it):** FM26 attributes are a single snapshot
   (summer 2026), so train the surge model on TM + performance + FM-state
   features over historical valuation windows (strictly causal), then apply
   it to the current candidate set with FM attrs as the quality/development
   screen. FM features used in historical training rows must be lagged to
   what would have been knowable (use attribute-based signals that are
   stable, and document the approximation).
3. **Target definition:** from `player_valuations` history, define "surge" —
   e.g., market value rose >= 50% within 12 months of a reference date (or
   another threshold you justify). Build causal training rows.
4. **Uncertainty:** outputs are probabilities/intervals, calibrated — never
   bare point predictions without dispersion.
5. **Deterministic:** fixed seeds; same input → same output.
6. **Rust-first:** implement in `crates/` per `architecture.md` (minimal
   deps, `std` in hot paths). A from-scratch logistic/linear model or small
   GBDT in Rust is acceptable. Wire up `apps/train` and `apps/score` to run
   end-to-end. Do not depend on Python at runtime for scoring.
7. **Missing is not zero:** handle coverage explicitly (FM `attr_source`,
   absent performance data, absent valuations).

## Deliverables (must exist and be verified)

1. `data/proposals_top100.csv` — top 100 proposals, columns:
   `player_id,name,club,competition,country,position,pos_cls,age,current_mv,fm_ca,fm_pa,attr_source,predicted_surge_probability,interval_10,interval_90,key_strengths,rationale`
2. `data/model_report.md` — model spec, feature list (FM + TM explicitly),
   target definition, walk-forward results (AUC, calibration/reliability,
   decile table of realized appreciation), limitations, and the final
   selection criteria.
3. Tests enforcing: candidates within MV range, outputs complete (no NaN),
   deterministic rerun with the same seed produces identical output.

## Iteration mandate

After implementing, run end-to-end: `cargo build --release`, `cargo test`,
train, score. Verify the outputs yourself. If walk-forward AUC < 0.60 or the
decile table shows no monotonic relationship, iterate — better features
(FM development signals, momentum ratios, age buckets, minutes trends,
league quality), better target definition, better model — until metrics are
solid or you have an honest, documented reason they cannot improve. Do not
stop at the first working version. Report final numbers honestly in
`data/model_report.md`.

## Final gate

`bash scripts/gate_check.sh` must pass: it checks the proposals file exists,
has >= 100 rows, all `current_mv` within [400k, 1M], no NaN in key columns,
FM columns present (`fm_ca`, `fm_pa`, `attr_source`), and `cargo test
--release` passes. Fix anything it flags and rerun until it passes. Do NOT
modify `scripts/gate_check.sh` to make it pass — fix the real problem.
