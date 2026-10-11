# CF Montréal Scout Mission: Completion Summary

## What was built

A full scouting pipeline for CF Montréal (club_id 4078) that ranks the top 20
candidates in the CFM scout basket (age < 23, EUR 400k to 2.5M, real FM attrs,
not currently at CFM) on psych fit (calibrated on successful alumni), positive
12-month yield (v2 forecast), spike potential (v3 expectile-0.9 tail score),
squad complementarity, and Monte Carlo spike paths. The report, data and
website section were produced without re-merging the pre-merged panel, without
touching the TM DuckDB oracle (read-only) and without overwriting any deployed
model artifact.

## Deliverables produced (all gate checks green)

1. `data/cfm_scout_top20.csv` - 20 rows, 50 cols: rank, identity, psych_fit +
   psy_attrs_used, tail_score, yield_12m_pct (all strictly positive), squad_fit
   + squad_gap_filled, SofaScore rating/goals/assists/apps + confidence, FBref
   per-90 columns flagged `unavailable` (Cloudflare 403), MC stats (median/p10/
   p90/p15x/p2x/p3x/downside, spike run, EUR), base-run median, spike_drift
   flag. No NaN (missing rendered as '' or explicit labels).
2. `data/cfm_scout_report.md` - full report: executive summary, methodology
   (archetype calibration, psych_fit formula, yield/spike definitions,
   squad-fit rules, MC config, join rules + coverage), alumni validation
   (18 rows with success multiples), squad gap analysis, per-player detail
   (psych profile, yield/spike, gap filled, SofaScore rating, MC fan stats,
   rationale), honest limitations, seed statement (42) + model hashes.
3. `artifacts/cfm_mc/` + `artifacts/cfm_mc_spike/` - paths_long.tsv,
   paths_basket.tsv, stats.tsv (20 rows each, EUR), 21 white-background fan
   charts each (overview + 20 players), seed 42, 1000 paths, sigma 0.4688.
4. `data/cfm_mc_input.tsv` + `data/cfm_mc_input_spike.tsv` - 11 cols each
   (reproducibility).
5. Website (`/home/drewclaw/footy-site`): new `CFMScoutSection.tsx` (trovecn
   Dialog + table, SF/Apple system typeface, no em dashes in copy), wired into
   `App.tsx` after `<ScoutSection />`, 42 static fan-chart images under
   `public/cfm/charts` and `public/cfm/charts_base`; `npm run build` passes and
   `dist/` contains the CFM section ("CFM Scout" and "CF Montréal" greppable).
6. `scripts/gate_check_cfm.sh` - runs green end to end (cargo test exit 0; no
   Rust changes made this session).

## Key numbers

- Basket: 1,255 players; coverage-gated pool: 617; final top-20.
- Psych archetype calibrated on top-half-by-success alumni: Joel Waterman 4.65x,
  Nathan-Dylan Saliba 3.75x, Aarón Herrera 3.23x, Ismaël Koné 3.12x, Raheem
  Edwards 2.32x. Alumni validation: psych_fit min 0.614, mean 0.670 (>= 0.6).
- Combined rank weights (documented): z(psych_fit) + 0.5 z(tail) + 0.3
  z(yield_12m) + 0.4 z(squad_fit); all 20 above the basket psych median (0.573)
  and spike median (0.845); tail scores in the top 40% of the basket (>= 0.867).
- Squad gaps filled: LW x3 (no LW in squad), LB x3 (only 2, both 26+), CM x3,
  DM x3, CB x3, AM x2, GK/CF/RW x1.
- MC (spike run): all 20 players have P(>=1.5x) >= 0.996 under the expectile-0.9
  drift; base-run medians are the conservative comparison (e.g. rank 1 Piñero:
  base median EUR 1.77M vs spike median EUR 10.48M).
- Top of the list: Lisandro Piñero (LW, Banfield, EUR 400k, psych 0.627,
  tail 1.119, yield 68%).

## Coverage counts (honest)

- SofaScore: 20/20 player ids resolved, 20/20 high/medium confidence, 20/20
  season ratings. Season line rule: current season (2026 / 25/26 at the June
  2026 vintage), youth national teams excluded, rated lines preferred, then
  most appearances (so loans/youth lines reflect where the player actually
  plays).
- FBref: 0/20 (Cloudflare 403 on plain HTTP and curl_cffi chrome
  impersonation; max-3-attempts rule respected; per-90 columns recorded as
  "unavailable").
- Hidden psych: present on ~32% of the panel (all top-20 have real values for
  the four attrs used); archetype hidden component uses the documented
  bridge-club prior (alumni carry only placeholder values).

## What the orchestrator must verify

- **Cloudflare deployment was NOT performed** (needs the user's Cloudflare
  token; run `npm run build` in /home/drewclaw/footy-site and deploy the
  `dist/` folder). The site build is green and `dist/` is ready.
- SofaScore data is live (Aug 2026) and can drift: re-running
  `scripts/enrich_sofascore.py` may produce slightly different season lines.
- Identity notes: Leandersson matched without middle name and with a 4-day DOB
  difference (2008-07-27 vs TM 2008-07-23), confidence medium; Spinaccè's
  SofaScore assists are null in Serie C and recorded as "missing" (not zero);
  Dorrington shows League Two 25/26 minutes per SofaScore while TM/FM list him
  at Aberdeen (mid-season loan not in the TM transfers table).
- MC stats.tsv in both artifact dirs is the chart-rendered version (cents
  already converted to EUR, p3x appended); inputs are the 11-col TSVs.
- Model artifacts untouched: forecast model.bin SHA-256
  1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f; tail and
  surge model mtimes unchanged (2026-08-10 10:56:55 and 2026-08-10 02:16:18).
