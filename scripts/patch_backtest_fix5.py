p = 'apps/matchforecast/src/backtest.rs'
s = open(p).read()

# Remove the misplaced anchor block from read_csv (from the FIX comment through 'models\n}')
start = s.index("    // Anchor the squad's aggregate shot output")
end = s.index("    models\n}\n", start) + len("    models\n}\n")
misplaced = s[start:end]
# restore read_csv's original tail
restored = "        .collect()\n}\n"
s = s[:start] + restored + s[end:]

# Insert into event_models_for: replace its tail
old = """            ]
        })
        .collect()
}

fn attributes_and_states_for("""
new = """            ]
        })
        .collect::<Vec<_>>();

    // Anchor the squad's aggregate shot output to the team's observed SOT rate. The raw means
    // come from a truncated squad sample, so their starting-XI weighted sum usually undershoots;
    // rescale so the modelled team volume matches reality (blended 50/50 to avoid overshoot).
    if sot_anchor_pm.is_finite() && sot_anchor_pm > 0.0 {
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
}

fn attributes_and_states_for("""
assert old in s, "event_models tail"
s = s.replace(old, new)

open(p, 'w').write(s)
print('relocated ok')
