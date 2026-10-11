# FOOTY-MODELS — Value-Surge Transfer Proposals (report placeholder)

Generated: 2026-08-10 07:00 EDT (target)

## Summary
PLACEHOLDER — final numbers from data/model_report.md

## Methodology (FM26 + Transfermarkt, data-backed)
- FM26 male database (563,382 men, full 1-20 attributes, CA/PA) as player universe + talent signal
- Transfermarkt (CC0 weekly dataset, valuations through 2026-06-12) as market overlay
- Join: 14,783 players (name+DOB / surname+DOB / name+birth-year)
- Candidate universe: MV EUR 400k-1M, age <= 28, FM attrs present (db or derived)
- Surge label: MV +50% within 12 months (from player_valuations history)
- Model: Rust, walk-forward validated, calibrated probabilities
- Selection: top-100 by predicted surge probability, FM quality gate (position-appropriate attributes)

## Results
PLACEHOLDER — walk-forward AUC, calibration, decile table

## Top 100 transfer proposals
PLACEHOLDER — embedded table from data/proposals_top100.csv

## Caveats
- FM attrs are a single snapshot (FM26, summer 2026); attr_source=derived rows are statistically generated (flagged)
- Transfermarkt data through 2026-06-12
- All outputs are probabilities with calibrated intervals, not guarantees
