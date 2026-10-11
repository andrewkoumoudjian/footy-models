#!/usr/bin/env python3
"""Desaturate report chart copies to pure greyscale (site originals untouched)."""
from PIL import Image, ImageOps
import pathlib

for sub in ["cfm/charts", "cfm/charts_base"]:
    d = pathlib.Path("/home/drewclaw/footy-models/artifacts/cfm_report") / sub
    n = 0
    for f in sorted(d.glob("*.png")):
        im = Image.open(f).convert("RGB")
        g = ImageOps.grayscale(im).convert("RGB")  # true grey
        g.save(f)
        n += 1
    print(sub, "->", n, "charts greyscaled")
