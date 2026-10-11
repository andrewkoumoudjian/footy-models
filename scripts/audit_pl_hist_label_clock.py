#!/usr/bin/env python3
"""Fail-closed v1-v2 StatsBomb event-clock materialization audit.

Features must stay byte-identical; ONLY post-match event time labels
may change. Never feed post-match label details into the match driver.
"""
import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path

KINDS = ("goals", "assists", "reds", "subs")
EXTRA_TIME_FIELDS = (
    "source_elapsed_minute", "source_second", "source_period",
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare(legacy, corrected):
    a_manifest = json.loads((legacy / "fixtures.json").read_text())
    b_manifest = json.loads((corrected / "fixtures.json").read_text())
    if a_manifest["schema_version"] != 1 or b_manifest["schema_version"] != 2:
        raise ValueError("legacy v1 and corrected v2 fixture manifests required")
    if b_manifest.get("label_clock_contract") != "statsbomb_elapsed_v2":
        raise ValueError("corrected labels lack explicit elapsed clock contract")
    for key in ("source", "feature_contract", "no_lookahead_rule", "fixtures"):
        if a_manifest[key] != b_manifest[key]:
            raise ValueError(f"as-of fixture contract changed unexpectedly: {key}")
    feature_files = ["team_rows.csv"]
    feature_files.extend(
        "players/" + f"{row['match_id']}.csv"
        for row in a_manifest["fixtures"]
    )
    for file in feature_files:
        if not (legacy / file).is_file() or not (corrected / file).is_file():
            raise ValueError(f"missing as-of feature file: {file}")
        if digest(legacy / file) != digest(corrected / file):
            raise ValueError(f"training or input feature drift: {file}")
    count = Counter()
    bin_changes = Counter()
    affected_fixtures = set()
    for fixture in a_manifest["fixtures"]:
        mid = fixture["match_id"]
        file = f"labels/{mid}.json"
        a = json.loads((legacy / file).read_text())
        b = json.loads((corrected / file).read_text())
        if b.get("label_clock_contract") != "statsbomb_elapsed_v2":
            raise ValueError(f"missing corrected fixture clock: {mid}")
        for key in a:
            if key in KINDS:
                continue
            if a[key] != b.get(key):
                raise ValueError(f"actual labels changed apart from timestamps: {mid}:{key}")
        for kind in KINDS:
            if len(a[kind]) != len(b[kind]):
                raise ValueError(f"event count changed: {mid}:{kind}")
            for old, new in zip(a[kind], b[kind]):
                stripped = {k: v for k, v in new.items() if k not in EXTRA_TIME_FIELDS}
                legacy_fields = {k: v for k, v in old.items() if k != "minute_index"}
                new_fields = {k: v for k, v in stripped.items() if k != "minute_index"}
                if legacy_fields != new_fields:
                    raise ValueError(f"non-time event labels changed: {mid}:{kind}")
                minute = new.get("source_elapsed_minute")
                second = new.get("source_second")
                period = new.get("source_period")
                if (
                    not isinstance(minute, int) or minute < 0
                    or not isinstance(second, int) or not 0 <= second < 60
                    or not isinstance(period, int) or period < 1
                ):
                    raise ValueError(f"missing/invalid source event clock: {mid}:{kind}")
                if old["minute_index"] != min(89, max(0, minute - 1)):
                    raise ValueError(f"legacy minute contract inconsistent: {mid}:{kind}")
                if new["minute_index"] != min(89, minute):
                    raise ValueError(f"corrected source-clock contract inconsistent: {mid}:{kind}")
                count[f"{kind}_events"] += 1
                if old["minute_index"] != new["minute_index"]:
                    count[f"{kind}_updated_minutes"] += 1
                    affected_fixtures.add(mid)
                if old["minute_index"] // 15 != new["minute_index"] // 15:
                    bin_changes[kind] += 1
    return {
        "contract": "statsbomb_asof_features_fixed_elapsed_label_rebuild_v2",
        "source": "StatsBomb 2015/16 event-covered Premier League fixture archive",
        "legacy_manifest_sha256": digest(legacy / "fixtures.json"),
        "corrected_manifest_sha256": digest(corrected / "fixtures.json"),
        "input_features_byte_identical": True,
        "feature_files_verified": len(feature_files),
        "fixtures_verified": len(a_manifest["fixtures"]),
        "events": dict(count),
        "changed_15min_bins_by_kind": dict(bin_changes),
        "fixtures_with_corrected_event_minutes": len(affected_fixtures),
        "no_lookahead_feature_rule": b_manifest["no_lookahead_rule"],
        "caution": (
            "Only post-match event label timing changed; model inputs unchanged. "
            "Elapsed-clock v2 audits replace v1 for timing comparisons but do "
            "not reset an already-observed OOS selection window."
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy", type=Path, default=Path("data/statsbomb/pl_hist"))
    parser.add_argument(
        "--corrected", type=Path, default=Path("data/statsbomb/pl_hist_elapsed_v2"),
    )
    parser.add_argument(
        "--output", type=Path,
        default=Path("artifacts/validation/pl-hist-label-clock-v2-audit.json"),
    )
    args = parser.parse_args()
    report = compare(args.legacy, args.corrected)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
