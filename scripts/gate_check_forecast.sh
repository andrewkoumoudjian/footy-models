#!/bin/bash
# Gate check for the footy-models 12-month value-forecast deliverables.
set -u
cd /home/drewclaw/footy-models
FAIL=0

F=data/forecast_u23_top100.csv
R=data/forecast_report.md

if [ ! -f "$F" ]; then
  echo "GATE FAIL: $F missing"
  exit 1
fi
if [ ! -f "$R" ]; then
  echo "GATE FAIL: $R missing"
  exit 1
fi

# Row count + column invariants via python
python3 - "$F" <<'PY'
import csv, sys
rows = list(csv.DictReader(open(sys.argv[1])))
req = ["player_id","name","age","current_mv","fm_ca","fm_pa","attr_source",
       "predicted_12m_mv","gap_eur","gap_pct","interval_10","interval_90",
       "key_strengths","rationale"]
errs = []
if len(rows) != 100:
    errs.append(f"row count {len(rows)} != 100")
for c in req:
    if c not in rows[0]:
        errs.append(f"missing column {c}")
for r in rows:
    try:
        age = float(r["age"]); mv = float(r["current_mv"]); pa = float(r["fm_pa"])
        p12 = float(r["predicted_12m_mv"]); gap = float(r["gap_eur"]); gp = float(r["gap_pct"])
    except (TypeError, ValueError):
        errs.append(f"non-numeric row {r.get('name')}"); continue
    if not (age < 23.0):
        errs.append(f"age {age} not <23: {r.get('name')}")
    if not (pa >= 140.0):
        errs.append(f"fm_pa {pa} <140: {r.get('name')}")
    if not (0 < mv <= 1_000_000):
        errs.append(f"current_mv {mv} out of range: {r.get('name')}")
    if not (p12 > 0):
        errs.append(f"predicted_12m_mv {p12} <= 0: {r.get('name')}")
    if gp < -1.0:
        errs.append(f"gap_pct {gp} < -100%: {r.get('name')}")
    if r.get("attr_source") not in ("db", "derived"):
        errs.append(f"attr_source {r.get('attr_source')}: {r.get('name')}")
    if not r.get("key_strengths") or not r.get("rationale"):
        errs.append(f"empty text fields: {r.get('name')}")
if errs:
    print("GATE FAIL:")
    for e in errs[:15]:
        print(" -", e)
    sys.exit(1)
print(f"GATE OK: forecast file valid, {len(rows)} rows")
PY
[ $? -eq 0 ] || FAIL=1

# Determinism: cargo tests must pass
if [ $FAIL -eq 0 ]; then
  cargo test --release 2>&1 | tail -2 | grep -q "test result: ok" || FAIL=1
fi

if [ $FAIL -eq 0 ]; then
  echo "ALL GATES PASSED"
  exit 0
fi
echo "GATE FAILED"
exit 1
