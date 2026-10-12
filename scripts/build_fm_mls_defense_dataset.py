#!/usr/bin/env python3
"""Build as-of, FM26-linked *observed* defensive-event labels (MLS SofaScore).

Mirrors scripts/build_fm_mls_shot_dataset.py exactly (same manifests, same
complete-everyone FM index, same usability gate, same causal histories and
chronological splits) for two defensive targets: totalTackle and
ballRecovery. A stat key absent from a match file is MISSING (row skipped
for that head), never zero. Output is research-only.
"""
import argparse
import hashlib
import json
import math
import unicodedata
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

ROLES = ("G", "D", "M", "F")
# Fixed a priori domain priors (tackles/recoveries per 90 by role), disclosed
# here rather than estimated on the evaluation window. Train means and test
# mean-calibration ratios are reported alongside; strength-5 shrinkage makes
# exact values second-order, and validation selects against the baseline.
TACKLE_ROLE_PRIORS = {"G": 0.10, "D": 2.00, "M": 2.20, "F": 0.80}
RECOVERY_ROLE_PRIORS = {"G": 2.00, "D": 6.50, "M": 7.50, "F": 4.00}
TARGETS = ("tackles", "recoveries")
STAT_KEY = {"tackles": "totalTackle", "recoveries": "ballRecovery"}


def normalize(name):
    return " ".join(
        "".join(c for c in unicodedata.normalize("NFKD", name or "")
                if not unicodedata.combining(c)).casefold().split()
    )


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def event_manifest(root):
    from build_fm_mls_shot_dataset import event_manifest as shared
    return shared(root)


def load_fm_index(path):
    from build_fm_mls_shot_dataset import load_fm_index as shared
    return shared(path)


def fm_row_usable(matches):
    from build_fm_mls_shot_dataset import fm_row_usable as shared
    return shared(matches)


