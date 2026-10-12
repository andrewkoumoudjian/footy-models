#!/usr/bin/env python3
"""Build as-of, FM26-linked *observed* player-shot labels for the Candle experiment.

Inputs are local SofaScore match snapshots and local FM26 v26.2 (March 2026).
Never join FM26 features to historical 2015/16 outcomes. Output is research-only.
"""
import argparse
import csv
import hashlib
import json
import math
import unicodedata
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

ATTRS = (
    "Finishing", "OffTheBall", "Dribbling", "FirstTouch", "Technique", "Composure",
    "Anticipation", "Decisions", "Pace", "Acceleration", "Stamina", "WorkRate",
    "Passing", "Vision", "Positioning", "Aggression",
)
ROLES = ("G", "D", "M", "F")
# Fixed *a priori* role means, not estimated using the later held-out fixtures.
SHOT_ROLE_PRIORS = {"G": 0.02, "D": 0.40, "M": 0.90, "F": 2.20}
FEATURE_NAMES = (
    *(f"role_{role}" for role in ROLES),
    "age_years_norm", "fm_ca_norm",
    *(f"fm_{attr}_norm" for attr in ATTRS),
    *(f"fm_{attr}_missing" for attr in ATTRS),
    "log_prior_rate",
    "log_prior_exposure",
)


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


def _union_complete_path():
    try:
        from fm26_complete import COMPLETE
    except ModuleNotFoundError:
        from scripts.fm26_complete import COMPLETE
    return COMPLETE


def event_manifest(root):
    events = {}
    sources = [
        root / "history_manifest.json",
        root / "charlotte_history_manifest.json",
        root / "opponent_history_manifest.json",
    ]
    for file in sources:
        raw = json.loads(file.read_text())
        batches = [raw] if isinstance(raw, list) else (
            [raw["events"]] if "events" in raw else list(raw.values())
        )
        for batch in batches:
            for event in batch:
                match_id = event.get("id")
                kickoff = event.get("startTimestamp")
                if not isinstance(match_id, int) or not isinstance(kickoff, int):
                    continue
                prior = events.get(match_id)
                if prior is not None and prior != kickoff:
                    raise ValueError(f"inconsistent kickoff for match {match_id}")
                events[match_id] = kickoff
    return events, sources


def load_fm_index(path, complete_path=None):
    """Complete-everyone (name, DOB) index with NO kind pre-filter.

    Every named row in either frame is indexed, including staff-kind rows:
    several real players are miscategorized as staff in the extracts. Kind
    and attr_source are decided per-row AFTER lookup (player/db is observed;
    staff/db with a valid block is rescued, flagged, and usable; derived or
    unknown-provenance rows are never observed traits). Uniqueness of the
    (name, DOB) key is still required — ambiguous keys stay unverified.
    """
    try:
        from fm26_complete import COMPLETE as _DEFAULT_COMPLETE, load_union as _load_union
    except ModuleNotFoundError:
        from scripts.fm26_complete import COMPLETE as _DEFAULT_COMPLETE, load_union as _load_union
    primary = Path(complete_path) if complete_path else _DEFAULT_COMPLETE
    union, _ = _load_union(primary, Path(path))
    index = defaultdict(list)
    for uid, record in union.items():
        name = normalize(record.get("name", ""))
        dob = record.get("dob", "")
        if not name or not dob:
            continue
        index[(name, dob)].append({
            **{f"attr_{field}": ("" if value is None else str(value))
               for field, value in (record.get("attributes") or {}).items()},
            # Explicit provenance keys come last so no trait-name collision
            # (e.g. a provenance column starting with `attr_`) can overwrite.
            "uid": uid, "name": record.get("name", ""), "dob": dob,
            "kind": record.get("kind", ""), "attr_source": record.get("attr_source", ""),
            "ca": ("" if record.get("ca") is None else str(record["ca"])),
            "dob_provenance": record.get("attr_source_provenance", ""),
        })
    return index


def fm_row_usable(matches):
    """Unique (name, DOB) linkage; staff-with-db-block rescued, rest filtered.

    Returns (row, rescued_staff) or (None, False). Derived, unknown
    provenance, and ambiguous keys are never observed FM traits.
    """
    if len(matches) != 1:
        return None, False
    row = matches[0]
    if row.get("attr_source") == "db" and row.get("kind") == "player":
        return row, False
    if row.get("attr_source") == "db" and row.get("kind") == "staff":
        valid = sum(1 for key, value in row.items() if key.startswith("attr_")
                    and key != "attr_source"
                    and value not in (None, "") and 1.0 <= float(value) <= 20.0)
        if valid >= 1:
            return row, True
    return None, False


