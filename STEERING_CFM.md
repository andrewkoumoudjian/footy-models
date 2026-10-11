# CF Montréal Scouting Mission — Top-20 Psych-Fit Players (STEERING)

You are a football recruitment quant for **CF Montréal** (MLS, club_id 4078). Your mission:
produce a **scouting report** ranking the **top 20 players** who (1) have the **psychological
type that thrives at CF Montréal**, (2) have **positive yield** (expected market-value
appreciation), (3) a **high probability of a value spike**, (4) are **compatible and
complementary with the current squad**, and (5) whose **spike scenario is passed through a
Monte Carlo** to generate ALL possible value paths. Enrich the shortlist with **FBref** and
**SofaScore** performance data, then **publish the report on the footy-models website**
(`/home/drewclaw/footy-site`, deployed to Cloudflare Workers).

The user drives this. Do not stop at the first working version — iterate until the gate passes.

---

## 1. DATA INVENTORY (pre-merged — DO NOT RE-MERGE)

Pre-merged files (created by the orchestrator; opening and joining these again causes a
Cartesian blowup. **MERGING IS DONE — DO NOT RE-MERGE.**):

- `data/cfm_scout_panel.csv` — **14,769 rows × 74 cols**. The candidate universe. One row per
  player with ALL of: TM overlay (`player_id`, `name`, `club`, `competition`, `country`,
  `pos_cls`, `age`, `current_mv`, `fm_ca`, `fm_pa`, `attr_source`), full FM26 attrs
  (`attr_*` incl. `attr_Determination`, `attr_Bravery`, `attr_Consistency`,
  `attr_ImportantMatches`, `attr_Composure`, `attr_WorkRate`, `attr_Teamwork`,
  `attr_Leadership`, `attr_Versatility`, `attr_NaturalFitness`, `attr_InjuryProneness`),
  hidden personality (`psy_Adaptability`, `psy_Ambition`, `psy_Loyalty`, `psy_Pressure`,
  `psy_Professionalism`, `psy_Sportsmanship`, `psy_Temperament`, `psy_Controversy` — present
  on ~28% of rows; blank = missing, NEVER zero-fill), forecast scores (`f36_*` from
  `forecast_36m_all.csv`: `f36_predicted_12m_mv`, `f36_gap_pct`, `f36_gap_36m_pct`,
  `f36_mu_12`, `f36_interval_10`, `f36_interval_90`), tail spike scores (`tail_tail_score`,
  `tail_predicted_12m_mv` from `candidates_v3_tail.csv`).
- `data/cfm_squad.tsv` — **33 rows**, current CF Montréal squad (club 4078): `player_id`,
  `name`, `position`, `sub_position`, `dob`, `market_value_eur`.
- `data/cfm_alumni.tsv` — **18 rows**, players who LEFT CF Montréal for a fee, with their
  transfer outcome + FM psych attrs where joinable: `player_id`, `player_name`,
  `transfer_date`, `to_club`, `transfer_fee`, `current_mv`, `current_club`, `comp`, `uid`,
  `fm_ca`, `fm_pa`, `pos_cls`, `attr_source`, `psy_*`, `attr_*` mental.

Read-only sources (never write):
- TM DuckDB oracle: `/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb`
  (`duckdb.connect(read_only=True)`). `player_valuations` runs through 2026-06-12.
- FM26 raw: `/home/drewclaw/fm26-data/work/people_v3.jsonl` (hidden personality by uid,
  INT keys) and `/home/drewclaw/fm26-data/fm26_men_full_attrs.csv` (attrs by uid, STRING keys).
- Deployed model artifacts (immutable, never overwrite): `data/forecast/model.bin`
  (v2 mean forecaster; SHA-256 `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f`),
  `data/tail/model_exp090_r1e6.bin` (v3 expectile-0.9 spike forecaster),
  `data/surge/model.bin` (surge classifier).

Candidate universe derivation (the panel is a JOIN OUTPUT — do not re-merge, just filter):
- Restrict to the **CFM scout basket**: `age < 23` (as floating years), `400000 <=
  current_mv <= 2500000` (CFM buys in this band), `attr_source != 'staff'`. Prefer rows with
  a `tail_tail_score` (v3 spike) present. Do not include players currently at CF Montréal
  (club 'Club de Foot Montréal' / compare against `cfm_squad.tsv` names).

## 2. CF MONTREAL PSYCHOLOGICAL ARCHETYPE (the core ask)

