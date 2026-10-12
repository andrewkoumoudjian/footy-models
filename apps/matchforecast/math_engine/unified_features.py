#!/usr/bin/env python3
"""One player-agnostic EPL/MLS pre-match feature contract for shared math models.

Reuses the semantics of footy-state PsychologyVector/AbilityVector, footy-
recruitment tactical/environment/squad fit, footy-market dated values, footy-
performance causal rolling windows and footy-match sampled XI probabilities.

This is feature engineering, not a replacement for those typed Rust primitives.
Unknown values remain None; predicted future values are *not* match inputs.
"""
from __future__ import annotations

from collections import defaultdict
from datetime import datetime, timezone
import argparse
import hashlib
import json
import math
from pathlib import Path

MENTAL = (
    "Anticipation", "Decisions", "Teamwork", "WorkRate", "Leadership",
    "Bravery", "Consistency", "Determination", "Composure", "Concentration",
    "Aggression", "ImportantMatches",
)
TECHNICAL = ("Passing", "Vision", "Dribbling", "FirstTouch", "Technique",
             "Finishing", "OffTheBall", "Marking", "Tackling", "Positioning")
PHYSICAL = ("Stamina", "Pace", "Strength", "Acceleration", "Agility", "Balance")
SOFA = ("rating", "expectedGoals", "expectedAssists", "totalShots",
        "totalPass", "accuratePass", "totalTackle", "interceptionWon",
        "bigChanceCreated", "totalDuel", "duelWon")
FBREF = ("minutes_per90", "shots_per90", "non_penalty_xg_per90",
         "assists_per90", "progressive_passes_per90", "progressive_carries_per90",
         "passes_completed_per90", "tackles_interceptions_per90")
FORM = ("form_residual_ewma", "form_residual_trend", "form_variance",
        "days_since_last_match", "played_minutes_30d", "played_minutes_90d")
RECRUITMENT = ("tactical_fit", "squad_role_complementarity", "environment_fit",
               "availability_probability")
VALUE = ("log_observed_market_value_eur", "market_value_uncertainty",
         "transfer_risk_12m", "contract_years_remaining")
MARKET_HISTORY = (
    "log_mv_180d_ratio", "log_mv_365d_ratio",
    "log_peak_prior_ratio", "log_min_prior_ratio",
    "log_n_vals_prior", "log_days_since_first_val",
    "val_freq", "log_days_since_last_val", "last_change_dir",
    "mv_vol_log", "ever_transferred", "log_days_since_transfer",
    "log_transfer_fee", "free_transfer", "log_mv_transfer_ratio",
)
PSYCHOLOGY = (
    "professionalism", "ambition", "consistency", "pressure",
    "adaptability", "leadership", "teamwork", "discipline",
)
LEAGUES = ("EPL", "MLS")

# Predictive *outputs* of other models, which must never quietly become
# features: 12m/24m/36m player future price, breakout labels, achieved transfer
# fee, actual minutes or target-match SofaScore rating, 2024-25 target-derived xG
# overperformance, post-match psychology, and the current fixture's XI.
FORBIDDEN = frozenset({
    "future_value_12m", "future_value_24m", "future_value_36m",
    "breakout_probability", "target_market_value", "future_transfer_fee",
    "actual_minutes", "actual_goals", "actual_shots", "post_match_rating",
    "post_match_morale", "xG_Diff", "xG_Overperformer",
})
FEATURE_GROUPS = {
    "fm_mental": tuple("fm_mental_" + s for s in MENTAL),
    "fm_technical": tuple("fm_technical_" + s for s in TECHNICAL),
    "fm_physical": tuple("fm_physical_" + s for s in PHYSICAL),
    "sofascore": tuple("sofascore_" + s for s in SOFA),
    "fbref": tuple("fbref_" + s for s in FBREF),
    "form": FORM,
    "recruitment_fit": RECRUITMENT,
    "market_context": VALUE,
    "recruitment_value_history": MARKET_HISTORY,
    "stable_psychology": tuple("stable_psychology_" + s for s in PSYCHOLOGY),
    "psychology_confidence": tuple("psychology_confidence_" + s for s in PSYCHOLOGY),
    "team_context": (
        "team_mental_mean", "team_mental_std", "team_leadership_mean",
        "team_teamwork_mean", "team_mental_coverage",
        "teammate_co_start_familiarity",
        "team_rating_recent", "team_xg_recent", "team_xga_recent",
    ),
    "player_context": (
        "age_years", "fm_ca", "fm_pa", "fm_ca_realization",
        "fm_attributes_observed", "fm_attributes_reconstructed",
        "position_gk", "position_df", "position_mf", "position_fw",
        "league_mls", "previous_start_probability",
    ),
}
FEATURE_NAMES = tuple(x for group in FEATURE_GROUPS.values() for x in group)


def utc(value: str) -> datetime:
    dt = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if dt.tzinfo is None or dt.utcoffset() is None:
        raise ValueError("all timestamps require offsets")
    return dt.astimezone(timezone.utc)


