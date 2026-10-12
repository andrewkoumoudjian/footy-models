#!/usr/bin/env python3
"""Single entry point for universal EPL forecasting.

One command builds a league-universal pre-match bundle (all 20 registered
clubs, as-of baselines, pooled residual features), runs the coherent Rust
match engine with the fitted EPL+MLS shot head applied internally, validates
forecast coherence, and writes a versioned, hash-pinned run directory.

Usage:
    python3 scripts/forecast_matchday.py \\
        --schedule /tmp/opencode/espn_board_20261017.json ... \\
        --as-of 2026-10-12 --out-dir data/ml/matchday_20261017_19 \\
        --paths 2500 --seed 42

The schedule boards are frozen copies of ESPN scoreboard responses (fetch
with curl; see docs/UNIFIED_FORECAST_OOS_2026-10-12.md). Only scheduled,
non-completed fixtures strictly after --as-of are accepted. Refuses to
overwrite an existing run directory.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "apps/matchforecast/math_engine"))
from build_universal_bundle import build_bundle  # noqa: E402
from run_pipeline import validate  # noqa: E402

MODEL = ROOT / "target/release/footy-matchforecast-math"


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--schedule", type=Path, nargs="+", required=True)
    ap.add_argument("--as-of", required=True)
    ap.add_argument("--out-dir", type=Path, required=True)
    ap.add_argument("--squads", type=Path, default=ROOT / "data/pl/squads_2026-10-11_full.json")
    ap.add_argument("--espn", type=Path, nargs="+", default=[
        ROOT / "data/ml/math_engine/espn_frozen_20261011.json",
        ROOT / "data/ml/math_engine/espn_extension_20261012.json"])
    ap.add_argument("--crosswalk", type=Path, default=
        ROOT / "data/ml/math_engine/espn_pipeline_final_oct12/crosswalk.json")
    ap.add_argument("--history", type=Path, default=ROOT / "data/pl/asof_team_stats.csv")
    ap.add_argument("--results", type=Path, default=ROOT / "data/pl/results.tsv")
    ap.add_argument("--ledger", type=Path, default=
        ROOT / "data/identity/reep/epl_canonical_clean_v11_evidence.json")
    ap.add_argument("--heads", type=Path,
                    default=ROOT / "data/ml/unified_forecast_heads_v2.json")
    ap.add_argument("--fm-priors", type=Path,
                    default=ROOT / "data/ml/fm26_team_priors_v1.json")
    ap.add_argument("--fm-coeff", type=float, default=0.1)
    ap.add_argument("--paths", type=int, default=2500)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--build", action="store_true",
                    help="rebuild the Rust engine before simulating")
    a = ap.parse_args()
    out = a.out_dir.resolve()
    if out.exists():
        ap.error(f"refusing to overwrite an existing run: {out}")
    if a.paths < 1:
        ap.error("positive path count required")
    as_of = date.fromisoformat(a.as_of)
    for path in [*a.schedule, a.squads, a.history, a.results, a.heads,
                 a.fm_priors, a.ledger, *a.espn, a.crosswalk]:
        if not path.exists():
            ap.error(f"missing input {path}")
    out.mkdir(parents=True)
    frozen_schedule = []
    for board in a.schedule:
        dest = out / "schedule" / board.name
        dest.parent.mkdir(exist_ok=True)
        shutil.copyfile(board, dest)
        frozen_schedule.append(dest)

    from build_universal_bundle import fixtures_from_schedule
    squads_raw = json.loads(a.squads.read_text())
    fixtures = fixtures_from_schedule(frozen_schedule, squads_raw["teams"], as_of)
    bundle = build_bundle(squads_path=a.squads, fixtures=fixtures, as_of=as_of,
                          espn_paths=a.espn, crosswalk_path=a.crosswalk,
                          history_path=a.history, results_path=a.results,
                          ledger_path=a.ledger)
    inputs = [a.squads, a.history, a.results, *frozen_schedule, *a.espn,
              a.crosswalk, a.ledger]
    bundle["sources"] = {str(p): sha(p) for p in inputs}
    bundle_path = out / "bundle.json"
    bundle_path.write_text(json.dumps(bundle, sort_keys=True, separators=(",", ":"))
                           + "\n")

    if a.build:
        subprocess.run(["cargo", "build", "--release", "-p", "footy-matchforecast",
                        "--bin", "footy-matchforecast-math"],
                       cwd=ROOT, check=True)
    if not MODEL.exists():
        ap.error(f"engine binary missing; rerun with --build: {MODEL}")
    predictions_path = out / "predictions.json"
    sim = [str(MODEL), str(bundle_path), str(predictions_path),
           "--paths", str(a.paths), "--seed", str(a.seed),
           "--red-per90", "0", "--score-effect", "0", "--man-advantage", "0",
           "--fm-effect", "0", "--selection-marginals", "false",
           "--max-subs", "0", "--heads-artifact", str(a.heads),
           "--fm-priors", str(a.fm_priors), "--fm-coeff", str(a.fm_coeff)]
    subprocess.run(sim, cwd=ROOT, check=True, capture_output=True, text=True)
    validation = validate(bundle_path, predictions_path, paths=a.paths,
                          default_effects=True)
    manifest = {
        "schema": "universal_matchday_forecast_run_v1",
        "status": "research_only",
        "as_of": a.as_of,
        "bundle_sha256": sha(bundle_path),
        "predictions_sha256": sha(predictions_path),
        "binary_sha256": sha(MODEL),
        "heads_sha256": sha(a.heads),
        "fm_priors_sha256": sha(a.fm_priors),
        "fm_coefficient": a.fm_coeff,
        "simulation_arguments": sim[3:],
        "validation": validation,
        "fixtures": [(f["date"], f["home"], f["away"]) for f in fixtures],
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True)
                                       + "\n")
    report = json.loads(predictions_path.read_text())
    print(f"PASS: {validation}; run saved to {out}")
    for fx in report["fixtures"]:
        print(f"{fx['date']} {fx['home']} v {fx['away']}: "
              f"W {fx['home_win']:.3f} D {fx['draw']:.3f} A {fx['away_win']:.3f} | "
              f"xG {fx['expected_home_goals']:.2f}-{fx['expected_away_goals']:.2f}")


if __name__ == "__main__":
    main()
