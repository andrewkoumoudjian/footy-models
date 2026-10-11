#!/bin/bash
# Gate check for the footy-models value-surge deliverables.
set -u
cd /home/drewclaw/footy-models
FAIL=0

if [ ! -f data/proposals_top100.csv ]; then
  echo "GATE FAIL: data/proposals_top100.csv missing"; FAIL=1
else
  python3 - <<'PY'
import csv, sys
rows = list(csv.DictReader(open('data/proposals_top100.csv')))
errs = []
if len(rows) < 100:
    errs.append(f"fewer than 100 rows: {len(rows)}")
req = ["player_id","name","current_mv","fm_ca","fm_pa","attr_source","predicted_surge_probability","rationale"]
for c in req:
    if c not in rows[0]:
        errs.append(f"missing column {c}")
for i, r in enumerate(rows):
    mv = r.get("current_mv","")
    try: mv = float(mv)
    except: errs.append(f"row {i}: current_mv not numeric ({mv!r})"); continue
    if not (400000 <= mv <= 1000000):
        errs.append(f"row {i}: current_mv {mv} out of [400k,1M]")
    for c in ["fm_ca","fm_pa","predicted_surge_probability"]:
        v = r.get(c,"")
        if v in ("","nan","NaN","None"):
            errs.append(f"row {i}: {c} empty/NaN")
            break
if errs:
    print("GATE FAIL (proposals):")
    for e in errs[:10]: print("  -", e)
    sys.exit(1)
print("GATE OK: proposals file valid,", len(rows), "rows")
PY
  [ $? -ne 0 ] && FAIL=1
fi

if [ ! -f data/model_report.md ]; then
  echo "GATE FAIL: data/model_report.md missing"; FAIL=1
else
  if ! grep -qi "walk-forward" data/model_report.md; then
    echo "GATE FAIL: model_report.md missing walk-forward validation description"; FAIL=1
  fi
  if ! grep -qi "fm_universe\|football manager" data/model_report.md; then
    echo "GATE FAIL: model_report.md missing FM data usage description"; FAIL=1
  fi
fi

if [ $FAIL -eq 0 ]; then
  echo "ALL GATES PASSED"
  exit 0
else
  echo "GATE FAILURES PRESENT"
  exit 1
fi
