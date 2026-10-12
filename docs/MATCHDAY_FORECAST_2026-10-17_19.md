# EPL matchday forecast 2026-10-17–19 — single entry point

**Status: executed, research-only.** All 10 fixtures, all 20 clubs, one command.
Probabilities are calibrated-simulation estimates with measured OOS accuracy
stated below; not betting odds.

## Single entry point

```bash
python3 scripts/forecast_matchday.py \
  --schedule espn_board_20261017.json espn_board_20261018.json espn_board_20261019.json \
  --as-of 2026-10-12 --out-dir data/ml/matchday_20261017_19 \
  --paths 2500 --seed 42
```

Layers: `scripts/build_universal_bundle.py` (20-club as-of bundle: 1,427
eligible players, EB baselines for 126 ESPN-linked players, cold-start role
priors otherwise, **630 FM-verified trait blocks** — 441 squad-verified plus
189 ledger-directed staff-db rescues, of which 17 heal squad-lagging missing
uids — 477 stable provisional IDs, 1,190 prior team matches + 552
Championship priors) → `footy-matchforecast-math
--heads-artifact` (in-engine pooled residual, `PooledShotCorrection`) →
coherence validation (10/10 fixtures, analytic-Poisson consistent) →
hash-pinned run dir (`bundle.json`, `predictions.json`, `manifest.json`,
frozen `schedule/`). Design: Python owns data plumbing, Rust owns all math;
no rate formula is implemented twice. Refuses to overwrite existing runs.

## Forecasts (corrected run, 2500 paths, seed 42, MC SE ≈ 0.01)

| Fixture | Home | Draw | Away | xG | xS | Top P(goal) |
|---|---|---|---|---|---|---|
| Brentford v Liverpool | 0.382 | 0.249 | 0.369 | 1.57–1.54 | 12.9–14.5 | Schade 0.237, Anthony 0.232, Ouattara 0.224 |
| Everton v Chelsea | 0.393 | 0.263 | 0.344 | 1.47–1.33 | 13.2–12.1 | Pedro Neto 0.244, Barry 0.228, Dewsbury-Hall 0.168 |
| Fulham v Hull | 0.309 | 0.336 | 0.355 | 0.90–0.98 | 13.9–12.0 | Iwobi 0.168, Ansah 0.164, Cho 0.149 |
| Man City v Ipswich | 0.786 | 0.133 | 0.081 | 2.84–0.81 | 16.8–9.3 | Haaland 0.455, Semenyo 0.297, E. Fernandez 0.228 |
| Newcastle v Aston Villa | 0.451 | 0.225 | 0.324 | 1.74–1.43 | 14.7–12.1 | B. Toure 0.199, Barnes 0.190, Abraham 0.180 |
| Bournemouth v Sunderland | 0.484 | 0.258 | 0.258 | 1.58–1.09 | 15.2–10.8 | Adli 0.226, Brooks 0.223, Brobbey 0.140 |
| Brighton v Crystal Palace | 0.573 | 0.223 | 0.204 | 1.89–1.03 | 14.5–10.8 | Minteh 0.233, Rutter 0.212, Mitoma 0.210 |
| Leeds v Man United | 0.392 | 0.248 | 0.360 | 1.41–1.33 | 14.4–12.2 | Calvert-Lewin 0.200, Okafor 0.167, Mbeumo 0.167 |
| Nott'm Forest v Arsenal | 0.234 | 0.274 | 0.492 | 0.94–1.50 | 12.2–12.8 | Madueke 0.219, Gyokeres 0.216, Tzolis 0.213 |
| Tottenham v Coventry | 0.470 | 0.257 | 0.273 | 1.45–1.05 | 15.0–11.6 | Tel 0.262, Odobert 0.229, Tchaouna 0.227 |

Rescue audit behind the 630 (`scripts/fm_staff_rescue.py`): of 732 quarantined
ledger attachments, 218 staff-kind/db-source/DOB-corroborated/tier≥2 rows
(183 via the native Reep typed FM bridge) carry 5+ valid union attrs and are
admitted with `fm_rescued_staff` provenance — the same validity gate the MLS
labels already used. Still out, with reasons: 326 absent from all FM exports
(no row exists — no bridge can conjure one), ~44 DOB-conflict rows (right
kind, wrong person — the Matheus Nunes rule), 107+ derived/unknown
provenance (prior-grade, not observed), 8 tier 0-1. No refit was needed: the
heads OOS already admits staff-db rows through the identical union gate, so
the published coefficients already price rescued-trait performance.

## Measured accuracy behind these numbers

- Team goals: rolling-origin OOS over 816 EPL fixtures, WDL log-loss 1.0115
  vs 1.0839 home-advantage-only (95% CI excludes 0); see
  `docs/UNIFIED_FORECAST_OOS_2026-10-12.md`.
- Player shots: pooled chronological OOS, Poisson Δ−0.0075 with CI excluding 0.
- Conversion/participation heads are gated challengers and do not enter these
  forecasts (neutral xG, minutes-weighted lineups).

## Limitations

- Histories end 2026-10-11 (team CSV through 10-10): one round stale by
  kickoff; the 2026-10-12 match is correctly excluded as same-day.
- 1,301/1,427 players use cold-start role priors (no linked 2026-27 history);
  986 carry no verified FM traits. Missing stays missing with per-player
  provenance in `predictions.json`.
- Retrospective-chronological validation only; prospective scoring of this
  matchday against actuals is the next required evidence step.
- Each club's lone-GK cases (Chelsea, Ipswich, Tottenham) simulate a GK
  dismissal without a replacement keeper; effect is second-order at base rates.
