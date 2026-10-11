#![forbid(unsafe_code)]

use footy_backtest::{
    compare_to_baselines, expected_calibration_error, gem_metrics, log_ratio_metrics,
    regression_deciles, Baseline, BaselineResult, ClassificationObservation,
    GemObservation, GemMetrics, LogRatioObservation, RegressionDecile,
};
use footy_core::{Date, EuroCents, PlayerId};
use footy_models::auc;
use footy_recruitment::surge::{FORECAST_FEATURE_NAMES, SURGE_FEATURE_NAMES, fm_pa_band_midpoint};
use std::env;
use std::error::Error;
use std::fs;

const T0_DEFAULT: &str = "2025-11-06";
const END_DEFAULT: &str = "2026-06-12";
const FORECAST_PA_MIN: f64 = 140.0;
const FORECAST_MAX_AGE: f64 = 23.0;
const FORECAST_MAX_MV: f64 = 1_000_000.0;
const SURGE_MAX_AGE: f64 = 28.0;
const SURGE_MIN_MV: f64 = 400_000.0;
const SURGE_MAX_MV: f64 = 1_000_000.0;
const SPIKE_RETURN: f64 = 0.5;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("forecast") => forecast(&args[1..]),
        Some("surge") => surge(&args[1..]),
        Some("tail") => tail(&args[1..]),
        _ => Err("usage:\n  footy-forward forecast --model model.bin --candidates candidates.tsv --realized realized.tsv --meta meta.tsv [--as-of YYYY-MM-DD] [--horizon-end YYYY-MM-DD] [--seed N] [--label NAME] [--output report.md] [--metrics metrics.tsv]\n  footy-forward surge --model model.bin --candidates candidates.tsv --realized realized.tsv --meta meta.tsv [--as-of YYYY-MM-DD] [--horizon-end YYYY-MM-DD] [--seed N] [--label NAME] [--output report.md] [--metrics metrics.tsv]
  footy-forward tail --model model.bin --candidates candidates.tsv --realized realized.tsv --meta meta.tsv [--as-of YYYY-MM-DD] [--horizon-end YYYY-MM-DD] [--seed N] [--label NAME] [--output report.md] [--metrics metrics.tsv]".into()),
    }
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    let index = args.iter().position(|arg| arg == flag).ok_or_else(|| format!("missing {flag}"))?;
    args.get(index + 1).map(String::as_str).ok_or_else(|| format!("missing value for {flag}").into())
}

fn optional_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1).map(String::as_str)
}

fn parse_date(value: &str) -> Result<Date, Box<dyn Error>> {
    let mut parts = value.split('-');
    let year = parts.next().ok_or("missing year")?.parse::<i32>()?;
    let month = parts.next().ok_or("missing month")?.parse::<u8>()?;
    let day = parts.next().ok_or("missing day")?.parse::<u8>()?;
    if parts.next().is_some() { return Err("date must be YYYY-MM-DD".into()); }
    Ok(Date::from_ymd(year, month, day)?)
}

fn format_date(date: Date) -> String {
    let (year, month, day) = date.to_ymd();
    format!("{year:04}-{month:02}-{day:02}")
}

fn parse_float(field: &str) -> f64 {
    match field.trim() {
        "" | "NA" | "nan" | "NaN" | "None" => f64::NAN,
        value => value.parse::<f64>().unwrap_or(f64::NAN),
    }
}

fn fmt(value: f64, decimals: usize) -> String {
    if value.is_finite() { format!("{value:.decimals$}") } else { "n/a".to_string() }
}

// ---------------------------------------------------------------------------
// TSV parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CandidateRow {
    player: PlayerId,
    t: Date,
    mv_t: f64,
    features: Vec<f64>,
}

/// Parse a candidate TSV with the fixed 47-feature schema + player_id, t, mv_t.
fn parse_candidates(path: &str, expected: &[&str; 47]) -> Result<Vec<CandidateRow>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("candidate TSV is empty")?.split('\t').collect();
    if header.len() < 50 {
        return Err(format!("candidate TSV has {} columns; expected >= 50", header.len()).into());
    }
    for (index, expected_name) in expected.iter().enumerate() {
        if header[index] != *expected_name {
            return Err(format!("candidate TSV column {index} is {:?}; expected {:?}", header[index], expected_name).into());
        }
    }
    if header[47..50] != ["player_id", "t", "mv_t"] {
        return Err("candidate TSV trailing columns must be player_id, t, mv_t".into());
    }
    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != header.len() {
            return Err(format!("candidate row {} has {} columns; expected {}", offset + 2, fields.len(), header.len()).into());
        }
        let player = PlayerId(fields[47].trim().parse::<u64>()?);
        let t = Date(fields[48].trim().parse::<i32>()?);
        let mv_t = parse_float(fields[49]);
        if !mv_t.is_finite() || mv_t <= 0.0 {
            return Err(format!("candidate row {} has invalid mv_t {mv_t}", offset + 2).into());
        }
        let features = fields[..47].iter().map(|field| parse_float(field)).collect();
        rows.push(CandidateRow { player, t, mv_t, features });
    }
    if rows.is_empty() { return Err("candidate TSV contains no rows".into()); }
    Ok(rows)
}

#[derive(Debug, Clone)]
struct RealizedRow {
    player: PlayerId,
    mv_realized: f64,
    realized_date: Date,
    horizon_days: i64,
    peak_realized: f64,
}

fn parse_realized(path: &str) -> Result<Vec<RealizedRow>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("realized TSV is empty")?.split('\t').collect();
    let expected = ["player_id", "t", "mv_t", "mv_realized", "realized_date", "horizon_days", "peak_realized"];
    if header != expected {
        return Err(format!("realized TSV header mismatch: {header:?}").into());
    }
    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != header.len() {
            return Err(format!("realized row {} has {} columns", offset + 2, fields.len()).into());
        }
        rows.push(RealizedRow {
            player: PlayerId(fields[0].trim().parse::<u64>()?),
            mv_realized: parse_float(fields[3]),
            realized_date: Date(fields[4].trim().parse::<i32>()?),
            horizon_days: fields[5].trim().parse::<i64>()?,
            peak_realized: parse_float(fields[6]),
        });
    }
    if rows.is_empty() { return Err("realized TSV contains no rows".into()); }
    Ok(rows)
}

#[derive(Debug, Clone)]
struct MetaRow {
    player: PlayerId,
    age: f64,
    ca: f64,
    pa: f64,
    attr_source: String,
    mins_365: f64,
    price_bucket: i64,
}

fn parse_meta(path: &str) -> Result<Vec<MetaRow>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("meta TSV is empty")?.split('\t').collect();
    let index = |name: &str| header.iter().position(|column| *column == name);
    let required = ["player_id", "age", "current_mv", "ca", "pa", "pa_eff", "attr_source", "mins_365", "price_bucket"];
    for name in required {
        if index(name).is_none() {
            return Err(format!("meta TSV missing column {name}").into());
        }
    }
    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != header.len() {
            return Err(format!("meta row {} has {} columns", offset + 2, fields.len()).into());
        }
        let get = |name: &str| -> &str { fields[index(name).expect("checked above")].trim() };
        rows.push(MetaRow {
            player: PlayerId(get("player_id").parse::<u64>()?),
            age: parse_float(get("age")),
            ca: parse_float(get("ca")),
            pa: parse_float(get("pa")),
            attr_source: get("attr_source").to_string(),
            mins_365: parse_float(get("mins_365")),
            price_bucket: get("price_bucket").parse::<i64>().unwrap_or(0),
        });
    }
    if rows.is_empty() { return Err("meta TSV contains no rows".into()); }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Shared forward-test evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ForwardRow {
    player: PlayerId,
    t: Date,
    mv_t: f64,
    mv_realized: f64,
    horizon_days: i64,
    peak_realized: f64,
    score: f64,
    age: f64,
    ca: f64,
    pa_eff: f64,
    attr_source: String,
    mins_365: f64,
    price_bucket: i64,
    realized_return: f64,
    realized_log_ratio: f64,
}

