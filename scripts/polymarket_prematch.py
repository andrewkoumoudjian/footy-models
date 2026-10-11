#!/usr/bin/env python3
"""Produce a strictly pre-kickoff 1X2 market *reference*, never overwrite engine paths.

Input spec explicitly identifies three YES tokens from mutually exclusive,
exhaustive 90-minute match-result contracts. Market discovery and settlement
verification are manual because sports markets can mean qualification, outright
winner, or a two-way result excluding draws. Price history is indicative, not
necessarily executable (spreads, fees, depth are not in this endpoint).
"""
import argparse
from datetime import datetime
import json
import math
from pathlib import Path
from urllib.parse import urlencode
from urllib.request import urlopen, Request

CLOB_URL = "https://clob.polymarket.com/prices-history"
OUTCOMES = ("home", "draw", "away")


def unix_time(iso):
    parsed = datetime.fromisoformat(iso.replace("Z", "+00:00"))
    if parsed.utcoffset() is None:
        raise ValueError("timestamps must carry a timezone")
    return int(parsed.timestamp())


def validated_spec(spec):
    if spec.get("settlement") != "regulation_90_minutes_plus_stoppage_1x2":
        raise ValueError("verify the exact 90-minute draw-inclusive market settlement rule")
    if not spec.get("event_slug") or not spec.get("fixture_id"):
        raise ValueError("market must be mapped to an explicit fixture ID and event slug")
    tokens = spec.get("yes_tokens", {})
    if set(tokens) != set(OUTCOMES) or len(set(str(x) for x in tokens.values())) != 3:
        raise ValueError("provide distinct home, draw, away YES token IDs")
    for name, token in tokens.items():
        if not str(token).isdigit() or int(token) <= 0:
            raise ValueError(f"invalid {name} token ID")
    return tokens


def select_prematch_quotes(spec, histories, max_age_minutes=360):
    """Reject any stale, missing, future, in-play or ambiguous snapshot."""
    validated_spec(spec)
    kickoff = unix_time(spec["kickoff_utc"])
    cutoff = unix_time(spec["snapshot_utc"])
    if cutoff >= kickoff:
        raise ValueError("snapshot must precede kickoff")
    if not (1 <= max_age_minutes <= 1440):
        raise ValueError("max age must be between 1 and 1440 minutes")
    raw, timestamps = {}, {}
    for outcome in OUTCOMES:
        points = histories.get(outcome, [])
        candidates = []
        for point in points:
            timestamp = int(point["t"])
            price = float(point["p"])
            if not math.isfinite(price) or not 0 < price < 1:
                raise ValueError(f"invalid {outcome} probability")
            if timestamp <= cutoff:
                candidates.append((timestamp, price))
        if not candidates:
            raise ValueError(f"no as-of quote for {outcome}")
        ts, price = max(candidates)
        if cutoff - ts > 60 * max_age_minutes:
            raise ValueError(f"stale {outcome} quote, {cutoff-ts}s old")
        raw[outcome] = price
        timestamps[outcome] = ts
    total = sum(raw.values())
    # Three separately traded binaries may not sum to exactly 1. Refuse
    # suspicious market sets rather than normalizing unrelated contracts.
    if not 0.8 <= total <= 1.2:
        raise ValueError(f"market contracts not plausible as mutually exhaustive 1X2: sum={total}")
    return {
        "schema_version": 1, "fixture_id": spec["fixture_id"], "event_slug": spec["event_slug"],
        "settlement": spec["settlement"], "kickoff_utc": spec["kickoff_utc"],
        "snapshot_utc": spec["snapshot_utc"], "yes_tokens": spec["yes_tokens"],
        "quote_timestamps_unix": timestamps, "raw_yes_prices": raw,
        "raw_probability_sum": total,
        "normalized_reference": {side: raw[side] / total for side in OUTCOMES},
        "status": "reference_only_not_calibrated_or_tradable",
        "notes": "Last available sampled trade prices, not three-way executable odds. Account for overround, fees, spreads, liquidity, and source timestamp uncertainty before production use.",
    }


def attach_engine_comparison(reference, snapshot):
    """Attach a same-as-of diagnostic, leaving the Rust path output untouched."""
    as_of = snapshot.get("as_of")
    if not as_of or reference["snapshot_utc"][:10] > as_of:
        raise ValueError("market quote is newer than the engine snapshot's date cutoff")
    matches = [f for f in snapshot.get("fixtures", [])
               if reference["fixture_id"] == f"{f['date']}|{f['home']}|{f['away']}"]
    if len(matches) != 1:
        raise ValueError("market fixture ID does not uniquely match the engine snapshot")
    fixture = matches[0]
    engine = {"home": float(fixture["home_win"]), "draw": float(fixture["draw"]),
              "away": float(fixture["away_win"])}
    if any(not math.isfinite(p) or p < 0 or p > 1 for p in engine.values()) or abs(sum(engine.values())-1) > 1e-4:
        raise ValueError("invalid engine 1X2 probabilities")
    reference["engine_reference"] = engine
    reference["market_minus_engine"] = {
        key: reference["normalized_reference"][key] - engine[key] for key in OUTCOMES
    }
    reference["comparison_policy"] = "Descriptive only; market does not alter the model's paths or release status."
    return reference


def download_history(token, cutoff, max_age_minutes):
    query = urlencode({"market": token, "startTs": cutoff - 60 * max_age_minutes,
                       "endTs": cutoff, "fidelity": 1})
    with urlopen(Request(f"{CLOB_URL}?{query}", headers={"User-Agent": "footy-models/1.0"}), timeout=25) as response:
        result = json.load(response)
    if not isinstance(result.get("history"), list):
        raise ValueError("CLOB history schema missing")
    return result["history"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", required=True, help="manually verified fixture, timestamps, settlement and three YES tokens")
    parser.add_argument("--histories", help="offline JSON: {home:[{t,p}],draw:[...],away:[...]}")
    parser.add_argument("--snapshot", help="optional pre-match engine JSON; emit diagnostic comparison only")
    parser.add_argument("--max-age-minutes", type=int, default=360)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    spec = json.loads(Path(args.spec).read_text())
    tokens = validated_spec(spec)
    if args.histories:
        histories = json.loads(Path(args.histories).read_text())
    else:
        cutoff = unix_time(spec["snapshot_utc"])
        histories = {side: download_history(token, cutoff, args.max_age_minutes)
                     for side, token in tokens.items()}
    result = select_prematch_quotes(spec, histories, args.max_age_minutes)
    if args.snapshot:
        result = attach_engine_comparison(result, json.loads(Path(args.snapshot).read_text()))
    path = Path(args.output)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2) + "\n")
    print(f"market reference for {result['fixture_id']} as of {result['snapshot_utc']}: {result['normalized_reference']}")


if __name__ == "__main__":
    main()
