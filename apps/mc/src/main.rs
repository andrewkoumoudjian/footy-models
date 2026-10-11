#![forbid(unsafe_code)]
//! 36-month Monte Carlo value simulation over the forecast top players.
//!
//! Uses the existing `footy_forecast` primitives (`Rng64`, `FootballPathModel`,
//! `monte_carlo_paths`). Each player's annual drift is the model's 12-month
//! log forecast annualized; annual vol comes from the player's conformal 80%
//! interval. Driver noise channels (development/performance) are disabled so
//! the simulated dispersion equals the forecast model's uncertainty; the
//! transfer hazard + contract bookkeeping from FootballPathModel are kept.
//!
//! Usage:
//!   footy-mc run --input data/mc_input.tsv --out artifacts/mc_36m
//!     [--paths 1000] [--seed 42] [--top-n 10] [--basket-n 25]

use footy_core::EuroCents;
use footy_forecast::{
    monte_carlo_paths, FootballPathModel, FootballPathParameters, MonteCarloConfig,
    SimulatedPlayerState,
};
use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
struct PlayerInput {
    player_id: i64,
    name: String,
    club: String,
    pos: String,
    age: f64,
    ca: f64,
    pa: f64,
    attr_source: String,
    current_mv: f64, // EUR
    mu_ann: f64,     // annual log drift from the model forecast
    sigma_ann: f64,  // annual vol from the conformal 80% interval
}

fn parse_input(path: &str) -> Result<Vec<PlayerInput>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || idx == 0 {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 11 {
            return Err(format!("input line {} has {} cols (need 11)", idx + 1, cols.len()).into());
        }
        out.push(PlayerInput {
            player_id: cols[0].parse()?,
            name: cols[1].to_string(),
            club: cols[2].to_string(),
            pos: cols[3].to_string(),
            age: cols[4].parse()?,
            ca: cols[5].parse()?,
            pa: cols[6].parse()?,
            attr_source: cols[7].to_string(),
            current_mv: cols[8].parse()?,
            mu_ann: cols[9].parse()?,
            sigma_ann: cols[10].parse()?,
        });
    }
    Ok(out)
}

fn run_player(p: &PlayerInput, paths_n: u32, seed: u64) -> (Vec<Vec<EuroCents>>, Vec<bool>) {
    let params = FootballPathParameters {
        annual_development_mean: 0.0,
        annual_development_sigma: 0.0, // driver noise off -> dispersion = forecast uncertainty
        performance_reversion: 0.4,
        annual_performance_sigma: 0.0,
        daily_unavailability_probability: 0.0005,
        unavailable_performance_factor: 0.4,
        annual_market_drift: p.mu_ann,
        annual_market_sigma: p.sigma_ann.max(1e-6),
        annual_transfer_hazard: 0.25,
        ability_value_elasticity: 1.0,
        performance_value_elasticity: 0.5,
        contract_value_decay_per_year: 0.0,
    };
    let model = FootballPathModel { parameters: params };
    let initial = SimulatedPlayerState {
        ability_index: 1.0,
        performance_index: 1.0,
        availability: 1.0,
        contract_remaining_days: 1_095,
        market_log_factor: 0.0,
        price: EuroCents((p.current_mv * 100.0).round() as i64),
        transferred: false,
    };
    let config = MonteCarloConfig {
        horizon_days: 1_080,
        step_days: 30,
        paths: paths_n,
        seed,
        downside_fraction: 0.5,
    };
    let paths = monte_carlo_paths(&model, &initial, config)
        .expect("monte_carlo_paths should succeed for valid config");
    (paths.paths, paths.transferred)
}

