#!/usr/bin/env python3
"""Build truly pre-match, two-team player *candidate sets* for set attention.

Each club's candidate pool is derived ONLY from earlier SofaScore appearances.
The current match's player list supplies labels AFTER candidate construction,
never current-XI or outcome information as model input. All data is research-only.
"""
import argparse
import json
import math
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

from build_fm_mls_shot_dataset import (
    ATTRS, FEATURE_NAMES, ROLES, SHOT_ROLE_PRIORS,
    feature_vector, load_fm_index, normalize, sha256,
)

EXTRA_FEATURE_NAMES = ("is_home", "last_seen_log_days", "fm_verified")
SET_FEATURE_NAMES = (*FEATURE_NAMES, *EXTRA_FEATURE_NAMES)


def read_events(root):
    files = [
        root / "history_manifest.json",
        root / "charlotte_history_manifest.json",
        root / "opponent_history_manifest.json",
    ]
    events = {}
    for file in files:
        raw = json.loads(file.read_text())
        groups = [raw] if isinstance(raw, list) else (
            [raw["events"]] if "events" in raw else list(raw.values())
        )
        for group in groups:
            for event in group:
                mid, ts = event.get("id"), event.get("startTimestamp")
                h, a = event.get("homeTeam") or {}, event.get("awayTeam") or {}
                if not isinstance(mid, int) or not isinstance(ts, int):
                    continue
                if not isinstance(h.get("id"), int) or not isinstance(a.get("id"), int):
                    continue
                info = (ts, h["id"], a["id"])
                if mid in events and events[mid] != info:
                    raise ValueError(f"inconsistent duplicate fixture {mid}")
                events[mid] = info
    return sorted(events.items(), key=lambda item: (item[1][0], item[0])), files


def age_feature(dob, kickoff):
    if not dob:
        return 0.0
    try:
        born = datetime.strptime(dob, "%Y-%m-%d").date()
    except ValueError:
        return 0.0
    age = (datetime.fromtimestamp(kickoff, timezone.utc).date() - born).days / 365.25
    return (max(15.0, min(50.0, age)) - 25.0) / 10.0


def candidate_features(player, fm_index, kickoff, is_home):
    role = player["role"]
    past_shots = player["shots"]
    past_minutes = player["minutes"]
    prior = (past_shots + 5.0 * SHOT_ROLE_PRIORS[role]) / (past_minutes / 90.0 + 5.0)
    matches = fm_index.get((normalize(player["name"]), player["dob"]), ())
    verified = len(matches) == 1
    if verified:
        features = feature_vector(
            {"kickoff": kickoff}, matches[0], role,
            past_shots, past_minutes, prior,
        )
    else:
        features = [float(role == value) for value in ROLES]
        features += [age_feature(player["dob"], kickoff), 0.0]
        features += [0.0] * len(ATTRS)
        features += [1.0] * len(ATTRS)
        features += [math.log(max(1e-5, prior)), math.log1p(past_minutes / 90.0)]
    days = max(0.0, (kickoff - player["last_seen"]) / 86400)
    features.extend((float(is_home), math.log1p(days) / 5.0, float(verified)))
    if len(features) != len(SET_FEATURE_NAMES):
        raise ValueError("candidate feature-width mismatch")
    return features, prior, verified