**Concept.** CF Montréal is a *bridge club*: it signs young players, develops them, and
sells them to Europe/stronger leagues (see `cfm_alumni.tsv`). The psychological type that
thrives there is the type that (a) handles the pressure of being a transferable asset in a
development pipeline, (b) is professional and ambitious enough to move, (c) stays
composed/consistent through a foreign environment and a demanding fanbase.

**Method (calibrate on alumni — evidence, not opinion).**
1. Success label per alumnus: `success = current_mv / transfer_fee` (already derivable from
   `cfm_alumni.tsv`). Alumni with `success >= 1.5` and a meaningful fee (> €300k) are the
   "thrived" reference set (Koné 3.1x, Johnston 2.3x, Saliba 3.75x, Waterman 4.65x,
   Crépeau 40x, Binks 36x, Herrera 3.2x, Sirois 6.3x...). Use the *top-half by success* as
   the archetype calibration set; report which alumni were used.
2. Build the **archetype vector** from their FM psych + mental attrs:
   - Hidden (where present): Pressure, Professionalism, Ambition, Adaptability, Temperament
     (Controversy is NEGATIVE — lower is better).
   - Mental: Determination, Bravery, Consistency, ImportantMatches, Composure, WorkRate,
     Teamwork, Leadership, Versatility, NaturalFitness (InjuryProneness is NEGATIVE).
   - Mean of non-missing per attr; if a column is missing for ALL alumni, use the FM-known
     "bridge-club" prior (Pressure ≥ 12, Professionalism ≥ 12, Ambition ≥ 12, Adaptability ≥
     12, Determination ≥ 12, Composure ≥ 11, Consistency ≥ 11, ImportantMatches ≥ 10,
     WorkRate ≥ 13, Teamwork ≥ 12, Bravery ≥ 11, Leadership ≥ 10, NaturalFitness ≥ 12,
     InjuryProneness ≤ 6, Controversy ≤ 8) and say so in the report.
3. **psych_fit score** per candidate = weighted similarity to the archetype over the
   available attributes. Handle missing honestly: score over the intersection of available
   candidate attrs ∩ archetype attrs; compute a coverage flag (`psy_attrs_used` count).
   Scale to [0,1] with a monotone transform (e.g. sigmoid of z-distance). Report the exact
   formula in the report. Sanity: the alumni themselves should score high (>= 0.6).

## 3. YIELD + SPIKE SCORES

- **Yield** (positive expected appreciation): `f36_gap_pct > 0` AND `f36_predicted_12m_mv >
  current_mv`. Require strictly positive for the shortlist.
- **Spike score**: `tail_tail_score` (v3 expectile-0.9 12m log-ratio) — the primary spike
  rank. Where absent, fall back to `f36_gap_36m_pct`. Require the final top-20 to be in the
  top ~40% of the basket by tail score.
- **Combined rank** (define and document): e.g. `z(psych_fit) + 0.5*z(tail_score) +
  0.3*z(yield_12m)` — the exact weights are yours to choose but MUST be written in the
  report. The top-20 must have positive yield and be above the psych-fit and spike medians.

## 4. SQUAD COMPATIBILITY (complementary, not duplicative)

Analyze `cfm_squad.tsv`. Known gaps (verify yourself): **NO left winger** (4 RW, 0 LW),
only **2 left-backs**, overstocked at CB (6), CF (5), RB (4). Also note age/expiry profile
from the squad file.

Score each candidate `squad_fit`:
- +positional need: LW and LB get the highest need weight; AM/CM/DM medium; CB/CF/RB low
  (already stocked). Use FM `pos_cls` (GK/DF/MF/FW) + TM `position`/`sub_position` from the
  panel where present; if only `pos_cls` exists, map FW→attack need, MF→mid need, DF→def
  need, GK→GK need (CFM has 3 GKs → low).
- +age fit: prefer 17–21 (MLS U22 initiative window); discount players near 23.
- +value band fit: 400k–1.2M ideal for CFM; 1.2M–2.5M acceptable for a positional need.
- Record the rationale per player (which gap they fill).

## 5. MONTE CARLO SPIKE PATHS (all possible paths)

For the **final top-20**, run the deterministic Rust MC (do NOT reimplement simulation):
```
./target/release/footy-mc run --input data/cfm_mc_input.tsv --out artifacts/cfm_mc \
  --paths 1000 --seed 42 --top-n 20 --basket-n 20
```
- Input TSV (11 cols): `player_id, name, club, pos_cls, age, fm_ca, fm_pa, attr_source,
  current_mv, mu_ann, sigma_ann` with `mu_ann = ln(f36_predicted_12m_mv / current_mv)`
  (annual log drift) and `sigma_ann = 0.4688` (calibrated annual vol; use the same constant
  the 36m basket run used).
