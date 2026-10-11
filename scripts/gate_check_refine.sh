#!/bin/bash
# Gate for the spike-refinement RLM run (STEERING_REFINE.md).
set -uo pipefail
cd /home/drewclaw/footy-models
FAIL=0
FORECAST_HASH="1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f"
SURGE_MTIME="2026-08-10 02:16:18.267411325 -0400"

echo "=== [1/6] cargo test --release ==="
cargo test --release -p footy-backtest -p footy-forecast -p footy-models 2>&1 | grep -E "test result:" | tail -3
if [ ${PIPESTATUS[0]} -ne 0 ]; then echo "GATE FAILED: cargo test"; FAIL=1; fi

echo "=== [2/6] refined top100 csv ==="
if [ ! -f data/spike_refined_top100.csv ]; then echo "GATE FAILED: csv missing"; FAIL=1; else
  uv run --with pandas python -c "
import pandas as pd, sys
p = pd.read_csv('data/spike_refined_top100.csv')
ok = True
if len(p) != 100: print('GATE FAILED: rows', len(p)); ok = False
if (p['age'] >= 23).any(): print('GATE FAILED: age'); ok = False
if ((p['current_mv'] < 400000) | (p['current_mv'] > 1200000)).any(): print('GATE FAILED: mv'); ok = False
if (p['attr_source'] == 'staff').any(): print('GATE FAILED: staff'); ok = False
if p.isna().any().any(): print('GATE FAILED: NaN'); ok = False
spike_cols = [c for c in p.columns if 'spike' in c.lower() or 'surge' in c.lower() or 'tail' in c.lower()]
if not spike_cols: print('GATE FAILED: no spike/surge/tail score col'); ok = False
sys.exit(0 if ok else 1)" 2>&1 | tail -3
  if [ ${PIPESTATUS[0]} -ne 0 ]; then FAIL=1; fi
fi

echo "=== [3/6] report md ==="
if [ ! -f data/spike_refinement_report.md ]; then echo "GATE FAILED: report missing"; FAIL=1; else
  for needle in "forward" "hit" "50%" "100%" "seed" "baseline" "basket"; do
    grep -qi "$needle" data/spike_refinement_report.md || { echo "GATE FAILED: missing '$needle'"; FAIL=1; }
  done
fi

echo "=== [4/6] spike hit rate improvement vs old 36m top100 ==="
if [ ! -f data/spike_refinement_report.md ] || [ ! -f data/forecast_36m_top100.csv ]; then
  echo "GATE FAILED: missing comparison inputs"; FAIL=1
else
  grep -qiE "spike.*(hit|rate)|hit.*(spike|rate).*(>=|≥|more|higher|improve)" data/spike_refinement_report.md || { echo "GATE FAILED: no explicit spike hit-rate comparison statement"; FAIL=1; }
fi

echo "=== [5/6] deployed models untouched ==="
AH=$(sha256sum data/forecast/model.bin | cut -d' ' -f1)
SM=$(stat -c '%y' data/surge/model.bin)
if [ "$AH" != "$FORECAST_HASH" ]; then echo "GATE FAILED: forecast model modified"; FAIL=1; fi
if [ "$SM" != "$SURGE_MTIME" ]; then echo "GATE FAILED: surge model modified"; FAIL=1; fi

echo "=== [6/6] determinism note ==="
grep -qiE "seed" data/spike_refinement_report.md || { echo "GATE FAILED: no seed"; FAIL=1; }

if [ $FAIL -eq 0 ]; then echo "ALL GATES PASSED"; else echo "GATE FAILED (see above)"; fi
exit $FAIL