fn effective_pa(pa: f64) -> f64 {
    if pa.is_finite() && pa < 0.0 {
        fm_pa_band_midpoint(pa).unwrap_or(f64::NAN)
    } else {
        pa
    }
}

/// Deterministic seeded splitmix64 hash -> [0,1) for the random-price-bucket baseline.
fn splitmix64(mut x: u64) -> f64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z = z ^ (z >> 31);
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Baseline scores. NaN scores rank last and are excluded from gem selection.
fn baseline_score(baseline: Baseline, row: &ForwardRow, seed: u64) -> f64 {
    match baseline {
        Baseline::TransfermarktValue => -row.mv_t.ln(),
        Baseline::FmCurrentAbility => row.ca,
        Baseline::FmPotentialAbility => row.pa_eff,
        Baseline::AgeMinutes => {
            if row.age.is_finite() {
                (row.mins_365 + 1.0).ln() - row.age
            } else {
                f64::NAN
            }
        }
        Baseline::RandomPriceBucket => {
            let bucket = row.price_bucket.max(0) as u64;
            splitmix64(seed ^ (bucket << 32) ^ row.player.0)
        }
        Baseline::PerformanceOnly => f64::NAN,
    }
}

fn gem_metrics_for(records: &[GemObservation], threshold: f64) -> Option<GemMetrics> {
    gem_metrics(records, threshold)
}

/// Select the top `frac` of records by score (gem-style selection).
fn top_frac(records: &[GemObservation], frac: f64, cutoff: Date) -> Vec<GemObservation> {
    let mut valid: Vec<GemObservation> = records
        .iter()
        .copied()
        .filter(|row| row.as_of <= cutoff && row.entry_price.cents() > 0 && row.future_price.cents() >= 0 && row.score.is_finite())
        .collect();
    valid.sort_by(|left, right| {
        right.score.total_cmp(&left.score).then_with(|| left.player.cmp(&right.player))
    });
    let count = ((valid.len() as f64) * frac).round().max(1.0) as usize;
    valid.truncate(count);
    valid
}

/// Build gem observations for one scoring signal (model or baseline).
fn gem_observations(rows: &[ForwardRow], score_of: impl Fn(&ForwardRow) -> f64) -> Vec<GemObservation> {
    rows.iter()
        .filter(|row| row.mv_t.is_finite() && row.mv_t > 0.0 && row.mv_realized.is_finite() && row.mv_realized >= 0.0)
        .map(|row| GemObservation {
            player: row.player,
            as_of: row.t,
            entry_price: EuroCents((row.mv_t * 100.0).round() as i64),
            future_price: EuroCents((row.mv_realized * 100.0).round() as i64),
            score: score_of(row),
        })
        .collect()
}

struct DecileSummary {
    deciles: Vec<RegressionDecile>,
    top_return: f64,
    bottom_return: f64,
}

fn decile_summary(rows: &[LogRatioObservation]) -> DecileSummary {
    let deciles = regression_deciles(rows, 10);
    let top_return = deciles.last().and_then(|d| d.mean_return).unwrap_or(f64::NAN);
    let bottom_return = deciles.first().and_then(|d| d.mean_return).unwrap_or(f64::NAN);
    DecileSummary { deciles, top_return, bottom_return }
}

fn hit_rate_positive(rows: &[ForwardRow]) -> f64 {
    let n = rows.len();
    if n == 0 { return f64::NAN; }
    rows.iter().filter(|row| row.realized_log_ratio.is_finite() && row.realized_log_ratio > 0.0).count() as f64 / n as f64
}

fn hit_rate_spike(rows: &[ForwardRow]) -> f64 {
    let n = rows.len();
    if n == 0 { return f64::NAN; }
    rows.iter().filter(|row| row.realized_return.is_finite() && row.realized_return >= SPIKE_RETURN).count() as f64 / n as f64
}

fn mean_return(rows: &[ForwardRow]) -> f64 {
    let returns: Vec<f64> = rows.iter().filter(|row| row.realized_return.is_finite()).map(|row| row.realized_return).collect();
    if returns.is_empty() { return f64::NAN; }
    returns.iter().sum::<f64>() / returns.len() as f64
}

