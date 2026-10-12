# UCL matchday forecast 2026-10-13–14 (MD2) — single entry point

**Status: executed, research-only.** All 18 MD2 fixtures, all 36 clubs, one
command through the same validated engine (TeamGoalModel + pooled shot/tackle
heads + Rust simulator) and the single entry point
(`scripts/forecast_matchday.py`). Probabilities are calibrated-simulation
estimates with measured OOS accuracy stated below; not betting odds.
Canonical run: `data/ucl/md2_20261013_14_v2` — v1 was the superseded run whose
engine limitation strings still said EPL (numbers identical and
deterministic; v2 corrects the labels via `--competition-label UCL`).

## Single entry point

```bash
python3 scripts/forecast_matchday.py \
  --schedule data/ucl/raw_espn/board_20261013.json data/ucl/raw_espn/board_20261014.json \
  --as-of 2026-10-12 --out-dir data/ucl/md2_20261013_14_v2 \
  --paths 2500 --seed 42 \
  --squads data/ucl/squads_2026-10-12_full.json \
  --espn data/ucl/espn_ucl_md1_20261012.json data/ucl/espn_ucl_202526_20261012.json \
         data/ucl/espn_ucl_202425_20261012.json \
  --crosswalk data/ucl/crosswalk_ucl_20261012.json \
  --history data/ucl/asof_team_stats_ucl_v1.csv \
  --results data/ucl/results_ucl_v1.tsv \
  --ledger data/ucl/squads_2026-10-12_canonical_identity_evidence.json \
  --heads data/ucl/ucl_shot_heads_v1.json \
  --fm-priors data/ucl/fm26_team_priors_ucl_v1.json --fm-coeff 0.1 \
  --expected-clubs 36 --club-map ucl
```

Defaults apply the UCL-fitted shot head (`--heads`), FM team priors at
coefficient 0.1 (`--fm-priors`, `--fm-coeff`), 2500 paths. Layers:
`scripts/build_universal_bundle.py` (36-club as-of bundle: 1,096 eligible
players, EB baselines for **755** ESPN-linked players, cold-start role priors
otherwise, **767 FM-verified trait blocks** incl. 248 staff-db rescues, tackle
baselines + tackle feature vectors for all, 415 prior UCL matches, 0
Championship priors — there is no lower-league UCL prior) →
`footy-matchforecast-math --heads-artifact --fm-priors --fm-coeff
--competition-label UCL` (in-engine UCL-fitted pooled shot residuals via
`PooledShotCorrection`, MLS-fitted tackle residuals via `PooledEventRate` as a
disclosed cross-league transfer, FM-adjusted team anchors) → coherence
validation (18/18 fixtures, analytic-Poisson consistent) → hash-pinned run
dir. Python owns data plumbing, Rust owns all math. Refuses to overwrite
existing runs.

Generalization notes (EPL behavior preserved bit-for-bit: regenerated EPL
bundle gives the same 1,427 players / 630 verified / 189 rescues / 126
corrected links as the v3 doc): `--expected-clubs`/`--club-map` on the bundle
and entry point; 36-club closed identity map (`UCL_ESPN_TO_CANONICAL`);
occurrence-indexed provisional ids for the one squad homonym collision
(Shakhtar's two distinct Pedrinhos — both correctly left unresolved, missing
stays missing); `--league/--dates` on `espn_feed.py`; `--competition-label`
on the engine for accurate limitation strings.

## Forecasts (v2: UCL shot head + FM team priors + tackles, 2500 paths, seed 42, MC SE ≈ 0.01)

| Fixture | Home | Draw | Away | xG | Top P(goal) | Top xTackles |
|---|---|---|---|---|---|---|
| Arsenal v Lille | 0.814 | 0.124 | 0.062 | 2.79–0.71 | Saka 0.209, Gyökeres 0.195 | Haraldsson 1.06, André 1.05 |
| Atlético Madrid v Man United | 0.556 | 0.203 | 0.240 | 2.32–1.49 | Sørloth 0.194, Álvarez 0.163 | Mount 0.97, Baleba 0.94 |
| Galatasaray v Barcelona | 0.345 | 0.224 | 0.430 | 1.70–1.96 | Yamal 0.166, Osimhen 0.130 | Gavi 0.98, de Jong 0.97 |
| Internazionale v Club Brugge | 0.707 | 0.162 | 0.131 | 2.44–0.96 | Thuram 0.173, Martínez 0.168 | Zielinski 1.01, Çalhanoglu 1.00 |
| Lens v Sporting CP | 0.489 | 0.231 | 0.280 | 1.90–1.38 | Thauvin 0.149, Kadile 0.131 | Bulatovic 0.97, Catamo 0.93 |
| RB Leipzig v PSV Eindhoven | 0.361 | 0.221 | 0.418 | 1.61–1.75 | Pepi 0.118, Perisic 0.116 | Baumgartner 0.94, Nusa 0.93 |
| Sabah FK v Slavia Prague | 0.481 | 0.218 | 0.300 | 1.89–1.46 | Lachaab 0.124, Mickels 0.118 | Aliyev 0.87, Severina 0.85 |
| Viking FK v Bayern Munich | 0.184 | 0.159 | 0.657 | 1.53–2.89 | Kane 0.238, Olise 0.228 | Kimmich 1.13, Olise 1.12 |
| Villarreal v Napoli | 0.484 | 0.228 | 0.288 | 1.68–1.22 | Pépé 0.126, Gaitán 0.110 | Thiam 1.03, Comesaña 1.01 |
| AS Roma v Real Madrid | 0.351 | 0.218 | 0.432 | 1.72–1.93 | Malen 0.171, Soulè 0.151 | Pisilli 1.14, Pellegrini 1.12 |
| Aston Villa v Fenerbahce | 0.586 | 0.210 | 0.204 | 2.15–1.23 | Jackson 0.148, Abraham 0.142 | Bogarde 0.98, Onana 0.97 |
| Bodo/Glimt v Borussia Dortmund | 0.398 | 0.220 | 0.382 | 1.91–1.84 | Fábio Silva 0.131, Guirassy 0.120 | Nmecha 0.88, Sabitzer 0.88 |
| Feyenoord v Como | 0.387 | 0.207 | 0.406 | 1.95–2.01 | Jesús Rodriguez 0.158, Kean 0.140 | Caqueret 1.03, Lahdo 0.99 |
| LASK Linz v Liverpool | 0.270 | 0.269 | 0.462 | 1.24–1.66 | Ekitike 0.128, Chiesa 0.122 | Horvath 0.98, Danek 0.97 |
| Man City v Paris Saint-Germain | 0.360 | 0.207 | 0.434 | 1.77–1.91 | Dembélé 0.192, Kvaratskhelia 0.165 | Zaïre-Emery 1.16, Neves 1.15 |
| Real Betis v FC Porto | 0.702 | 0.156 | 0.141 | 2.82–1.23 | Antony 0.191, Parrott 0.178 | Ceballos 0.99, Lo Celso 0.98 |
| Shakhtar v AEK Athens | 0.338 | 0.242 | 0.419 | 1.30–1.48 | Argyriou 0.100, Varga 0.099 | Zubkov 0.87, Kutesa 0.86 |
| Slovan Bratislava v VfB Stuttgart | 0.252 | 0.221 | 0.527 | 1.47–2.15 | Leweling 0.159, Demirovic 0.154 | Malanga 0.93, Führich 0.92 |

