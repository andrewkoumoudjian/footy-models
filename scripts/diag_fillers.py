p = 'apps/matchforecast/src/main.rs'
s = open(p).read()

# Relax qualification so real players fill squads, and make synthetic fillers fully consistent
# (event models + attribute state + matchup state) so validation passes.
old = """        let mut candidates = Vec::new();
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
if old not in s:
    # find actual text
    import re
    m = re.search(r"let mut candidates = Vec::new\(\);\n(.*?)\n        \}\n        LineupProjectionInput \{", s, re.S)
    print("PATTERN MISS; actual block:")
    print(m.group(0)[:2000] if m else "no match")
else:
    s = s.replace(old, "")
open(p,'w').write(s)
print('done')
