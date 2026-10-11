#!/usr/bin/env python3
"""Render data/spike_refinement_report.md to a white-theme PDF."""
import markdown
import weasyprint

SRC = "/home/drewclaw/footy-models/data/spike_refinement_report.md"
OUT = "/tmp/footy_spike_refinement_report.pdf"

md = open(SRC).read()
html_body = markdown.markdown(md, extensions=["tables", "fenced_code"])
html = """<!DOCTYPE html><html><head><meta charset="utf-8"><style>
body { background:#ffffff; color:#1a1a1a; font-family:-apple-system,'Segoe UI',Roboto,sans-serif; padding:28px 36px; }
h1 { color:#111111; font-size:22px; border-bottom:2px solid #111; padding-bottom:6px; }
h2 { color:#111111; font-size:16px; margin-top:22px; }
h3 { color:#333333; font-size:13px; }
table { border-collapse:collapse; width:100%; font-size:9px; margin:8px 0; }
th,td { border:1px solid #cccccc; padding:3px 5px; text-align:left; }
th { background:#f2f2f2; color:#111111; }
strong { color:#000000; } code { background:#f4f4f4; padding:1px 4px; border-radius:3px; }
hr { border:none; border-top:1px solid #dddddd; margin:14px 0; }
</style></head><body>""" + html_body + "</body></html>"
weasyprint.HTML(string=html).write_pdf(OUT)
print("PDF ok", OUT)