def observed(payload: dict | None, cutoff: datetime, expected: str):
    """Only measurements actually published strictly before forecast cutoff."""
    if payload is None:
        return None
    if not isinstance(payload, dict):
        raise ValueError(f"{expected} requires a dict")
    at = payload.get("published_at")
    if at is None:
        raise ValueError(f"{expected} missing publication timestamp")
    if utc(at) >= cutoff:
        raise ValueError(f"{expected} is not available before kickoff cutoff")
    data = payload.get("features")
    if not isinstance(data, dict):
        raise ValueError(f"{expected} missing features")
    if FORBIDDEN.intersection(data):
        raise ValueError(f"{expected} contains future target-derived data")
    return data


def finite(value):
    if value is None:
        return None
    if isinstance(value, bool):
        raise ValueError("boolean is not a statistical measurement")
    value = float(value)
    if not math.isfinite(value):
        raise ValueError("nonfinite model feature")
    return value


def mean(xs):
    return sum(xs) / len(xs) if xs else None


def team_profile(players, previous_costarts):
    """Measured FM stable-trait profile, not confirmed 11 starters or morale."""
    observed_means = []
    leaders = []
    teamwork = []
    trait_count = 0
    trait_values = []
    for p in players:
        fm = p.get("fm") or {}
        if fm.get("kind") != "player" or fm.get("attr_source") != "db":
            continue
        attrs = fm.get("attributes") or {}
        values = [finite(attrs.get(t)) for t in MENTAL]
        values = [v for v in values if v is not None and v > 0]
        if not values:
            continue
        observed_means.append(mean(values))
        trait_values.extend(values)
        trait_count += 1
        if attrs.get("Leadership") not in (None, "", 0):
            leaders.append(finite(attrs["Leadership"]))
        if attrs.get("Teamwork") not in (None, "", 0):
            teamwork.append(finite(attrs["Teamwork"]))
    out = {
        "team_mental_mean": mean(observed_means),
        "team_mental_std": (
            math.sqrt(mean([(v - mean(observed_means))**2 for v in observed_means]))
            if observed_means else None
        ),
        "team_leadership_mean": mean(leaders),
        "team_teamwork_mean": mean(teamwork),
        "team_mental_coverage": trait_count / len(players) if players else 0.0,
    }
    counts = [
        float(row["completed_co_starts"]) for row in previous_costarts
        if row.get("completed_co_starts") is not None
    ]
    out["teammate_co_start_familiarity"] = mean(counts)
    return out


def single_player(p, cutoff, team, team_features, league):
    identifier = p.get("canonical_player_id")
    if not isinstance(identifier, str) or not identifier:
        raise ValueError("player needs a canonical namespace-prefixed identity")
    if ":" not in identifier:
        raise ValueError("canonical ID has no namespace")
    if league not in LEAGUES:
        raise ValueError(f"unknown league {league}")
    data = {}
    fm_record = p.get("fm") or {}
    if fm_record:
        fm_date = fm_record.get("snapshot_at")
        if not fm_date or utc(fm_date) >= cutoff:
            raise ValueError("FM26 snapshot must precede match feature cutoff")
        if fm_record.get("kind") == "player" and fm_record.get("attr_source") in ("db", "derived"):
            attrs = fm_record.get("attributes") or {}
            for t in MENTAL:
                data["fm_mental_" + t] = finite(attrs.get(t))
            for t in TECHNICAL:
                data["fm_technical_" + t] = finite(attrs.get(t))
            for t in PHYSICAL:
                data["fm_physical_" + t] = finite(attrs.get(t))
        else:
            attrs = {}
        data["fm_attributes_observed"] = float(
            fm_record.get("kind") == "player" and fm_record.get("attr_source") == "db")
        data["fm_attributes_reconstructed"] = float(
            fm_record.get("kind") == "player" and fm_record.get("attr_source") == "derived")
        ca = finite(fm_record.get("ca")) if fm_record.get("kind") == "player" else None
        pa = finite(fm_record.get("pa")) if fm_record.get("kind") == "player" else None
    else:
        ca = pa = None
        data["fm_attributes_observed"] = 0.0
        data["fm_attributes_reconstructed"] = 0.0
    data["fm_ca"] = ca
    data["fm_pa"] = pa
    data["fm_ca_realization"] = ca / pa if ca is not None and pa is not None and pa > 0 else None
    for source, fields in (("sofascore", SOFA), ("fbref", FBREF)):
        measured = observed(p.get(source), cutoff, source)
        for field in fields:
            data[source + "_" + field] = finite(measured.get(field)) if measured else None
    for source, fields in (("form", FORM), ("recruitment_fit", RECRUITMENT),
                           ("market_context", VALUE),
                           ("market_value_history", MARKET_HISTORY)):
        measured = observed(p.get(source), cutoff, source)
        for field in fields:
            data[field] = finite(measured.get(field)) if measured else None
    # Professionalism, pressure, teamwork, etc. are separate stable
    # psychology measures in footy-state. For a partial FM export, missing
    # hidden traits stay missing rather than being generated from CA.
    psycho = observed(p.get("stable_psychology"), cutoff, "stable_psychology")
    confidence = observed(p.get("psychology_confidence"), cutoff, "psychology_confidence")
    for field in PSYCHOLOGY:
        data["stable_psychology_" + field] = finite(psycho.get(field)) if psycho else None
        value = finite(confidence.get(field)) if confidence else None
        if value is not None and not (0 <= value <= 1):
            raise ValueError("psychology confidence outside 0..1")
        data["psychology_confidence_" + field] = value
    for field in FEATURE_GROUPS["team_context"]:
        data[field] = team_features.get(field)
    role = p.get("role") or ""
    for token in ("GK", "DF", "MF", "FW"):
        data["position_" + token.lower()] = float(role == token)
    data["age_years"] = finite(p.get("age_years"))
    data["league_mls"] = float(league == "MLS")
    prior = observed(p.get("participation"), cutoff, "participation")
    data["previous_start_probability"] = finite(prior.get("start_probability")) if prior else None
    if p.get("team") != team:
        raise ValueError("player has been assigned to the wrong club")
    if any(key not in FEATURE_NAMES for key in data):
        raise ValueError("feature contract introduced unknown column")
    return {
        "canonical_player_id": identifier,
        "team": team,
        "features": {k: data.get(k) for k in FEATURE_NAMES},
        "mask": {k: data.get(k) is not None for k in FEATURE_NAMES},
        "origin_status": {
            "fm_attribute_source": fm_record.get("attr_source", "missing"),
            "sofascore": "as_of_verified" if p.get("sofascore") else "missing",
            "fbref": "as_of_verified" if p.get("fbref") else "missing",
            "psychology": "latent_unobserved_morale_not_imputed",
        },
    }


