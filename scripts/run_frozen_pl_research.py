#!/usr/bin/env python3
"""Freeze an EPL research experiment's exact executable, inputs and outcomes.

This deliberately bypasses the public release pipeline: an experiment that
reuses the October 2026 pre-match inputs *after* the actual result is known
can be used for mechanism diagnostics, never unbiased model selection.

Usage:
    python3 scripts/run_frozen_pl_research.py --output-dir data/ml/liv-base \
      --only Liverpool --paths 8 --repeat 2
    python3 scripts/run_frozen_pl_research.py --verify data/ml/liv-base
"""
import argparse
import hashlib
import json
import math
import shutil
import subprocess
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target/release/footy-matchforecast-pl"
DEFAULT_SOURCES = {
    "results": ROOT / "data/pl/asof_team_stats.csv",
    "players": ROOT / "data/pl/player_fm_join.csv",
    "baseline": ROOT / "data/pl/forecasts.json",
}


def sha256(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def strict_date(value):
    d = date.fromisoformat(value)
    if d.isoformat() != value:
        raise ValueError("date must be YYYY-MM-DD")
    return d


def preflight(*, binary, inputs, as_of, snapshot_as_of, only, paths, seed, repeat):
    if not binary.is_file() or not binary.stat().st_size:
        raise ValueError(f"missing or empty executable: {binary}")
    if any(not f.is_file() or f.stat().st_size <= 0 for f in inputs.values()):
        raise ValueError("all three dated inputs must exist and be nonempty")
    if not (strict_date(snapshot_as_of) < strict_date(as_of)):
        raise ValueError("snapshot cutoff must precede simulated forecast date")
    if not (1 <= paths <= 10_000 and 0 <= seed < 2**64 and repeat in (1, 2)):
        raise ValueError("invalid path/seed/repeat limits")
    baseline = json.loads(inputs["baseline"].read_text())
    eligible = [
        f for f in baseline.get("fixtures", [])
        if only is None or only in f["home"] or only in f["away"]
    ]
    if not eligible:
        raise ValueError("no matching fixtures")
    if any(not strict_date(snapshot_as_of) < strict_date(f["date"]) for f in eligible):
        raise ValueError("a fixture occurs at or before input cutoff")
    return [(f["date"], f["home"], f["away"]) for f in eligible]


def checksums(paths):
    return {name: sha256(path) for name, path in paths.items()}


def verify_archive(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest["contract"] != "frozen_exact_binary_pre_match_research_v1":
        raise ValueError("unrecognized manifest")
    paths = {
        name: directory / relpath
        for name, relpath in manifest["archived_files"].items()
    }
    for name, path in paths.items():
        if not path.is_file():
            raise ValueError(f"missing archive item: {name}")
        if sha256(path) != manifest["sha256"][name]:
            raise ValueError(f"archive hash mismatch: {name}")
    if any(not path.is_file() for path in paths.values()):
        raise ValueError("incomplete archive")
    forecast = json.loads(paths["forecast_1"].read_text())
    if forecast.get("as_of") != manifest["cutoff"]:
        raise ValueError("forecast time cutoff differs from manifest")
    if manifest["repeat"] == 2 and (
        manifest["sha256"]["forecast_1"] != manifest["sha256"]["forecast_2"]
    ):
        raise ValueError("repeated identical command did not produce same bytes")
    if len(forecast.get("fixtures", [])) != len(manifest["fixtures"]):
        raise ValueError("wrong fixture count")
    for row, expected in zip(forecast["fixtures"], manifest["fixtures"]):
        if (row["date"], row["home"], row["away"]) != tuple(expected):
            raise ValueError("fixture identity changed")
        if int(row["paths"]) != manifest["paths"]:
            raise ValueError("path count mismatch")
    return manifest


def create_experiment(args):
    if args.output_dir.exists():
        raise FileExistsError(f"refusing to modify existing run: {args.output_dir}")
    inputs = {
        key: Path(value).resolve()
        for key, value in {
            "results": args.results, "players": args.players, "baseline": args.baseline,
        }.items()
    }
    binary = args.binary.resolve()
    fixtures = preflight(
        binary=binary, inputs=inputs, as_of=args.as_of,
        snapshot_as_of=args.snapshot_as_of, only=args.only, paths=args.paths,
        seed=args.seed, repeat=args.repeat,
    )
    if args.no_tactical_subs and args.red_probability is None:
        variant = "no_tactical_subs"
    elif args.no_tactical_subs and args.red_probability is not None:
        variant = "no_tactical_subs_and_red_prior"
    elif args.red_probability is not None:
        variant = "red_prior_only"
    else:
        variant = "default"
    args.output_dir.mkdir(parents=True)
    archive = {"binary": Path("bin") / binary.name}
    archive.update({name: Path("inputs") / f"{name}{path.suffix}" for name, path in inputs.items()})
    for name, source in {"binary": binary, **inputs}.items():
        destination = args.output_dir / archive[name]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    command = [
        str(args.output_dir / archive["binary"]),
        "--paths", str(args.paths),
        "--seed", str(args.seed),
        "--as-of", args.as_of,
        "--snapshot-as-of", args.snapshot_as_of,
        "--results", str(args.output_dir / archive["results"]),
        "--players", str(args.output_dir / archive["players"]),
        "--baseline", str(args.output_dir / archive["baseline"]),
    ]
    if args.only is not None:
        command += ["--only", args.only]
    if args.no_tactical_subs:
        command.append("--research-no-tactical-subs")
    if args.red_probability is not None:
        command += ["--research-red-probability-per90", str(args.red_probability)]
    for attempt in range(1, args.repeat + 1):
        output = Path(f"forecast_{attempt}.json")
        archive[f"forecast_{attempt}"] = output
        request = command + ["--output", str(args.output_dir / output)]
        result = subprocess.run(request, cwd=ROOT, text=True, capture_output=True, check=True)
        (args.output_dir / f"stdout_{attempt}.txt").write_text(result.stdout)
        if result.stderr:
            (args.output_dir / f"stderr_{attempt}.txt").write_text(result.stderr)
    input_hashes = checksums({k: args.output_dir / rel for k, rel in archive.items()})
    from run_pl_pipeline import model_code_fingerprint
    git_rev = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT,
        text=True, capture_output=True, check=True,
    ).stdout.strip()
    git_dirty = bool(subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=no"], cwd=ROOT,
        text=True, capture_output=True, check=True,
    ).stdout)
    manifest = {
        "contract": "frozen_exact_binary_pre_match_research_v1",
        "purpose": "post-event mechanism diagnostics; never clean out-of-sample scoring",
        "variant": variant,
        "git_revision": git_rev,
        "working_tree_tracked_dirty_at_run": git_dirty,
        "code_fingerprint_at_run_unverified_against_binary": model_code_fingerprint(),
        "cutoff": args.snapshot_as_of,
        "forecast_date": args.as_of,
        "fixtures": fixtures,
        "paths": args.paths,
        "seed": args.seed,
        "repeat": args.repeat,
        "research_flags": {
            "no_tactical_subs": args.no_tactical_subs,
            "red_probability_per90": args.red_probability,
        },
        "archived_files": {key: str(rel) for key, rel in archive.items()},
        "sha256": input_hashes,
        "limitations": [
            "Input snapshots are frozen copies; binary hash identifies exact executable bytes.",
            "Code fingerprint recorded at run time does not prove binary-source correspondence.",
            "Repeats test fixed executable, inputs and seed. Different variants consume RNG differently.",
            "Simulation after the actual result is known cannot validate future forecasting accuracy.",
        ],
    }
    (args.output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    verify_archive(args.output_dir)
    return manifest


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--verify", type=Path, help="Check a frozen existing experiment without running it")
    p.add_argument("--output-dir", type=Path)
    p.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    for name, default in DEFAULT_SOURCES.items():
        p.add_argument(f"--{name}", type=Path, default=default)
    p.add_argument("--as-of", default="2026-10-11")
    p.add_argument("--snapshot-as-of", default="2026-10-10")
    p.add_argument("--paths", type=int, default=8)
    p.add_argument("--seed", type=int, default=42)
    p.add_argument("--only", default="Liverpool")
    p.add_argument("--repeat", type=int, choices=(1, 2), default=2)
    p.add_argument("--no-tactical-subs", action="store_true")
    p.add_argument("--red-probability", type=float)
    args = p.parse_args()
    if args.verify:
        if args.output_dir is not None:
            p.error("choose --verify or --output-dir, not both")
        manifest = verify_archive(args.verify)
    else:
        if args.output_dir is None:
            p.error("supply --output-dir")
        if args.red_probability is not None and not (
            math.isfinite(args.red_probability)
            and 0 <= args.red_probability <= 0.2
        ):
            p.error("--red-probability must be finite and in [0,0.2]")
        manifest = create_experiment(args)
    print(json.dumps({
        key: manifest[key]
        for key in ("variant", "fixtures", "paths", "repeat", "sha256")
    }, indent=2))


if __name__ == "__main__":
    main()
