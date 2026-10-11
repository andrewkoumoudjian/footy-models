# MLS Forecast Pipeline — Walk-Forward Backtest Results

**Repo:** ~/footy-models · **Binary:** `apps/matchforecast/src/backtest.rs` (`footy-matchforecast-backtest`)
**Engine:** `simulate_second_resolved_dynamic_mental_match` (merged footy-match crate)
**Data:** SofaScore 2026 season (98 match lineups, 115 team-stat files) + FM26 attributes (name-join)
**Protocol:** strictly as-of walk-forward — for each of the last 12 CF Montréal fixtures, team
windows and player aggregates use ONLY matches before that fixture's date; FM26 contributes
attributes only (club-blind name join), squads come from observed 2026 minutes.

## Run history (300 paths/fixture, seed 42)

| Version | Changes | Top-pick acc | Brier | Log loss |
|---|---|---|---|---|
| v1 | baseline | 3/12 = 25% | 0.8006 | 1.270 |
| v2 smoke | goal-rate anchor + SOT anchoring | 3/12 = 25% | 0.8538 | 1.332 |
| **v3 final** | + squad freshness filter (45d), fresh as-of prep | 3/12 = 25% | **0.7646** | **1.2200** |

Uniform-random references: Brier 0.667, log loss 1.099. Majority-class baseline: 50%.

## What improved

- Brier 0.801 → 0.765 and log loss 1.270 → 1.220: probabilities are better aligned even though
  top-pick accuracy is flat (12 fixtures with 5 draws is a brutal sample for argmax accuracy).
- Goal rates now in a realistic band (~2.4–5.3 total xG vs actual MLS ~2.8).
- Squad staleness fixed: players with no appearance in the last 45 days are excluded.

## What is still broken (honest assessment)

1. **Model is still worse than random on Brier** (0.765 > 0.667). The main offender: extreme
   away-favorite calls that failed (CFM home games: model gave visitors 51–95%, reality was
   5 draws + 1 win for CFM at home in this window).
   - The CF-vs-Inter fixture shows overconfidence blowup: P(away)=0.95 from an anchored
     xGA of 6.28 — the SOT anchor compounding with Miami's elite player means.
2. **Home advantage appears inverted for CFM**: model rates CFM's home games as visitor-leaning.
   Suspect the team-context residual adjustments double-count opponent strength.
3. **Draws remain systematically underpredicted** (model P(draw) avg 0.19 vs reality 42%).

## Recommended next steps (priority order)

1. Clamp per-team anchor scale harder (cap xG/xGA windows at realistic MLS bounds, e.g. ≤2.6)
   to kill overconfidence blowups like the 0.95 call.
2. Add explicit draw mass: blend outcome probs with an empirical draw base rate (~28% MLS)
   via temperature/logit-prior calibration fitted on walk-forward output.
3. Investigate home/away residual sign convention in SpecificTeamContext (attack adj for the
   HOME club may need to be relative to league-home-average, not raw).
4. Only after Brier < 0.66: raise paths to 2000+ and trust forward forecasts.

## Reproduce

```bash
python3 scripts/prep_backtest.py 12          # rebuild as-of player files
cargo build --release -p footy-matchforecast
./target/release/footy-matchforecast-backtest --paths 300 --seed 42
```

Artifacts: artifacts_backtest.txt (v1), artifacts_backtest_smoke.txt (v2), artifacts_backtest_v2.txt (v3),
data/sofascore_cfm/squad_staleness_audit.json (FM26/SofaScore staleness audit).
