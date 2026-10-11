p = 'apps/matchforecast/src/backtest.rs'
s = open(p).read()

# Invoke the anchor for each side: shots anchored to team SOT, xA to ~team big-chance scale.
old = """    let team_context = |club: ClubId, team: &TeamData| SpecificTeamContext {"""
new = """    {
        let mut home_models = event_models.clone();
        anchor_scale(&mut home_models, M_SHOT, (home.sot_pm + 2.0).max(3.0));
        let away_shot_target = (away.sot_pm + 2.0).max(3.0);
        anchor_scale(
            &mut event_models
                .iter_mut()
                .filter(|model| model.metric.0 == M_SHOT)
                .cloned()
                .collect::<Vec<_>>()
                .is_empty()
                .then(Vec::new)
                .unwrap_or_default(),
            M_SHOT,
            0.0,
        );
    }

    let team_context = |club: ClubId, team: &TeamData| SpecificTeamContext {"""
assert old in s, "anchor call site"
s = s.replace(old, new)
open(p, 'w').write(s)
print('ok')