def build_sets(fm_file, root, valid_from="2026-04-01", train_end="2026-06-01",
               validation_end="2026-08-01", test_end="2026-09-01",
               max_age_days=120, candidates_per_team=25):
    if not (valid_from < train_end < validation_end < test_end):
        raise ValueError("nonchronological split cutoffs")
    if max_age_days <= 0 or candidates_per_team < 11:
        raise ValueError("invalid candidate-set limits")
    events, files = read_events(root)
    fm_index = load_fm_index(fm_file)
    history = defaultdict(dict)
    counters = Counter()
    records = []
    for match_id, (kickoff, home_team, away_team) in events:
        file = root / "player_matches" / f"{match_id}.json"
        if not file.is_file():
            continue
        raw = json.loads(file.read_text())
        date = datetime.fromtimestamp(kickoff, timezone.utc).date().isoformat()
        sides = ((home_team, "home"), (away_team, "away"))
        current = {}
        for team_id, side in sides:
            current[team_id] = {}
            for entry in raw.get(side, {}).get("players", []):
                person, statistics = entry.get("player") or {}, entry.get("statistics") or {}
                player_id = person.get("id")
                minutes, shots = statistics.get("minutesPlayed"), statistics.get("totalShots", 0)
                role = (entry.get("position") or person.get("position") or "").upper()[:1]
                if (not isinstance(player_id, int) or not isinstance(minutes, (int, float))
                        or not math.isfinite(minutes) or minutes <= 0 or minutes > 130
                        or not isinstance(shots, int) or shots < 0 or role not in ROLES):
                    continue
                date_of_birth = person.get("dateOfBirthTimestamp")
                dob = (datetime.fromtimestamp(date_of_birth, timezone.utc).date().isoformat()
                       if isinstance(date_of_birth, int) else "")
                if player_id in current[team_id]:
                    raise ValueError("duplicate player within match side")
                current[team_id][player_id] = {
                    "name": person.get("name") or "",
                    "dob": dob, "role": role,
                    "minutes": float(minutes), "shots": shots,
                }
        if valid_from <= date < test_end:
            members, joined, observed = [], 0, 0
            # Build both team candidate sets BEFORE updating any history with current match.
            for team_id, side in sides:
                prior = [
                    (player_id, state) for player_id, state in history[team_id].items()
                    if 0 < kickoff - state["last_seen"] <= max_age_days * 86400
                ]
                prior.sort(key=lambda pair: (
                    -pair[1]["last_seen"], -pair[1]["minutes"], pair[0],
                ))
                for player_id, state in prior[:candidates_per_team]:
                    features, rate, verified = candidate_features(
                        state, fm_index, kickoff, side == "home",
                    )
                    label = current[team_id].get(player_id)
                    if label is not None:
                        observed += 1
                    if verified and label is not None:
                        joined += 1
                    members.append({
                        "player_id": player_id, "side": side, "team_id": team_id,
                        "features": features, "asof_shots_per90": rate,
                        "fm_verified": verified,
                        # Null label means not observed in this match; it does NOT
                        # imply the player was available or played zero minutes.
                        "label": (
                            {"shots": label["shots"], "minutes": label["minutes"]}
                            if label is not None else None
                        ),
                    })
            if any(sum(1 for m in members if m["side"] == s) < 1 for s in ("home", "away")):
                counters["too_few_historical_candidates"] += 1
            elif joined == 0:
                counters["no_verified_fm_and_observed_labels"] += 1
            else:
                split = ("train" if date < train_end else
                         "validation" if date < validation_end else "test")
                records.append({
                    "fixture_id": match_id,
                    "date": date, "kickoff": kickoff, "split": split,
                    "home_team_id": home_team, "away_team_id": away_team,
                    "candidate_count": len(members),
                    "verified_observed_labels": joined,
                    "observed_candidate_labels": observed,
                    "candidates": members,
                })
                counters[f"{split}_fixtures"] += 1
                counters[f"{split}_fm_labels"] += joined
        # Update both teams together, only after pre-match sample construction.
        for team_id, side in sides:
            for player_id, player in current[team_id].items():
                state = history[team_id].get(player_id)
                if state is None:
                    state = dict(player, last_seen=kickoff)
                    history[team_id][player_id] = state
                else:
                    state["name"], state["dob"], state["role"] = (
                        player["name"], player["dob"], player["role"]
                    )
                    state["minutes"] += player["minutes"]
                    state["shots"] += player["shots"]
                    state["last_seen"] = kickoff
        counters["historical_matches_processed"] += 1
    manifest = {
        "contract": "fm26_mls_pre_match_candidate_set_v1",
        "vintage": "FM26 v26.2 March 2026; labels Apr-Aug only",
        "cutoffs": {
            "valid_from": valid_from, "train_end": train_end,
            "validation_end": validation_end, "test_end": test_end,
        },
        "input_provenance": {
            "fm26_sha256": sha256(fm_file),
            **{file.name + "_sha256": sha256(file) for file in files},
        },
        "feature_names": SET_FEATURE_NAMES,
        "candidates_per_team": candidates_per_team,
        "max_age_days": max_age_days,
        "counts": dict(counters),
        "limitations": [
            "Candidates are inferred from historical appearances; not confirmed registered squads.",
            "Current match player lists only supply target labels after feature creation.",
            "Unobserved candidate label null does not imply availability or playing 0 minutes.",
            "Observed minutes is a label exposure; not a pre-match model input.",
            "Attribute coverage differs by team and FM matchability.",
            "2026 August was examined for a prior MLP pilot; transformer test is exploratory.",
        ],
    }
    return records, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fm-file", type=Path,
                        default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--sofa-root", type=Path, default=Path("data/sofascore_cfm"))
    parser.add_argument("--output", type=Path,
                        default=Path("data/ml/fm26-mls-candidate-sets.jsonl"))
    parser.add_argument("--max-age-days", type=int, default=120)
    parser.add_argument("--candidates-per-team", type=int, default=25)
    args = parser.parse_args()
    data, manifest = build_sets(args.fm_file, args.sofa_root,
                                max_age_days=args.max_age_days,
                                candidates_per_team=args.candidates_per_team)
    if not data:
        raise ValueError("no leakage-safe candidate sets available")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as writer:
        for fixture in data:
            writer.write(json.dumps(fixture, sort_keys=True, separators=(",", ":")) + "\n")
    manifest["dataset_sha256"] = sha256(args.output)
    args.output.with_suffix(".manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"output": str(args.output), **manifest["counts"]}, indent=2))


if __name__ == "__main__":
    main()