def assemble(bundle):
    if bundle.get("schema") != "unified_math_fixture_v1":
        raise ValueError("unexpected unified model input")
    league = bundle["league"]
    cutoff = utc(bundle["feature_cutoff_utc"])
    kickoff = utc(bundle["fixture_kickoff_utc"])
    if kickoff <= cutoff:
        raise ValueError("prediction cutoff must precede kickoff")
    players = bundle.get("registered_players")
    if not isinstance(players, list):
        raise ValueError("registered player list missing")
    ids = [p.get("canonical_player_id") for p in players]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate player identities in a fixture")
    clubs = tuple(bundle["clubs"])
    if len(clubs) != 2 or clubs[0] == clubs[1]:
        raise ValueError("fixture must contain two distinct clubs")
    result = []
    teams = {}
    previous = bundle.get("prior_confirmed_co_starts", [])
    if not isinstance(previous, list):
        raise ValueError("prior co-start history must be a list")
    for row in previous:
        if utc(row["published_at"]) >= cutoff:
            raise ValueError("future co-start lineup was included")
    for team in clubs:
        people = [p for p in players if p.get("team") == team]
        if len(people) < 11:
            raise ValueError(f"{team}: fewer than 11 registered candidates")
        profile = team_profile(people, [r for r in previous if r["club"] == team])
        recent = observed((bundle.get("team_recent") or {}).get(team), cutoff, "team_recent")
        profile["team_rating_recent"] = finite(recent.get("rating")) if recent else None
        profile["team_xg_recent"] = finite(recent.get("xg")) if recent else None
        profile["team_xga_recent"] = finite(recent.get("xga")) if recent else None
        teams[team] = profile
        result.extend(single_player(p, cutoff, team, profile, league) for p in people)
    if len(result) != len(players):
        raise ValueError("player registered to neither fixture club")
    return {
        "schema": "unified_math_features_v1",
        "league": league,
        "feature_cutoff_utc": cutoff.isoformat(),
        "fixture_kickoff_utc": kickoff.isoformat(),
        "all_candidates_kept": True,
        "feature_names": list(FEATURE_NAMES),
        "teams": teams,
        "players": result,
        "rules": [
            "No player or team ID enters a numerical feature vector.",
            "Team mental averages use only actual FM db attributes, never generated mental traits as if observed.",
            "Value forecast outputs and match labels are auxiliary targets, never simultaneous predictors.",
            "Co-start familiarity uses only earlier published confirmed team lineups.",
            "Missing SofaScore, FBref, psychology or FM mental data remain explicitly masked.",
            "One shared mathematical parameterization is fitted on EPL and MLS with league context.",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, required=True,
                        help="versioned source-neutral as-of player roster and feature bundle")
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    if options.output.exists():
        parser.error("refusing to overwrite immutable feature snapshot")
    raw = options.fixture.read_bytes()
    model_input = assemble(json.loads(raw))
    model_input["input_sha256"] = hashlib.sha256(raw).hexdigest()
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(model_input, sort_keys=True, indent=2) + "\n")
    print(f"Retained {len(model_input['players'])} source-masked player candidates "
          f"and {len(FEATURE_NAMES)} shared mathematical predictors")


if __name__ == "__main__":
    main()
