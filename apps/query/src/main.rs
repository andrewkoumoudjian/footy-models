#![forbid(unsafe_code)]

use footy_artifacts::{
    read_dataset_manifest, read_feature_matrix, read_header, read_linear_price_model,
    read_prediction_batch,
};
use footy_core::{Date, PlayerId};
use footy_dataset::{feature_name, read_canonical_dataset};
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("header") => header(&args[1..]),
        Some("dataset") => dataset(&args[1..]),
        Some("canonical") => canonical(&args[1..]),
        Some("matrix") => matrix(&args[1..]),
        Some("model") => model(&args[1..]),
        Some("predictions") => predictions(&args[1..]),
        _ => Err("usage: footy-query <header|dataset|canonical|matrix|model|predictions> --input artifact [--player ID] [--as-of YYYY-MM-DD]".into()),
    }
}

fn header(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let header = read_header(input)?;
    println!("kind={:?}", header.kind);
    println!("schema_version={}", header.schema_version);
    println!("dataset_version={}", header.versions.dataset.0);
    println!("feature_set_version={}", optional_u64(header.versions.feature_set.map(|value| value.0)));
    println!("model_version={}", optional_u64(header.versions.model.map(|value| value.0)));
    println!("calibration_version={}", optional_u64(header.versions.calibration.map(|value| value.0)));
    println!("cutoff={}", header.versions.cutoff.map(format_date).unwrap_or_else(|| "none".into()));
    println!("payload_len={}", header.payload_len);
    println!("payload_checksum={}", header.payload_checksum);
    Ok(())
}

fn dataset(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let manifest = read_dataset_manifest(input)?;
    println!("dataset_version={} complete={} sources={}", manifest.dataset_version.0, manifest.complete, manifest.sources.len());
    println!("source\tschema\tbytes\trecords\tissues\tchecksum\tpath");
    for source in manifest.sources {
        println!("{:?}\t{}\t{}\t{}\t{}\t{}\t{}", source.source, source.schema_version, source.byte_len, source.record_count, source.issue_count, source.checksum, source.input_path);
    }
    Ok(())
}

fn canonical(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let player_filter = optional_value(args, "--player").map(str::parse::<u64>).transpose()?.map(PlayerId);
    let dataset = read_canonical_dataset(input)?;
    println!("dataset_version={}", dataset.dataset_version.0);
    println!("source_manifest_checksum={}", dataset.source_manifest_checksum);
    println!("players={}", dataset.players.len());
    println!("identities={}", dataset.identities.len());
    println!("unresolved_identities={}", dataset.unresolved_identities.len());
    println!("issues={}", dataset.issues.len());
    println!("fm_snapshots={}", dataset.fm_snapshots.len());
    println!("market_values={}", dataset.market_values.len());
    println!("transfers={}", dataset.transfers.len());
    println!("contracts={}", dataset.contracts.len());
    println!("performance_observations={}", dataset.performance.len());
    println!("salary_observations={}", dataset.salaries.len());
    println!("metrics={}", dataset.metric_registry.len());
    if let Some(player) = player_filter {
        let canonical = dataset.player(player).ok_or("canonical player not found")?;
        println!("player_id={} name={} fm_external_id={} birth_date={}", player.0, canonical.name, canonical.fm_external_id, canonical.birth_date.map(format_date).unwrap_or_else(|| "unknown".into()));
        println!("source\texternal_id\tscore\tmethod\tverified");
        for identity in dataset.identities.iter().filter(|identity| identity.player == player) {
            println!("{:?}\t{}\t{}\t{:?}\t{}", identity.source, identity.external_id, identity.score.get(), identity.method, identity.verified);
        }
        println!("player_fm_snapshots={}", dataset.fm_snapshots.iter().filter(|value| value.player == player).count());
        println!("player_market_values={}", dataset.market_values.iter().filter(|value| value.player == player).count());
        println!("player_transfers={}", dataset.transfers.iter().filter(|value| value.player == player).count());
        println!("player_performance={}", dataset.performance.iter().filter(|value| value.player == player).count());
        println!("player_salaries={}", dataset.salaries.iter().filter(|value| value.player == player).count());
    } else if !dataset.unresolved_identities.is_empty() {
        println!("unresolved_source\texternal_id\tname\treason\tcandidates");
        for unresolved in dataset.unresolved_identities.iter().take(100) {
            println!("{:?}\t{}\t{}\t{:?}\t{}", unresolved.source, unresolved.external_id, unresolved.name, unresolved.reason, unresolved.candidates.len());
        }
    }
    Ok(())
}