def make_dataset(fm_file, root, valid_from, train_end, valid_end, test_end):
    events, sources = event_manifest(root)
    fm = load_fm_index(fm_file)
    counters = Counter()
    samples = []
    # Per-stat causal histories: observed prior events and minutes per player.
    history = defaultdict(lambda: {"tackles": 0, "recoveries": 0, "minutes": 0.0,
                                   "last_date": None})
    for match_id, kickoff in sorted(events.items(), key=lambda item: (item[1], item[0])):
        path = root / "player_matches" / f"{match_id}.json"
        if not path.is_file():
            counters["missing_player_match_file"] += 1
            continue
        event = json.loads(path.read_text())
        match_rows = []
        seen = set()
        day = datetime.fromtimestamp(kickoff, timezone.utc).date().isoformat()
        for side in ("home", "away"):
            for entry in event.get(side, {}).get("players", []):
                person = entry.get("player", {})
                player_id = person.get("id")
                stats = entry.get("statistics") or {}
                minutes = stats.get("minutesPlayed")
                if not isinstance(player_id, int) or not isinstance(minutes, (int, float)):
                    counters["no_minutes"] += 1
                    continue
                if player_id in seen:
                    raise ValueError(f"duplicate player {player_id} in match {match_id}")
                seen.add(player_id)
                if not math.isfinite(minutes) or not 0 < minutes <= 130:
                    counters["invalid_label"] += 1
                    continue
                role = (entry.get("position") or person.get("position") or "").upper()[:1]
                if role not in ROLES:
                    counters["unknown_role"] += 1
                    continue
                present = {}
                valid = True
                for target in TARGETS:
                    raw = stats.get(STAT_KEY[target])
                    if raw is None:
                        continue
                    if not isinstance(raw, int) or raw < 0:
                        valid = False
                        break
                    present[target] = raw
                if not valid:
                    counters["invalid_label"] += 1
                    continue
                t = person.get("dateOfBirthTimestamp")
                dob = datetime.fromtimestamp(t, timezone.utc).date().isoformat() \
                    if isinstance(t, int) else ""
                match_rows.append((player_id, minutes, role,
                                   person.get("name") or "", dob, present))
        for player_id, minutes, role, name, dob, present in match_rows:
            if not (valid_from <= day < test_end):
                continue
            candidates = fm.get((normalize(name), dob), [])
            usable, rescued = fm_row_usable(candidates)
            if usable is None:
                counters["fm_unverified_masked_but_kept"] += 1
                usable, rescued = None, False
            elif rescued:
                counters["staff_kind_db_rescued"] += 1
            prior_minutes = history[player_id]["minutes"]
            last = history[player_id]["last_date"]
            last_seen = (math.log1p(120) / 5.0 if last is None
                         else math.log1p(max(0, (datetime.fromisoformat(day).date()
                                                 - last).days)) / 5.0)
            labels = {}
            for target, priors in (("tackles", TACKLE_ROLE_PRIORS),
                                   ("recoveries", RECOVERY_ROLE_PRIORS)):
                if target not in present:
                    continue
                prior = ((history[player_id][target] + 5.0 * priors[role])
                         / (prior_minutes / 90.0 + 5.0))
                labels[target] = {"events": present[target], "prior_rate": prior}
            if not labels:
                counters["no_defensive_stat_recorded"] += 1
                continue
            split = ("train" if day < train_end else "validation"
                     if day < valid_end else "test")
            samples.append({
                "split": split, "date": day, "match_id": match_id,
                "player_id": player_id, "fm_uid": usable["uid"] if usable else None,
                "role": role, "minutes": float(minutes),
                "prior_minutes": prior_minutes,
                "last_seen_log_days": last_seen,
                "fm_verified": usable is not None,
                "rescued_staff": bool(rescued),
                "labels": labels,
            })
            counters[f"{split}_joined"] += 1
        for player_id, minutes, _, _, _, present in match_rows:
            history[player_id]["minutes"] += minutes
            history[player_id]["last_date"] = datetime.fromisoformat(day).date()
            for target in TARGETS:
                if target in present:
                    history[player_id][target] += present[target]
        counters["matches_with_event_data"] += 1
    if not (valid_from < train_end < valid_end < test_end):
        raise ValueError("chronological split cutoffs must be strictly increasing")
    if not samples:
        raise ValueError("no verified post-vintage FM26 defensive labels")
    by_split = Counter(s["split"] for s in samples)
    manifest = {
        "contract": "fm26_db_observed_attribute_v26.2_mls_defense_asof_v1",
        "model_input": "FM26 on-disk attributes (attr_source=db), pre-kickoff role and player history",
        "targets": {t: f"observed SofaScore {STAT_KEY[t]} conditioned on actual played minutes"
                    for t in TARGETS},
        "label_caveat": "actual played minutes is an evaluation exposure, not a pre-match feature; "
                        "absent stat keys are missing rows, never zeros",
        "vintage": "FM26 v26.2 March 2026; assume conservatively available starting " + valid_from,
        "cutoffs": dict(valid_from=valid_from, train_end=train_end,
                        validation_end=valid_end, test_end=test_end),
        "role_priors": {"tackles": TACKLE_ROLE_PRIORS,
                        "recoveries": RECOVERY_ROLE_PRIORS},
        "split_counts": dict(by_split), "audit_counts": dict(counters),
    }
    return samples, manifest, sources


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fm-file", type=Path,
                        default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--sofa-root", type=Path, default=Path("data/sofascore_cfm"))
    parser.add_argument("--output", type=Path, default=Path("data/ml/fm26-mls-defense.jsonl"))
    parser.add_argument("--valid-from", default="2026-04-01")
    parser.add_argument("--train-end", default="2026-06-01")
    parser.add_argument("--validation-end", default="2026-08-01")
    parser.add_argument("--test-end", default="2026-09-01")
    args = parser.parse_args()
    samples, manifest, sources = make_dataset(
        args.fm_file, args.sofa_root, args.valid_from, args.train_end,
        args.validation_end, args.test_end,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as stream:
        for item in samples:
            stream.write(json.dumps(item, sort_keys=True, separators=(",", ":")) + "\n")
    manifest["dataset_sha256"] = sha256(args.output)
    manifest_path = args.output.with_suffix(".manifest.json")
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({
        "dataset": str(args.output), "manifest": str(manifest_path),
        "splits": manifest["split_counts"], "audit": manifest["audit_counts"],
    }, indent=2))


if __name__ == "__main__":
    main()
