#!/bin/bash
# Re-run the real forward test with the deployed v2 forecast model (independent check).
set -uo pipefail
cd /home/drewclaw/footy-models
./target/release/footy-forward forecast \
  --model data/forecast/model.bin \
  --candidates data/forward_test/forecast_candidates.tsv \
  --realized data/forward_test/forecast_realized.tsv \
  --meta data/forward_test/forecast_meta.tsv \
  --as-of 2025-11-06 \
  --horizon-end 2026-06-12 \
  --seed 42 \
  --label verify_v2 \
  --output /tmp/forward_verify.md \
  --metrics /tmp/forward_verify_metrics.tsv
echo "EXIT:$?"