fn percentile(sorted: &[i64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)] as f64
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let input = required(&args, "--input")?;
    let out_dir = required(&args, "--out")?;
    let paths_n: u32 = optional(&args, "--paths").unwrap_or("1000".into()).parse()?;
    let seed: u64 = optional(&args, "--seed").unwrap_or("42".into()).parse()?;
    let top_n: usize = optional(&args, "--top-n").unwrap_or("10".into()).parse()?;
    let basket_n: usize = optional(&args, "--basket-n").unwrap_or("25".into()).parse()?;

    fs::create_dir_all(&out_dir)?;
    let players = parse_input(&input)?;
    if players.is_empty() {
        return Err("no players in input".into());
    }
    let individual: Vec<&PlayerInput> = players.iter().take(top_n).collect();
    let basket: Vec<&PlayerInput> = players.iter().take(basket_n).collect();

    let mut paths_out = fs::File::create(Path::new(&out_dir).join("paths_long.tsv"))?;
    writeln!(paths_out, "player_id\tpath\tmonth\tprice_cents")?;
    let mut stats_out = fs::File::create(Path::new(&out_dir).join("stats.tsv"))?;
    writeln!(
        stats_out,
        "player_id\tname\tclub\tpos\tage\tca\tpa\tattr_source\tcurrent_mv\tmu_ann\tsigma_ann\tmean_36m\tmedian_36m\tp10\tp90\tp15x\tp2x\ttransfer_prob\tdownside_prob"
    )?;

    // individual simulations + stats
    let mut basket_index: Vec<Vec<f64>> = Vec::new();
    for p in &players {
        let (paths, transferred) = run_player(p, paths_n, seed);
        // write paths
        for (pi, series) in paths.iter().enumerate() {
            for (m, price) in series.iter().enumerate() {
                writeln!(paths_out, "{}\t{}\t{}\t{}", p.player_id, pi, m, price.cents())?;
            }
        }
        // stats from terminal prices
        let mut terminal: Vec<i64> = paths.iter().map(|s| s[s.len() - 1].cents()).collect();
        terminal.sort_unstable();
        let cur = (p.current_mv * 100.0) as f64;
        let mean = terminal.iter().sum::<i64>() as f64 / terminal.len() as f64;
        let p15x = terminal.iter().filter(|&&v| v as f64 >= 1.5 * cur).count() as f32 / terminal.len() as f32;
        let p2x = terminal.iter().filter(|&&v| v as f64 >= 2.0 * cur).count() as f32 / terminal.len() as f32;
        let transfer_prob = transferred.iter().filter(|&&t| t).count() as f32 / transferred.len() as f32;
        let downside = terminal.iter().filter(|&&v| v as f64 <= 0.5 * cur).count() as f32 / terminal.len() as f32;
        writeln!(
            stats_out,
            "{}\t{}\t{}\t{}\t{:.2}\t{:.0}\t{:.0}\t{}\t{}\t{:.6}\t{:.6}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.4}\t{:.4}\t{:.4}\t{:.4}",
            p.player_id, p.name, p.club, p.pos, p.age, p.ca, p.pa, p.attr_source,
            p.current_mv, p.mu_ann, p.sigma_ann,
            mean, percentile(&terminal, 0.5), percentile(&terminal, 0.1), percentile(&terminal, 0.9),
            p15x, p2x, transfer_prob, downside,
        )?;
        // normalized index contribution (for basket)
        let normalized: Vec<f64> = paths.iter().map(|s| s[s.len() - 1].cents() as f64 / cur).collect();
        basket_index.push(normalized);
        let _ = (individual.len(), basket.len());
    }

    // basket (equal-weight index over basket_n players; normalized per player)
    if basket.len() >= 2 {
        let n_paths = basket_index[0].len();
        // recompute full series per player for the basket
        let mut basket_paths: Vec<Vec<f64>> = vec![vec![0.0; 37]; n_paths];
        let mut count = 0.0_f64;
        for p in basket.iter().take(basket_n) {
            let (paths, _) = run_player(p, paths_n, seed);
            let cur = (p.current_mv * 100.0) as f64;
            for (pi, series) in paths.iter().enumerate() {
                for (m, price) in series.iter().enumerate() {
                    basket_paths[pi][m] += price.cents() as f64 / cur;
                }
            }
            count += 1.0;
        }
        let mut basket_out = fs::File::create(Path::new(&out_dir).join("paths_basket.tsv"))?;
        writeln!(basket_out, "player_id\tpath\tmonth\tindex_cents")?;
        for (pi, series) in basket_paths.iter().enumerate() {
            for (m, idx) in series.iter().enumerate() {
                // index 1.0 -> 100 cents-of-index; divide by count for equal weight
                let v = (idx / count * 100.0).round() as i64;
                writeln!(basket_out, "BASKET\t{}\t{}\t{}", pi, m, v)?;
            }
        }
        let _ = n_paths;
    }

    println!("MC run complete: {} players, {} paths each, seed {}", players.len(), paths_n, seed);
    println!("  individual paths  -> {}/paths_long.tsv", out_dir);
    println!("  stats             -> {}/stats.tsv", out_dir);
    println!("  basket (n={})      -> {}/paths_basket.tsv", basket_n.min(basket.len()), out_dir);
    Ok(())
}

fn required(args: &[String], flag: &str) -> Result<String, Box<dyn Error>> {
    optional(args, flag).ok_or_else(|| format!("missing {}", flag).into())
}

fn optional<'a>(args: &'a [String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).map(|i| args.get(i + 1).cloned()).flatten()
}
