"""Regenerate causal MLS artifacts and run a fixed match/player forecast ablation matrix.

The matrix separates player-rate learning, participation learning, the legacy team-xG residual,
and walk-forward result calibration. Every configuration uses the same fixtures, seed, Monte Carlo
path count, and prepared strictly-as-of inputs. Raw stdout/stderr plus a compact JSON summary are
persisted so model changes can be compared without hand-copying console output.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / "data" / "sofascore_cfm" / "ablation"


CONFIGS = {
    "legacy": [
        "--no-player-learning",
        "--no-participation-learning",
        "--legacy-team-attack-residual",
        "--no-result-calibration",
    ],
    "rate_learning": [
        "--no-participation-learning",
        "--legacy-team-attack-residual",
        "--no-result-calibration",
    ],
    "participation_learning": [
        "--no-player-learning",
        "--legacy-team-attack-residual",
        "--no-result-calibration",
    ],
    "learned_heads_legacy_residual": [
        "--legacy-team-attack-residual",
        "--no-result-calibration",
    ],
    "structural": ["--no-result-calibration"],
    "structural_calibrated": [],
}


FLOAT = r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"


def run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(command), flush=True)
    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=capture,
    )


def scalar(text: str, label: str) -> float | None:
    match = re.search(rf"^{re.escape(label)}\s*:\s*({FLOAT})", text, flags=re.MULTILINE)
    return float(match.group(1)) if match else None


def parse_summary(text: str) -> dict[str, object]:
    result: dict[str, object] = {
        "mean_brier": scalar(text, "mean Brier score"),
        "mean_log_loss": scalar(text, "mean log loss"),
        "raw_brier": scalar(text, "raw Brier score"),
        "raw_log_loss": scalar(text, "raw log loss"),
    }
    accuracy = re.search(r"top-pick accuracy\s*:\s*(\d+)/(\d+)\s*=\s*(%s)%%" % FLOAT, text)
    if accuracy:
        result["top_pick_correct"] = int(accuracy.group(1))
        result["fixtures"] = int(accuracy.group(2))
        result["top_pick_accuracy"] = float(accuracy.group(3)) / 100.0

    goal = re.search(
        rf"goal forecast\s*:\s*n=(\d+)\s+MAE=({FLOAT})\s+RMSE=({FLOAT})\s+bias=({FLOAT})\s+P10-P90 coverage=({FLOAT})%\s+width=({FLOAT})",
        text,
    )
    if goal:
        result["goals"] = {
            "n": int(goal.group(1)),
            "mae": float(goal.group(2)),
            "rmse": float(goal.group(3)),
            "bias": float(goal.group(4)),
            "p10_p90_coverage": float(goal.group(5)) / 100.0,
            "mean_interval_width": float(goal.group(6)),
        }

    player_metrics: dict[str, object] = {}
    for label in ("shots", "xG", "xA", "passes", "progressive carries"):
        match = re.search(
            rf"^{re.escape(label)}\s*:\s*n=(\d+)\s+MAE=({FLOAT})\s+RMSE=({FLOAT})\s+bias=({FLOAT})\s+P10-P90=({FLOAT})%\s+width=({FLOAT})",
            text,
            flags=re.MULTILINE,
        )
        if match:
            player_metrics[label] = {
                "n": int(match.group(1)),
                "mae": float(match.group(2)),
                "rmse": float(match.group(3)),
                "bias": float(match.group(4)),
                "p10_p90_coverage": float(match.group(5)) / 100.0,
                "mean_interval_width": float(match.group(6)),
            }
    result["player_metrics"] = player_metrics

    minutes = re.search(
        rf"expected minutes\s*:\s*n=(\d+)\s+MAE=({FLOAT})\s+RMSE=({FLOAT})\s+bias=({FLOAT})",
        text,
    )
    if minutes:
        result["minutes"] = {
            "n": int(minutes.group(1)),
            "mae": float(minutes.group(2)),
            "rmse": float(minutes.group(3)),
            "bias": float(minutes.group(4)),
        }
    for key, label in (
        ("start_brier", "start probability"),
        ("appearance_brier", "appearance probability"),
    ):
        match = re.search(rf"^{re.escape(label)}\s*:\s*n=(\d+)\s+Brier=({FLOAT})", text, flags=re.MULTILINE)
        if match:
            result[key] = {"n": int(match.group(1)), "brier": float(match.group(2))}
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--paths", type=int, default=1000)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--fixtures", type=int, default=12)
    parser.add_argument("--skip-prep", action="store_true")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    if not args.skip_prep:
        run(["python3", "scripts/build_asof_table.py"])
        run(["python3", "scripts/prep_backtest.py", str(args.fixtures)])
        run(["python3", "scripts/build_form_series.py"])
        run(["python3", "scripts/build_selection_training.py"])

    # Fail before expensive Monte Carlo if source changes do not compile.
    run(["cargo", "check", "-p", "footy-forecast", "-p", "footy-match", "-p", "footy-matchforecast"])

    summaries: dict[str, object] = {}
    for name, flags in CONFIGS.items():
        command = [
            "cargo", "run", "--release", "-q", "-p", "footy-matchforecast",
            "--bin", "footy-matchforecast-backtest", "--",
            "--paths", str(args.paths), "--seed", str(args.seed), *flags,
        ]
        completed = run(command, capture=True)
        stdout_path = output_dir / f"{name}.stdout.txt"
        stderr_path = output_dir / f"{name}.stderr.txt"
        stdout_path.write_text(completed.stdout)
        stderr_path.write_text(completed.stderr)
        summaries[name] = parse_summary(completed.stdout)
        print(completed.stdout, end="")

    payload = {
        "paths": args.paths,
        "seed": args.seed,
        "fixtures_requested": args.fixtures,
        "configs": summaries,
    }
    summary_path = output_dir / "summary.json"
    summary_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    print(f"wrote {summary_path}")


if __name__ == "__main__":
    main()
