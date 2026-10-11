#!/usr/bin/env python3
"""Frozen chronological OOS audit of modeled football momentum/timing.

Scores locked 200-path same-engine simulations vs StatsBomb 2015/16 events.
Observed match events are NEVER joined to forecast inputs. The pressure
fraction is a DESCRIPTIVE quantity, not a calibrated probability.
"""
import argparse
import json
import math
import random
from pathlib import Path

from audit_pl_momentum_timing import pressure_index


def fit_only_goal_clock_prior(
    fit_fixtures, labels_dir, smoothing_per_bin=3.0,
    *, source_elapsed=False, events_dir=None,
):
    """Estimate a fixed time-of-goal reference using ONLY earlier fixtures.

    Each scored goal contributes to one of six bins. The predetermined
    symmetric Dirichlet(3,3,3,3,3,3) prior controls small-sample extremes.
    No fit/choice can depend on the held-out OOS result.
    """
    if not fit_fixtures or smoothing_per_bin <= 0:
        raise ValueError("fit fixtures and positive prior smoothing required")
    counts = [float(smoothing_per_bin)] * 6
    n_goals = 0
    for f in fit_fixtures:
        label = json.loads((labels_dir / f"{f['match_id']}.json").read_text())
        if (label["date"], label["home"], label["away"]) != (
            f["date"], f["home"], f["away"]
        ):
            raise ValueError("fit-only goal-clock label mismatch")
        if source_elapsed:
            if events_dir is None:
                raise ValueError("elapsed-clock prior needs raw earlier events")
            events = json.loads((events_dir / f"{f['match_id']}.json").read_text())
            observed = observed_windows(f, events, label["goals"], source_elapsed=True)
            actual = sum(w[side + "_goals"] for w in observed for side in ("home", "away"))
            if actual != len(label["goals"]):
                raise ValueError("fit-only source goal count mismatch")
            for index, row in enumerate(observed):
                n = row["home_goals"] + row["away_goals"]
                counts[index] += n
                n_goals += n
        else:
            for event in label["goals"]:
                if event["team"] not in (f["home"], f["away"]):
                    raise ValueError("fit-only goal team mismatch")
                minute = int(event["minute_index"])
                if not 0 <= minute < 90:
                    raise ValueError("invalid historical goal minute")
                counts[minute // 15] += 1
                n_goals += 1
    total = sum(counts)
    return [x / total for x in counts], n_goals


def oos_split(fixtures, count):
    ordered = sorted(fixtures, key=lambda f: (f["date"], f["match_id"]))
    if len(ordered) <= count or count < 1:
        raise ValueError("not enough earlier fixtures to define OOS")
    split = len(ordered) - count
    while split and ordered[split - 1]["date"] == ordered[split]["date"]:
        split -= 1
    if split < 1:
        raise ValueError("OOS split crosses all fixtures")
    return ordered[:split], ordered[split:]


def observed_windows(forecast, events, goals, *, source_elapsed=False):
    """StatsBomb shots and official parsed goals, six disjoint 15-minute bins."""
    home, away = forecast["home"], forecast["away"]
    data = [
        {"home_shots": 0, "away_shots": 0, "home_goals": 0, "away_goals": 0}
        for _ in range(6)
    ]
    for event in events:
        kind = (event.get("type") or {}).get("name")
        club = (event.get("team") or {}).get("name")
        if club not in (home, away):
            continue
        minute = event.get("minute")
        if not isinstance(minute, int):
            continue
        # StatsBomb minute is elapsed-clock zero-based, e.g. a 45:00
        # halftime substitution belongs to 45-60. Frozen historical
        # label builder erroneously subtracts one; preserve the legacy
        # mode ONLY for reproducing the earlier published v1 audit.
        index = min(89, max(0, minute if source_elapsed else minute - 1)) // 15
        if kind == "Shot":
            data[index]["home_shots" if club == home else "away_shots"] += 1
            if source_elapsed and (event.get("shot") or {}).get("outcome", {}).get("name") == "Goal":
                data[index]["home_goals" if club == home else "away_goals"] += 1
        elif source_elapsed and kind == "Own Goal Against":
            data[index]["away_goals" if club == home else "home_goals"] += 1
    if not source_elapsed:
        for event in goals:
            club = event["team"]
            if club not in (home, away):
                raise ValueError("unexpected scoring team in historical label")
            minute = int(event["minute_index"])
            if not 0 <= minute < 90:
                raise ValueError("bad goal minute index")
            data[minute // 15]["home_goals" if club == home else "away_goals"] += 1
    return data


def fit_only_substitution_clock(fit_fixtures, events_dir, pseudocount=2.0):
    if pseudocount <= 0 or not fit_fixtures:
        raise ValueError("positive smoothing and fit period required")
    incidence = [float(pseudocount)] * 6
    total = len(fit_fixtures) + 2 * pseudocount
    for f in fit_fixtures:
        events = json.loads((events_dir / f"{f['match_id']}.json").read_text())
        any_bin = observed_substitution_bins(f, events)
        for i, flag in enumerate(any_bin):
            incidence[i] += flag
    return [x / total for x in incidence]


def observed_substitution_bins(forecast, events):
    clubs = (forecast["home"], forecast["away"])
    bins = [0] * 6
    for event in events:
        if (event.get("type") or {}).get("name") != "Substitution":
            continue
        if (event.get("team") or {}).get("name") not in clubs:
            continue
        minute = event.get("minute")
        if isinstance(minute, int):
            bins[min(89, max(0, minute)) // 15] = 1
    return bins


def score_fixture(
    forecast, label, events, *, fit_only_clock=None,
    source_elapsed=False, fit_only_sub_prior=None,
):
    if (forecast["match_id"], forecast["date"], forecast["home"], forecast["away"]) != (
        label["match_id"], label["date"], label["home"], label["away"]
    ):
        raise ValueError("wrong match label")
    expected = forecast["windows_15min"]
    if len(expected) != 6 or len(forecast["minutes_1min"]) != 90:
        raise ValueError("incompatible native engine event window contract")
    observed = observed_windows(
        forecast, events, label["goals"], source_elapsed=source_elapsed,
    )
    for team, side in ((forecast["home"], "home"), (forecast["away"], "away")):
        observed_goals = sum(w[side + "_goals"] for w in observed)
        if observed_goals != label[side + "_goals"]:
            raise ValueError(f"ground truth goal count mismatch: {team}")
        observed_shots = sum(w[side + "_shots"] for w in observed)
        if observed_shots != label["teams"][team]["shots"]:
            raise ValueError(f"ground truth shot count mismatch: {team}")

    predicted_pressure = [
        (float(w["expected_home_shots"]) + 3 * float(w["expected_home_goals"]),
         float(w["expected_away_shots"]) + 3 * float(w["expected_away_goals"]))
        for w in expected
    ]
    observed_pressure = [
        (w["home_shots"] + 3.0 * w["home_goals"],
         w["away_shots"] + 3.0 * w["away_goals"])
        for w in observed
    ]
    totals = tuple(sum(x[k] for x in predicted_pressure) for k in (0, 1))
    global_share = totals[0] / sum(totals)
    signed_errors, baseline_errors = [], []
    peak = min(range(6), key=lambda i: pressure_index(expected[i]))
    # The forecast peak is based on signed RELATIVE pressure, not the
    # absolute raw home-away count difference. Compare like with like.
    observed_away_margins = [
        (h - a) / (h + a) if h + a > 0 else 0.0
        for h, a in observed_pressure
    ]
    strongest_observed_away = min(range(6), key=lambda i: observed_away_margins[i])
    tied_actual_peaks = [
        i for i in range(6)
        if observed_away_margins[i] == observed_away_margins[strongest_observed_away]
    ]
    directional_correct = directional_reference = 0
    directional_evaluable = 0
    for (h, a), (truth_h, truth_a) in zip(predicted_pressure, observed_pressure):
        if not (h + a > 0 and truth_h + truth_a > 0):
            continue
        fraction = h / (h + a)
        observed_fraction = truth_h / (truth_h + truth_a)
        signed_errors.append((fraction - observed_fraction)**2)
        baseline_errors.append((global_share - observed_fraction)**2)
        if truth_h != truth_a:
            directional_evaluable += 1
            directional_correct += int((h > a) == (truth_h > truth_a))
            directional_reference += int((global_share > 0.5) == (truth_h > truth_a))

    if fit_only_clock is None:
        fit_only_clock = [1 / 6] * 6
    if (len(fit_only_clock) != 6
            or any(not math.isfinite(p) or p <= 0 for p in fit_only_clock)
            or abs(sum(fit_only_clock) - 1.0) > 1e-9):
        raise ValueError("fit-only goal-clock prior must be a strictly positive PMF")
    goal_time_model, goal_time_flat, goal_time_fit_only = 0.0, 0.0, 0.0
    goal_count = 0
    for side in ("home", "away"):
        rate = [float(w[f"expected_{side}_goals"]) for w in expected]
        total = sum(rate)
        if total <= 0:
            raise ValueError("nonpositive simulated total goal rate")
        counts = [x[side + "_goals"] for x in observed]
        for i, n in enumerate(counts):
            if n == 0:
                continue
            if rate[i] <= 0:
                # Finite log-loss floor derived only from number of MC paths.
                p = 1.0 / (200 * 6)
            else:
                p = rate[i] / total
            goal_time_model -= n * math.log(p)
            goal_time_flat -= n * math.log(1 / 6)
            goal_time_fit_only -= n * math.log(fit_only_clock[i])
            goal_count += n
    result = {
        "date": forecast["date"], "match_id": forecast["match_id"],
        "home": forecast["home"], "away": forecast["away"],
        "pressure_sq_error": sum(signed_errors),
        "full_match_constant_pressure_sq_error": sum(baseline_errors),
        "pressure_windows_evaluated": len(signed_errors),
        "direction_correct": directional_correct,
        "reference_direction_correct": directional_reference,
        "directional_windows_evaluated": directional_evaluable,
        "city_like_away_pressure_peak_window": peak,
        "actual_strongest_away_pressure_window": strongest_observed_away,
        "away_pressure_peak_exact_window_hit": int(peak == strongest_observed_away),
        "away_peak_match_fraction_tie_adjusted": (
            1.0 / len(tied_actual_peaks) if peak in tied_actual_peaks else 0.0
        ),
        "actual_away_peak_tie_count": len(tied_actual_peaks),
        "goal_time_log_loss": goal_time_model,
        "uniform_goal_time_log_loss": goal_time_flat,
        "fit_only_goal_time_log_loss": goal_time_fit_only,
        "goal_count": goal_count,
    }
    if source_elapsed:
        if fit_only_sub_prior is None or len(fit_only_sub_prior) != 6:
            raise ValueError("raw elapsed-clock audit needs earlier-only substitution prior")
        actual_subs = observed_substitution_bins(forecast, events)
        if sum(
            (event.get("type") or {}).get("name") == "Substitution"
            and (event.get("team") or {}).get("name") in (forecast["home"], forecast["away"])
            for event in events
        ) != len(label["subs"]):
            raise ValueError("raw source substitution label count mismatch")
        completed_flags = ["p_completed_substitution" in w for w in expected]
        if any(completed_flags) and not all(completed_flags):
            raise ValueError("mixed attempted/completed substitution forecast contracts")
        completed_contract = all(completed_flags)
        prediction_key = (
            "p_completed_substitution" if completed_contract else "p_substitution"
        )
        predicted_subs = [float(w[prediction_key]) for w in expected]
        if any(not 0 <= p <= 1 for p in predicted_subs + fit_only_sub_prior):
            raise ValueError("invalid event probability")
        result.update({
            "actual_completed_substitution_bins": actual_subs,
            ("predicted_completed_substitution_probability" if completed_contract
             else "expected_any_substitution_triggers"): predicted_subs,
            "forecast_substitution_target": (
                "completed_replacements" if completed_contract
                else "attempted_substitution_or_injury_events"
            ),
            "substitution_proxy_brier_sum_engine": sum(
                (p - float(y)) ** 2 for p, y in zip(predicted_subs, actual_subs)
            ),
            "substitution_proxy_brier_sum_fit_only": sum(
                (p - float(y)) ** 2 for p, y in zip(fit_only_sub_prior, actual_subs)
            ),
            "substitution_bins": 6,
        })
    return result


def summarize(rows):
    sums = lambda field: sum(float(r[field]) for r in rows)
    windows = sums("pressure_windows_evaluated")
    goals = sums("goal_count")
    direction = sums("directional_windows_evaluated")
    if not rows or windows <= 0 or goals <= 0 or direction <= 0:
        raise ValueError("no scoreable OOS pressure/goal-time samples")
    summary = {
        "fixtures": len(rows),
        "pressure_windows": int(windows),
        "fractional_pressure_mse_engine": sums("pressure_sq_error") / windows,
        "fractional_pressure_mse_constant_full_match_share": (
            sums("full_match_constant_pressure_sq_error") / windows
        ),
        "pressure_mse_improvement_engine_minus_baseline": (
            sums("pressure_sq_error") - sums("full_match_constant_pressure_sq_error")
        ) / windows,
        "directional_windows_without_exact_tie": int(direction),
        "direction_accuracy_engine": sums("direction_correct") / direction,
        "direction_accuracy_constant_full_match_share": (
            sums("reference_direction_correct") / direction
        ),
        "away_peak_exact_window_match_rate_first_tie": sums("away_pressure_peak_exact_window_hit") / len(rows),
        "away_peak_match_rate_tie_adjusted": sums("away_peak_match_fraction_tie_adjusted") / len(rows),
        "away_peak_random_uniform_chance": 1.0 / 6,
        "actual_scored_goals": int(goals),
        "goal_time_loss_engine_per_goal": sums("goal_time_log_loss") / goals,
        "goal_time_loss_flat_per_goal": sums("uniform_goal_time_log_loss") / goals,
        "goal_time_loss_fit_only_per_goal": sums("fit_only_goal_time_log_loss") / goals,
        "goal_time_loss_improvement_engine_minus_flat": (
            sums("goal_time_log_loss") - sums("uniform_goal_time_log_loss")
        ) / goals,
        "goal_time_loss_improvement_engine_minus_fit_only": (
            sums("goal_time_log_loss") - sums("fit_only_goal_time_log_loss")
        ) / goals,
    }
    if "substitution_proxy_brier_sum_engine" in rows[0]:
        targets = {x["forecast_substitution_target"] for x in rows}
        if len(targets) != 1:
            raise ValueError("mixed substitution event definitions in scored fixtures")
        bins = sums("substitution_bins")
        engine_brier = (
            sums("substitution_proxy_brier_sum_engine") / bins
        )
        prior_brier = (
            sums("substitution_proxy_brier_sum_fit_only") / bins
        )
        summary["substitution_proxy_brier_delta_engine_minus_fit_only"] = (
            sums("substitution_proxy_brier_sum_engine")
            - sums("substitution_proxy_brier_sum_fit_only")
        ) / bins
        if targets == {"completed_replacements"}:
            summary["substitution_forecast_is_completed_event_matched"] = True
            summary["completed_substitution_brier_engine"] = engine_brier
            summary["completed_substitution_brier_fit_only"] = prior_brier
        else:
            # Historical v1 forecast only emitted attempts. Do not label these
            # probabilities calibrated completed substitutions.
            summary["substitution_trigger_vs_completed_brier_engine"] = engine_brier
            summary["substitution_trigger_vs_completed_brier_fit_only"] = prior_brier
    return summary


def bootstrap(rows, draws=2000, seed=42):
    rng = random.Random(seed)
    out = {"pressure_mse_improvement_engine_minus_baseline": [],
           "goal_time_loss_improvement_engine_minus_flat": [],
           "goal_time_loss_improvement_engine_minus_fit_only": []}
    if "substitution_proxy_brier_sum_engine" in rows[0]:
        out["substitution_proxy_brier_delta_engine_minus_fit_only"] = []
    for _ in range(draws):
        sample = [rows[rng.randrange(len(rows))] for _ in rows]
        item = summarize(sample)
        for key in out:
            out[key].append(item[key])
    for values in out.values():
        values.sort()
    return {
        key: [round(values[int(draws * 0.025)], 7),
              round(values[int(draws * 0.975)], 7)]
        for key, values in out.items()
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predictions", type=Path,
                        default=Path("artifacts/validation/pl-hist-same-engine-paths.json"))
    parser.add_argument("--labels", type=Path, default=Path("data/statsbomb/pl_hist/labels"))
    parser.add_argument("--events", type=Path, default=Path("data/statsbomb/events"))
    parser.add_argument("--oos", type=int, default=24)
    parser.add_argument("--source-elapsed-clock", action="store_true",
                        help="New v2 audit: use raw StatsBomb elapsed minute (NOT legacy minute-1)")
    parser.add_argument("--output", type=Path,
                        default=Path("artifacts/validation/pl-hist-momentum-oos.json"))
    args = parser.parse_args()
    forecast = json.loads(args.predictions.read_text())
    if forecast["feature_contract"] != "observed_stats_only_no_fm26_no_current_squads":
        raise ValueError("unexpected or time-leaking engine feature contract")
    if int(forecast["paths"]) != 200:
        raise ValueError("expected frozen 200-path simulator")
    fit, oos = oos_split(forecast["fixtures"], args.oos)
    if max(x["date"] for x in fit) >= min(x["date"] for x in oos):
        raise ValueError("chronological split failed")
    fitted_goal_clock, fit_only_goals = fit_only_goal_clock_prior(
        fit, args.labels, source_elapsed=args.source_elapsed_clock,
        events_dir=args.events,
    )
    fit_sub_prior = (
        fit_only_substitution_clock(fit, args.events)
        if args.source_elapsed_clock else None
    )
    rows = []
    for f in oos:
        match_id = f["match_id"]
        label = json.loads((args.labels / f"{match_id}.json").read_text())
        events = json.loads((args.events / f"{match_id}.json").read_text())
        rows.append(score_fixture(
            f, label, events, fit_only_clock=fitted_goal_clock,
            source_elapsed=args.source_elapsed_clock, fit_only_sub_prior=fit_sub_prior,
        ))
    result = {
        "contract": (
            "statsbomb_2015_16_frozen_oos_elapsed_clock_v2"
            if args.source_elapsed_clock
            else "statsbomb_2015_16_frozen_oos_momentum_timing_v1"
        ),
        "data_scope": "2015-16 StatsBomb EPL event-covered fixtures only",
        "source_forecast": str(args.predictions),
        "source_engine_paths": forecast["paths"],
        "source_sampler": (
            "role_neutral_exact_cardinality_research"
            if forecast.get("input_contract", {}).get("exact_cardinality_lineup")
            else "production_sequential_lineup"
        ),
        "fresh_independent_oos": False if forecast.get("input_contract", {}).get("exact_cardinality_lineup") else None,
        "fit_fixtures": len(fit),
        "fit_only_goals": fit_only_goals,
        "fit_only_clock_prior": fitted_goal_clock,
        "fit_only_substitution_clock_prior": fit_sub_prior,
        "fit_only_clock_pseudocount_per_bin": 3.0,
        "oos_boundary_date": min(x["date"] for x in oos),
        "summary": summarize(rows),
        "bootstrap_95pct_engine_minus_reference": bootstrap(rows),
        "bootstrap_unit": "entire match, 2000 draws, deterministic seed 42",
        "target_definition": "Observed StatsBomb Shots and credited Goals per team per 15-minute window",
        "event_clock": (
            "source elapsed event minute, clamped stoppage into regulation minute 89"
            if args.source_elapsed_clock
            else "legacy historical label minute_index = source minute-1"
        ),
        "pressure_definition": "shots + 3*goals, scaled to home/(home+away); descriptive, not calibrated event probability",
        "baseline_definition": "constant full-match simulated pressure share; uniform-in-time and earlier-fixture-only smoothed goal clock priors",
        "caveats": [
            "Small 25-match chronologically protected event-covered subset, selected by available event coverage.",
            "Goal-time log score conditions on observed team total and uses iid arrival approximation.",
            "Flat timed-goal benchmark is deliberately simple; stronger as-of time priors are needed.",
            "Goal-weight 3 and the display index were already authored, not selected using these OOS goals.",
            "This evaluates observed-stat-only 2015/16 model, not FM26 in October 2026.",
            ("Research sampler was proposed after these held-out fixtures had already been inspected; do not use these results to promote or tune it."
             if forecast.get("input_contract", {}).get("exact_cardinality_lineup")
             else "Frozen reference forecasting inputs must not be retuned using the evaluated fixtures."),
            ("Completed-substitution Brier compares actual successful replacements to the matching modeled completed-event occurrence; evidence remains research-only."
             if rows and rows[0].get("forecast_substitution_target") == "completed_replacements"
             else "Legacy substitution Brier compares model attempts to completed replacements; these are not identical event definitions."),
        ],
        "oos_fixtures": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
    print(json.dumps(result["summary"], indent=2))
    print("CI", json.dumps(result["bootstrap_95pct_engine_minus_reference"], indent=2))


if __name__ == "__main__":
    main()