- **Spike scenario**: ALSO run a second input where `mu_ann = tail_tail_score` (the
  expectile-0.9 drift IS the spike scenario) → `artifacts/cfm_mc_spike`. This shows all
  possible paths under the spike regime.
- Render **white-background fan charts** for each of the top-20 (all paths faint blue,
  mean bold red `#c62828`, median dashed `#57606a`, 10–90% band `#d6e6fb`, subtitle BELOW
  the x-axis), plus an overview basket chart — reuse the style from
  `scripts/mc_36m_white.py`. Charts under `artifacts/cfm_mc/charts/` and
  `artifacts/cfm_mc_spike/charts/`.
- Stats table per player: median 36m, p10/p90, P(≥1.5×), P(≥2×), P(≥3×), downside prob
  (from `stats.tsv` — remember MC stats are CENTS, divide by 100 for EUR).

## 6. FBref + SofaScore ENRICHMENT (new data sources — use scraper repos)

Use the **soccerdata** repo (`pip` package `soccerdata`, v1.9.1, github.com/probberechts/soccerdata)
which implements scraping for BOTH FBref and SofaScore, plus the documented SofaScore public
API (github.com/pseudo-r/Public-Sofascore-API; endpoint `api.sofascore.com/api/v1/...`).

For the final top-20 shortlist ONLY (keep it polite, low-volume):
1. **SofaScore** (primary — the API works from this machine, verified):
   - Player search: `GET https://api.sofascore.com/api/v1/search/all?q=<name>` (UA header
     `Mozilla/5.0 ... Chrome/120`).
   - Player profile: `GET https://api.sofascore.com/api/v1/player/{id}` → position, team, DOB.
   - Season statistics: `GET https://api.sofascore.com/api/v1/player/{id}/statistics/seasons`
     and the season detail endpoint → rating (average), goals, assists, appearances.
   - Match by normalized name (+ club where possible); record `sofascore_id` and match
     confidence; missing → blank, never zero.
2. **FBref** (secondary; Cloudflare blocks plain HTTP AND curl_cffi chrome impersonation from this
   machine — verified 403; soccerdata's selenium path is broken with this chromium (`_driver`
   bug). If you can get it working with `/usr/bin/chromium` + `--user-data-dir=/tmp/...` after
   a couple of attempts, use it; otherwise record `fbref_unavailable` per player and move on.
   Do NOT hammer FBref — max 3 attempts total, then stop.):
   - `soccerdata.FBref(leagues=[...], seasons=[...], path_to_browser='/usr/bin/chromium')`
     `read_player_season_stats(stat_type='standard')` (plus 'shooting', 'misc', 'passing'
     for the shortlist if cheap). Note: FBref league coverage is Big-5 + MLS + others per
     `available_leagues()` — only pull players whose league is covered.
   - Per-90 metrics to keep: goals, assists, xG, xA, progressive carries, progressive
     passes, pressures, tackles. Join by name+DOB; confidence flag.
3. Merge as extra columns on the top-20 table: `sofascore_rating`, `sofascore_goals`,
   `sofascore_assists`, `fbref_90_goals`, `fbref_90_xg`, `fbref_90_xa`,
   `fbref_90_prog_passes`, `fbref_90_prog_carries`, `fbref_90_pressures`, + `_src` flags.
   Report coverage counts honestly (how many of 20 resolved in each source).

## 7. DELIVERABLES (all required)

1. `data/cfm_scout_top20.csv` — 20 rows, one per player: rank, name, club, competition,
   country, pos_cls, age, current_mv, fm_ca, fm_pa, attr_source, psych_fit, psy attrs used
   count, tail_score, yield_12m_pct, squad_fit, squad_gap_filled, sofascore_rating,
   sofascore_goals/assists, fbref per-90 cols (+_src flags), MC stats (median_36m, p10, p90,
   p15x, p2x, p3x, downside_prob), spike_drift flag. NO NaN (blank = '').