Top scorelines per fixture are in `predictions.json` (e.g. Arsenal–Lille
2–0 0.121, 3–0 0.099; Man City–PSG 1–2 0.094, 1–1 0.082).

Identity notes: 767/1,096 (70.0%) FM-verified trait blocks — European clubs
are indeed better covered than EPL's 44.2%, measured not assumed. Post-window
squad currentness is real: Varga/Zubkov resolve FM-verified to AEK Athens in
the Oct-2026 roster snapshot. 130 registrations stay unresolved (incl. both
Pedrinhos); Shakhtar (21), Sabah (10) and Bodo/Glimt (8) account for the
largest shares.

## Measured accuracy behind these numbers

- Team goals: rolling-origin OOS over **415 UCL fixtures** (Feb 2024 → Sep
  2026, 241 scored, 50 matchday clusters), WDL log-loss **0.9475 vs 1.0290**
  home-advantage-only (Δ−0.0816, CI [−0.115, −0.046]); WDL Brier 0.5604 vs
  0.6205; scoreline 3.2943 vs 3.3984; both seasons favor the fitted model.
  FM priors at 0.1 improve the post-vintage slice (0.8485 vs 0.8678 no-FM on
  the same 45 fixtures from 2026-03-01; small-n, same direction as EPL) —
  this is the demonstrated gain licensing `--fm-coeff 0.1`.
- Player shots (UCL-fitted head, `scripts/fit_ucl_shot_head.py`): train
  2024-25 / val 2025-26 (8,199 rows: 0.8611→0.8525) / test MD1 Sep 2026
  (798 rows, 18 matchdays): pooled Δ**−0.0245**, CI **[−0.0377, −0.0118]** —
  3x the EPL+MLS pooled magnitude, FM attributes in the head throughout.
  Wired in under the promotion rule.
- Player tackles: no UCL defensive labels exist anywhere → the MLS-fitted
  tackles head applies as a **disclosed, ungated cross-league transfer**
  (`transfer_unvalidated` provenance in the engine limitations), exactly the
  status tackles held in the EPL v3 run.
- Conversion/participation/recoveries: stay gated challengers (no UCL label
  evaluation run; not wired in).

## Limitations

- Team histories end 2026-09-10 (MD1): two matchdays stale by MD3 kickoff;
  retrospective-chronological validation only — scoring MD2 against actuals
  is the next required evidence.
- Player-event magnitudes are compressed vs EPL (top P(goal) ~0.24 vs ~0.51):
  UCL `minutes_prior` is uniformly missing (0.0 experience weight) where EPL
  carries measured 2526 minutes, so expected minutes run low. Team-level
  WDL/xG are unaffected (means preserved by construction) but per-player
  goal/tackle magnitudes should be read as conservative.
- Minnow-home xGs (Viking 1.53, Sabah 1.89) reflect thin UCL histories for
  debutants under the league-average prior; the aggregate OOS covers this
  regime only to the extent past debutants appear in 2024–26 histories.
- 10 ET/penalty knockout fixtures are excluded from team history
  (unrecoverable regulation scores); 3 rejected ESPN summaries stay rejected.
- ClubElo never entered (API 502); no rating of any kind is in the inputs.
  FBref/SofaScore/WhoScored/FotMob UCL coverage: none via soccerdata 1.9.1
  (see `docs/UCL_PROVIDER_DISCOVERY_2026-10-12.md`). An EB team-strength
  baseline beyond home-advantage-only is not implemented for UCL (same as
  EPL). MD3 (Oct 20–21) is cached and forecastable with the same command.
