#!/usr/bin/env python3
"""Import dated FBref player-match statistics from probberechts/soccerdata.

Default mode processes explicit local CSV exports; no network requests.
--download explicitly asks soccerdata.FBref to fetch a bounded set of matches.
The output contains observed *labels*, not leakage-safe input features.
"""
import argparse
import csv
import hashlib
import json
import math
import re
import unicodedata
from collections import Counter
from datetime import date, datetime
from pathlib import Path

SCHEDULE_REQUIRED = {"league", "season", "game", "date", "home_team", "away_team"}
PLAYER_REQUIRED = {"league", "season", "game", "team", "player"}
SHOT_COLUMNS = ("sh", "performance_sh", "standard_sh", "shots", "total_shots",
                "performance_shots", "standard_shots")
MINUTE_COLUMNS = ("min", "minutes", "minutes_played", "playing_time_min", "playing_time_minutes")


def text_normalize(value):
    value = unicodedata.normalize("NFKD", str(value or ""))
    value = "".join(c for c in value if not unicodedata.combining(c))
    return re.sub(r"\s+", " ", value.strip()).casefold()


def column_normalize(name):
    # FBref/Soccerdata may return hierarchical columns such as Performance|Sh.
    if isinstance(name, tuple):
        name = "_".join(str(item) for item in name if str(item) not in ("", "nan"))
    return re.sub(r"[^a-z0-9]+", "_", str(name).strip().casefold()).strip("_")


def normalize_row(raw):
    result = {}
    for key, value in raw.items():
        name = column_normalize(key)
        if name.startswith("unnamed_"):
            continue
        if name in result and str(value).strip() and str(value).strip() != str(result[name]).strip():
            raise ValueError(f"colliding nonidentical source column {name}")
        result[name] = str(value).strip() if value is not None else ""
    return result


def export_df_rows(frame):
    """Translate a soccerdata pandas DataFrame to plain rows with index retained."""
    flattened = frame.reset_index()
    return [normalize_row(record) for record in flattened.to_dict(orient="records")]


def csv_rows(path):
    with open(path, newline="", encoding="utf-8-sig") as stream:
        return [normalize_row(row) for row in csv.DictReader(stream)]


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def first_present(row, names):
    for name in names:
        if row.get(name, "") != "":
            return row[name]
    return None


def parse_date(value):
    value = str(value)[:10]
    try:
        return date.fromisoformat(value).isoformat()
    except ValueError as exc:
        raise ValueError(f"invalid match date: {value!r}") from exc


def as_nonnegative_int(value, field):
    if value is None or str(value).strip() in ("", "nan", "NaN", "-"):
        return None
    x = float(value)
    if not math.isfinite(x) or x < 0 or int(x) != x:
        raise ValueError(f"bad {field} count: {value!r}")
    return int(x)