2. `data/cfm_scout_report.md` — FULL report:
   - Executive summary (top-20 headline table).
   - Methodology: psych archetype calibration (list the alumni used + their success
     multiples), psych_fit formula, yield/spike definitions, squad-fit rules, MC config
     (seed, paths, sigma, drift), FBref/SofaScore join rules + coverage counts.
   - Alumni validation section (the "past players who went on to successful transfers"
     evidence — table from `cfm_alumni.tsv` with success multiples).
   - Squad gap analysis (from `cfm_squad.tsv`).
   - Top-20 detail: per player — psych profile summary (top attrs), yield/spike numbers,
     squad gap filled, SofaScore rating, FBref per-90, MC fan stats, one-line rationale.
   - Honest limitations: hidden-psych coverage (~28%), FBref coverage, TM as-of vintage
     (FM26 ≈ Nov 2025, TM through Jun 2026), tail scores are model outputs not guarantees.
   - Seed statement (42 everywhere) + model hashes used.
3. `artifacts/cfm_mc/` + `artifacts/cfm_mc_spike/` — MC outputs incl. `paths_long.tsv`,
   `stats.tsv`, `paths_basket.tsv` and white fan charts (overview + 20 players each).
4. `data/cfm_mc_input.tsv` + `data/cfm_mc_input_spike.tsv` — the MC inputs (reproducibility).
5. **Website publish** (`/home/drewclaw/footy-site`):
   - Add a new `CFMScoutSection.tsx` (mirror `ScoutSection.tsx` structure; trovecn
     components only; SF/Apple system typeface; NO em dashes in copy — use ": " or commas;
     white-background fan charts embedded as static images from `artifacts/cfm_mc/charts/`).
     Show: section header (CF Montréal Scout Report, as-of/vintage line), the top-20 table
     (rank, name, club, age, pos, current_mv, psych_fit, tail/spike, yield, sofascore rating,
     MC median/p15x), a per-player expandable card (Dialog or Drawer, trovecn pattern) with
     the fan chart image + psych profile + rationale + MC stats.
   - Wire it into `App.tsx` (import + render after `<ScoutSection />`).
   - Build must pass: `npm run build`. Do NOT deploy (deployment needs the user's
     Cloudflare token — the orchestrator handles that).
6. `scripts/gate_check_cfm.sh` — the quality gate (see §9).

## 8. NON-NEGOTIABLES

- DO NOT RE-MERGE the panel/squad/alumni. Read them.
- TM DuckDB is read-only. Deployed model artifacts never overwritten.
- Missing is not zero: blank attrs stay blank; coverage is reported everywhere.
- Deterministic seeds (42). No Python in the Rust scoring path (use the existing binaries).
- Do NOT overwrite `data/spike_refined_top100.csv`, `data/spike_refinement_report.md`,
  `data/forecast_36m_all.csv`, or any `data/forecast/model.bin`-family artifact.
- Hermes lifecycle guard: run compiled Rust binaries ONLY through a `scripts/*.sh` wrapper
  (e.g. `scripts/run_cfm_mc.sh`); put Python in `scripts/*.py` files, run with
  `uv run --with pandas --with requests python scripts/...` — no long inline `python -c`.
- Website copy: SF/Apple system stack, no serif, no em dashes. trovecn components only.
- Honesty: single-window model scores are not guarantees; the report must say so.

## 9. GATE (`scripts/gate_check_cfm.sh`) — completion criterion

Must ALL pass (script exits non-zero listing failures):
1. `cargo test --release -p footy-backtest -p footy-forecast -p footy-models` green (if Rust
   changed; else note "no Rust changes").
2. `data/cfm_scout_top20.csv`: exactly 20 data rows; has columns psych_fit, tail_score,
   yield (positive), squad_fit, sofascore_rating; no NaN; all 20 have positive
   `f36_gap_pct`-derived yield; sorted by combined rank.
3. `data/cfm_scout_report.md` present with needles: "archetype", "alumni", "psych_fit",
   "Monte Carlo", "seed", "SofaScore", "FBref", "limitations", "gap".
4. `artifacts/cfm_mc/stats.tsv` + `artifacts/cfm_mc_spike/stats.tsv` exist with 20 player
   rows each; chart count ≥ 20 in each chart dir (overview + players).
5. `data/cfm_mc_input.tsv` + `data/cfm_mc_input_spike.tsv` exist (11 cols).
6. `footy-site` build: `cd /home/drewclaw/footy-site && npm run build` succeeds and
   `dist/` contains the new JS bundle with CFM section (grep dist for "CF Montréal" or
   "CFM Scout").
7. Model hashes unchanged (`data/forecast/model.bin` SHA-256 matches the value in §1; record
   `data/tail/model_exp090_r1e6.bin` + `data/surge/model.bin` mtimes at start and end).
8. Seed statement present in the report.

Iterate until the gate is green. When the gate passes, write a short completion summary to
`data/cfm_scout_completion.md` (what you built, key numbers, coverage counts, anything the
orchestrator must verify).
