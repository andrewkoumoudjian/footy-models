"""Optional Modal CPU runner for the historical Rust match simulation.

Install the Modal Python SDK outside the Rust workspace and run:
  modal run scripts/modal_pl_hist.py --paths 200 --seed 42 --exact-cardinality

The job ships only the public StatsBomb as-of feature cache and the already-
built release executable. Raw licensed FM exports are never uploaded. This is
an opt-in remote execution adapter, not part of any production runtime.
"""

from __future__ import annotations

import gzip
import hashlib
import json
from pathlib import Path

import modal

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target/release/footy-matchforecast-pl-hist"
INPUT_DIR = ROOT / "data/statsbomb/pl_hist"


def feature_fingerprint(directory: Path) -> str:
    """Digest the exact as-of input files read by the Rust historical driver."""
    paths = [directory / "fixtures.json", directory / "team_rows.csv"]
    paths += sorted((directory / "players").glob("*.csv"))
    digest = hashlib.sha256()
    for path in paths:
        digest.update(path.relative_to(directory).as_posix().encode())
        digest.update(b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()

# Ubuntu 24.04's glibc supports binaries built on a matching Linux host.
# The SHA256 check below guards against a silently different remote binary.
image = (
    modal.Image.from_registry("ubuntu:24.04", add_python="3.12")
    .workdir("/app")
    .add_local_file(BIN, "/app/footy-matchforecast-pl-hist")
    .add_local_dir(INPUT_DIR, "/app/data/statsbomb/pl_hist")
)
app = modal.App("footy-historical-rust-validation", image=image)


@app.function(cpu=8, memory=8192, timeout=7200)
def evaluate(
    paths: int,
    seed: int,
    exact_cardinality: bool,
    expected_binary_sha256: str,
    expected_features_sha256: str,
    smoke: bool,
) -> bytes:
    import subprocess

    executable = Path("/app/footy-matchforecast-pl-hist")
    actual_binary_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest()
    if actual_binary_sha256 != expected_binary_sha256:
        raise ValueError("Modal executable differs from the audited local Rust binary")
    if feature_fingerprint(Path("/app/data/statsbomb/pl_hist")) != expected_features_sha256:
        raise ValueError("Modal historical feature files differ from the audited local inputs")
    args = [
        str(executable), "--paths", str(paths), "--seed", str(seed),
        "--jobs", "8", "--output", "/tmp/footy-historical.json",
    ]
    if exact_cardinality:
        args.append("--exact-cardinality-lineup")
    if smoke:
        fixtures = Path("/app/data/statsbomb/pl_hist/fixtures.json")
        document = json.loads(fixtures.read_text())
        fixture = next((item for item in document["fixtures"] if item.get("evaluable")), None)
        if fixture is None:
            raise ValueError("historical cache has no evaluable fixture")
        subset = Path("/tmp/footy-smoke-fixture.json")
        subset.write_text(json.dumps({"fixtures": [fixture]}))
        args += ["--fixtures", str(subset)]
    subprocess.run(args, cwd="/app", check=True, capture_output=True, text=True, timeout=7000)
    payload = Path("/tmp/footy-historical.json").read_bytes()
    result = json.loads(payload)
    if result.get("paths") != paths or result.get("seed") != seed:
        raise ValueError("remote output seed/path count does not match requested job")
    if bool(result["input_contract"].get("exact_cardinality_lineup")) != exact_cardinality:
        raise ValueError("remote output does not match requested sampler")
    return gzip.compress(payload, compresslevel=6)


@app.local_entrypoint()
def main(
    paths: int = 200,
    seed: int = 42,
    exact_cardinality: bool = False,
    smoke: bool = False,
    output: str = "data/ml/modal-pl-historical.json",
) -> None:
    if paths < 1 or paths > 20000:
        raise ValueError("paths must lie in [1, 20000]")
    if not BIN.is_file() or not (INPUT_DIR / "fixtures.json").is_file():
        raise FileNotFoundError("build the local release binary and historical cache first")
    destination = ROOT / output
    if destination.exists():
        raise FileExistsError(f"refusing to overwrite the archived result: {destination}")
    binary_sha256 = hashlib.sha256(BIN.read_bytes()).hexdigest()
    features_sha256 = feature_fingerprint(INPUT_DIR)
    compressed = evaluate.remote(paths, seed, exact_cardinality, binary_sha256, features_sha256, smoke)
    raw = gzip.decompress(compressed)
    # Only claim completion once the complete JSON output is recovered locally.
    result = json.loads(raw)
    if len(result["fixtures"]) < 1:
        raise ValueError("remote run did not return historical fixtures")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(raw)
    print(f"saved {destination}; {len(result['fixtures'])} fixtures; binary SHA256 {binary_sha256}; input SHA256 {features_sha256}")
