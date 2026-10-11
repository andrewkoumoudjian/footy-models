p = 'apps/matchforecast/src/main.rs'
s = open(p).read()

# The filler block exists but fillers only get lineup candidates - they also need event models,
# attribute states, and matchup states. Replace the filler block with one that adds all four.
old = """        let mut candidates = candidates_of.remove(&club).expect("squad candidates");
        while candidates.len() < 11 {
            candidates.push(LineupCandidate {
                player: PlayerId(u64::from(club.0) * 10000 + candidates.len() as u64 + 900000),
                start_probability: 0.02,
                availability_probability: 0.9,
                ability_index: 8.0,
                fatigue: 0.1,
                expected_minutes_if_start: 45.0,
                minutes_sigma: 12.0,
                substitute_probability: 0.5,
                role_fits: (1_u16..=11).map(|role| RoleFit { role: RoleId(role), fit: 0.8 }).collect(),
            });
        }"""
if old not in s:
    # try the rustfmt'd variant
    old = """        let mut candidates = candidates_of.remove(&club).expect("squad candidates");
        while candidates.len() < 11 {
            candidates.push(LineupCandidate {
                player: PlayerId(u64::from(club.0) * 10000 + candidates.len() as u64 + 900000),
                start_probability: 0.02,
                availability_probability: 0.9,
                ability_index: 8.0,
                fatigue: 0.1,
                expected_minutes_if_start: 45.0,
                minutes_sigma: 12.0,
                substitute_probability: 0.5,
                role_fits: (1_u16..=11)
                    .map(|role| RoleFit {
                        role: RoleId(role),
                        fit: 0.8,
                    })
                    .collect(),
            });
        }"""
assert old in s, "filler block not found"
new = """        let mut candidates = candidates_of.remove(&club).expect("squad candidates");
        while candidates.len() < 11 {
            let filler = PlayerId(u64::from(club.0) * 10000 + candidates.len() as u64 + 990000);
            candidates.push(LineupCandidate {
                player: filler,
                start_probability: 0.02,
                availability_probability: 0.9,
                ability_index: 8.0,
                fatigue: 0.1,
                expected_minutes_if_start: 45.0,
                minutes_sigma: 12.0,
                substitute_probability: 0.5,
                role_fits: (1_u16..=11).map(|role| RoleFit { role: RoleId(role), fit: 0.8 }).collect(),
            });
            // Fillers must satisfy every per-player validation requirement.
            event_models.push(SpecificPlayerEventModel {
                player: filler,
                metric: MetricId(METRIC_SHOT),
                family: StatFamily::Count,
                mean_per90: league_shot_per90,
                variance_per90: league_shot_per90 * 2.0,
                epistemic_sigma: 0.15,
                reference_per90: league_shot_per90,
                aggregate_matchup_sensitivity: 0.04,
                attribute_matchup_sensitivity: 0.03,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.10,
            });
            event_models.push(SpecificPlayerEventModel {
                player: filler,
                metric: MetricId(METRIC_PASS),
                family: StatFamily::Count,
                mean_per90: league_pass_per90,
                variance_per90: league_pass_per90 * 2.0,
                epistemic_sigma: 2.5,
                reference_per90: league_pass_per90,
                aggregate_matchup_sensitivity: 0.02,
                attribute_matchup_sensitivity: 0.015,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.08,
            });
            event_models.push(SpecificPlayerEventModel {
                player: filler,
                metric: MetricId(METRIC_XG),
                family: StatFamily::ContinuousNonNegative,
                mean_per90: league_xg_per90,
                variance_per90: league_xg_per90 * 3.0,
                epistemic_sigma: 0.05,
                reference_per90: league_xg_per90,
                aggregate_matchup_sensitivity: 0.04,
                attribute_matchup_sensitivity: 0.03,
                teammate_sensitivity: 0.0,
                role_fit_sensitivity: 0.05,
                fatigue_sensitivity: 0.08,
            });
            attribute_states.push(PlayerAttributeState {
                player: filler,
                attributes: vec![
                    PlayerAttributeValue { attribute: AttributeId(ATTR_TECHNICAL), value: 12.0 },
                    PlayerAttributeValue { attribute: AttributeId(ATTR_FINISHING), value: 12.0 },
                    PlayerAttributeValue { attribute: AttributeId(ATTR_DEFENDING), value: 12.0 },
                ],
            });
            player_states.push(PlayerMatchupState {
                player: filler,
                ability: footy_forecast_stub_ability(8.0),
            });
        }"""
s = s.replace(old, new)
open(p, 'w').write(s)
print('ok')
