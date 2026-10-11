#!/usr/bin/env python3
"""Render the designed CFM report HTML to PDF via headless Chromium (Letter)."""
import subprocess, pathlib

BASE = pathlib.Path("/home/drewclaw/footy-models/artifacts/cfm_report")
OUT = BASE / "CFM_Scout_Report_2026-08-10_design.pdf"

r = subprocess.run(
    [
        "/usr/bin/chromium", "--headless=new", "--disable-gpu", "--no-sandbox",
        "--no-pdf-header-footer",
        f"--print-to-pdf={OUT}",
        f"file://{BASE / 'CFM_Scout_Report_design.html'}",
    ],
    capture_output=True, text=True, timeout=180,
)
print("exit:", r.returncode)
if r.stderr.strip():
    print(r.stderr.strip()[-300:])
if OUT.exists():
    print(f"PDF: {OUT} ({OUT.stat().st_size} bytes)")
