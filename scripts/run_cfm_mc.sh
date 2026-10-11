#!/bin/bash
# Hermes lifecycle guard: run the compiled Rust MC binary ONLY through this wrapper.
set -euo pipefail
cd /home/drewclaw/footy-models

echo "=== CFM MC base run (seed 42, 1000 paths, drift = 12m forecast annualized) ==="
./target/release/footy-mc run --input data/cfm_mc_input.tsv --out artifacts/cfm_mc \
  --paths 1000 --seed 42 --top-n 20 --basket-n 20

echo "=== CFM MC spike run (mu_ann = tail_tail_score expectile-0.9 drift) ==="
./target/release/footy-mc run --input data/cfm_mc_input_spike.tsv --out artifacts/cfm_mc_spike \
  --paths 1000 --seed 42 --top-n 20 --basket-n 20
