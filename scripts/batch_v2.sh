#!/bin/bash
# Batch-score v2 forecast model over the full candidate matrix -> raw predictions.bin
set -euo pipefail
cd /home/drewclaw/footy-models
./target/release/footy-score batch \
  --model data/forecast/model.bin \
  --features data/forecast/candidates_matrix.bin \
  --output data/demo/candidates_v2_predictions.bin
echo "batch done"
ls -la data/demo/candidates_v2_predictions.bin
