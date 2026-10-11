#!/usr/bin/env python3
"""Print the CFM scout report page to PDF via headless Chromium."""
import subprocess, sys, pathlib

out = pathlib.Path("/home/drewclaw/footy-models/artifacts/cfm_report/CFM_Scout_Report_2026-08-10.pdf")
out.parent.mkdir(parents=True, exist_ok=True)

r = subprocess.run(
    [
        "/usr/bin/chromium",
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--no-pdf-header-footer",
        f"--print-to-pdf={out}",
        "http://localhost:4173/#cfm-report",
    ],
    capture_output=True,
    text=True,
    timeout=120,
)
print(r.stderr.strip()[-300:] if r.stderr.strip() else "chromium ok")
print(f"exit={r.returncode}")
if out.exists():
    print(f"PDF: {out} ({out.stat().st_size} bytes)")
