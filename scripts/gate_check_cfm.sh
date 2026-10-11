#!/bin/bash
# Gate for the CFM scouting RLM run (STEERING_CFM.md).
set -uo pipefail
cd /home/drewclaw/footy-models
FAIL=0
FORECAST_HASH="1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f"
TAIL_MTIME="2026-08-10 10:56:55.016172448 -0400"
SURGE_MTIME="2026-08-10 02:16:18.267411325 -0400"

echo "=== [1/7] cargo test --release ==="
cargo test --release -p footy-backtest -p footy-forecast -p footy-models 2>&1 | grep -E "test result:" | tail -3
if [ ${PIPESTATUS[0]} -ne 0 ]; then echo "GATE FAILED: cargo test"; FAIL=1; fi

echo "=== [2/7] top20 csv ==="
if [ ! -f data/cfm_scout_top20.csv ]; then echo "GATE FAILED: csv missing"; FAIL=1; else
  uv run --with pandas python -c "
import pandas as pd, sys
p = pd.read_csv('data/cfm_scout_top20.csv')
ok = True
if len(p) != 20: print('GATE FAILED: rows', len(p)); ok = False
need = ['psych_fit','tail_score','squad_fit','sofascore_rating']
for c in need:
    if c not in p.columns: print('GATE FAILED: missing col', c); ok = False
if 'yield_12m_pct' in p.columns and (p['yield_12m_pct'] <= 0).any(): print('GATE FAILED: non-positive yield'); ok = False
if p.isna().any().any(): print('GATE FAILED: NaN'); ok = False
sys.exit(0 if ok else 1)" 2>&1 | tail -3
  if [ ${PIPESTATUS[0]} -ne 0 ]; then FAIL=1; fi
fi

echo "=== [3/7] report md needles ==="
if [ ! -f data/cfm_scout_report.md ]; then echo "GATE FAILED: report missing"; FAIL=1; else
  for needle in archetype alumni psych_fit "Monte Carlo" seed SofaScore FBref limitations gap; do
    grep -qi "$needle" data/cfm_scout_report.md || { echo "GATE FAILED: missing '$needle'"; FAIL=1; }
  done
fi

echo "=== [4/7] MC outputs ==="
for d in artifacts/cfm_mc artifacts/cfm_mc_spike; do
  if [ ! -f "$d/stats.tsv" ]; then echo "GATE FAILED: $d/stats.tsv missing"; FAIL=1; fi
done
if [ -f artifacts/cfm_mc/stats.tsv ]; then
  N=$(tail -n +2 artifacts/cfm_mc/stats.tsv | wc -l)
  if [ "$N" -lt 20 ]; then echo "GATE FAILED: cfm_mc stats rows $N"; FAIL=1; fi
fi
for d in artifacts/cfm_mc/charts artifacts/cfm_mc_spike/charts; do
  if [ -d "$d" ]; then
    CN=$(ls "$d"/*.png 2>/dev/null | wc -l)
    if [ "$CN" -lt 20 ]; then echo "GATE FAILED: $d chart count $CN"; FAIL=1; fi
  else
    echo "GATE FAILED: $d missing"; FAIL=1
  fi
done

echo "=== [5/7] MC inputs ==="
for f in data/cfm_mc_input.tsv data/cfm_mc_input_spike.tsv; do
  if [ ! -f "$f" ]; then echo "GATE FAILED: $f missing"; FAIL=1; else
    C=$(head -1 "$f" | awk -F'\t' '{print NF}')
    if [ "$C" != "11" ]; then echo "GATE FAILED: $f cols $C"; FAIL=1; fi
  fi
done

echo "=== [6/7] footy-site build ==="
if [ -d /home/drewclaw/footy-site ]; then
  (cd /home/drewclaw/footy-site && npm run build 2>&1 | tail -4) > /tmp/cfm_gate_sitebuild.log 2>&1
  if [ ${PIPESTATUS[0]} -ne 0 ]; then echo "GATE FAILED: site build"; FAIL=1; fi
  if ! grep -rq "CFM Scout\|CF Montréal" /home/drewclaw/footy-site/dist 2>/dev/null; then
    echo "GATE FAILED: CFM section not in dist"; FAIL=1
  fi
else
  echo "GATE FAILED: footy-site missing"; FAIL=1
fi

echo "=== [7/7] models untouched ==="
AH=$(sha256sum data/forecast/model.bin | cut -d' ' -f1)
if [ "$AH" != "$FORECAST_HASH" ]; then echo "GATE FAILED: forecast model modified"; FAIL=1; fi
ET=$(stat -c '%y' data/tail/model_exp090_r1e6.bin 2>/dev/null)
ES=$(stat -c '%y' data/surge/model.bin 2>/dev/null)
if [ "$ET" != "$TAIL_MTIME" ]; then echo "GATE FAILED: tail model modified ($ET)"; FAIL=1; fi
if [ "$ES" != "$SURGE_MTIME" ]; then echo "GATE FAILED: surge model modified ($ES)"; FAIL=1; fi

if [ $FAIL -eq 0 ]; then echo "ALL GATES PASSED"; else echo "GATE FAILED (see above)"; fi
exit $FAIL
