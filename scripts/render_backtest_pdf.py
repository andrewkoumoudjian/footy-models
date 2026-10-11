#!/usr/bin/env python3
"""Render data/backtest_report.md to a dark-theme PDF."""
import markdown
import weasyprint

SRC = "/home/drewclaw/footy-models/data/backtest_report.md"
OUT = "/tmp/footy_backtest_report.pdf"

md = open(SRC).read()
html_body = markdown.markdown(md, extensions=["tables", "fenced_code"])
html = """<!DOCTYPE html><html><head><meta charset="utf-8"><style>
body { background:#0d1117; color:#e6edf3; font-family:-apple-system,'Segoe UI',Roboto,sans-serif; padding:28px 36px; }
h1 { color:#00d4aa; font-size:22px; } h2 { color:#00d4aa; font-size:16px; margin-top:22px; }
table { border-collapse:collapse; width:100%; font-size:9px; margin:8px 0; }
th,td { border:1px solid #30363d; padding:3px 5px; text-align:left; }
th { background:#1c2128; color:#00d4aa; }
strong { color:#f0f6fc; } code { background:#161b22; padding:1px 4px; border-radius:3px; }
</style></head><body>""" + html_body + "</body></html>"
weasyprint.HTML(string=html).write_pdf(OUT)
print("PDF ok", OUT)