fn matrix(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let matrix = read_feature_matrix(input)?;
    let targets = matrix.rows.iter().filter(|row| row.target_log_price.is_some()).count();
    let min_date = matrix.rows.iter().map(|row| row.as_of).min();
    let max_date = matrix.rows.iter().map(|row| row.as_of).max();
    println!("dataset_version={}", matrix.dataset_version.0);
    println!("feature_set_version={}", matrix.feature_set_version.0);
    println!("source_manifest_checksum={}", matrix.source_manifest_checksum);
    println!("features={}", matrix.feature_ids.len());
    println!("rows={}", matrix.rows.len());
    println!("target_rows={targets}");
    println!("min_as_of={}", min_date.map(format_date).unwrap_or_else(|| "none".into()));
    println!("max_as_of={}", max_date.map(format_date).unwrap_or_else(|| "none".into()));
    print!("feature_ids=");
    for (index, id) in matrix.feature_ids.iter().enumerate() { if index > 0 { print!(","); } print!("{}", id.0); }
    println!();
    Ok(())
}

fn model(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let model = read_linear_price_model(input)?;
    println!("dataset_version={}", model.metadata.dataset_version.0);
    println!("feature_set_version={}", model.metadata.feature_set_version.0);
    println!("model_version={}", model.metadata.model_version.0);
    println!("calibration_version={}", model.calibration_version.0);
    println!("trained_through={}", format_date(model.metadata.trained_through));
    println!("calibration_cutoff={}", format_date(model.calibration_cutoff));
    println!("seed={}", model.metadata.seed);
    println!("ridge={}", model.ridge);
    println!("features={}", model.feature_ids.len());
    println!("p50_radius={} p50_samples={}", model.p50_calibrator.radius, model.p50_calibrator.calibration_size);
    println!("p80_radius={} p80_samples={}", model.p80_calibrator.radius, model.p80_calibrator.calibration_size);
    Ok(())
}

fn predictions(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = required_value(args, "--input")?;
    let player_filter = optional_value(args, "--player").map(str::parse::<u64>).transpose()?;
    let date_filter = optional_value(args, "--as-of").map(parse_date).transpose()?;
    let batch = read_prediction_batch(input)?;
    println!("player_id\tas_of\tp10_cents\tp25_cents\tp50_cents\tp75_cents\tp90_cents\tconfidence");
    let mut shown = 0_usize;
    for row in batch.rows {
        if player_filter.is_some_and(|player| row.player.0 != player) || date_filter.is_some_and(|date| row.as_of != date) { continue; }
        println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", row.player.0, format_date(row.as_of), row.value.p10.cents(), row.value.p25.cents(), row.value.p50.cents(), row.value.p75.cents(), row.value.p90.cents(), row.value.confidence.get());
        shown += 1;
    }
    eprintln!("rows={shown} scoring_dataset={} training_dataset={} feature_set={} model={} calibration={}", batch.scoring_dataset.0, batch.training_dataset.0, batch.feature_set_version.0, batch.model_version.0, batch.calibration_version.0);
    Ok(())
}

fn format_date(date: Date) -> String {
    let (year, month, day) = date.to_ymd();
    format!("{year:04}-{month:02}-{day:02}")
}

fn parse_date(value: &str) -> Result<Date, Box<dyn Error>> {
    let mut parts = value.split('-');
    let year = parts.next().ok_or("missing year")?.parse::<i32>()?;
    let month = parts.next().ok_or("missing month")?.parse::<u8>()?;
    let day = parts.next().ok_or("missing day")?.parse::<u8>()?;
    if parts.next().is_some() { return Err("date must be YYYY-MM-DD".into()); }
    Ok(Date::from_ymd(year, month, day)?)
}

fn optional_u64(value: Option<u64>) -> String { value.map(|number| number.to_string()).unwrap_or_else(|| "none".into()) }

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    optional_value(args, flag).ok_or_else(|| format!("missing {flag}").into())
}

fn optional_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1).map(String::as_str)
}

#[allow(dead_code)]
fn feature_label(canonical_path: &str, feature: footy_core::FeatureId) -> Result<String, Box<dyn Error>> {
    let dataset = read_canonical_dataset(canonical_path)?;
    Ok(feature_name(&dataset, feature).unwrap_or_else(|| format!("feature:{}", feature.0)))
}
