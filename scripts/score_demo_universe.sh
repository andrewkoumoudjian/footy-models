#!/bin/bash
# Score the full candidate universe with v2 forecast + v3 tail (expectile-0.9).
set -euo pipefail
cd /home/drewclaw/footy-models
mkdir -p data/demo

echo "=== v2 forecast (mean) — all candidates ==="
./target/release/footy-score forecast \
  --model data/forecast/model.bin \
  --candidates data/forecast/candidates_matrix.bin \
  --meta data/forecast/candidates_meta.tsv \
  --rows 0 \
  --output data/demo/candidates_v2_forecast.csv

echo "=== v3 tail (expectile-0.9) — all candidates ==="
./target/release/footy-score tail-forecast \
  --model data/tail/model_exp090_r1e6.bin \
  --candidates data/forecast/candidates_matrix.bin \
  --meta data/forecast/candidates_meta.tsv \
  --output data/demo/candidates_v3_tail.csv

echo "=== done ==="
wc -l data/demo/candidates_v2_forecast.csv data/demo/candidates_v3_tail.csv
