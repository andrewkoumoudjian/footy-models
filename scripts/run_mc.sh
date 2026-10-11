#!/bin/bash
# Run the 36-month Monte Carlo simulation (footy-mc) on the forecast top players.
set -euo pipefail
cd /home/drewclaw/footy-models
./target/release/footy-mc run \
  --input data/mc_input.tsv \
  --out artifacts/mc_36m \
  --paths 1000 \
  --seed 42 \
  --top-n 10 \
  --basket-n 25
