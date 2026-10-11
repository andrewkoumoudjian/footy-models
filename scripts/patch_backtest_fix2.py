p = 'apps/matchforecast/src/backtest.rs'
s = open(p).read()

# Revert the broken call-site insert if present
bad_start = s.find("    {\n        let mut home_models = event_models.clone();")
if bad_start != -1:
    bad_end = s.index("    let team_context = |club: ClubId", bad_start)
    s = s[:bad_start] + s[bad_end:]

# event_models_for now takes an anchor scale applied to shot models.
old = """fn event_models_for(players: &[PlayerRow], team_name: &str) -> Vec<SpecificPlayerEventModel> {
    players"""
new = """fn event_models_for(
    players: &[PlayerRow],
    team_name: &str,
    sot_anchor_pm: f64,
) -> Vec<SpecificPlayerEventModel> {
    players"""
assert old in s, "signature"
s = s.replace(old, new)

# Apply scaling at the end of event_models_for: wrap the .collect()
old = """        })
        .collect()
}"""
new = """        })
        .collect::<Vec<_>>();

    // Anchor the squad's aggregate shot output to the team's observed SOT rate. The raw means
    // come from a truncated squad sample, so their starting-XI weighted sum usually undershoots;
    // rescale so the modelled team volume matches reality (blended 50/50 to avoid overshoot).
    if sot_anchor_pm.is_finite() && sot_anchor_pm > 0.0 && models.len() >= 8 {
        let mut by_mean: Vec<&SpecificPlayerEventModel> =
            models.iter().filter(|model| model.metric.0 == M_SHOT).collect();
        by_mean.sort_by(|a, b| b.mean_per90.total_cmp(&a.mean_per90));
        let xi_sum: f64 = by_mean.iter().take(11).map(|model| model.mean_per90 * 0.78).sum();
        if xi_sum > 1e-6 {
            let scale = ((sot_anchor_pm / xi_sum) * 0.5 + 0.5).clamp(0.6, 2.5);
            for model in models.iter_mut() {
                if model.metric.0 == M_SHOT || model.metric.0 == M_XA || model.metric.0 == M_PROGC {
                    model.mean_per90 *= scale;
                    model.variance_per90 *= scale;
                }
            }
        }
    }
    models
}"""
assert old in s, "collect tail"
s = s.replace(old, new)

# Call sites pass team windows
old = """    let mut event_models = event_models_for(players, home_name);
    let away_models = event_models_for(players, away_name);
    event_models.extend(away_models);"""
new = """    // Team SOT plus a +2 floor: player-level sums under full-squad coverage always trail the
    // team's all-player total, so a pure ratio would under-anchor early-season squads.
    let home_anchor = (home.sot_pm + 2.0).max(4.0);
    let away_anchor = (away.sot_pm + 2.0).max(4.0);
    let mut event_models = event_models_for(players, home_name, home_anchor);
    let away_models = event_models_for(players, away_name, away_anchor);
    event_models.extend(away_models);"""
assert old in s, "call sites"
s = s.replace(old, new)

open(p, 'w').write(s)
print('fix2 clean ok')
