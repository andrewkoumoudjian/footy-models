#!/usr/bin/env python3
"""Download and verify the current immutable Reep DuckDB identity register.

Stores the original CC0 snapshot locally and never commits the large database.
Uses the publisher's release manifest and SHA-256; a partial download cannot
become the active register. Re-run to skip a verified existing release.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import urllib.request
from pathlib import Path

LATEST = "https://data.reep.football/releases/latest.json"
DUCKDB_KEY = "duckdb/reep-register-v1.duckdb"
HEADERS = {"User-Agent": "footy-models-reep-register-sync/1.0"}


def request(url: str, **headers: str):
    return urllib.request.urlopen(
        urllib.request.Request(url, headers={**HEADERS, **headers}), timeout=90
    )


def load_json(url: str) -> tuple[dict, bytes]:
    with request(url) as response:
        raw = response.read()
    return json.loads(raw), raw


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(4 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def sync(output: Path) -> dict:
    latest, _ = load_json(LATEST)
    stamp = latest["stamp"]
    if not isinstance(stamp, str) or not stamp or not stamp.replace("T", "").replace("Z", "").isdigit():
        raise ValueError("unexpected Reep release stamp")
    manifest, manifest_bytes = load_json(latest["manifest_url"])
    if hashlib.sha256(manifest_bytes).hexdigest() != latest["manifest_sha256"]:
        raise ValueError("Reep release manifest checksum mismatch")
    if manifest["stamp"] != stamp:
        raise ValueError("release pointer / manifest stamp mismatch")
    record = manifest["files"][DUCKDB_KEY]
    size = int(record["bytes"])
    sha = record["sha256"]
    url = record["url"]
    if not url.startswith(f"https://data.reep.football/releases/{stamp}/"):
        raise ValueError("unexpected Reep asset host or version")
    output.mkdir(parents=True, exist_ok=True)
    target = output / f"reep-{stamp}.duckdb"
    part = target.with_suffix(".duckdb.part")
    if target.is_file():
        if target.stat().st_size == size and digest(target) == sha:
            print(f"Verified existing {target} ({size:,} bytes)", flush=True)
        else:
            raise ValueError(f"existing snapshot invalid, refuse overwrite: {target}")
    else:
        existing = part.stat().st_size if part.exists() else 0
        if existing >= size:
            if existing > size:
                raise ValueError(f"partial file exceeds release length: {existing} > {size}")
        else:
            headers = {"Range": f"bytes={existing}-"} if existing else {}
            with request(url, **headers) as response:
                if existing and response.status != 206:
                    raise ValueError("server did not acknowledge requested resume range")
                if existing and not response.headers.get("Content-Range", "").startswith(f"bytes {existing}-"):
                    raise ValueError("server returned unexpected resume range")
                if not existing and response.status != 200:
                    raise ValueError(f"unexpected download HTTP status: {response.status}")
                with part.open("ab" if existing else "wb") as handle:
                    while True:
                        chunk = response.read(4 << 20)
                        if not chunk:
                            break
                        handle.write(chunk)
                    handle.flush()
                    os.fsync(handle.fileno())
        if part.stat().st_size != size:
            raise ValueError(f"incomplete download: {part.stat().st_size:,}/{size:,} bytes; retry to resume")
        actual = digest(part)
        if actual != sha:
            raise ValueError(f"download checksum mismatch: {actual} != {sha}; partial retained for investigation")
        os.replace(part, target)
        print(f"Downloaded and verified {target} ({size:,} bytes)", flush=True)
    state = {"source": LATEST, "stamp": stamp, "path": str(target.resolve()),
             "bytes": size, "sha256": sha, "manifest_sha256": latest["manifest_sha256"],
             "licence": manifest.get("licence"), "duckdb_asset_url": url}
    (output / "active.json").write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")
    return state


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("data/identity/reep"))
    options = parser.parse_args()
    try:
        print(json.dumps(sync(options.output), indent=2))
    except (OSError, ValueError, KeyError, urllib.error.URLError) as error:
        print(f"Reep synchronization failed: {error}", file=sys.stderr)
        sys.exit(1)
