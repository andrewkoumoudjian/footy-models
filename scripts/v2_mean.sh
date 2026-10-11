#!/bin/bash
# v2 mean forecast for ALL candidates (open filters, no top-n cap)
set -euo pipefail
cd /home/drewclaw/footy-models
./target/release/footy-score forecast \
  --model data/forecast/model.bin \
  --candidates data/forecast/candidates_matrix.bin \
  --meta data/forecast/candidates_meta.tsv \
  --top-n 99999 --min-pa 0 --max-mv 999999999999 --rows 0 \
  --output data/demo/candidates_v2_mean.csv
echo "done"
wc -l data/demo/candidates_v2_mean.csv
head -1 data/demo/candidates_v2_mean.csv | cut -c1-180