/// Top-N by score: hit rates and mean realized return.
fn top_n_summary(rows: &[ForwardRow], score_of: impl Fn(&ForwardRow) -> f64, ns: &[usize]) -> Vec<(usize, f64, f64, f64)> {
    let mut sorted: Vec<&ForwardRow> = rows.iter().filter(|row| score_of(row).is_finite()).collect();
    sorted.sort_by(|left, right| score_of(right).total_cmp(&score_of(left)).then_with(|| left.player.cmp(&right.player)));
    ns.iter()
        .map(|n| {
            let slice: Vec<ForwardRow> = sorted.iter().take(*n).map(|row| (*row).clone()).collect();
            (*n, hit_rate_positive(&slice), hit_rate_spike(&slice), mean_return(&slice))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Forecast forward test
// ---------------------------------------------------------------------------

fn forecast(args: &[String]) -> Result<(), Box<dyn Error>> {
    let model_path = required_value(args, "--model")?;
    let candidates_path = required_value(args, "--candidates")?;
    let realized_path = required_value(args, "--realized")?;
    let meta_path = required_value(args, "--meta")?;
    let as_of = optional_value(args, "--as-of").map(parse_date).transpose()?.unwrap_or(parse_date(T0_DEFAULT)?);
    let horizon_end = optional_value(args, "--horizon-end").map(parse_date).transpose()?.unwrap_or(parse_date(END_DEFAULT)?);
    let seed = optional_value(args, "--seed").map(str::parse).transpose()?.unwrap_or(42_u64);
    let label = optional_value(args, "--label").unwrap_or("forecast").to_string();
    let output = optional_value(args, "--output").map(str::to_string).unwrap_or_else(|| "data/forward_test/forecast_forward.md".to_string());
    let metrics_out = optional_value(args, "--metrics");

    let model = footy_artifacts::read_forecast_model(model_path)?;
    let candidates = parse_candidates(candidates_path, &FORECAST_FEATURE_NAMES)?;
    let realized = parse_realized(realized_path)?;
    let meta = parse_meta(meta_path)?;

    let realized_by_player: std::collections::HashMap<u64, &RealizedRow> =
        realized.iter().map(|row| (row.player.0, row)).collect();
    let meta_by_player: std::collections::HashMap<u64, &MetaRow> =
        meta.iter().map(|row| (row.player.0, row)).collect();

    // Score candidates with the fitted model (strictly causal: model trained
    // through 2025-06-12 < as-of; features computed strictly before t).
    let mut rows: Vec<ForwardRow> = Vec::new();
    let mut n_missing_realized = 0_usize;
    let mut n_missing_meta = 0_usize;
    for candidate in &candidates {
        if candidate.t > as_of {
            return Err(format!("candidate {} has entry after as-of", candidate.player.0).into());
        }
        let realized_row = match realized_by_player.get(&candidate.player.0) {
            Some(row) => row,
            None => { n_missing_realized += 1; continue; }
        };
        if realized_row.mv_realized <= 0.0 || !realized_row.mv_realized.is_finite() {
            n_missing_realized += 1;
            continue;
        }
        let (pred, _lo, _hi) = model.predict_log_ratio(&candidate.features)?;
        let meta_row = match meta_by_player.get(&candidate.player.0) {
            Some(row) => row,
            None => { n_missing_meta += 1; continue; }
        };
        let realized_return = realized_row.mv_realized / candidate.mv_t - 1.0;
        rows.push(ForwardRow {
            player: candidate.player,
            t: candidate.t,
            mv_t: candidate.mv_t,
            mv_realized: realized_row.mv_realized,
            horizon_days: realized_row.horizon_days,
            peak_realized: realized_row.peak_realized,
            score: pred,
            age: meta_row.age,
            ca: meta_row.ca,
            pa_eff: effective_pa(meta_row.pa),
            attr_source: meta_row.attr_source.clone(),
            mins_365: meta_row.mins_365,
            price_bucket: meta_row.price_bucket,
            realized_return,
            realized_log_ratio: realized_return.ln_1p(),
        });
    }
    if rows.is_empty() { return Err("forecast forward test has no scored rows".into()); }

    // Population slices
    let full: Vec<ForwardRow> = rows.clone();
    let eligible: Vec<ForwardRow> = rows.iter()
        .filter(|row| {
            row.age.is_finite() && row.age < FORECAST_MAX_AGE
                && row.pa_eff.is_finite() && row.pa_eff >= FORECAST_PA_MIN
                && row.mv_t > 0.0 && row.mv_t <= FORECAST_MAX_MV
                && (row.attr_source == "db" || row.attr_source == "derived")
        })
        .cloned()
        .collect();
    let mv_band: Vec<ForwardRow> = rows.iter()
        .filter(|row| row.mv_t >= 10_000.0 && row.mv_t <= FORECAST_MAX_MV)
        .cloned()
        .collect();

    let metrics_of = |population: &[ForwardRow]| -> Result<(), Box<dyn Error>> {
        let obs: Vec<LogRatioObservation> = population.iter()
            .filter(|row| row.score.is_finite() && row.realized_log_ratio.is_finite())
            .map(|row| LogRatioObservation { predicted: row.score, actual: row.realized_log_ratio, realized_return: Some(row.realized_return) })
            .collect();
        let m = log_ratio_metrics(&obs).ok_or("no finite forecast pairs")?;
        let ic = m.spearman_ic.unwrap_or(f64::NAN);
        let radius = model.walkforward.conformal_radius;
        let covered = obs.iter().filter(|row| (row.actual - row.predicted).abs() <= radius).count();
        let coverage = covered as f64 / obs.len() as f64;
        let ds = decile_summary(&obs);
        let baselines = baseline_comparison(population, seed);
        println!("label={label} n={} ic={:.4} rmse={:.4} mae={:.4} coverage_partial={:.4} top_decile_return={:.4} bottom_decile_return={:.4}",
            m.count, ic, m.rmse, m.mae, coverage, ds.top_return, ds.bottom_return);
        if let Some(metrics_path) = metrics_out {
            append_metrics_row(MetricsRow {
                path: metrics_path,
                label: &label,
                walkforward_metric: model.walkforward.pooled_spearman_ic,
                forward_metric: ic,
                n: m.count as usize,
                coverage,
                top_return: ds.top_return,
                bottom_return: ds.bottom_return,
                baselines: &baselines,
            })?;
        }
        Ok(())
    };
    metrics_of(&full)?;
    if !eligible.is_empty() { metrics_of(&eligible)?; }
    if !mv_band.is_empty() { metrics_of(&mv_band)?; }

    let mut report = String::new();
    report.push_str(&format!("# Forward Test — Forecast model ({label})\n\n"));
    report.push_str(&format!("- **As-of anchor**: {} (FM26 database vintage / start of FM26)\n", format_date(as_of)));
    report.push_str(&format!("- **Realized oracle end**: {} (last Transfermarkt valuation in the read-only overlay)\n", format_date(horizon_end)));
    report.push_str(&format!("- **Model**: version {}, dataset {}, feature-set {}, calibration {}, ridge {}, seed {}; trained through {} (rows observed on or before this date)\n",
        model.metadata.model_version.0, model.metadata.dataset_version.0, model.metadata.feature_set_version.0,
        model.calibration_version.0, model.ridge, model.metadata.seed, format_date(model.metadata.trained_through)));
    report.push_str(&format!("- **Entry**: last TM valuation on or before {} per player ({} players in `forecast_candidates.csv` have one); realized = last TM valuation in (entry, {}]; players with no realized valuation after entry are dropped (censored), never encoded as zero\n",
        format_date(as_of), full.len(), format_date(horizon_end)));
    report.push_str(&format!("- **Realized window (measured)**: median horizon {} days (min {}, max {}); realized valuation dates span {} to {}; the anchor-to-end window is {} days but most entry valuations predate the anchor (TM summer 2025 revaluation wave), so the effective median realized horizon is ~11.5 months\n",
        median_horizon(&full), full.iter().map(|r| r.horizon_days).min().unwrap_or(0), full.iter().map(|r| r.horizon_days).max().unwrap_or(0),
        format_date(realized.iter().map(|r| r.realized_date).min().unwrap()), format_date(realized.iter().map(|r| r.realized_date).max().unwrap()),
        horizon_end.days_since(as_of)));
    report.push_str(&format!("- **BacktestRunRecord**: cutoff={}, dataset_version={}, feature_set_version={}, model_version={}, calibration_version={}, seed={}\n\n",
        format_date(as_of), model.metadata.dataset_version.0, model.metadata.feature_set_version.0, model.metadata.model_version.0, model.calibration_version.0, seed));

    report.push_str(&format!("## Full pool (n = {})\n\n", full.len()));
    report.push_str(&forecast_section(&full, &model.walkforward.conformal_radius)?);
    report.push_str(&format!("\n## Deployable population (age < 23, effective PA >= {}, 0 < mv <= 1M, attr_source in {{db, derived}}) (n = {})\n\n", FORECAST_PA_MIN as u32, eligible.len()));
    report.push_str(&forecast_section(&eligible, &model.walkforward.conformal_radius)?);
    report.push_str(&format!("\n## MV-band restricted (10k <= mv <= 1M, any age) (n = {})\n\n", mv_band.len()));
    report.push_str(&forecast_section(&mv_band, &model.walkforward.conformal_radius)?);
    report.push_str("\n## Notes\n\n- The conformal 80% interval radius (");
    report.push_str(&fmt(model.walkforward.conformal_radius, 4));
    report.push_str(") was calibrated on 12-month out-of-fold residuals; coverage here is measured on the **partial** realized window and is reported honestly as partial-horizon coverage (partial-period dispersion is narrower than full-period, so coverage is expected to exceed 80% for a well-specified model).\n");
    report.push_str(&format!("- Walk-forward (pooled OOF, same config): RMSE {}, MAE {}, Spearman IC {}.\n",
        fmt(model.walkforward.pooled_rmse, 4), fmt(model.walkforward.pooled_mae, 4),
        model.walkforward.pooled_spearman_ic.map_or_else(|| "n/a".to_string(), |ic| fmt(ic, 4))));
    fs::write(&output, report)?;
    println!("wrote forward-test report -> {output} (n_full={} n_eligible={} n_mv_band={} missing_realized={} missing_meta={})", full.len(), eligible.len(), mv_band.len(), n_missing_realized, n_missing_meta);
    Ok(())
}

fn median_horizon(rows: &[ForwardRow]) -> i64 {
    let mut horizons: Vec<i64> = rows.iter().map(|row| row.horizon_days).collect();
    horizons.sort_unstable();
    let n = horizons.len();
    if n == 0 { return 0; }
    horizons[n / 2]
}

fn forecast_section(population: &[ForwardRow], radius: &f64) -> Result<String, Box<dyn Error>> {
    let obs: Vec<LogRatioObservation> = population.iter()
        .filter(|row| row.score.is_finite() && row.realized_log_ratio.is_finite())
        .map(|row| LogRatioObservation { predicted: row.score, actual: row.realized_log_ratio, realized_return: Some(row.realized_return) })
        .collect();
    let m = log_ratio_metrics(&obs).ok_or("no finite forecast pairs")?;
    let ic = m.spearman_ic.unwrap_or(f64::NAN);
    let covered = obs.iter().filter(|row| (row.actual - row.predicted).abs() <= *radius).count();
    let coverage = covered as f64 / obs.len() as f64;
    let ds = decile_summary(&obs);
    let mut out = String::new();
    out.push_str("### Headline metrics\n\n");
    out.push_str("| metric | value |\n|---|---|\n");
    out.push_str(&format!("| n | {} |\n", m.count));
    out.push_str(&format!("| Spearman IC (predicted vs realized partial-period log-ratio) | **{}** |\n", fmt(ic, 4)));
    out.push_str(&format!("| RMSE (partial-period log-ratio) | {} |\n", fmt(m.rmse, 4)));
    out.push_str(&format!("| MAE (partial-period log-ratio) | {} |\n", fmt(m.mae, 4)));
    out.push_str(&format!("| hit rate (realized log-ratio > 0) | {} |\n", fmt(hit_rate_positive(population), 4)));
    out.push_str(&format!("| hit rate (realized partial return >= +50%) | {} |\n", fmt(hit_rate_spike(population), 4)));
    out.push_str(&format!("| mean realized partial return | {} |\n", fmt(mean_return(population), 4)));
    out.push_str(&format!("| top-decile vs bottom-decile realized return | {} vs {} |\n", fmt(ds.top_return, 4), fmt(ds.bottom_return, 4)));
    out.push_str(&format!("| 80% conformal interval partial-horizon coverage (radius {}) | {} |\n", fmt(*radius, 4), fmt(coverage, 4)));
    out.push_str("\n### Decile table (ranked by predicted 12m log-ratio; realized = partial window)\n\n");
    out.push_str("| decile | count | mean predicted log_ratio | mean realized log_ratio | mean realized return |\n|---|---|---|---|---|\n");
    for d in &ds.deciles {
        out.push_str(&format!("| {} | {} | {} | {} | {} |\n", d.decile, d.count, fmt(d.mean_predicted, 4), fmt(d.mean_realized, 4), d.mean_return.map_or_else(|| "n/a".to_string(), |v| fmt(v, 4))));
    }
    let model_gem = gem_metrics_for(&top_frac(&gem_observations(population, |row| row.score), 0.1, Date(i32::MAX)), SPIKE_RETURN).ok_or("no gem metrics")?;
    let baselines = baseline_comparison(population, 42);
    out.push_str("\n### Baseline comparison (top decile by signal; hit = partial return >= +50%)\n\n");
    out.push_str("| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |\n|---|---|---|---|---|\n");
    out.push_str(&format!("| **model (predicted log-ratio)** | {} | {} | — | — |\n", fmt(model_gem.median_return, 4), fmt(model_gem.hit_rate, 4)));
    for (b, c) in &baselines {
        out.push_str(&format!("| {} | {} | {} | {} | {} |\n",
            baseline_name(b.baseline), fmt(b.metrics.median_return, 4), fmt(b.metrics.hit_rate, 4),
            fmt(c.median_return_excess, 4), fmt(c.hit_rate_excess, 4)));
    }
    let top_ns = top_n_summary(population, |row| row.score, &[10, 25, 50, 100]);
    out.push_str("\n### Top-N realized performance (by predicted log-ratio)\n\n");
    out.push_str("| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |\n|---|---|---|---|\n");
    for (n, hr_pos, hr_spike, mean) in top_ns {
        out.push_str(&format!("| {} | {} | {} | {} |\n", n, fmt(hr_pos, 4), fmt(hr_spike, 4), fmt(mean, 4)));
    }
    out.push('\n');
    Ok(out)
}

fn baseline_name(baseline: Baseline) -> &'static str {
    match baseline {
        Baseline::TransfermarktValue => "TM value (cheapest first)",
        Baseline::FmCurrentAbility => "FM CA (highest first)",
        Baseline::FmPotentialAbility => "FM PA (highest first)",
        Baseline::AgeMinutes => "age+minutes (young + proven minutes)",
        Baseline::RandomPriceBucket => "random price-bucket (seeded)",
        Baseline::PerformanceOnly => "performance-only",
    }
}

/// Compare the model's top-decile gem metrics against each baseline's.
fn baseline_comparison(population: &[ForwardRow], seed: u64) -> Vec<(BaselineResult, footy_backtest::BaselineComparison)> {
    let model_obs = gem_observations(population, |row| row.score);
    let model_gem = gem_metrics_for(&top_frac(&model_obs, 0.1, Date(i32::MAX)), SPIKE_RETURN).unwrap();
    let mut results = Vec::new();
    let all_baselines = [
        Baseline::TransfermarktValue,
        Baseline::FmCurrentAbility,
        Baseline::FmPotentialAbility,
        Baseline::AgeMinutes,
        Baseline::RandomPriceBucket,
    ];
    for baseline in all_baselines {
        let obs = gem_observations(population, |row| baseline_score(baseline, row, seed));
        let gem = gem_metrics_for(&top_frac(&obs, 0.1, Date(i32::MAX)), SPIKE_RETURN).unwrap();
        let comparison = compare_to_baselines(model_gem, &[BaselineResult { baseline, metrics: gem }]);
        results.push((BaselineResult { baseline, metrics: gem }, comparison[0]));
    }
    results
}

/// Append one optimization-log row to a TSV (config, walkforward_ic, forward_ic,
/// n, coverage, top/bottom decile return, baseline excesses).
struct MetricsRow<'a> {
    path: &'a str,
    label: &'a str,
    walkforward_metric: Option<f64>,
    forward_metric: f64,
    n: usize,
    coverage: f64,
    top_return: f64,
    bottom_return: f64,
    baselines: &'a [(BaselineResult, footy_backtest::BaselineComparison)],
}

fn append_metrics_row(row: MetricsRow<'_>) -> Result<(), Box<dyn Error>> {
    let MetricsRow { path, label, walkforward_metric, forward_metric, n, coverage, top_return, bottom_return, baselines } = row;
    let header = "config\twalkforward_ic\tforward_ic\tn\tpartial_coverage\ttop_decile_return\tbottom_decile_return\tbaseline_excess_median_return\tbaseline_excess_hit_rate";
    let median_excess = baselines.iter().map(|(_, c)| c.median_return_excess).fold(0.0_f64, f64::max);
    let hit_excess = baselines.iter().map(|(_, c)| c.hit_rate_excess).fold(0.0_f64, f64::max);
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        label,
        walkforward_metric.map_or_else(|| "n/a".to_string(), |ic| fmt(ic, 6)),
        fmt(forward_metric, 6),
        n,
        fmt(coverage, 6),
        fmt(top_return, 6),
        fmt(bottom_return, 6),
        fmt(median_excess, 6),
        fmt(hit_excess, 6)
    );
    let mut text = String::new();
    if !std::path::Path::new(path).exists() {
        text.push_str(header);
        text.push('\n');
    }
    text.push_str(&line);
    text.push('\n');
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    use std::io::Write;
    file.write_all(text.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Surge forward test
// ---------------------------------------------------------------------------

fn surge(args: &[String]) -> Result<(), Box<dyn Error>> {
    let model_path = required_value(args, "--model")?;
    let candidates_path = required_value(args, "--candidates")?;
    let realized_path = required_value(args, "--realized")?;
    let meta_path = required_value(args, "--meta")?;
    let as_of = optional_value(args, "--as-of").map(parse_date).transpose()?.unwrap_or(parse_date(T0_DEFAULT)?);
    let horizon_end = optional_value(args, "--horizon-end").map(parse_date).transpose()?.unwrap_or(parse_date(END_DEFAULT)?);
    let seed = optional_value(args, "--seed").map(str::parse).transpose()?.unwrap_or(42_u64);
    let label = optional_value(args, "--label").unwrap_or("surge").to_string();
    let output = optional_value(args, "--output").map(str::to_string).unwrap_or_else(|| "data/forward_test/surge_forward.md".to_string());
    let metrics_out = optional_value(args, "--metrics");

    let model = footy_artifacts::read_surge_model(model_path)?;
    let candidates = parse_candidates(candidates_path, &SURGE_FEATURE_NAMES)?;
    let realized = parse_realized(realized_path)?;
    let meta = parse_meta(meta_path)?;

    let realized_by_player: std::collections::HashMap<u64, &RealizedRow> =
        realized.iter().map(|row| (row.player.0, row)).collect();
    let meta_by_player: std::collections::HashMap<u64, &MetaRow> =
        meta.iter().map(|row| (row.player.0, row)).collect();

    let mut rows: Vec<ForwardRow> = Vec::new();
    for candidate in &candidates {
        let realized_row = match realized_by_player.get(&candidate.player.0) {
            Some(row) => row,
            None => continue,
        };
        if realized_row.mv_realized <= 0.0 || !realized_row.mv_realized.is_finite() {
            continue;
        }
        let (prob, _, _) = model.predict_surge(&candidate.features)?;
        let meta_row = match meta_by_player.get(&candidate.player.0) {
            Some(row) => row,
            None => continue,
        };
        let realized_return = realized_row.mv_realized / candidate.mv_t - 1.0;
        rows.push(ForwardRow {
            player: candidate.player,
            t: candidate.t,
            mv_t: candidate.mv_t,
            mv_realized: realized_row.mv_realized,
            horizon_days: realized_row.horizon_days,
            peak_realized: realized_row.peak_realized,
            score: prob,
            age: meta_row.age,
            ca: meta_row.ca,
            pa_eff: effective_pa(meta_row.pa),
            attr_source: meta_row.attr_source.clone(),
            mins_365: meta_row.mins_365,
            price_bucket: meta_row.price_bucket,
            realized_return,
            realized_log_ratio: realized_return.ln_1p(),
        });
    }
    if rows.is_empty() { return Err("surge forward test has no scored rows".into()); }

    let full: Vec<ForwardRow> = rows.clone();
    let deployable: Vec<ForwardRow> = rows.iter()
        .filter(|row| row.age.is_finite() && row.age <= SURGE_MAX_AGE
            && row.mv_t >= SURGE_MIN_MV && row.mv_t <= SURGE_MAX_MV)
        .cloned()
        .collect();

    // Partial surge label: max valuation in (entry, end] >= 1.5 * mv_t
    let partial_surge = |row: &ForwardRow| -> bool { row.peak_realized >= 1.5 * row.mv_t };

    let evaluate = |population: &[ForwardRow], report: &mut String| -> Result<(), Box<dyn Error>> {
        let labels: Vec<f64> = population.iter().map(|row| if partial_surge(row) { 1.0 } else { 0.0 }).collect();
        let scores: Vec<f64> = population.iter().map(|row| row.score).collect();
        let auc = auc(&labels, &scores).unwrap_or(f64::NAN);
        let cls: Vec<ClassificationObservation> = population.iter()
            .map(|row| ClassificationObservation { label: if partial_surge(row) { 1.0 } else { 0.0 }, score: row.score, realized_return: Some(row.realized_return) })
            .collect();
        let deciles = footy_backtest::reliability_deciles(&cls, 10);
        let ece = expected_calibration_error(&cls, 10).unwrap_or(f64::NAN);
        report.push_str("### Headline metrics\n\n");
        report.push_str("| metric | value |\n|---|---|\n");
        report.push_str(&format!("| n | {} |\n", population.len()));
        report.push_str(&format!("| AUC (model score vs partial-window surge label) | **{}** |\n", fmt(auc, 4)));
        report.push_str(&format!("| partial surge rate (peak >= 1.5x entry within window) | {} |\n", fmt(labels.iter().sum::<f64>() / labels.len() as f64, 4)));
        report.push_str(&format!("| hit rate (realized partial return >= +50%) | {} |\n", fmt(hit_rate_spike(population), 4)));
        report.push_str(&format!("| mean realized partial return | {} |\n", fmt(mean_return(population), 4)));
        let top = top_frac(&gem_observations(population, |row| row.score), 0.1, Date(i32::MAX));
        let bottom = bottom_frac(&gem_observations(population, |row| row.score), 0.1, Date(i32::MAX));
        let top_gem = gem_metrics_for(&top, SPIKE_RETURN).ok_or("no top decile gem metrics")?;
        let bottom_gem = gem_metrics_for(&bottom, SPIKE_RETURN).ok_or("no bottom decile gem metrics")?;
        report.push_str(&format!("| top-decile vs bottom-decile mean realized return | {} vs {} |\n", fmt(top_gem.mean_return, 4), fmt(bottom_gem.mean_return, 4)));
        report.push_str(&format!("| top-decile vs bottom-decile median realized return | {} vs {} |\n", fmt(top_gem.median_return, 4), fmt(bottom_gem.median_return, 4)));
        report.push_str(&format!("| ECE (10 bins) | {} |\n", fmt(ece, 4)));
        report.push_str("\n### Decile table (ranked by predicted surge probability; realized = partial window)\n\n");
        report.push_str("| decile | count | mean predicted prob | realized partial-surge rate | mean realized return |\n|---|---|---|---|---|\n");
        for d in &deciles {
            report.push_str(&format!("| {} | {} | {} | {} | {} |\n", d.decile, d.count, fmt(d.mean_score, 4), fmt(d.realized_rate, 4), d.mean_return.map_or_else(|| "n/a".to_string(), |v| fmt(v, 4))));
        }
        let baselines = baseline_comparison(population, seed);
        report.push_str("\n### Baseline comparison (top decile by signal; hit = partial return >= +50%)\n\n");
        report.push_str("| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |\n|---|---|---|---|---|\n");
        report.push_str(&format!("| **model (predicted surge probability)** | {} | {} | — | — |\n", fmt(top_gem.median_return, 4), fmt(top_gem.hit_rate, 4)));
        for (b, c) in &baselines {
            report.push_str(&format!("| {} | {} | {} | {} | {} |\n",
                baseline_name(b.baseline), fmt(b.metrics.median_return, 4), fmt(b.metrics.hit_rate, 4),
                fmt(c.median_return_excess, 4), fmt(c.hit_rate_excess, 4)));
        }
        let top_ns = top_n_summary(population, |row| row.score, &[10, 25, 50, 100]);
        report.push_str("\n### Top-N realized performance (by predicted surge probability)\n\n");
        report.push_str("| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |\n|---|---|---|---|\n");
        for (n, hr_pos, hr_spike, mean) in top_ns {
            report.push_str(&format!("| {} | {} | {} | {} |\n", n, fmt(hr_pos, 4), fmt(hr_spike, 4), fmt(mean, 4)));
        }
        report.push('\n');
        if let Some(metrics_path) = metrics_out {
            let baselines = baseline_comparison(population, seed);
            let median_excess = baselines.iter().map(|(_, c)| c.median_return_excess).fold(0.0_f64, f64::max);
            let hit_excess = baselines.iter().map(|(_, c)| c.hit_rate_excess).fold(0.0_f64, f64::max);
            append_metrics_row(MetricsRow {
                path: metrics_path,
                label: &label,
                walkforward_metric: Some(model.walkforward.pooled_auc),
                forward_metric: auc,
                n: population.len(),
                coverage: ece,
                top_return: top_gem.mean_return,
                bottom_return: bottom_gem.mean_return,
                baselines: &baselines,
            })?;
            let _ = median_excess;
            let _ = hit_excess;
        }
        Ok(())
    };

    let mut report = String::new();
    report.push_str(&format!("# Forward Test — Surge model ({label})\n\n"));
    report.push_str(&format!("- **As-of anchor**: {} (FM26 database vintage / start of FM26)\n", format_date(as_of)));
    report.push_str(&format!("- **Realized oracle end**: {} (last Transfermarkt valuation in the read-only overlay)\n", format_date(horizon_end)));
    report.push_str(&format!("- **Model**: version {}, dataset {}, feature-set {}, calibration {}, ridge {}, seed {}; trained through {} (rows observed on or before this date)\n",
        model.metadata.model_version.0, model.metadata.dataset_version.0, model.metadata.feature_set_version.0,
        model.calibration_version.0, model.ridge, model.metadata.seed, format_date(model.metadata.trained_through)));
    report.push_str(&format!("- **Entry**: last TM valuation on or before {} per candidate ({} surge candidates have one); realized = last TM valuation in (entry, {}]; censored players dropped\n",
        format_date(as_of), full.len(), format_date(horizon_end)));
    report.push_str(&format!("- **Realized window (measured)**: median horizon {} days (min {}, max {}); realized valuation dates span {} to {}\n",
        median_horizon(&full), full.iter().map(|r| r.horizon_days).min().unwrap_or(0), full.iter().map(|r| r.horizon_days).max().unwrap_or(0),
        format_date(realized.iter().map(|r| r.realized_date).min().unwrap()), format_date(realized.iter().map(|r| r.realized_date).max().unwrap())));
    report.push_str(&format!("- **BacktestRunRecord**: cutoff={}, dataset_version={}, feature_set_version={}, model_version={}, calibration_version={}, seed={}\n\n",
        format_date(as_of), model.metadata.dataset_version.0, model.metadata.feature_set_version.0, model.metadata.model_version.0, model.calibration_version.0, seed));

    report.push_str(&format!("## Full candidate pool (n = {})\n\n", full.len()));
    evaluate(&full, &mut report)?;
    report.push_str(&format!("\n## Deployable population (age <= {}, 400k <= mv <= 1M at entry) (n = {})\n\n", SURGE_MAX_AGE as u32, deployable.len()));
    evaluate(&deployable, &mut report)?;
    report.push_str(&format!("\n## Notes\n\n- Partial-window surge label: `max(valuation in (entry, {}]) >= 1.5 * mv_t`; this is a **partial** 12-month target (median horizon {} days), honestly labeled.\n", format_date(horizon_end), median_horizon(&full)));
    report.push_str(&format!("- Walk-forward (pooled OOF, same config): AUC {}, AUC without FM features {}.\n",
        fmt(model.walkforward.pooled_auc, 4), fmt(model.walkforward.auc_without_fm, 4)));
    fs::write(&output, report)?;
    println!("wrote forward-test report -> {output} (n_full={} n_deployable={})", full.len(), deployable.len());
    Ok(())
}

fn bottom_frac(records: &[GemObservation], frac: f64, cutoff: Date) -> Vec<GemObservation> {
    let mut valid: Vec<GemObservation> = records
        .iter()
        .copied()
        .filter(|row| row.as_of <= cutoff && row.entry_price.cents() > 0 && row.future_price.cents() >= 0 && row.score.is_finite())
        .collect();
    valid.sort_by(|left, right| {
        left.score.total_cmp(&right.score).then_with(|| left.player.cmp(&right.player))
    });
    let count = ((valid.len() as f64) * frac).round().max(1.0) as usize;
    valid.truncate(count);
    valid
}

// ---------------------------------------------------------------------------
// Tail-aware forecast forward test
// ---------------------------------------------------------------------------

fn tail(args: &[String]) -> Result<(), Box<dyn Error>> {
    let model_path = required_value(args, "--model")?;
    let candidates_path = required_value(args, "--candidates")?;
    let realized_path = required_value(args, "--realized")?;
    let meta_path = required_value(args, "--meta")?;
    let as_of = optional_value(args, "--as-of").map(parse_date).transpose()?.unwrap_or(parse_date(T0_DEFAULT)?);
    let horizon_end = optional_value(args, "--horizon-end").map(parse_date).transpose()?.unwrap_or(parse_date(END_DEFAULT)?);
    let seed = optional_value(args, "--seed").map(str::parse).transpose()?.unwrap_or(42_u64);
    let label = optional_value(args, "--label").unwrap_or("tail").to_string();
    let output = optional_value(args, "--output").map(str::to_string).unwrap_or_else(|| "data/forward_test/tail_forward.md".to_string());
    let metrics_out = optional_value(args, "--metrics");

    let model = footy_artifacts::read_tail_forecast_model(model_path)?;
    let (objective_name, objective_param) = match model.objective {
        footy_artifacts::TailObjective::Quantile { tau } => ("quantile", tau),
        footy_artifacts::TailObjective::Expectile { omega } => ("expectile", omega),
    };
    let candidates = parse_candidates(candidates_path, &FORECAST_FEATURE_NAMES)?;
    let realized = parse_realized(realized_path)?;
    let meta = parse_meta(meta_path)?;

    let realized_by_player: std::collections::HashMap<u64, &RealizedRow> =
        realized.iter().map(|row| (row.player.0, row)).collect();
    let meta_by_player: std::collections::HashMap<u64, &MetaRow> =
        meta.iter().map(|row| (row.player.0, row)).collect();

    let mut rows: Vec<ForwardRow> = Vec::new();
    let mut n_missing_realized = 0_usize;
    let mut n_missing_meta = 0_usize;
    for candidate in &candidates {
        if candidate.t > as_of {
            return Err(format!("tail candidate {} has entry after as-of", candidate.player.0).into());
        }
        let realized_row = match realized_by_player.get(&candidate.player.0) {
            Some(row) => row,
            None => { n_missing_realized += 1; continue; }
        };
        if realized_row.mv_realized <= 0.0 || !realized_row.mv_realized.is_finite() {
            n_missing_realized += 1;
            continue;
        }
        let score = model.predict_tail(&candidate.features)?;
        let meta_row = match meta_by_player.get(&candidate.player.0) {
            Some(row) => row,
            None => { n_missing_meta += 1; continue; }
        };
        let realized_return = realized_row.mv_realized / candidate.mv_t - 1.0;
        rows.push(ForwardRow {
            player: candidate.player,
            t: candidate.t,
            mv_t: candidate.mv_t,
            mv_realized: realized_row.mv_realized,
            horizon_days: realized_row.horizon_days,
            peak_realized: realized_row.peak_realized,
            score,
            age: meta_row.age,
            ca: meta_row.ca,
            pa_eff: effective_pa(meta_row.pa),
            attr_source: meta_row.attr_source.clone(),
            mins_365: meta_row.mins_365,
            price_bucket: meta_row.price_bucket,
            realized_return,
            realized_log_ratio: realized_return.ln_1p(),
        });
    }
    if rows.is_empty() { return Err("tail forward test has no scored rows".into()); }

    let full: Vec<ForwardRow> = rows.clone();
    let evaluate = |population: &[ForwardRow], report: &mut String| -> Result<(), Box<dyn Error>> {
        let obs: Vec<LogRatioObservation> = population.iter()
            .filter(|row| row.score.is_finite() && row.realized_log_ratio.is_finite())
            .map(|row| LogRatioObservation { predicted: row.score, actual: row.realized_log_ratio, realized_return: Some(row.realized_return) })
            .collect();
        let m = log_ratio_metrics(&obs).ok_or("no finite tail pairs")?;
        let ic = m.spearman_ic.unwrap_or(f64::NAN);
        // High-quantile calibration: for a tau-quantile model, P(actual <= predicted) ~= tau.
        let covered = obs.iter().filter(|row| row.actual <= row.predicted).count();
        let q_coverage = covered as f64 / obs.len() as f64;
        let spike50 = hit_rate_spike(population);
        let spike100 = population.iter()
            .filter(|row| row.realized_return.is_finite() && row.realized_return >= 1.0)
            .count() as f64 / population.len() as f64;
        let returns: Vec<f64> = population.iter().filter(|row| row.realized_return.is_finite()).map(|row| row.realized_return).collect();
        let mean_r = returns.iter().sum::<f64>() / returns.len() as f64;
        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(f64::total_cmp);
        let median_r = sorted_returns[sorted_returns.len() / 2];
        let deciles = tail_decile_table(population);
        let top_gem = gem_metrics_for(&top_frac(&gem_observations(population, |row| row.score), 0.1, Date(i32::MAX)), SPIKE_RETURN).ok_or("no top decile gem metrics")?;
        let bottom_gem = gem_metrics_for(&bottom_frac(&gem_observations(population, |row| row.score), 0.1, Date(i32::MAX)), SPIKE_RETURN).ok_or("no bottom decile gem metrics")?;
        let baselines = baseline_comparison(population, seed);

        report.push_str("### Headline metrics\n\n");
        report.push_str("| metric | value |\n|---|---|\n");
        report.push_str(&format!("| n | {} |\n", population.len()));
        report.push_str(&format!("| Spearman IC (tail score vs realized partial-period log-ratio) | **{}** |\n", fmt(ic, 4)));
        report.push_str(&format!("| high-quantile coverage P(realized log-ratio <= tail score) | {} (target {}) |\n", fmt(q_coverage, 4), objective_param));
        report.push_str(&format!("| hit rate (realized partial return >= +50%) | {} |\n", fmt(spike50, 4)));
        report.push_str(&format!("| hit rate (realized partial return >= +100%) | {} |\n", fmt(spike100, 4)));
        report.push_str(&format!("| mean realized partial return | {} |\n", fmt(mean_r, 4)));
        report.push_str(&format!("| median realized partial return | {} |\n", fmt(median_r, 4)));
        report.push_str(&format!("| top-decile vs bottom-decile mean return | {} vs {} |\n", fmt(top_gem.mean_return, 4), fmt(bottom_gem.mean_return, 4)));
        report.push_str(&format!("| top-decile vs bottom-decile median return | {} vs {} |\n", fmt(top_gem.median_return, 4), fmt(bottom_gem.median_return, 4)));
        report.push_str("\n### Decile table (ranked by tail score; realized = partial window)\n\n");
        report.push_str("| decile | count | mean predicted | mean realized log-ratio | mean return | median return | spike50 rate | spike100 rate |\n|---|---|---|---|---|---|---|---|\n");
        for d in &deciles {
            report.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} |\n", d.decile, d.count, fmt(d.mean_predicted, 4), fmt(d.mean_realized, 4), fmt(d.mean_return, 4), fmt(d.median_return, 4), fmt(d.spike50_rate, 4), fmt(d.spike100_rate, 4)));
        }
        report.push_str("\n### Baseline comparison (top decile by signal; hit = partial return >= +50%)\n\n");
        report.push_str("| baseline | top-decile median return | top-decile hit rate | median-return excess vs model | hit-rate excess vs model |\n|---|---|---|---|---|\n");
        report.push_str(&format!("| **model (tail score)** | {} | {} | — | — |\n", fmt(top_gem.median_return, 4), fmt(top_gem.hit_rate, 4)));
        for (b, c) in &baselines {
            report.push_str(&format!("| {} | {} | {} | {} | {} |\n",
                baseline_name(b.baseline), fmt(b.metrics.median_return, 4), fmt(b.metrics.hit_rate, 4),
                fmt(c.median_return_excess, 4), fmt(c.hit_rate_excess, 4)));
        }
        let top_ns = top_n_summary(population, |row| row.score, &[10, 25, 50, 100]);
        report.push_str("\n### Top-N realized performance (by tail score)\n\n");
        report.push_str("| N | hit rate (return > 0) | hit rate (return >= +50%) | mean realized return |\n|---|---|---|---|\n");
        for (n, hr_pos, hr_spike, mean) in top_ns {
            report.push_str(&format!("| {} | {} | {} | {} |\n", n, fmt(hr_pos, 4), fmt(hr_spike, 4), fmt(mean, 4)));
        }
        report.push('\n');
        if let Some(metrics_path) = metrics_out {
            let median_excess = baselines.iter().map(|(_, c)| c.median_return_excess).fold(0.0_f64, f64::max);
            let hit_excess = baselines.iter().map(|(_, c)| c.hit_rate_excess).fold(0.0_f64, f64::max);
            append_metrics_row(MetricsRow {
                path: metrics_path,
                label: &label,
                walkforward_metric: model.walkforward.pooled_rank_ic,
                forward_metric: ic,
                n: population.len(),
                coverage: q_coverage,
                top_return: top_gem.mean_return,
                bottom_return: bottom_gem.mean_return,
                baselines: &baselines,
            })?;
            let _ = median_excess;
            let _ = hit_excess;
        }
        Ok(())
    };

    let mut report = String::new();
    report.push_str(&format!("# Forward Test — Tail-aware forecast model ({label})\n\n"));
    report.push_str(&format!("- **As-of anchor**: {} (FM26 database vintage / start of FM26)\n", format_date(as_of)));
    report.push_str(&format!("- **Realized oracle end**: {} (last Transfermarkt valuation in the read-only overlay)\n", format_date(horizon_end)));
    report.push_str(&format!("- **Model**: version {}, dataset {}, feature-set {}, calibration {}, ridge {}, seed {}; objective {}-{}; trained through {} (rows observed on or before this date)\n",
        model.metadata.model_version.0, model.metadata.dataset_version.0, model.metadata.feature_set_version.0,
        model.calibration_version.0, model.ridge, model.metadata.seed, objective_name, objective_param, format_date(model.metadata.trained_through)));
    report.push_str(&format!("- **Entry**: last TM valuation on or before {} per player; realized = last TM valuation in (entry, {}]; players with no realized valuation after entry are dropped (censored), never encoded as zero\n",
        format_date(as_of), format_date(horizon_end)));
    report.push_str(&format!("- **Realized window (measured)**: median horizon {} days (min {}, max {}); realized valuation dates span {} to {}\n",
        median_horizon(&full), full.iter().map(|r| r.horizon_days).min().unwrap_or(0), full.iter().map(|r| r.horizon_days).max().unwrap_or(0),
        format_date(realized.iter().map(|r| r.realized_date).min().unwrap()), format_date(realized.iter().map(|r| r.realized_date).max().unwrap())));
    report.push_str(&format!("- **BacktestRunRecord**: cutoff={}, dataset_version={}, feature_set_version={}, model_version={}, calibration_version={}, seed={}\n\n",
        format_date(as_of), model.metadata.dataset_version.0, model.metadata.feature_set_version.0, model.metadata.model_version.0, model.calibration_version.0, seed));

    report.push_str(&format!("## Scored pool (n = {})\n\n", full.len()));
    evaluate(&full, &mut report)?;
    report.push_str(&format!("\n## Notes\n\n- The tail score is the fitted conditional {}-{} of the 12-month log value ratio; higher = stronger spike signal. Coverage measures the share of realized partial-period log-ratios at or below the prediction; for a well-calibrated {} model this is close to {} (partial-window dispersion is narrower, so slight under-coverage is expected here).\n", objective_name, objective_param, objective_name, objective_param));
    report.push_str(&format!("- Walk-forward (pooled OOF, same config): rank IC {}, q-coverage {}, top-decile spike50 {} / spike100 {}; top-decile median return {} (mean {}).\n",
        model.walkforward.pooled_rank_ic.map_or_else(|| "n/a".to_string(), |ic| fmt(ic, 4)),
        fmt(model.walkforward.q_coverage, 4),
        fmt(model.walkforward.deciles.first().map_or(f64::NAN, |d| d.spike50_rate), 4),
        fmt(model.walkforward.deciles.first().map_or(f64::NAN, |d| d.spike100_rate), 4),
        fmt(model.walkforward.deciles.first().map_or(f64::NAN, |d| d.median_return), 4),
        fmt(model.walkforward.deciles.first().map_or(f64::NAN, |d| d.mean_return), 4)));
    fs::write(&output, report)?;
    println!("wrote forward-test report -> {output} (n={} missing_realized={} missing_meta={})", full.len(), n_missing_realized, n_missing_meta);
    Ok(())
}

/// Deterministic decile snapshots of the forward-test pool ranked by the tail
/// score, with spike hit rates at >= +50% and >= +100% partial returns.
fn tail_decile_table(rows: &[ForwardRow]) -> Vec<TailForwardDecile> {
    let mut sorted: Vec<&ForwardRow> = rows.iter().filter(|row| row.score.is_finite()).collect();
    sorted.sort_by(|left, right| right.score.total_cmp(&left.score).then_with(|| left.player.cmp(&right.player)));
    let n = sorted.len();
    let size = (n + 9) / 10;
    let mut out = Vec::with_capacity(10);
    for decile in 0..10 {
        let slice: Vec<&ForwardRow> = sorted.iter().skip(decile * size).take(size).copied().collect();
        let count = slice.len();
        let mean_predicted = slice.iter().map(|row| row.score).sum::<f64>() / count as f64;
        let mean_realized = slice.iter().map(|row| row.realized_log_ratio).sum::<f64>() / count as f64;
        let returns: Vec<f64> = slice.iter().filter(|row| row.realized_return.is_finite()).map(|row| row.realized_return).collect();
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(f64::total_cmp);
        let median_return = sorted_returns[sorted_returns.len() / 2];
        out.push(TailForwardDecile {
            decile: decile as u8,
            count: count.min(u32::MAX as usize) as u32,
            mean_predicted,
            mean_realized,
            mean_return,
            median_return,
            spike50_rate: slice.iter().filter(|row| row.realized_return.is_finite() && row.realized_return >= 0.5).count() as f64 / count as f64,
            spike100_rate: slice.iter().filter(|row| row.realized_return.is_finite() && row.realized_return >= 1.0).count() as f64 / count as f64,
        });
    }
    out
}

#[derive(Debug, Clone)]
struct TailForwardDecile {
    decile: u8,
    count: u32,
    mean_predicted: f64,
    mean_realized: f64,
    mean_return: f64,
    median_return: f64,
    spike50_rate: f64,
    spike100_rate: f64,
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(player: u64, score: f64, mv_t: f64, mv_r: f64) -> ForwardRow {
        ForwardRow {
            player: PlayerId(player),
            t: Date(20398),
            mv_t,
            mv_realized: mv_r,
            horizon_days: 218,
            peak_realized: mv_r,
            score,
            age: 20.0,
            ca: 120.0,
            pa_eff: 150.0,
            attr_source: "db".to_string(),
            mins_365: 1000.0,
            price_bucket: 0,
            realized_return: mv_r / mv_t - 1.0,
            realized_log_ratio: (mv_r / mv_t).ln(),
        }
    }

    #[test]
    fn splitmix64_is_deterministic_and_in_unit_interval() {
        let a = splitmix64(42);
        let b = splitmix64(42);
        assert_eq!(a, b);
        assert!((0.0..1.0).contains(&a));
    }

    #[test]
    fn parse_candidates_round_trips_schema() {
        let path = std::env::temp_dir().join("footy-forward-test-candidates.tsv");
        let mut text = FORECAST_FEATURE_NAMES.join("\t");
        text.push_str("\tplayer_id\tt\tmv_t\n");
        let mut values: Vec<String> = vec!["1.0".to_string(); 47];
        values.extend(["123", "20398", "500000"].iter().map(|v| v.to_string()));
        text.push_str(&values.join("\t"));
        text.push('\n');
        std::fs::write(&path, &text).unwrap();
        let rows = parse_candidates(path.to_str().unwrap(), &FORECAST_FEATURE_NAMES).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].player.0, 123);
        assert_eq!(rows[0].t, Date(20398));
        assert_eq!(rows[0].mv_t, 500_000.0);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn forecast_metrics_rank_perfectly() {
        // 20 players, score perfectly aligned with realized return
        let rows: Vec<ForwardRow> = (0..20)
            .map(|i| sample_row(i as u64, i as f64, 100.0, 100.0 + 10.0 * i as f64))
            .collect();
        let obs: Vec<LogRatioObservation> = rows.iter()
            .map(|row| LogRatioObservation { predicted: row.score, actual: row.realized_log_ratio, realized_return: Some(row.realized_return) })
            .collect();
        let ic = log_ratio_metrics(&obs).unwrap().spearman_ic.unwrap();
        assert!((ic - 1.0).abs() < 1e-9);
        let ds = decile_summary(&obs);
        assert!(ds.top_return > ds.bottom_return);
    }

    #[test]
    fn baseline_random_price_bucket_is_seeded() {
        let rows: Vec<ForwardRow> = (0..50).map(|i| sample_row(i, 0.5, 300_000.0 + i as f64 * 10_000.0, 400_000.0)).collect();
        let s1 = baseline_score(Baseline::RandomPriceBucket, &rows[0], 42);
        let s2 = baseline_score(Baseline::RandomPriceBucket, &rows[0], 42);
        let s3 = baseline_score(Baseline::RandomPriceBucket, &rows[0], 43);
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }
}
