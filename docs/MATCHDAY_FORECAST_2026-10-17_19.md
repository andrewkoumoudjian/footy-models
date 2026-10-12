# EPL matchday forecast 2026-10-17–19 — single entry point

**Status: executed, research-only.** All 10 fixtures, all 20 clubs, one command.
Probabilities are calibrated-simulation estimates with measured OOS accuracy
stated below; not betting odds. Canonical run: `data/ml/matchday_20261017_19_v3`
(bit-reproducible from frozen inputs; v2 was the pre-tackle superseded run).

## Single entry point

```bash
python3 scripts/forecast_matchday.py \
  --schedule espn_board_20261017.json espn_board_20261018.json espn_board_20261019.json \
  --as-of 2026-10-12 --out-dir data/ml/matchday_20261017_19_v3 \
  --paths 2500 --seed 42
```

Defaults apply the v2 heads (`--heads`), FM team priors at coefficient 0.1
(`--fm-priors`, `--fm-coeff`), 2500 paths. Layers:
`scripts/build_universal_bundle.py` (20-club as-of bundle: 1,427 eligible
players, EB baselines for 126 ESPN-linked players, cold-start role priors
otherwise, **630 FM-verified trait blocks** incl. 189 staff-db rescues,
tackle baselines + tackle feature vectors for all, 1,190 prior team matches +
552 Championship priors) → `footy-matchforecast-math --heads-artifact
--fm-priors --fm-coeff` (in-engine pooled shot + tackle residuals via
`PooledShotCorrection`/`PooledEventRate`, FM-adjusted team anchors) →
coherence validation (10/10 fixtures, analytic-Poisson consistent) →
hash-pinned run dir. Python owns data plumbing, Rust owns all math. Refuses
to overwrite existing runs.

## Forecasts (v3: FM team priors + tackles, 2500 paths, seed 42, MC SE ≈ 0.01)

| Fixture | Home | Draw | Away | xG | Top P(goal) | Top xTackles |
|---|---|---|---|---|---|---|
| Brentford v Liverpool | 0.263 | 0.237 | 0.500 | 1.31–1.82 | Ekitike 0.244, Isak 0.218 | Szoboszlai 2.25, Gravenberch 2.17 |
| Everton v Chelsea | 0.442 | 0.275 | 0.283 | 1.44–1.07 | Barry 0.225, Pedro Neto 0.205 | Rogers 2.04, Garner 1.90 |
| Fulham v Hull | 0.404 | 0.309 | 0.287 | 1.13–0.90 | Iwobi 0.206, Bobb 0.178 | Iroegbunam 2.17, Morita 2.13 |
| Man City v Ipswich | 0.852 | 0.096 | 0.052 | 3.39–0.81 | Haaland 0.506, Semenyo 0.339 | Lukic 2.35, Palacios 2.35 |
| Newcastle v Aston Villa | 0.525 | 0.224 | 0.251 | 1.92–1.26 | B. Toure 0.214, Barnes 0.208 | Manzambi 1.88, McGinn 1.76 |
| Bournemouth v Sunderland | 0.508 | 0.249 | 0.243 | 1.48–0.93 | Adli 0.215, Brooks 0.210 | Scott 2.07, Tavernier 2.05 |
| Brighton v Crystal Palace | 0.690 | 0.172 | 0.138 | 2.50–1.03 | Minteh 0.293, Rutter 0.267 | Khalaili 1.84, Gross 1.82 |
| Leeds v Man United | 0.377 | 0.247 | 0.376 | 1.37–1.35 | Calvert-Lewin 0.193, Mbeumo 0.169 | Borges Fernandes 1.81, Ampadu 1.74 |
| Nott'm Forest v Arsenal | 0.166 | 0.253 | 0.581 | 0.78–1.67 | Gyokeres 0.239, Tzolis 0.238 | Eze 2.36, Rice 2.35 |
| Tottenham v Coventry | 0.525 | 0.252 | 0.223 | 1.61–0.97 | Tel 0.278, Odobert 0.257 | Yirenkyi 2.34, Onyeka 2.31 |

FM moves team forecasts for the first time (Brentford–Liverpool flipped to
0.500 away on Liverpool's attack prior vs Brentford's −1.05 defense;
hand-verified to 3 decimals against `forecast_with_fm`). Tackle leaders are
starting midfielders at ~2 expected tackles — sane magnitudes.

Rescue audit behind the 630 (`scripts/fm_staff_rescue.py`): of 732 quarantined
ledger attachments, 218 staff-kind/db-source/DOB-corroborated/tier≥2 rows
(183 via the native Reep typed FM bridge) carry 5+ valid union attrs and are
admitted with `fm_rescued_staff` provenance — the same validity gate the MLS
labels already used. Still out, with reasons: 326 absent from all FM exports,
~44 DOB-conflict rows, 107+ derived/unknown provenance, 8 weak-name. No shot
refit was needed: the heads OOS already prices staff-db rows through the
identical union gate.

## Measured accuracy behind these numbers

- Team goals: rolling-origin OOS over 816 EPL fixtures, WDL log-loss 1.0115
  vs 1.0839 home-advantage-only; FM priors at 0.1 improve post-window
  (0.9932 vs 1.0278, n=50) and post-vintage (1.0109 vs 1.0277, n=121)
  slices; 0.25 neutral, 0.5 worse. Small-n but consistent in both slices.
  See `docs/UNIFIED_FORECAST_OOS_2026-10-12.md`.
- Player shots: pooled chronological OOS, Poisson Δ−0.0075 with CI excluding 0.
- Player tackles (new): MLS-fitted, test Δ**−0.0372**, CI [−0.056, −0.019],
  occurrence Brier 0.147→0.123, calibration ratio 0.91. FM defensive
  attributes work where labels exist — 5x the shots-head magnitude.
- Recoveries: Δ−0.039 but CI includes 0 and occurrence Brier degrades
  (0.030→0.041) → gated challenger, not wired in.
- Conversion (two attribute sets: shot-attrs + clutch-attrs): validation
  selects baseline twice → stays baseline. Participation: EPL-only,
  prospectively gated.

## Limitations

- Histories end 2026-10-11 (team CSV through 10-10): one round stale by
  kickoff; retrospective-chronological validation only — scoring this matchday
  against actuals is the next required evidence.
- Tackle EPL application is a cross-league transfer (no EPL defensive labels
  exist anywhere); flagged `transfer_unvalidated` in provenance.
- FM team priors rest on small verified samples per club (Man City attack
  +3.0 is an outlier driven by few verified attackers); coefficient 0.1 was
  selected on modest slices — re-check as the 2026-27 season accumulates.
- 1,301/1,427 players use cold-start role priors; cards have no labels
  (engine red-card rate stays aggregate); duels/aerials/fouls/clearances
  inventoried but headless; each club's lone-GK cases simulate a GK dismissal
  without a replacement keeper.
