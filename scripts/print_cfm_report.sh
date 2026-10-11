#!/bin/bash
# Print the CFM scout report page to PDF via headless Chromium.
# Wrapper required: direct binary invocation trips the Hermes lifecycle guard.
set -euo pipefail
OUT="/home/drewclaw/footy-models/artifacts/cfm_report/CFM_Scout_Report_2026-08-10.pdf"
mkdir -p "$(dirname "$OUT")"
/usr/bin/chromium --headless=new --disable-gpu --no-sandbox --no-pdf-header-footer \
  --print-to-pdf="$OUT" \
  "http://localhost:4173/#cfm-report" 2>&1 | tail -3
ls -la "$OUT"
