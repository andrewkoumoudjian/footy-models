#!/usr/bin/env python3
"""Import the *actual CSV archive* in the galsaar Hugging Face repository.

The Hub viewer indexes six images, but results.csv.zip has historical EPL
fixtures. The archive is a dated results source, never contemporaneous FM data.
Output stays untracked; this script makes no modifications to frozen forecasts.
"""
import argparse
import csv
import hashlib
import io
import json
import urllib.request
import zipfile
from collections import Counter
from datetime import datetime
from pathlib import Path

SOURCE = "https://huggingface.co/datasets/galsaar/EnglishPremierLeagueEPLResults/resolve/main/results.csv.zip"
REQUIRED = ("Season", "DateTime", "HomeTeam", "AwayTeam", "FTHG", "FTAG", "FTR")
OPTIONAL = ("HTHG", "HTAG", "HS", "AS", "HST", "AST", "HC", "AC", "HR", "AR")


def parse_archive(payload):
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        if archive.namelist() != ["results.csv"]:
            raise ValueError("unexpected archive content; expected exactly results.csv")
        data = archive.read("results.csv").decode("cp1252").lstrip("\ufeff")
    reader = csv.DictReader(io.StringIO(data))
    if not reader.fieldnames or any(field not in reader.fieldnames for field in REQUIRED):
        raise ValueError("source schema changed: missing required match fields")
    seen = set()
    fixtures = []
    for line, row in enumerate(reader, 2):
        try:
            date = datetime.fromisoformat(row["DateTime"].replace("Z", "+00:00")).date().isoformat()
            home, away = row["HomeTeam"].strip(), row["AwayTeam"].strip()
            hg, ag = int(row["FTHG"]), int(row["FTAG"])
            outcome = "H" if hg > ag else "A" if ag > hg else "D"
            if min(hg, ag) < 0 or home == away or not home or not away or row["FTR"] != outcome:
                raise ValueError("invalid score, teams, or outcome")
        except (ValueError, TypeError, AttributeError) as err:
            raise ValueError(f"invalid required result at line {line}: {err}") from err
        key = (date, home, away)
        if key in seen:
            raise ValueError(f"duplicate fixture {key}")
        seen.add(key)
        item = {"date": date, "season": row["Season"], "home": home, "away": away,
                "home_goals": hg, "away_goals": ag, "result": outcome}
        for field in OPTIONAL:
            raw = row.get(field, "NA")
            value = None if raw in (None, "NA", "") else int(raw)
            if value is not None and value < 0:
                raise ValueError(f"negative {field} at line {line}")
            item[field.lower()] = value
        fixtures.append(item)
    fixtures.sort(key=lambda x: (x["date"], x["home"], x["away"]))
    return fixtures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", help="existing ZIP; otherwise retrieve the versioned upstream archive")
    parser.add_argument("--output", default="data/pl/historical_galsaar_results.csv")
    parser.add_argument("--manifest", default="data/pl/historical_galsaar_manifest.json")
    args = parser.parse_args()
    if args.archive:
        payload = Path(args.archive).read_bytes()
    else:
        with urllib.request.urlopen(urllib.request.Request(SOURCE, headers={"User-Agent": "footy-models/1.0"}), timeout=30) as response:
            payload = response.read()
    fixtures = parse_archive(payload)
    if not fixtures:
        raise ValueError("no matches in archive")
    path = Path(args.output)
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = list(fixtures[0])
    with path.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fields)
        writer.writeheader()
        writer.writerows(fixtures)
    manifest = {"source": SOURCE, "archive_sha256": hashlib.sha256(payload).hexdigest(),
                "fixtures": len(fixtures), "first_date": fixtures[0]["date"],
                "last_date": fixtures[-1]["date"],
                "seasons": dict(sorted(Counter(f["season"] for f in fixtures).items())),
                "nullable_fields": list(OPTIONAL), "contains_player_attributes": False,
                "policy": "Historical results only; no post-match column may be used as a pre-kickoff feature."}
    manifest_path = Path(args.manifest)
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Imported {len(fixtures)} fixtures ({manifest['first_date']} to {manifest['last_date']}); SHA-256 {manifest['archive_sha256']}")


if __name__ == "__main__":
    main()