def sha_data(rows):
    source = json.dumps(rows, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(source.encode("utf-8")).hexdigest()


def convert(schedule_rows, player_rows, *, league=None, season=None, start=None, end=None):
    matches = {}
    warnings = Counter()
    for source in schedule_rows:
        row = normalize_row(source)
        if not SCHEDULE_REQUIRED.issubset(row):
            raise ValueError(f"schedule is missing columns: {sorted(SCHEDULE_REQUIRED - row.keys())}")
        if league and row["league"] != league:
            continue
        if season and row["season"] != str(season):
            continue
        day = parse_date(row["date"])
        if start and day < start:
            continue
        if end and day >= end:
            continue
        home, away = row["home_team"], row["away_team"]
        if not home or not away or text_normalize(home) == text_normalize(away):
            raise ValueError("invalid fixture home/away team")
        match_id = row.get("game_id", "").strip()
        # A canonical match report ID is mandatory; generated game label alone
        # is insufficient to align source player-match statistics reliably.
        if not re.fullmatch(r"[A-Za-z0-9]{8}", match_id):
            warnings["schedule_no_report_id"] += 1
            continue
        key = (row["league"], row["season"], row["game"])
        record = {
            "source": "FBref via probberechts/soccerdata",
            "league": key[0], "season": key[1], "game": key[2],
            "match_id": match_id, "date": day, "home_team": home,
            "away_team": away,
        }
        if key in matches and matches[key] != record:
            raise ValueError(f"duplicate conflicting match {key}")
        matches[key] = record

    output = []
    seen = set()
    for source in player_rows:
        row = normalize_row(source)
        if not PLAYER_REQUIRED.issubset(row):
            raise ValueError(f"player match is missing columns: {sorted(PLAYER_REQUIRED - row.keys())}")
        key = (row["league"], row["season"], row["game"])
        fixture = matches.get(key)
        if fixture is None:
            warnings["player_unmatched_schedule"] += 1
            continue
        team = row["team"]
        if text_normalize(team) not in (
            text_normalize(fixture["home_team"]), text_normalize(fixture["away_team"])
        ):
            raise ValueError(f"player team not in fixture: {key} / {team}")
        player = row["player"]
        if not player or not player.strip():
            warnings["player_missing_name"] += 1
            continue
        # The index stores names, NOT stable FBref person IDs. Never claim a
        # historical FM join solely from a fuzzy/same-name match.
        identity = (
            fixture["match_id"], text_normalize(team), text_normalize(player),
        )
        if identity in seen:
            raise ValueError(f"duplicate player in fixture: {identity}")
        seen.add(identity)
        minutes = as_nonnegative_int(first_present(row, MINUTE_COLUMNS), "minutes")
        shots = as_nonnegative_int(first_present(row, SHOT_COLUMNS), "shots")
        if minutes is None or shots is None:
            warnings["missing_player_minutes_or_shots"] += 1
            continue
        if minutes > 130:
            raise ValueError(f"invalid match minutes: {minutes}")
        if minutes == 0 and shots > 0:
            raise ValueError("positive shots with zero minutes")
        if minutes == 0:
            # Player may be on the unused bench; no exposure, no training label.
            warnings["zero_minutes"] += 1
            continue
        output.append({
            "source": "FBref via probberechts/soccerdata",
            "source_match_id": fixture["match_id"], "game": fixture["game"],
            "league": fixture["league"], "season": fixture["season"],
            "date": fixture["date"], "home_team": fixture["home_team"],
            "away_team": fixture["away_team"],
            "team": team, "player": player,
            "player_identity_status": "NAME_ONLY_NOT_VETTED_FOR_FM_JOIN",
            "position": first_present(row, ("pos", "position")) or "",
            "minutes": minutes, "shots": shots,
        })
    output.sort(key=lambda x: (x["date"], x["source_match_id"], x["team"], x["player"]))
    manifest = {
        "contract": "fbref_soccerdata_observed_player_match_shots_v1",
        "source": "https://github.com/probberechts/soccerdata",
        "source_method": "FBref.read_schedule + FBref.read_player_match_stats('summary')",
        "fixtures_with_report": len(matches), "player_match_rows": len(output),
        "unique_player_names": len({(r["team"], r["player"]) for r in output}),
        "warnings": dict(warnings),
        "asof_rule": "Only match dates before target kickoff can supply causal history",
        "identity_rule": "FBref name/team NOT sufficient to join historical FM player IDs",
        "label_rule": "Observed match shots/minutes are targets/exposure, not pre-match features",
        "source_sha256": {
            "schedule_records": sha_data(schedule_rows),
            "player_records": sha_data(player_rows),
        },
    }
    return output, manifest


def download(limit, *, league, season, start, end):
    if limit < 1 or limit > 50:
        raise ValueError("--max-matches must be in [1,50], to avoid bulk scraping")
    try:
        import soccerdata as sd
    except ImportError as exc:
        raise RuntimeError("soccerdata missing; install in a separate venv") from exc
    fbref = sd.FBref(leagues=league, seasons=season)
    schedule = export_df_rows(fbref.read_schedule())
    selected = []
    for item in schedule:
        report = item.get("game_id", "")
        day = parse_date(item["date"])
        if re.fullmatch(r"[a-zA-Z0-9]{8}", report) and (not start or day >= start) and (not end or day < end):
            selected.append((day, report))
    selected.sort()
    limited = list(dict.fromkeys(report for _, report in selected))[:limit]
    if not limited:
        raise ValueError("no completed matches with report IDs in requested date window")
    player_rows = export_df_rows(fbref.read_player_match_stats("summary", match_id=limited))
    return schedule, player_rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schedule-csv", type=Path)
    parser.add_argument("--players-csv", type=Path)
    parser.add_argument("--download", action="store_true",
                        help="Explicitly permit bounded soccerdata FBref collection")
    parser.add_argument("--league", default="ENG-Premier League")
    parser.add_argument("--season", default="2020-2021")
    parser.add_argument("--from-date")
    parser.add_argument("--until-date", help="Exclusive end date")
    parser.add_argument("--max-matches", type=int, default=5)
    parser.add_argument("--output", type=Path, default=Path("data/fbref/player_match_shots.jsonl"))
    args = parser.parse_args()
    if args.download:
        if args.schedule_csv or args.players_csv:
            parser.error("CSV inputs cannot be combined with --download")
        schedule, players = download(
            args.max_matches, league=args.league, season=args.season,
            start=args.from_date, end=args.until_date,
        )
    else:
        if not args.schedule_csv or not args.players_csv:
            parser.error("offline mode requires --schedule-csv and --players-csv")
        schedule, players = csv_rows(args.schedule_csv), csv_rows(args.players_csv)
    rows, manifest = convert(
        schedule, players, league=args.league, season=args.season,
        start=args.from_date, end=args.until_date,
    )
    if not rows:
        raise ValueError("no verified FBref per-player shots+minutes in selected fixtures")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as stream:
        for row in rows:
            stream.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
    manifest["output_sha256"] = digest(args.output)
    args.output.with_suffix(".manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({
        "output": str(args.output), "rows": len(rows), "warnings": manifest["warnings"]
    }, indent=2))


if __name__ == "__main__":
    main()