def feature_vector(player, fm, role, previous_shots, previous_minutes, prior):
    born = datetime.strptime(fm["dob"], "%Y-%m-%d").date()
    kickoff_date = datetime.fromtimestamp(player["kickoff"], timezone.utc).date()
    age = max(15.0, min(50.0, (kickoff_date - born).days / 365.25))
    ca_raw = fm.get("ca") or ""
    ca = float(ca_raw) if ca_raw else 0.0
    feature = [float(role == value) for value in ROLES] + [
        (age - 25.0) / 10.0,
        (ca - 100.0) / 100.0 if ca > 0 else 0.0,
    ]
    missing = []
    for name in ATTRS:
        raw = fm.get(f"attr_{name}", "")
        value = float(raw) if raw else 0.0
        # 0 is unset in the exported FM schema; preserve missingness distinctly.
        is_missing = value <= 0.0
        feature.append((value - 10.0) / 10.0 if not is_missing else 0.0)
        missing.append(float(is_missing))
    feature.extend(missing)
    feature.extend((math.log(max(1e-5, prior)), math.log1p(previous_minutes / 90.0)))
    if len(feature) != len(FEATURE_NAMES) or not all(math.isfinite(x) for x in feature):
        raise ValueError("invalid feature vector")
    return feature


def make_dataset(fm_file, root, valid_from, train_end, valid_end, test_end):
    events, sources = event_manifest(root)
    fm = load_fm_index(fm_file)
    counters = Counter()
    samples = []
    appearance_history = defaultdict(lambda: [0, 0.0])  # observed prior shots, minutes
    # Sorting by actual kickoff makes rolling player statistics strictly causal.
    for match_id, kickoff in sorted(events.items(), key=lambda item: (item[1], item[0])):
        path = root / "player_matches" / f"{match_id}.json"
        if not path.is_file():
            counters["missing_player_match_file"] += 1
            continue
        event = json.loads(path.read_text())
        match_rows = []
        seen = set()
        date = datetime.fromtimestamp(kickoff, timezone.utc).date().isoformat()
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
                shots = stats.get("totalShots", 0)
                if (not isinstance(shots, int) or shots < 0
                        or not math.isfinite(minutes) or not 0 < minutes <= 130):
                    counters["invalid_label"] += 1
                    continue
                role = (entry.get("position") or person.get("position") or "").upper()[:1]
                if role not in ROLES:
                    counters["unknown_role"] += 1
                    continue
                t = person.get("dateOfBirthTimestamp")
                dob = datetime.fromtimestamp(t, timezone.utc).date().isoformat() if isinstance(t, int) else ""
                match_rows.append((player_id, shots, minutes, role, person.get("name") or "", dob))
                if date < valid_from or date >= test_end:
                    continue
                candidates = fm.get((normalize(person.get("name")), dob), [])
                usable, rescued = fm_row_usable(candidates)
                if usable is None:
                    counters["missing_or_ambiguous_fm"] += 1
                    continue
                if rescued:
                    counters["staff_kind_db_rescued"] += 1
                matches_so_far, previous_minutes = appearance_history[player_id]
                # Gamma-Poisson empirical Bayes prior, with 5 full-match exposures.
                prior = (matches_so_far + 5.0 * SHOT_ROLE_PRIORS[role]) / (
                    previous_minutes / 90.0 + 5.0
                )
                example = {"kickoff": kickoff}
                features = feature_vector(example, usable, role, matches_so_far, previous_minutes, prior)
                split = ("train" if date < train_end else "validation"
                         if date < valid_end else "test")
                samples.append({
                    "split": split, "date": date, "match_id": match_id,
                    "player_id": player_id, "fm_uid": usable["uid"],
                    "shots": shots, "minutes": float(minutes),
                    "prior_rate": prior, "features": features,
                })
                counters[f"{split}_joined"] += 1
            # Do not update the history within the match before both sides have been read.
        for player_id, shots, minutes, _, _, _ in match_rows:
            history = appearance_history[player_id]
            history[0] += shots
            history[1] += minutes
        counters["matches_with_event_data"] += 1
    if not (valid_from < train_end < valid_end < test_end):
        raise ValueError("chronological split cutoffs must be strictly increasing")
    if not samples:
        raise ValueError("no verified post-vintage FM26 event labels")
    by_split = Counter(s["split"] for s in samples)
    manifest = {
        "contract": "fm26_db_observed_attribute_v26.2_mls_shots_asof_v1",
        "model_input": "FM26 on-disk attributes (attr_source=db), pre-kickoff role and player history",
        "target": "observed SofaScore per-player totalShots conditioned on actual played minutes",
        "label_caveat": "actual played minutes is an evaluation exposure, not a pre-match feature",
        "vintage": "FM26 v26.2 March 2026; assume conservatively available starting " + valid_from,
        "cutoffs": dict(valid_from=valid_from, train_end=train_end,
                        validation_end=valid_end, test_end=test_end),
        "feature_names": FEATURE_NAMES, "n_features": len(FEATURE_NAMES),
        "split_counts": dict(by_split), "audit_counts": dict(counters),
        "raw_inputs": {
            "fm26_sha256": sha256(fm_file),
            "fm26_complete_everyone_sha256": sha256(_union_complete_path()),
            **{file.name + "_sha256": sha256(file) for file in sources},
        },
    }
    return samples, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fm-file", type=Path,
                        default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--sofa-root", type=Path, default=Path("data/sofascore_cfm"))
    parser.add_argument("--output", type=Path, default=Path("data/ml/fm26-mls-shots.jsonl"))
    parser.add_argument("--valid-from", default="2026-04-01")
    parser.add_argument("--train-end", default="2026-06-01")
    parser.add_argument("--validation-end", default="2026-08-01")
    parser.add_argument("--test-end", default="2026-09-01")
    args = parser.parse_args()
    samples, manifest = make_dataset(
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
