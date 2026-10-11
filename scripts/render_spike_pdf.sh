#!/bin/bash
# Render the spike refinement report markdown to a white-theme PDF.
set -euo pipefail
uv run --with weasyprint --with markdown python3 /home/drewclaw/footy-models/scripts/render_spike_pdf.py 2>&1 | tail -1
ls -la /tmp/footy_spike_refinement_report.pdf
