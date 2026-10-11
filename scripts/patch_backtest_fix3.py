p = 'apps/matchforecast/src/backtest.rs'
s = open(p).read()

# ---- FIX 3: apply goal-rate anchoring in goal_process from team windows ----
old = """    let team_context = |club: ClubId, team: &TeamData| SpecificTeamContext {"""
new = """    // FIX 1: anchor the base goal rate to the two teams' actual scoring environment instead of
    // a league constant. Average attack/defense rates of the four relevant windows.
    let env_rate =
        ((home.gf_pm + home.ga_pm + away.gf_pm + away.ga_pm) / 4.0).clamp(0.9, 2.2);
    // Blend with observed SOT-derived expectation: goals ~ SOT * conversion (~0.30).
    let sot_rate = ((home.sot_pm * 0.30 + away.sot_face_pm * 0.30)
        + (away.sot_pm * 0.30 + home.sot_face_pm * 0.30))
        / 4.0;
    let blended_rate = (env_rate * 0.6 + sot_rate.clamp(0.9, 2.4) * 0.4).clamp(1.05, 2.1);

    let team_context = |club: ClubId, team: &TeamData| SpecificTeamContext {"""
assert old in s, "fix3 insert"
s = s.replace(old, new)

old = """        goal_process: SpecificGoalProcess {
            goals_per_team_match: 1.42,
            home_advantage_multiplier: 1.09,
            path_log_rate_sigma: 0.12,
            max_abs_segment_log_adjustment: 1.2,
            segments: 6,
        },"""
new = """        goal_process: SpecificGoalProcess {
            goals_per_team_match: blended_rate,
            home_advantage_multiplier: 1.09,
            path_log_rate_sigma: 0.28,
            max_abs_segment_log_adjustment: 1.2,
            segments: 6,
        },"""
assert old in s, "goal process"
s = s.replace(old, new)

open(p, 'w').write(s)
print('fix3 ok')
