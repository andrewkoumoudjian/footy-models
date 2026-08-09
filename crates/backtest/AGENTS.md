# `backtest` crate

Own walk-forward evaluation, historical freeze/cutoff logic, model metrics, gem/portfolio backtests, baseline comparisons, and reproducibility records.

- Never use random row splits for headline valuation/gem evidence.
- A backtest cutoff freezes source data, feature transforms, league-strength estimates, calibration, and model fitting at the historical date.
- Record dataset, feature-set, model, calibration versions, and random seed for every run.
- Always compare gem signals against simple baselines: Transfermarkt value, FM CA, FM PA, age+minutes, performance-only, and price-bucket random selection.
- Report subgroup metrics by age, role, league/tier, price, minutes/data density, contract, and route.
- Interval coverage is a required metric, not optional decoration.
- Prevent production crates from depending on this crate.
