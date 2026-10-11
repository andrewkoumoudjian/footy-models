#!/bin/bash
# Gate for the backtest + optimize RLM run (STEERING_BACKTEST.md).
# Exit 0 = pass, non-zero = fail (output fed back to the agent for repair).
set -uo pipefail
cd /home/drewclaw/footy-models
FAIL=0

echo "=== [1/5] cargo test --release ==="
cargo test --release -p footy-backtest -p footy-forecast -p footy-models 2>&1 | grep -E "test result:" | tail -5
CARGO_STATUS=${PIPESTATUS[0]}
if [ $CARGO_STATUS -ne 0 ]; then
    echo "GATE FAILED: cargo test failed (exit $CARGO_STATUS)"
    FAIL=1
fi

echo "=== [2/5] backtest report ==="
if [ ! -f data/backtest_report.md ]; then
    echo "GATE FAILED: data/backtest_report.md missing"
    FAIL=1
else
    for needle in "as-of" "forward" "IC" "baseline" "optimization" "recommend"; do
        if ! grep -qi "$needle" data/backtest_report.md; then
            echo "GATE FAILED: report missing section containing '$needle'"
            FAIL=1
        fi
    done
    # forward test must have real numbers: a Spearman IC line
    if ! grep -qiE "spearman|rank ic|ic[ :=]" data/backtest_report.md; then
        echo "GATE FAILED: report missing forward-test IC number"
        FAIL=1
    fi
    # optimization log must have at least 2 configs
    N_CONFIGS=$(grep -ciE "^\s*\|?\s*(config|λ|ridge|run|variant)" data/backtest_report.md)
    if [ "$N_CONFIGS" -lt 2 ]; then
        echo "GATE FAILED: optimization log needs >=2 config rows (found $N_CONFIGS)"
        FAIL=1
    fi
fi

echo "=== [3/5] existing deliverables intact ==="
for f in data/proposals_top100.csv data/forecast_u23_top100.csv data/forecast/train.tsv data/forecast/model.bin; do
    if [ ! -s "$f" ]; then
        echo "GATE FAILED: $f missing or empty"
        FAIL=1
    fi
done
if [ -f data/forecast_u23_top100.csv ]; then
    ROWS=$(($(wc -l < data/forecast_u23_top100.csv) - 1))
    if [ "$ROWS" -lt 90 ]; then
        echo "GATE FAILED: forecast_u23_top100.csv has only $ROWS rows"
        FAIL=1
    fi
fi

echo "=== [4/5] no NaN in reports ==="
if grep -qiE "nan|inf(inity)?" data/backtest_report.md; then
    echo "GATE FAILED: NaN/Inf found in backtest_report.md"
    FAIL=1
fi

echo "=== [5/5] determinism note ==="
if ! grep -qiE "seed" data/backtest_report.md; then
    echo "GATE FAILED: report missing seed statement"
    FAIL=1
fi

if [ $FAIL -eq 0 ]; then
    echo "ALL GATES PASSED"
else
    echo "GATE FAILED (see above)"
fi
exit $FAIL
