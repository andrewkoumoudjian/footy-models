#!/usr/bin/env python3
"""One versioned EPL/MLS pre-match feature contract, with honest source masks.

FM26 mental/technical traits, dated SofaScore team/player features, verified
FBref person-ID stats, and as-of team aggregates share one player-set input.
All target outputs remain downstream of this bundle. It is NOT a calibrated
model: missing measurements and unverified identities are never fabricated.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
from collections import Counter, defaultdict
from datetime import date, datetime, timezone
from pathlib import Path

from match_premier_league_fm26 import TEAM_ALIASES, norm

MENTAL = ("Anticipation", "Decisions", "Teamwork", "WorkRate", "Leadership",
          "Bravery", "Consistency", "Determination", "Composure", "Concentration",
          "Aggression", "ImportantMatches", "Dirtiness")
PSYCHOLOGY_TRAITS = ("professionalism", "ambition", "consistency", "pressure",
                     "adaptability", "leadership", "teamwork", "discipline")
SURGE_FEATURE_NAMES = (
    "log_mv", "log_mv_180d_ratio", "log_mv_365d_ratio", "log_peak_prior_ratio",
    "log_min_prior_ratio", "log_n_vals_prior", "log_days_since_first_val",
    "val_freq", "log_days_since_last_val", "last_change_dir", "mv_vol_log",
    "perf_present_730", "log_mins_365", "log_apps_365", "g_per90_365", "a_per90_365",
    "log_mins_180", "mins_trend", "g_per90_180", "ever_transferred",
    "log_days_since_transfer", "log_transfer_fee", "free_transfer",
    "log_mv_transfer_ratio", "age", "age_sq", "pos_gk", "pos_df", "pos_mf",
    "height_cm", "comp_mapped", "first_tier", "confed_uefa", "confed_sam",
    "fm_present", "fm_ca", "fm_pa", "fm_pa_gap", "fm_ca_age_pct",
    "fm_sig_technical", "fm_sig_physical", "fm_sig_mental", "fm_sig_defensive",
    "fm_sig_attacking", "fm_sig_creative", "fm_attr_db", "fm_attr_derived",
)
SOFA_FIELDS = ("rating", "expectedGoals", "expectedAssists", "totalShots",
               "totalPass", "accuratePass", "totalTackle", "interceptionWon",
               "bigChanceCreated", "totalDuel", "duelWon", "goalAssist", "goals",
               "keyPass", "totalClearance", "possessionLostCtrl", "accurateLongBalls",
               "aerialWon", "totalOffside", "onTargetScoringAttempt")
TEAM_FIELDS = ("xg", "goals", "sot", "possession", "passes_acc", "big_chances",
               "corners", "fouls", "yellow", "red")
FBREF_FIELDS = ("xG", "xAG", "npxG", "SCA", "GCA", "Sh", "SoT", "KP",
                "PrgP", "PrgC", "Ast", "Tkl", "Int", "Blocks")
ROOT = Path(__file__).resolve().parents[1]


def hash_id(*parts):
    return hashlib.sha256("|".join(str(s).casefold() for s in parts).encode()).hexdigest()[:24]


def plausible_height(value):
    """FM height is cm; raw 65354-style sentinel values are missing, not tall."""
    if value is None:
        return None
    try:
        height = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(height) or not 140.0 <= height <= 220.0:
        return None
    return height


def plausible_ability(value):
    if value is None or value == "":
        return None
    try:
        ability = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(ability) or not 1.0 <= ability <= 200.0:
        return None
    return ability


def number(value):
    if value is None or value == "":
        return None
    try:
        n = float(value)
        return n if n == n and abs(n) != float("inf") else None
    except (TypeError, ValueError):
        return None


def fm_vintage_allowed(cutoff):
    # FM26 database snapshot is not contemporaneous with 2015/16 StatsBomb.
    if date.fromisoformat(cutoff) < date(2026, 4, 1):
        raise ValueError("FM26.2 snapshot cannot be joined to pre-vintage historical forecasts")


def birth(person):
    stamp = person.get("dateOfBirthTimestamp")
    if not isinstance(stamp, int) or stamp <= 0:
        return None
    return datetime.fromtimestamp(stamp, timezone.utc).date().isoformat()


def missing(reason):
    return {"available": False, "reason": reason, "features": {}, "mask": {}}


def rolling(observations, cutoff, fields, limit=10):
    rows = sorted((r for r in observations if r["date"] < cutoff),
                  key=lambda r: (r["date"], r["match_id"]))[-limit:]
    if not rows:
        return missing("no_observations_before_cutoff")
    values, mask = {}, {}
    for field in fields:
        actual = [number(row["stats"].get(field)) for row in rows]
        actual = [value for value in actual if value is not None]
        values[field] = sum(actual) / len(actual) if actual else None
        mask[field] = len(actual) > 0
    return {"available": True, "features": values, "mask": mask,
            "n_matches": len(rows), "last_observation": rows[-1]["date"],
            "method": "dated_past_10_observed_match_means"}


def mls_match_ids(directory):
    """Exclude CONCACAF/friendlies: all labels must actually be MLS matches."""
    ids = set()
    for name in ("history_manifest.json", "charlotte_history_manifest.json",
                 "opponent_history_manifest.json"):
        file = directory / name
        if not file.exists():
            continue
        data = json.loads(file.read_text())
        batches = [data] if isinstance(data, list) else [data.get("events", [])] + [
            value for key, value in data.items() if key != "events" and isinstance(value, list)
        ]
        for batch in batches:
            for event in batch:
                if not isinstance(event, dict) or event.get("id") is None:
                    continue
                tournament = event.get("tournament") or {}
                unique = tournament.get("uniqueTournament") or {}
                if unique.get("id") == 242 or unique.get("slug") == "mls":
                    ids.add(str(event["id"]))
    return ids


def load_sofa(directory, cutoff, allowed_matches=None):
    """Only confirmed lineups from matches before cutoff; never target XIs."""
    with (directory / "asof_team_stats.csv").open(newline="", encoding="utf-8-sig") as handle:
        lookup = {r["match_id"]: r for r in csv.DictReader(handle) if r["date"] < cutoff}
    players, teams, lineups, identities = defaultdict(list), defaultdict(list), defaultdict(list), {}
    for file in sorted((directory / "player_matches").glob("*.json")):
        if allowed_matches is not None and file.stem not in allowed_matches:
            continue
        metadata = lookup.get(file.stem)
        if metadata is None: continue
        sheet = json.loads(file.read_text())
        if sheet.get("confirmed") is not True: continue
        for side in ("home", "away"):
            club = metadata[side]
            observed = (sheet.get(side) or {}).get("players")
            if not isinstance(observed, list) or sum(not x.get("substitute", True) for x in observed) != 11:
                continue
            if any((item.get("player") or {}).get("id") is None for item in observed):
                continue
            day = metadata["date"]
            starts = set()
            for entry in observed:
                person = entry.get("player") or {}
                if person.get("id") is None: continue
                pid = f"sofascore:{int(person['id'])}"
                candidate = (person.get("name", ""), birth(person))
                if pid in identities and identities[pid] != candidate:
                    old = identities[pid]
                    if old[1] is not None and candidate[1] is not None and old[1] != candidate[1]:
                        raise ValueError(f"contradictory DOB for provider player ID {pid}")
                if pid not in identities or identities[pid][1] is None:
                    identities[pid] = candidate
                started = not entry.get("substitute", True)
                if started: starts.add(pid)
                stats = entry.get("statistics") or {}
                players[(club, pid)].append({"date": day, "match_id": file.stem,
                    "stats": {field: number(stats.get(field)) for field in SOFA_FIELDS},
                    "role": person.get("position", ""), "started": started,
                    "minutes": number(stats.get("minutesPlayed"))})
            lineups[club].append((day, file.stem, starts))
            team_stats = {key.removeprefix(side + "_"): number(value)
                          for key, value in metadata.items() if key.startswith(side + "_")
                          and key not in (side + "_id",)}
            teams[club].append({"date": day, "match_id": file.stem, "stats": team_stats})
    return players, teams, lineups, identities


def load_fm(path, requested_identities, requested_uids, *, complete_fm_path=None):
    pairs = defaultdict(list)
    by_uid = {}
    ambiguous_uids = set()
    with path.open(encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle):
            key = (norm(row.get("name", "")), row.get("dob", ""))
            uid = str(row.get("uid") or "")
            if key not in requested_identities and uid not in requested_uids:
                continue
            record = {"uid": uid, "name": row.get("name", ""), "dob": row.get("dob", ""),
                "kind": row.get("kind", "unknown"), "attr_source": row.get("attr_source", "none"),
                "ca": plausible_ability(row.get("ca")), "pa": plausible_ability(row.get("pa")),
                "height_cm": plausible_height(row.get("height")),
                "binary_record_source":"men_full_attrs",
                "attributes": {k.removeprefix("attr_"): number(v) for k, v in row.items()
                               if k.startswith("attr_")}}
            if key in requested_identities: pairs[key].append(record)
            if uid in requested_uids:
                if uid in by_uid:
                    # The complete FM26 export contains a handful of native
                    # UID collisions spanning distinct staff/player records.
                    # NEVER resolve one by last-row wins or by highest CA.
                    ambiguous_uids.add(uid)
                    by_uid.pop(uid, None)
                elif uid not in ambiguous_uids:
                    by_uid[uid] = record
    if complete_fm_path is not None:
        # The gender-partitioned men extract omits 27 Reep-identified FM
        # players who ARE present in the fuller binary frame. Recover these
        # by exact typed FM UID, never by guessed names (names/DOB may be
        # unavailable in the complete-v6 binary frame).
        outstanding=requested_uids-set(by_uid)-ambiguous_uids
        if outstanding:
            duplicate_fallback=set()
            with complete_fm_path.open(encoding="utf-8-sig",newline="") as handle:
                for row in csv.DictReader(handle):
                    uid=str(row.get("uid") or "")
                    if uid not in outstanding:
                        continue
                    attributes={k.removeprefix("attr_"):number(v) for k,v in row.items()
                                if k.startswith("attr_")}
                    positive=sum(v is not None and 1<=v<=20 for v in attributes.values())
                    record={"uid":uid,"name":row.get("name",""),"dob":row.get("dob",""),
                            "kind":row.get("kind","unknown"),
                            "attr_source":"db" if positive else "none",
                            "ca":plausible_ability(row.get("ca")),"pa":plausible_ability(row.get("pa")),
                            "height_cm":plausible_height(row.get("height")),
                            "attributes":attributes,
                            "binary_record_source":"complete_v6_uid_exact"}
                    if uid in by_uid:
                        duplicate_fallback.add(uid)
                        by_uid.pop(uid,None)
                    elif uid not in duplicate_fallback:
                        by_uid[uid]=record
    return pairs, by_uid


def fm_features(record, identity_evidence, prior=None):
    if record is None:
        return {"available": False, "identity_evidence": identity_evidence,
                "attribute_source": "missing", "ca": None, "pa": None, "height_cm": None,
                "technical": {}, "mental": {},
                "observed_mental_mask": {}, "reconstructed_mental_mask": {}}
    is_player = record["kind"] == "player"
    # The FM extractor wrongly labeled some genuine Reep fm:player IDs as
    # staff although they have a directly stored, complete DB attribute block.
    # Only a *unique typed Reep player ID* can rescue a nonplayer kind, and
    # only when its actual binary-derived block exists and is plausible.
    typed_reep_player = identity_evidence.startswith(
        "reep_to_local_fm_uid_nonplayer_kind_")
    valid_block_count = sum(
        1 for field in ("Anticipation","Decisions","Composure","Concentration",
                       "Leadership","Teamwork","WorkRate","Bravery","Finishing",
                       "Pace","Passing","Technique")
        if record["attributes"].get(field) is not None and
        1<=record["attributes"][field]<=20
    )
    recovered_db = (not is_player and typed_reep_player and
                    record["attr_source"] == "db" and valid_block_count>=1)
    measured = (is_player and record["attr_source"] == "db") or recovered_db
    estimated = is_player and record["attr_source"] == "derived"
    reconstructed = bool(prior and not measured and not estimated)
    attributes = record["attributes"] if measured or estimated else (
        {k: v["mean"] for k,v in prior["traits"].items()} if reconstructed else {})
    # Zero in the extracted per-player mental block often means unassigned or
    # missing. Never model it as a genuinely observed 0 on FM's 1–20 scale.
    mental = {field: (value if value is not None and value>0 else None)
              for field in MENTAL for value in (attributes.get(field),)}
    return {"available": measured or estimated or reconstructed,
            "identity_evidence": identity_evidence,
            "fm_uid": record["uid"], "record_kind": record["kind"],
            "attribute_source": ("recovered_partial_database_block_reep_fm_player_id"
                                  if recovered_db and valid_block_count<9 else
                                  "recovered_database_block_reep_fm_player_id" if recovered_db else
                                  "database_block" if measured else
                                  "reconstructed_distribution" if estimated else
                                  "generated_conditional_prior" if reconstructed else
                                  "untrusted_record_kind"),
            "attribute_uncertainty":prior["traits"] if reconstructed else None,
            "recovered_source_block_valid_count":valid_block_count if recovered_db else None,
            "reconstruction_conditioning":prior["conditioning"] if reconstructed else None,
            "ca": record["ca"] if (is_player or recovered_db) else
                  prior["conditioning"]["ca"] if reconstructed else None,
            "pa": record.get("pa") if is_player or recovered_db else None,
            "height_cm": record.get("height_cm") if is_player or recovered_db else None,
            "mental": mental, "technical": {k: v for k, v in attributes.items() if k not in MENTAL},
            "observed_mental_mask": {k: measured and v is not None and v > 0 for k, v in mental.items()},
            "reconstructed_mental_mask": {k: (estimated or reconstructed) and v is not None and v > 0
                                          for k,v in mental.items()}}


def epl_native_attribute_admissible(official_name, native, identity_evidence=None):
    """Do not feed attributes merely because Reep and FM publish the same UID.

    The complete-v6 binary frame contains UIDs, CA and traits, but often no
    name/DOB. Several Reep-linked UIDs also belong to entirely different
    names in the gender-specific extraction. Identity discovery and trait
    admissibility are independent gates. Returning False keeps the canonical
    identity, but clears all individual FM attribute inputs.
    """
    if native is None:
        return False
    identity_evidence=identity_evidence or {}
    if identity_evidence.get("fm_native_record_trusted_for_this_player") is False:
        return False
    try:
        from resolve_epl_reep_first import source_name_support
    except ModuleNotFoundError:
        # The bundle may also be imported as scripts.build_unified_prematch_bundle
        # by a test or another package, rather than executed as a CLI script.
        from scripts.resolve_epl_reep_first import source_name_support
    actual_name=native.get("name") or ""
    if not actual_name:
        return False
    if not source_name_support(official_name,actual_name):
        # Distinct provider-spelling variants (e.g., Phil/Philip or Ukrainian
        # transliteration) can have different names yet the native player is
        # independently verified. Allow only an explicit affirmative audit
        # grounded in BOTH the native name AND a matching native/TM DOB.
        audit=identity_evidence.get("identity_evidence") or {}
        if not (
            identity_evidence.get("fm_native_record_trusted_for_this_player") is True
            and audit.get("native_dob_agrees_with_tm") is True
            and (audit.get("native_name_evidence_tier") or 0)>=1
        ):
            return False
    # An explicit audited native-name/DOB confirmation is stronger evidence.
    # Name compatible + typed Reep ID is provisional; never mislabel it as
    # issuer-verified official registration.
    return True


def load_provider_observations(path, cutoff, provider, provider_key):
    """Strictly earlier dated observations with independently known publication time.

    Reep supplies provider ID bridges; it does not certify the *data* exists.
    A hypothetical future player valuation/FBref row must never enter as-of
    training or be converted to a missing-is-zero feature.
    """
    out = defaultdict(list)
    if path is None:
        return out
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            row = json.loads(line)
            if row.get("provider") != provider or not row.get(provider_key):
                raise ValueError(f"expected explicit {provider} provider identity")
            if not isinstance(row.get("features"), dict) or not row.get("published_at") or not row.get("date"):
                raise ValueError(f"{provider} row missing timestamp or features")
            observed = date.fromisoformat(row["date"])
            published = date.fromisoformat(row["published_at"][:10])
            if published < observed:
                raise ValueError(f"{provider} row published before observation date")
            if observed.isoformat() >= cutoff or published.isoformat() >= cutoff:
                continue
            values = {str(k): number(v) for k, v in row["features"].items()}
            out[str(row[provider_key])].append({
                "date": observed.isoformat(), "published_at": published.isoformat(),
                "match_id": str(row.get("match_id") or ""), "stats": values,
            })
    return out


def psychology_vector(fm):
    """Maps only genuinely available FM trait proxies; hidden traits stay null."""
    # 'ImportantMatches' is not a direct measurement of pressure resilience;
    # 'Dirtiness' is not the full psychological construct of discipline.
    sources = {"consistency": "Consistency", "leadership": "Leadership",
               "teamwork": "Teamwork"}
    estimates = {}
    for trait in PSYCHOLOGY_TRAITS:
        field = sources.get(trait)
        raw = fm["mental"].get(field) if field else None
        measured = bool(field and fm["observed_mental_mask"].get(field))
        reconstructed = bool(field and fm["reconstructed_mental_mask"].get(field))
        estimates[trait] = {"value": (raw - 10.5) / 9.5 if raw is not None and raw > 0
                            and (measured or reconstructed) else None,
                            "confidence": 1.0 if measured else 0.15 if reconstructed else 0.0,
                            "source_trait": field if measured or reconstructed else None,
                            "source_state": "fm_db" if measured else "fm_reconstructed" if reconstructed else "missing"}
    return {"primitive": "footy-state::PsychologyVector",
            "stable_traits": estimates,
            "other_mental_inputs": {k: fm["mental"].get(k) for k in MENTAL},
            "dynamic_morale": None, "self_belief": None, "motivation": None,
            "reason_dynamic_missing": "no independent dated direct morale observation"}


def form_window(observations, cutoff, horizon_days=365, half_life_days=60):
    """PerformanceWindow-style rate, EWMA, trend and observed exposure."""
    end = date.fromisoformat(cutoff)
    history = [r for r in observations
               if 0 < (end - date.fromisoformat(r["date"])).days <= horizon_days]
    history.sort(key=lambda r: (r["date"], r["match_id"]))
    if not history:
        return missing("no_pre_cutoff_form_history")
    metrics = {}
    for key in ("totalShots", "goals", "goalAssist", "expectedGoals", "expectedAssists",
                "keyPass", "onTargetScoringAttempt"):
        entries = [(r, number(r["stats"].get(key))) for r in history]
        entries = [(r, value) for r, value in entries if value is not None
                   and (r.get("minutes") or 0) > 0]
        if not entries:
            metrics[key] = {"per90": None, "ewma_per90": None, "trend_per90": None,
                            "rate_variance": None, "observations": 0}
            continue
        exposure = sum(r["minutes"] for r, _ in entries)
        rate = sum(v for _, v in entries) * 90 / exposure
        numerator = denominator = 0.0
        by_age = [[], []]
        sample_rates = []
        for row, value in entries:
            age = (end - date.fromisoformat(row["date"])).days
            weight = math.exp(-math.log(2) * age / half_life_days)
            numerator += value * weight
            denominator += row["minutes"] * weight
            sample_rates.append(value * 90 / row["minutes"])
            by_age[int(age >= horizon_days / 2)].append((row["minutes"], value))
        recent = [sum(v for _, v in group) * 90 / sum(m for m, _ in group)
                  if group else None for group in by_age]
        metrics[key] = {"per90": rate, "ewma_per90": 90 * numerator / denominator,
                        "trend_per90": recent[0] - recent[1] if all(v is not None for v in recent) else None,
                        "rate_variance": (sum((v - sum(sample_rates) / len(sample_rates)) ** 2
                                              for v in sample_rates) / len(sample_rates)
                                          if len(sample_rates) > 1 else None),
                        "observations": len(entries)}
    minutes = sum(r.get("minutes") or 0 for r in history)
    return {"available": True, "primitive": "footy-performance::PerformanceWindow",
            "horizon_days": horizon_days, "half_life_days": half_life_days,
            "n_listings": len(history), "minutes": minutes,
            "low_sample": minutes < 450, "metrics": metrics,
            "last_date": history[-1]["date"]}


def observed_roles(observations, cutoff):
    actual = [r for r in observations if r["date"] < cutoff and r.get("started")]
    counts = Counter(str(r.get("role") or "").upper()[:1] for r in actual)
    counts = {k: n for k, n in counts.items() if k in ("G", "D", "M", "F")}
    total = sum(counts.values())
    return {"available": total > 0, "primitive": "footy-state::RoleVector",
            "source": "previous_confirmed_starts", "n_starts": total,
            "roles": {r: counts.get(r, 0) / total for r in ("G", "D", "M", "F")}
            if total else {},
            "tactical_fit": None,
            "tactical_fit_reason": "requires dated manager tactical requirement/formation"}


def value_primitive_features(fm, dob, position, form, market_obs, cutoff):
    """Preserve all 47 SURGE/FORECAST feature positions without filling fakes."""
    row = {name: None for name in SURGE_FEATURE_NAMES}
    if dob:
        age = (date.fromisoformat(cutoff) - date.fromisoformat(dob)).days / 365.2425
        if 15 <= age <= 55:
            row["age"] = age
            row["age_sq"] = age * age
    if position["available"]:
        for source, feature in (("G", "pos_gk"), ("D", "pos_df"), ("M", "pos_mf")):
            row[feature] = position["roles"].get(source, 0.0)
    if fm["available"]:
        row["fm_present"] = 1.0
        row["fm_ca"] = fm["ca"]
        row["fm_pa"] = fm.get("pa") if (fm.get("pa") or 0) > 0 else None
        if row["fm_ca"] is not None and row["fm_pa"] is not None:
            row["fm_pa_gap"] = row["fm_pa"] - row["fm_ca"]
        row["fm_attr_db"] = float(fm["attribute_source"] == "database_block")
        row["fm_attr_derived"] = float(fm["attribute_source"] == "reconstructed_distribution")
        row["height_cm"] = plausible_height(fm.get("height_cm"))
        categories = {
            "technical": ("Technique", "FirstTouch", "Passing", "Dribbling"),
            "physical": ("Pace", "Acceleration", "Stamina", "Strength"),
            "mental": ("Anticipation", "Decisions", "Teamwork", "WorkRate", "Composure"),
            "defensive": ("Marking", "Tackling", "Positioning"),
            "attacking": ("Finishing", "OffTheBall", "Dribbling"),
            "creative": ("Vision", "Passing", "Flair"),
        }
        for group, names in categories.items():
            vals = [fm["technical"].get(field, fm["mental"].get(field)) for field in names]
            if all(v is not None and v > 0 for v in vals):
                row[f"fm_sig_{group}"] = sum(vals) / len(vals)
    if form["available"]:
        row["perf_present_730"] = 1.0
        row["log_mins_365"] = math.log1p(form["minutes"])
        row["log_apps_365"] = math.log1p(form["n_listings"])
        for src, dest in (("goals", "g_per90_365"), ("goalAssist", "a_per90_365")):
            row[dest] = form["metrics"][src]["per90"]
    market = sorted(market_obs, key=lambda r: (r["date"], r["published_at"]))
    if market:
        valid = [(date.fromisoformat(r["date"]), number(r["stats"].get("market_value_eur"))) for r in market]
        valid = [(d, v) for d, v in valid if v is not None and v > 0]
        if valid:
            now, price = valid[-1]
            row["log_mv"] = math.log(price)
            row["log_n_vals_prior"] = math.log1p(len(valid))
            row["log_days_since_last_val"] = math.log1p((date.fromisoformat(cutoff) - now).days)
            row["log_days_since_first_val"] = math.log1p((date.fromisoformat(cutoff) - valid[0][0]).days)
            row["log_peak_prior_ratio"] = math.log(price / max(v for _, v in valid))
            row["log_min_prior_ratio"] = math.log(price / min(v for _, v in valid))
            for horizon, field in ((180, "log_mv_180d_ratio"), (365, "log_mv_365d_ratio")):
                older = [v for d, v in valid if (now - d).days >= horizon]
                if older:
                    row[field] = math.log(price / older[-1])
            if len(valid) > 1:
                row["last_change_dir"] = float(price > valid[-2][1]) - float(price < valid[-2][1])
                returns = [math.log(v / old) for (_, old), (_, v) in zip(valid, valid[1:])]
                mean = sum(returns) / len(returns)
                row["mv_vol_log"] = (sum((x - mean) ** 2 for x in returns) / len(returns)) ** 0.5
    return {"primitive": "footy-recruitment::surge::FORECAST_FEATURE_NAMES",
            "features": row, "mask": {key: value is not None for key, value in row.items()},
            "notes": "market changes use only previous verified TM-ID observations; future market-value forecast is NOT an input"}


def mental_aggregate(candidates):
    """Keep measured/derived aggregates separate, not fictional XI averages."""
    out = {}
    for field in MENTAL:
        parts = {}
        for provenance, mask in (("measured", "observed_mental_mask"),
                                 ("reconstructed", "reconstructed_mental_mask")):
            values = [p["fm26"]["mental"][field] for p in candidates
                      if p["fm26"][mask].get(field)]
            parts[provenance] = {"n": len(values), "mean": sum(values) / len(values) if values else None,
                "variance": (sum((v - sum(values) / len(values)) ** 2 for v in values) / len(values)
                             if values else None)}
        out[field] = parts
    return {"interpretation": "registered_or_recently_observed_candidates_not_confirmed_XI",
            "traits": out}


def chemistry(candidates, previous_lineups, provider_to_canonical=None):
    """Prior co-start counts are observational team familiarity, not causality."""
    ids = {p["canonical_player_id"] for p in candidates}
    pairs = Counter()
    seen = Counter()
    provider_to_canonical = provider_to_canonical or {}
    for _, _, lineup in previous_lineups:
        subset = sorted({provider_to_canonical.get(pid, pid) for pid in lineup} & ids)
        for uid in subset: seen[uid] += 1
        for i, left in enumerate(subset):
            for right in subset[i + 1:]: pairs[(left, right)] += 1
    return {"available": bool(previous_lineups), "method": "prior_confirmed_co_starts",
            "pair_edges": [{"left": a, "right": b, "shared_starts": n,
                "normalized_shared_starts": n / min(seen[a], seen[b])} for (a, b), n in sorted(pairs.items())]}


def player_usage(observations, club_matches, cutoff):
    """As-of `footy-state::UsageVector` and Bayesian participation priors."""
    today = date.fromisoformat(cutoff)
    records = [r for r in observations if r["date"] < cutoff]
    if not records:
        return missing("no_previous_provider_observations")
    first = min(r["date"] for r in records)
    match_days = {(day, mid) for day, mid, _ in club_matches if first <= day < cutoff}
    n_club = len(match_days)
    if n_club < 1:
        return missing("no_previous_fully_covered_club_fixtures")
    apps = sum((r.get("minutes") or 0) > 0 for r in records)
    starts = sum(r["started"] for r in records)
    match_count = max(n_club, starts)
    return {"available": True, "primitive": "footy-state::UsageVector",
            "minutes_30": sum(r.get("minutes") or 0 for r in records if (today - date.fromisoformat(r["date"])).days <= 30),
            "minutes_90": sum(r.get("minutes") or 0 for r in records if (today - date.fromisoformat(r["date"])).days <= 90),
            "minutes_365": sum(r.get("minutes") or 0 for r in records if (today - date.fromisoformat(r["date"])).days <= 365),
            "starts_365": starts, "club_match_exposure": match_count,
            "availability_ratio_observed_not_medical": min(1.0, apps / match_count),
            "start_prior": min(0.99, (starts + 1) / (match_count + 2)),
            "appearance_prior": min(0.995, max((starts + 1) / (match_count + 2),
                                                (apps + 1.5) / (match_count + 2))),
            "last_listed": max(r["date"] for r in records)}


def team_sources(club, team_rows, candidates, starts, cutoff, identity_alias):
    team_stats = rolling(team_rows, cutoff, (
        "xg", "goals", "sot", "possession", "passes_acc", "big_chances",
        "corners", "fouls", "yellow", "red"), 10)
    prior_lineups = sorted(((day, mid, lineup) for day, mid, lineup in starts if day < cutoff),
                           key=lambda r: (r[0], r[1]))[-10:]
    chemistry_data = chemistry(candidates, prior_lineups, identity_alias)
    return {
        "club": club, "n_candidates": len(candidates),
        "team_performance": team_stats,
        "mental_traits_distribution": mental_aggregate(candidates),
        "chemistry": chemistry_data,
        "collective_morale": {"observed": None, "uncertainty": "latent_prior_required"},
        "manager_tactical_requirement": missing("no_verified_asof_manager_formation_data"),
        "squad_value": missing("no_complete_verified_asof_transfermarkt_market_series"),
        "primitive_sources": ["footy-performance::TeamPerformanceWindow",
                              "footy-match::active_XI_cohesion", "footy-state::PsychologyVector",
                              "footy-recruitment::SquadState"],
        "source_caveat": "candidate roster, not a confirmed matchday lineup or real psychological measurement",
    }


def build_bundle(league, cutoff, *, roster, reep, sofa_root, fm_path, fbref_path=None,
                 market_path=None, fm_attr_priors=None, fm_complete_path=None):
    fm_vintage_allowed(cutoff)
    if league not in ("EPL", "MLS"):
        raise ValueError("only EPL and MLS supported")
    if league == "EPL" and (roster is None or roster["as_of"] > cutoff):
        raise ValueError("EPL registration snapshot missing or newer than cutoff")
    if league == "EPL" and (set(roster["teams"]) != set(TEAM_ALIASES)
                            or roster.get("roster_scope") == "fixture_subset"):
        raise ValueError("EPL feature bundle requires the complete 20-club official registration; regenerate without --fixtures")
    identity_versions=("reep_first_all_20_clubs_v1","canonical_reep_first_v2")
    if not reep.get("stamp") or "sofascore_reep_links_all" not in reep:
        raise ValueError("Reep audit must contain direct-bridge v4 identity contract")
    all_sofa_reep = {item["sofascore_id"]: item for item in reep["sofascore_reep_links_all"]}
    linked_fm = {item["sofascore_id"]: item for item in reep["sofascore_fm_links"]}
    epl_links = {(p["club"], p["official_name"], str(p["fm_uid"])): p
                 for p in reep["epl_fm_reep_links"]}

    if league == "MLS":
        mls_ids = mls_match_ids(sofa_root)
        if not mls_ids:
            raise ValueError("no identified MLS tournament fixture IDs; refuse mislabeled data")
        sofa_players, sofa_teams, starts, sofa_identities = load_sofa(sofa_root, cutoff, mls_ids)
        # Do not carry former players into another club after their latest
        # observed transfer/listing; memberships are evidence-dated.
        most_recent_club = {}
        for (club, pid), records in sofa_players.items():
            latest = max((r["date"], r["match_id"]) for r in records)
            if pid not in most_recent_club or latest > most_recent_club[pid][0]:
                most_recent_club[pid] = (latest, club)
        # Preserve every most-recent-club assignment, even when the latest
        # observation is stale (>120d) or the player is injured/seldom-used.
        # Recency is a dated feature (days_since_last_observed), never a
        # roster-deletion rule. This restores all 30 MLS clubs.
        candidate_ids = {(club, pid) for (club, pid) in sofa_players
                         if most_recent_club[pid][1] == club}
        requested_uids = {str(link["fm_uids"][0]) for sid, link in linked_fm.items()
                          if link["local_fm_status"] in (
                              "linked_player_with_db_attributes_and_matching_dob",
                              "linked_player_with_reconstructed_attributes_and_matching_dob")
                          and len(link["fm_uids"]) == 1}
    else:
        sofa_players, sofa_teams, starts, sofa_identities = {}, {}, {}, {}
        candidate_ids = set()
        requested_uids = {str(p["fm_uid"]) for team in roster["teams"].values()
                          for p in team["players"] if p.get("fm_uid")}
    _, local_fm = load_fm(
        fm_path,set(),requested_uids,
        complete_fm_path=(fm_complete_path if league=="EPL" and
                          roster.get("identity_rebuild") in identity_versions else None))
    fbref = load_provider_observations(fbref_path, cutoff, "fbref", "fbref_person_id")
    transfermarkt = load_provider_observations(market_path, cutoff, "transfermarkt", "transfermarkt_player_id")

    candidates = defaultdict(list)
    quality = Counter()
    alias = {}
    if fm_attr_priors is not None:
        if league!="EPL" or roster.get("identity_rebuild") not in identity_versions:
            raise ValueError("conditional FM priors require Reep-first 20-club EPL identities")
        if fm_attr_priors.get("schema")!="fm26_missing_trait_conditional_priors_v1":
            raise ValueError("unknown/unsupported generated FM attribute prior source")
        priors_by_uid = {}
        for item in fm_attr_priors["priors"]:
            uid=str(item["fm_uid"])
            if uid in priors_by_uid:
                raise ValueError("duplicated generated FM trait identity")
            priors_by_uid[uid]=item
    else:
        priors_by_uid={}
    if league == "EPL":
        for club, team in roster["teams"].items():
            for p in team["players"]:
                uid = str(p.get("fm_uid") or "")
                reep_first = roster.get("identity_rebuild") in identity_versions
                bridge = None if reep_first else epl_links.get((club, p["official_name"], uid))
                # A Reep/FM bridge verifies an FM ID, not a name-only official
                # registration -> FM assignment. Keep the official namespace.
                canonical = f"official_epl:{hash_id(roster['as_of'], club, p['official_name'])}"
                record = local_fm.get(uid)
                assert record is None or record["uid"] == uid
                # A Reep FM provider ID can map to a *different native name*
                # in this imperfect FM export. Keep the candidate UID for
                # review, but NEVER feed another person's mental/CA traits
                # into the match model. The guard is supplied only by the
                # independent FM/Transfermarkt/DOB identity reconciliation.
                native_trust = p.get("fm_native_record_trusted_for_this_player")
                # Every EPL roster path—legacy provisional matching, the
                # Reep-first audit, and later independent FM/TM recovery—
                # must pass the same native-record/person corroboration gate.
                # A missing explicit trust flag is NOT itself a positive
                # identity/trait assertion.
                native_admissible = epl_native_attribute_admissible(
                    p["official_name"], record, p)
                if not native_admissible:
                    record = None
                state = (
                    p.get("identity_state") or p.get("identity_verification") or "unresolved_reep_search"
                    if reep_first else
                    (p.get("identity_verification") if record else "no_local_fm_record")
                    or "unverified_official_to_fm_name_join"
                )
                prior=priors_by_uid.get(uid) if native_admissible else None
                if prior and (
                    record is None or
                    prior["reep_id"]!=p.get("reep_id_candidate") or
                    prior["official_name"]!=p["official_name"] or
                    prior["club"]!=club or
                    prior["fm_uid"]!=uid
                ):
                    raise ValueError("prior candidate does not match unique Reep FM identity")
                feature = fm_features(record, state, prior=prior)
                if reep_first:
                    feature["candidate_fm_uid_held_separately"]=uid or None
                    feature["native_trait_admission"]=(
                        "name_corrob_native_id_provisional"
                        if native_admissible else
                        "unverified_or_conflicting_native_identity_traits_missing"
                    )
                if record is not None:
                    feature["binary_record_source"]=record.get("binary_record_source")
                # The native FM record may be linked to Reep by a typed ID,
                # but the official league list contains names, not person
                # DOB/provider IDs. Do not upgrade the official-roster join
                # to verified-player status without separate corroboration.
                feature["identity_verified_for_real_player"] = False
                if reep_first:
                    hinted = p.get("provider_ids_candidate") or {}
                    person_evidence=p.get("identity_evidence") or {}
                    source_ids = {
                        "reep_id_candidate": p.get("reep_id_candidate") or p.get("reep_id"),
                        "fm26_uid_candidate": uid or None,
                        "fbref_person_ids_candidate": hinted.get("fbref", []),
                        "sofascore_ids_candidate": hinted.get("sofascore", []),
                        "transfermarkt_player_ids_candidate": hinted.get("transfermarkt", []) or
                            ([str(person_evidence["tm_player_id"])] if person_evidence.get("tm_player_id") else []),
                        "official_to_reep_identity_issuer_verified": False,
                    }
                else:
                    source_ids = {
                        "reep_id_candidate": (p.get("reep_id") or bridge["reep_id"] if bridge else p.get("reep_id")),
                        "fm26_uid_candidate": uid or None,
                        "fbref_person_ids_candidate": bridge.get("fbref_person_ids", []) if bridge else [],
                        "transfermarkt_player_ids_candidate": bridge.get("transfermarkt_player_ids", []) if bridge else [],
                    }
                dob = record["dob"] if record else (
                    (p.get("identity_evidence") or {}).get("tm_birth_date")
                    if p.get("identity_evidence") else None)
                candidate = player_bundle_row(canonical, club, p["official_name"], "EPL", cutoff,
                                              feature, source_ids, dob, [], [], [], [],
                                              identity_state=state)
                candidate["registration"] = p["registration"]
                candidate["loaned_out"] = bool(p.get("loaned_out"))
                if reep_first:
                    candidate["multi_club_eligibility_unresolved"]=bool(
                        p.get("multi_club_eligibility_unresolved"))
                candidate["epl_fm_identity_evidence"] = p.get("transfermarkt_evidence") or {
                    "source": p.get("match_method"),
                    "verification": p.get("identity_verification")}
                candidates[club].append(candidate)
                quality[state] += 1
    else:
        for club, sid in sorted(candidate_ids):
            raw_sid = sid.removeprefix("sofascore:")
            bridge = all_sofa_reep.get(raw_sid)
            fm_link = linked_fm.get(raw_sid)
            fm_status = fm_link.get("local_fm_status") if fm_link else "no_local_fm_link"
            approved = fm_link and fm_status in (
                "linked_player_with_db_attributes_and_matching_dob",
                "linked_player_with_reconstructed_attributes_and_matching_dob")
            fm_uid = fm_link["fm_uids"][0] if approved else None
            fm_record = local_fm.get(fm_uid) if fm_uid else None
            canonical = "reep:" + bridge["reep_id"] if bridge else sid
            if any(existing["canonical_player_id"] == canonical for existing in candidates[club]):
                raise ValueError("duplicate canonical player in one club: " + canonical)
            alias[sid] = canonical
            names, birth_date = sofa_identities[sid]
            if approved and (fm_record is None or fm_record["dob"] != birth_date):
                raise ValueError("approved FM bridge disagrees with local FM DOB")
            feature = fm_features(fm_record, fm_status)
            feature["identity_verified_for_real_player"] = bool(approved)
            source_ids = {"sofascore_id": raw_sid, "reep_id": bridge["reep_id"] if bridge else None,
                          "fm26_uid": fm_uid,
                          "fbref_person_ids": bridge.get("fbref_person_ids", []) if bridge else [],
                          "transfermarkt_player_ids": bridge.get("transfermarkt_player_ids", []) if bridge else []}
            historical = sofa_players[(club, sid)]
            fbref_hist = [r for fb in source_ids["fbref_person_ids"] for r in fbref.get(fb, [])]
            market_hist = [r for tm in source_ids["transfermarkt_player_ids"] for r in transfermarkt.get(tm, [])]
            candidate = player_bundle_row(canonical, club, names, "MLS", cutoff, feature,
                                          source_ids, birth_date, historical, starts[club],
                                          fbref_hist, market_hist, identity_state=fm_status)
            candidates[club].append(candidate)
            quality[fm_status] += 1

    result_teams = {}
    for club, members in sorted(candidates.items()):
        members.sort(key=lambda p: p["canonical_player_id"])
        team_inputs = team_sources(club, sofa_teams.get(club, []), members,
                                   starts.get(club, []), cutoff, alias)
        for member in members:
            member["model_input"] = flatten_model_features(member, team_inputs)
        result_teams[club] = {
            "inputs": team_inputs,
            "players": members,
        }
    feature_sets = {tuple(sorted(p["model_input"]["features"])) for team in result_teams.values()
                    for p in team["players"]}
    if len(feature_sets) != 1:
        raise ValueError("incompatible EPL/MLS player feature columns")
    return {"schema": "unified_epl_mls_player_set_v1", "forecast_cutoff_date": cutoff,
            "league": league, "model_status": "feature_contract_only_not_a_trained_or_calibrated_model",
            "roster_scope": "full_league" if league == "EPL" else "historically_observed_clubs",
            "roster_registration_source": roster.get("registration_source") if league == "EPL" else None,
            "roster_registration_published": roster.get("registration_source_published") if league == "EPL" else None,
            "identity_register": {"source": "Reep", "stamp": reep["stamp"], "sha256": reep["sha256"]},
            "input_modalities": ["sofascore_player", "sofascore_team", "fbref_player",
                                 "fm26_ability_mental", "mental_state", "player_form",
                                 "usage_participation", "tactical_roles", "team_co_starts",
                                 "recruitment_value_features", "market_value_history",
                                 "transfer_risk", "environment_fit", "development_state"],
            "source_policy": "all observation dates strictly before forecast date; provider IDs typed; missing != zero",
            "missing_data_policy": "null+mask+provenance; do not impute zeros or create pseudo-psychology",
            "model_feature_schema": {"numeric_columns": sorted(next(iter(feature_sets))),
                                     "feature_count": len(next(iter(feature_sets))),
                                     "unit_of_observation": "one asof eligible player per latest evidenced club",
                                     "network": "dated co-start graph, not a fitted latent compatibility score",
                                     "training": "one player-set model across EPL/MLS; no fitted weights in this bundle"},
            "primitive_registry": {
                "observed_or_available": ["footy-state::PlayerState/AbilityVector/PsychologyVector",
                                          "footy-performance::PerformanceWindow/TeamPerformanceWindow",
                                          "footy-state::UsageVector/RoleVector/DevelopmentVector",
                                          "footy-recruitment::surge::FORECAST_FEATURE_NAMES",
                                          "footy-match::cohesion_co_starts",
                                          "footy-forecast::next_match_participation_and_shot_rate_inputs"],
                "eligible_but_unobserved": ["footy-recruitment::EnvironmentFit/tactical_fit/squad_complementarity",
                                            "footy-market::TransferHazard/MarketCalibration",
                                            "footy-state::dynamic_morale/self_belief/motivation",
                                            "quarcc-factor-primitives::HMM/GARCH_regime_features"],
                "excluded_future_targets": ["footy-recruitment::CandidateAnalysis::future_12m/24m/36m",
                                            "footy-recruitment::historical_analogues::future_return",
                                            "realized_fixture_score_or_lineup",
                                            "unpublished_fbref_or_market_snapshots"]},
            "identity_quality_counts": dict(quality),
            "team_count": len(result_teams), "player_count": sum(len(t["players"]) for t in result_teams.values()),
            "source_counts": {"fbref_rows_prior": sum(len(v) for v in fbref.values()),
                              "market_rows_prior": sum(len(v) for v in transfermarkt.values()),
                              "sofascore_club_player_histories": len(sofa_players),
                              "fm_generated_priors_count":len(priors_by_uid)},
            "teams": result_teams}


def player_bundle_row(canonical, club, name, league, cutoff, fm, ids, dob,
                      history, team_lineups, fbref_history, market_history, *, identity_state):
    form = form_window(history, cutoff)
    roles = observed_roles(history, cutoff)
    usage = player_usage(history, team_lineups, cutoff)
    fbref_features = (rolling(fbref_history, cutoff, FBREF_FIELDS, 10)
                      if fbref_history else missing("no_verified_fbref_stats_archive"))
    value = value_primitive_features(fm, dob, roles, form, market_history, cutoff)
    development = {"primitive": "footy-state::DevelopmentVector",
                   "ca_realization": (fm["ca"] / fm["pa"] if fm.get("ca") is not None
                                       and fm.get("pa") is not None and fm["pa"] > 0 else None),
                   "ability_velocity_per_year": None, "ability_acceleration_per_year2": None,
                   "reason_missing_velocity": "requires dated multiple FM ability snapshots"}
    tactical = {"primitive": "footy-recruitment::tactical_fit/squad_complementarity",
                "role_vector": roles, "tactical_fit": None,
                "environment_fit": {"adaptation": None, "league_transition": None,
                                    "language_culture": None,
                                    "playing_time_path": usage.get("appearance_prior") if usage.get("available") else None},
                "reason": "no pre-match manager requirement or measured cultural fit; minutes history only informs playing-time path"}
    return {"canonical_player_id": canonical, "club": club, "league": league,
            "name_display": name, "identity_state": identity_state,
            "provider_ids": ids, "source_coverage": {
                "sofascore": bool(history), "fbref": fbref_features.get("available", False),
                "fm26": fm.get("available", False), "market_value": bool(market_history),
                "direct_psychology": False},
            "fm26": fm, "psychology": psychology_vector(fm),
            "sofascore_player": rolling(history, cutoff, SOFA_FIELDS),
            "fbref_player": fbref_features,
            "form_state": form, "usage_participation": usage,
            "tactical_and_environment_fit": tactical,
            "development": development,
            "market_and_value_features": value,
            "transfer_hazard": {"value": None, "reason": "no asof transfer hazard observation or trained model"},
            "value_forecast_12m": None, "value_forecast_24m": None, "value_forecast_36m": None,
            "value_target_policy": "future value is a prediction target, never a current-match feature"}


def flatten_model_features(player, team):
    """One stable numeric/missingness contract across EPL and MLS.

    No train-set normalization or learned imputation is applied at ingestion.
    Team/pair edges remain as independently typed inputs for a set/graph model.
    """
    fm = player["fm26"]
    numbers = {}
    for key in MENTAL:
        numbers[f"fm_mental_{key}"] = fm["mental"].get(key)
        numbers[f"fm_mental_{key}_observed"] = int(bool(fm["observed_mental_mask"].get(key)))
        numbers[f"fm_mental_{key}_reconstructed"] = int(bool(fm["reconstructed_mental_mask"].get(key)))
    for key in ("ca", "pa", "height_cm"):
        numbers[f"fm_{key}"] = fm.get(key)
    for key in ("Finishing", "Pace", "Acceleration", "Stamina", "Strength", "Vision",
                "Passing", "Tackling", "Marking", "Positioning", "Technique",
                "FirstTouch", "Dribbling", "OffTheBall", "Reflexes"):
        numbers[f"fm_attr_{key}"] = fm["technical"].get(key)
    for trait in PSYCHOLOGY_TRAITS:
        numbers[f"psych_{trait}"] = player["psychology"]["stable_traits"][trait]["value"]
    for key in SOFA_FIELDS:
        numbers[f"sofa_{key}_recent10_mean"] = player["sofascore_player"]["features"].get(key)
    for key in FBREF_FIELDS:
        numbers[f"fbref_{key}_recent10_mean"] = player["fbref_player"]["features"].get(key)
    for key in ("totalShots", "goals", "goalAssist", "expectedGoals", "expectedAssists",
                "keyPass", "onTargetScoringAttempt"):
        stat = player["form_state"].get("metrics", {}).get(key, {})
        for metric in ("per90", "ewma_per90", "trend_per90", "rate_variance"):
            numbers[f"form_{key}_{metric}"] = stat.get(metric)
    for key in ("minutes_30", "minutes_90", "minutes_365", "starts_365",
                "availability_ratio_observed_not_medical", "start_prior", "appearance_prior"):
        numbers[f"usage_{key}"] = player["usage_participation"].get(key)
    roles = player["tactical_and_environment_fit"]["role_vector"].get("roles", {})
    for key in ("G", "D", "M", "F"):
        numbers[f"role_prior_{key}"] = roles.get(key) if roles else None
    numbers["development_ca_realization"] = player["development"]["ca_realization"]
    for key in SURGE_FEATURE_NAMES:
        numbers[f"value_primitive_{key}"] = player["market_and_value_features"]["features"][key]
    for key in TEAM_FIELDS:
        numbers[f"team_sofa_{key}_recent10_mean"] = team["team_performance"]["features"].get(key)
    for key in MENTAL:
        for origin in ("measured", "reconstructed"):
            dist = team["mental_traits_distribution"]["traits"][key][origin]
            numbers[f"team_{origin}_mental_{key}_mean"] = dist["mean"]
            numbers[f"team_{origin}_mental_{key}_count"] = dist["n"]
    # Stable column universe, null is missing and never implicitly observed 0.
    if any(value is not None and (not isinstance(value, (int, float)) or not math.isfinite(value))
           for value in numbers.values()):
        raise ValueError("non-finite model feature")
    return {"schema": "unified_player_numeric_features_v1", "features": numbers,
            "observed_mask": {key: value is not None for key, value in numbers.items()},
            "categorical": {"league": player["league"], "fm_attribute_source": fm["attribute_source"],
                            "identity_state": player["identity_state"]},
            "graph_input": "team.inputs.chemistry.pair_edges",
            "imputation_policy": "no imputation; train-only normalization and missing-mask handling required"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--league", choices=("EPL", "MLS"), required=True)
    parser.add_argument("--cutoff", required=True, help="YYYY-MM-DD, local date, strictly after latest source observations")
    parser.add_argument("--reep-audit", type=Path, default=ROOT / "data/identity/reep/coverage_full_all_sources_v1.json")
    parser.add_argument("--epl-roster", type=Path, default=ROOT / "data/identity/reep/epl_full_all_sources_v1.json")
    parser.add_argument("--sofascore-root", type=Path, default=ROOT / "data/sofascore_cfm")
    parser.add_argument("--fm-file", type=Path, default=Path("/home/drewclaw/fm26-data/fm26_men_full_attrs.csv"))
    parser.add_argument("--fm-complete-file", type=Path,
                        default=Path("/home/drewclaw/fm26-data/fm26_complete_v6.csv"))
    parser.add_argument("--fbref-observations", type=Path)
    parser.add_argument("--market-observations", type=Path)
    parser.add_argument("--fm-attr-priors", type=Path,
                        help="immutable FM26 conditional distribution priors keyed by unique Reep-to-FM ID")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        date.fromisoformat(args.cutoff)
        if args.output.exists():
            raise ValueError("refusing to overwrite a prior model-input feature snapshot")
        reep = json.loads(args.reep_audit.read_text())
        roster = json.loads(args.epl_roster.read_text()) if args.league == "EPL" else None
        priors=json.loads(args.fm_attr_priors.read_text()) if args.fm_attr_priors else None
        bundle = build_bundle(args.league, args.cutoff, roster=roster, reep=reep,
                              sofa_root=args.sofascore_root, fm_path=args.fm_file,
                              fbref_path=args.fbref_observations, market_path=args.market_observations,
                              fm_attr_priors=priors, fm_complete_path=args.fm_complete_file)
        if args.fm_attr_priors:
            bundle["generated_fm_traits_source_sha256"]=hashlib.sha256(
                args.fm_attr_priors.read_bytes()).hexdigest()
            bundle["generated_fm_traits_published_vintage_note"]=(
                "FM26 empirical conditional priors compiled later from March-2026 "
                "static raw attributes; research replay, not a prospective pre-release forecast.")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        content = json.dumps(bundle, sort_keys=True, ensure_ascii=False, allow_nan=False) + "\n"
        args.output.write_text(content, encoding="utf-8")
        print(json.dumps({k: v for k, v in bundle.items() if k not in ("teams", "input_modalities")}, indent=2))
    except (ValueError, KeyError, OSError) as exc:
        parser.error(str(exc))


if __name__ == "__main__":
    main()
