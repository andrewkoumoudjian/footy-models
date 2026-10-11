# Premier League pre-match forecast — 11–12 October 2026

**Model as-of:** 2026-10-10, after the six Saturday fixtures. No October 11–12 results are included.

## Pre-match forecast

| Date | Fixture | Home | Draw | Away | Expected goals | Modal score | FM coverage |
|---|---|---:|---:|---:|---|---|---|
| 2026-10-11 | Crystal Palace vs Nott'm Forest | 32.4% | 32.7% | 35.0% | 1.23–1.28 | 1-1 | 100% / 100% |
| 2026-10-11 | Hull vs Everton | 36.9% | 40.3% | 22.8% | 1.02–0.78 | 1-0 | 33% / 100% |
| 2026-10-11 | Liverpool vs Man City | 28.2% | 28.0% | 43.8% | 1.38–1.69 | 1-1 | 100% / 100% |
| 2026-10-12 | Coventry vs Newcastle | 23.2% | 29.6% | 47.2% | 1.13–1.59 | 1-1 | 0% / 100% |

## Source and model contracts

- **Recent match results:** historical English Premier League E0 (`2023/24`–`2026/27`) and 2025/26 Championship E1 from [football-data.co.uk](https://www.football-data.co.uk/data.php), cached as untracked CSV. October 10 results supplied in the checked fixture configuration and sourced to the contemporary Reuters recap.
- **FM26:** `data/fm_tm_joined.csv`, the existing FM26 / Transfermarkt joined June 2026 snapshot. No remerging or inferred player-team identity. For each club take the strongest 15 validly matched players by FM Current Ability and combine `ca` (73% of index) with `sig_attacking` + `sig_creative` for attack, `sig_defensive` + `sig_mental` for defence (27%). GK offensive contributions use the neutral value; derived rather than database attributes get 0.4 reliability. Mean-center against covered EPL clubs, standardize, shrink by coverage. Positive defensive strength suppresses opponent scoring intensity.
- **Model:** Regularized, 270-day half-life Poisson attack/defence goal intensities fitted on dated results with an explicit home advantage; 2025/26 Championship is a constrained prior for promoted clubs. FM26 attack/defence adjustments enter predicted log goal rates with fixed coefficient `0.16` and are **not** used to fit the older-season historical model.
- **Calibration:** Use the existing `footy-match::OutcomeCalibrator`, fitted solely on previous-season chronological predictions (temperature and draw-logit shift), never on held-out matches.
- **Coverage limitations:** Hull has 5 usable current linked FM26 players (33% reliability); Coventry has no reliable linked players (0% FM input, result-driven baseline). June 2026 player-club mappings cannot account for transfers, injuries, suspensions, or final starting XIs. Liverpool and Manchester City each have 15/15 coverage of the selected FM proxies; **these are not confirmed lineups**.
- **Model boundary:** The player-event second-resolved Monte Carlo engine remains in `crates/match`. It is not invoked for these fixtures because the repository lacks validated current PL event data and lineup inputs. The new team-goals path uses its existing calibrator and keeps FM player priors explicit instead of inventing player observations.

## Historical validation and uncertainty

| Pre-match model variant | Brier (lower better) | Log loss (lower better) |
|---|---:|---:|
| Results only + existing calibration | 0.6068 | 1.0139 |
| Results + FM26 + existing calibration | 0.5949 | 0.9923 |

- Chronological walk-forward holdout: **56 matches** from the current PL season, using only pre-kickoff match outcomes for fitted goal model. Earlier **380 samples** fit the probability calibrator.
- FM Brier improvement day-block bootstrap 95% interval: **-0.0153 to 0.0450**. It includes zero. Any incremental FM predictive improvement remains **unproven**, even though point estimates improve. The `0.16` adjustment is an explicitly fixed heuristic, not tuned to held-out results.
- Independent Poisson scoring ignores shared tempo, red-card timing, and correlated scoring; the given probabilities are model estimates, not calibrated epistemic credible intervals.

## Reproduction

```bash
# From the repository root. Downloads source score CSVs; keeps raw data local.
python3 scripts/forecast_premier_league.py --release --fixtures configs/pl_2026-10-11.json
# Reproduce from cached source CSVs without new network requests:
python3 scripts/forecast_premier_league.py --offline --release
cargo test -p footy-match -p footy-plforecast
```

Results go to `data/pl/forecasts.json`, FM coverage audit to `data/pl/fm_coverage.json`, and exact source provenance to `data/pl/provenance.json`. These generated files remain local; this document preserves the frozen October 10 forecast. Supply a new configuration with an earlier-than-kickoff as-of and future fixtures to reuse on another matchweek. For a different season, refresh the FM club snapshot before using FM priors.
