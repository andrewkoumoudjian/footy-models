#!/bin/bash
# Gate for the 36-month forecast RLM run (STEERING_36M.md).
set -uo pipefail
cd /home/drewclaw/footy-models
FAIL=0
MODEL_HASH="1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f"

echo "=== [1/7] cargo test --release ==="
cargo test --release -p footy-forecast -p footy-backtest -p footy-models 2>&1 | grep -E "test result:" | tail -3
if [ ${PIPESTATUS[0]} -ne 0 ]; then echo "GATE FAILED: cargo test"; FAIL=1; fi

echo "=== [2/7] top-100 csv: 100 rows + filter + ranking + no NaN ==="
if [ ! -f data/forecast_36m_top100.csv ]; then echo "GATE FAILED: top100 missing"; FAIL=1; else
  uv run --with pandas python -c "
import pandas as pd, sys
p = pd.read_csv('data/forecast_36m_top100.csv')
ok = True
if len(p) != 100: print('GATE FAILED: rows', len(p)); ok = False
if (p['age'] >= 23).any(): print('GATE FAILED: age'); ok = False
if ((p['current_mv'] < 400000) | (p['current_mv'] > 1200000)).any(): print('GATE FAILED: mv band'); ok = False
if 'attr_source' in p.columns and (p['attr_source'] == 'staff').any(): print('GATE FAILED: staff'); ok = False
col = 'gap_pct' if 'gap_pct' in p.columns else None
if col is None: print('GATE FAILED: no gap_pct col'); ok = False
elif list(p[col]) != sorted(p[col], reverse=True): print('GATE FAILED: not ranked'); ok = False
if p.isna().any().any(): print('GATE FAILED: NaN'); ok = False
sys.exit(0 if ok else 1)" 2>&1 | tail -3
  if [ ${PIPESTATUS[0]} -ne 0 ]; then FAIL=1; fi
fi

echo "=== [3/7] all csv >= 900 rows + filter ==="
if [ ! -f data/forecast_36m_all.csv ]; then echo "GATE FAILED: all missing"; FAIL=1; else
  uv run --with pandas python -c "
import pandas as pd, sys
p = pd.read_csv('data/forecast_36m_all.csv')
ok = len(p) >= 900 and (p['age'] < 23).all() and ((p['current_mv'] >= 400000) & (p['current_mv'] <= 1200000)).all()
print('all rows:', len(p)); sys.exit(0 if ok else 1)" 2>&1 | tail -2
  if [ ${PIPESTATUS[0]} -ne 0 ]; then echo "GATE FAILED: all pool"; FAIL=1; fi
fi

echo "=== [4/7] report md ==="
if [ ! -f data/forecast_36m_report.md ]; then echo "GATE FAILED: report md missing"; FAIL=1; else
  for needle in "1d023e96" "400000" "1200000" "exp(3" "top" "MC"; do
    grep -qi "$needle" data/forecast_36m_report.md || { echo "GATE FAILED: md missing '$needle'"; FAIL=1; }
  done
fi

echo "=== [5/7] pdf + white charts ==="
if [ ! -f data/forecast_36m_report.pdf ]; then echo "GATE FAILED: pdf missing"; FAIL=1; fi
CH=$(ls artifacts/mc_36m_white/*.png 2>/dev/null | wc -l)
if [ "$CH" -lt 5 ]; then echo "GATE FAILED: only $CH white charts"; FAIL=1; fi

echo "=== [6/7] model untouched ==="
ACTUAL=$(sha256sum data/forecast/model.bin | cut -d' ' -f1)
if [ "$ACTUAL" != "$MODEL_HASH" ]; then echo "GATE FAILED: model modified $ACTUAL"; FAIL=1; fi

echo "=== [7/7] determinism note ==="
grep -qiE "seed" data/forecast_36m_report.md || { echo "GATE FAILED: no seed"; FAIL=1; }

if [ $FAIL -eq 0 ]; then echo "ALL GATES PASSED"; else echo "GATE FAILED (see above)"; fi
exit $FAIL
