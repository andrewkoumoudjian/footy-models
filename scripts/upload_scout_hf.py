#!/usr/bin/env python3
"""Create public HF repo akks88/footy-models-demo and upload scout.parquet."""
import json
from huggingface_hub import HfApi, login
from pathlib import Path

TOKEN = Path.home().joinpath(".cache/huggingface/token").read_text().strip()
login(token=TOKEN)

api = HfApi()
repo = "akks88/footy-models-demo"
try:
    api.create_repo(repo_id=repo, repo_type="dataset", private=False, exist_ok=True)
    print("repo ready:", repo)
except Exception as e:
    print("create_repo:", e)

base = Path("/home/drewclaw/footy-models/data/demo")
api.upload_file(
    path_or_fileobj=str(base / "scout.parquet"),
    path_in_repo="scout.parquet",
    repo_id=repo,
    repo_type="dataset",
)
api.upload_file(
    path_or_fileobj=str(base / "scout_meta.json"),
    path_in_repo="scout_meta.json",
    repo_id=repo,
    repo_type="dataset",
)
print("uploaded scout.parquet + scout_meta.json")

readme = """---
license: other
---
# footy-models scout demo

FM26 (Football Manager 2026) player universe (563k) with Transfermarkt overlay and
footy-models v2/v3 scores. See scout_meta.json for model versions.
- `scout.parquet` (zstd): 563,382 rows x 31 cols
- `scout_meta.json`: model metadata, as-of dates, MC sigma

Built 2026-08-10. FM data is proprietary (Sports Interactive); demo subset only.
"""
api.upload_file(
    path_or_fileobj=readme.encode(),
    path_in_repo="README.md",
    repo_id=repo,
    repo_type="dataset",
)
print("uploaded README.md")
