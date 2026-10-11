p = 'apps/matchforecast/src/backtest.rs'
s = open(p).read()

# ---- FIX 1a: TeamData fields ----
old = """struct TeamData {
    matches: usize,
    gf_pm: f64,
    ga_pm: f64,
    xg_pm: f64,
    xga_pm: f64,
}"""
new = """struct TeamData {
    matches: usize,
    gf_pm: f64,
    ga_pm: f64,
    xg_pm: f64,
    xga_pm: f64,
    sot_pm: f64,      // shots on target per match (own attack)
    sot_face_pm: f64, // opponent SOT in this team's matches
}"""
assert old in s, "struct"
s = s.replace(old, new)

old = "        return TeamData { matches: 0, gf_pm: 1.40, ga_pm: 1.40, xg_pm: 1.40, xga_pm: 1.40 };"
new = """        return TeamData {
            matches: 0,
            gf_pm: 1.40,
            ga_pm: 1.40,
            xg_pm: 1.40,
            xga_pm: 1.40,
            sot_pm: 4.5,
            sot_face_pm: 4.5,
        };"""
assert old in s, "zero case"
s = s.replace(old, new)

old = """        xga_pm: avg(&|row| if row["home"] == name { f(row, "away_xg") } else { f(row, "home_xg") }),
    }
}"""
new = """        xga_pm: avg(&|row| if row["home"] == name { f(row, "away_xg") } else { f(row, "home_xg") }),
        sot_pm: avg(&|row| {
            if row["home"] == name {
                f(row, "home_sot")
            } else {
                f(row, "away_sot")
            }
        }),
        sot_face_pm: avg(&|row| {
            if row["home"] == name {
                f(row, "away_sot")
            } else {
                f(row, "home_sot")
            }
        }),
    }
}"""
assert old in s, "xga tail"
s = s.replace(old, new)

open(p, 'w').write(s)
print('fix1 ok')
