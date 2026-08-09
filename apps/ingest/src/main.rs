#![forbid(unsafe_code)]

use footy_artifacts::{
    checksum_file, read_dataset_manifest, write_dataset_manifest, DatasetManifest, SourceKind,
    SourceManifest,
};
use footy_core::DatasetVersion;
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("stage") => stage(&args[1..]),
        Some("verify") => verify(&args[1..]),
        _ => Err(usage().into()),
    }
}

fn usage() -> &'static str {
    "usage:\n  footy-ingest stage --dataset-version N --output manifest.bin --source kind,schema,path [--source ...]\n  footy-ingest verify --manifest manifest.bin\nsource kinds: fm, transfermarkt, fbref, salary, manual"
}

fn stage(args: &[String]) -> Result<(), Box<dyn Error>> {
    let dataset_version = required_u64(args, "--dataset-version")?;
    let output = required_value(args, "--output")?;
    let specs = repeated_values(args, "--source");
    if specs.is_empty() {
        return Err("at least one --source is required".into());
    }

    let mut sources = Vec::with_capacity(specs.len());
    for spec in specs {
        let (kind, schema_version, path) = parse_source_spec(spec)?;
        let raw = fs::read(path)?;
        if raw.is_empty() {
            return Err(format!("source is empty: {path}").into());
        }
        let text = std::str::from_utf8(&raw).map_err(|_| format!("source must be UTF-8 in this staging slice: {path}"))?;
        let record_count = text.lines().filter(|line| !line.trim().is_empty()).count().saturating_sub(1) as u64;
        if record_count == 0 {
            return Err(format!("source has no non-header records: {path}").into());
        }
        sources.push(SourceManifest {
            source: kind,
            input_path: path.to_owned(),
            schema_version,
            byte_len: raw.len() as u64,
            checksum: footy_artifacts::checksum_bytes(&raw),
            record_count,
            issue_count: 0,
        });
    }

    let manifest = DatasetManifest {
        dataset_version: DatasetVersion(dataset_version),
        complete: true,
        sources,
    };
    let header = write_dataset_manifest(output, &manifest)?;
    println!(
        "dataset_version={} sources={} payload_checksum={} output={}",
        dataset_version,
        manifest.sources.len(),
        header.payload_checksum,
        output
    );
    for source in &manifest.sources {
        println!(
            "source={:?} schema={} bytes={} records={} checksum={}",
            source.source, source.schema_version, source.byte_len, source.record_count, source.checksum
        );
    }
    Ok(())
}

fn verify(args: &[String]) -> Result<(), Box<dyn Error>> {
    let manifest_path = required_value(args, "--manifest")?;
    let manifest = read_dataset_manifest(manifest_path)?;
    if !manifest.complete {
        return Err("dataset manifest is not complete".into());
    }
    for source in &manifest.sources {
        let (byte_len, checksum) = checksum_file(&source.input_path)?;
        if byte_len != source.byte_len || checksum != source.checksum {
            return Err(format!("source drift detected: {}", source.input_path).into());
        }
        println!(
            "verified source={:?} schema={} bytes={} records={} checksum={}",
            source.source, source.schema_version, byte_len, source.record_count, checksum
        );
    }
    println!("verified dataset_version={} manifest={manifest_path}", manifest.dataset_version.0);
    Ok(())
}

fn parse_source_spec(spec: &str) -> Result<(SourceKind, u16, &str), Box<dyn Error>> {
    let mut parts = spec.splitn(3, ',');
    let kind = match parts.next().unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "fm" | "footballmanager" => SourceKind::FootballManager,
        "transfermarkt" | "tm" => SourceKind::Transfermarkt,
        "fbref" => SourceKind::Fbref,
        "salary" | "wage" => SourceKind::Salary,
        "manual" => SourceKind::Manual,
        _ => return Err(format!("unknown source kind in {spec}").into()),
    };
    let schema_version = parts.next().ok_or("source spec missing schema version")?.parse::<u16>()?;
    let path = parts.next().ok_or("source spec missing path")?.trim();
    if path.is_empty() || !Path::new(path).is_file() {
        return Err(format!("source path is not a file: {path}").into());
    }
    Ok((kind, schema_version, path))
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    let index = args.iter().position(|arg| arg == flag).ok_or_else(|| format!("missing {flag}"))?;
    args.get(index + 1).map(String::as_str).ok_or_else(|| format!("missing value for {flag}").into())
}

fn required_u64(args: &[String], flag: &str) -> Result<u64, Box<dyn Error>> {
    Ok(required_value(args, flag)?.parse::<u64>()?)
}

fn repeated_values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.iter().enumerate().filter_map(|(index, arg)| {
        (arg == flag).then(|| args.get(index + 1).map(String::as_str)).flatten()
    }).collect()
}
