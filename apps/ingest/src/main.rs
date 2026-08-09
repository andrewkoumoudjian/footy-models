#![forbid(unsafe_code)]

use footy_artifacts::{
    checksum_file, read_dataset_manifest, write_dataset_manifest, DatasetManifest, SourceKind,
    SourceManifest,
};
use footy_core::{DatasetVersion, ProvenanceId, Source};
use footy_dataset::{
    build_canonical_dataset, infer_fm_schema, parse_external_players_tsv,
    parse_fbref_performance_tsv, parse_identity_overrides_tsv, parse_salary_tsv,
    parse_tm_contracts_tsv, parse_tm_market_values_tsv, parse_tm_transfers_tsv,
    write_canonical_dataset, CanonicalBuildConfig, CanonicalProvenance, ExternalPlayerRecord,
};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("stage") => stage(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some("canonicalize") => canonicalize(&args[1..]),
        _ => Err(usage().into()),
    }
}

fn usage() -> &'static str {
    "usage:\n  footy-ingest stage --dataset-version N --output manifest.bin --source kind,schema,path [--source ...]\n  footy-ingest verify --manifest manifest.bin\n  footy-ingest canonicalize --manifest manifest.bin --fm fm.tsv --output canonical.bin [--external-players source,path ...] [--tm-values path ...] [--tm-transfers path ...] [--tm-contracts path ...] [--fbref-performance path ...] [--salary path ...] [--identity-overrides path] [--minimum-identity-score F] [--ambiguity-margin F]\nsource kinds: fm, transfermarkt, fbref, salary, manual\nnormalized FM columns: uid,date,name,[dob,ca,pa,reputation],attr:<id>...,position:<id>...,personality:<id>...,hidden:<id>..."
}

fn stage(args: &[String]) -> Result<(), Box<dyn Error>> {
    let dataset_version = required_u64(args, "--dataset-version")?;
    let output = required_value(args, "--output")?;
    let specs = repeated_values(args, "--source");
    if specs.is_empty() { return Err("at least one --source is required".into()); }

    let mut sources = Vec::with_capacity(specs.len());
    for spec in specs {
        let (kind, schema_version, path) = parse_source_spec(spec)?;
        let raw = fs::read(path)?;
        if raw.is_empty() { return Err(format!("source is empty: {path}").into()); }
        let text = std::str::from_utf8(&raw).map_err(|_| format!("source must be UTF-8 in this staging slice: {path}"))?;
        let record_count = text.lines().filter(|line| !line.trim().is_empty()).count().saturating_sub(1) as u64;
        if record_count == 0 { return Err(format!("source has no non-header records: {path}").into()); }
        sources.push(SourceManifest { source: kind, input_path: path.to_owned(), schema_version, byte_len: raw.len() as u64, checksum: footy_artifacts::checksum_bytes(&raw), record_count, issue_count: 0 });
    }

    let manifest = DatasetManifest { dataset_version: DatasetVersion(dataset_version), complete: true, sources };
    let header = write_dataset_manifest(output, &manifest)?;
    println!("dataset_version={} sources={} payload_checksum={} output={}", dataset_version, manifest.sources.len(), header.payload_checksum, output);
    for source in &manifest.sources {
        println!("source={:?} schema={} bytes={} records={} checksum={}", source.source, source.schema_version, source.byte_len, source.record_count, source.checksum);
    }
    Ok(())
}

fn verify(args: &[String]) -> Result<(), Box<dyn Error>> {
    let manifest_path = required_value(args, "--manifest")?;
    let manifest = read_dataset_manifest(manifest_path)?;
    verify_manifest(&manifest)?;
    println!("verified dataset_version={} manifest={manifest_path}", manifest.dataset_version.0);
    Ok(())
}

fn canonicalize(args: &[String]) -> Result<(), Box<dyn Error>> {
    let manifest_path = required_value(args, "--manifest")?;
    let fm_path = required_value(args, "--fm")?;
    let output = required_value(args, "--output")?;
    let manifest = read_dataset_manifest(manifest_path)?;
    verify_manifest(&manifest)?;
    let (_, source_manifest_checksum) = checksum_file(manifest_path)?;
    let manifest_index = ManifestIndex::new(&manifest)?;

    let (fm_provenance, fm_schema_version) = manifest_index.require(fm_path, SourceKind::FootballManager)?;
    let fm_text = fs::read_to_string(fm_path)?;
    let fm_schema = infer_fm_schema(&fm_text, fm_schema_version)?;
    let fm_report = footy_source_fm::parse_tsv(&fm_text, &fm_schema, fm_provenance);
    if !fm_report.issues.is_empty() {
        return Err(format!("FM parsing failed with {} issue(s); first: {:?}", fm_report.issues.len(), fm_report.issues.first()).into());
    }
    if !fm_report.unknown_columns.is_empty() {
        eprintln!("fm_unknown_columns={} (ignored by normalized schema)", fm_report.unknown_columns.len());
    }

    let mut external_players: Vec<ExternalPlayerRecord> = Vec::new();
    for spec in repeated_values(args, "--external-players") {
        let (source, kind, path) = parse_external_player_spec(spec)?;
        let (provenance, _) = manifest_index.require(path, kind)?;
        let report = parse_external_players_tsv(&fs::read_to_string(path)?, source, provenance);
        fail_parse_report(path, &report.issues)?;
        external_players.extend(report.records);
    }

    let mut tm_values = Vec::new();
    for path in repeated_values(args, "--tm-values") {
        let (provenance, _) = manifest_index.require(path, SourceKind::Transfermarkt)?;
        let report = parse_tm_market_values_tsv(&fs::read_to_string(path)?, provenance);
        fail_parse_report(path, &report.issues)?;
        tm_values.extend(report.records);
    }
    let mut tm_transfers = Vec::new();
    for path in repeated_values(args, "--tm-transfers") {
        let (provenance, _) = manifest_index.require(path, SourceKind::Transfermarkt)?;
        let report = parse_tm_transfers_tsv(&fs::read_to_string(path)?, provenance);
        fail_parse_report(path, &report.issues)?;
        tm_transfers.extend(report.records);
    }
    let mut tm_contracts = Vec::new();
    for path in repeated_values(args, "--tm-contracts") {
        let (provenance, _) = manifest_index.require(path, SourceKind::Transfermarkt)?;
        let report = parse_tm_contracts_tsv(&fs::read_to_string(path)?, provenance);
        fail_parse_report(path, &report.issues)?;
        tm_contracts.extend(report.records);
    }
    let mut fbref_performance = Vec::new();
    for path in repeated_values(args, "--fbref-performance") {
        let (provenance, _) = manifest_index.require(path, SourceKind::Fbref)?;
        let report = parse_fbref_performance_tsv(&fs::read_to_string(path)?, provenance);
        fail_parse_report(path, &report.issues)?;
        fbref_performance.extend(report.records);
    }
    let mut salaries = Vec::new();
    for path in repeated_values(args, "--salary") {
        let (provenance, _) = manifest_index.require(path, SourceKind::Salary)?;
        let report = parse_salary_tsv(&fs::read_to_string(path)?, provenance);
        fail_parse_report(path, &report.issues)?;
        salaries.extend(report.records);
    }
    let overrides = if let Some(path) = optional_value(args, "--identity-overrides") {
        let _ = manifest_index.require(path, SourceKind::Manual)?;
        let report = parse_identity_overrides_tsv(&fs::read_to_string(path)?);
        fail_parse_report(path, &report.issues)?;
        report.records
    } else { Vec::new() };

    let minimum_identity_score = optional_value(args, "--minimum-identity-score").map(str::parse::<f32>).transpose()?.unwrap_or(0.88);
    let ambiguity_margin = optional_value(args, "--ambiguity-margin").map(str::parse::<f32>).transpose()?.unwrap_or(0.05);
    let dataset = build_canonical_dataset(
        manifest.dataset_version,
        source_manifest_checksum,
        manifest_index.provenances(),
        fm_report.records,
        external_players,
        tm_values,
        tm_transfers,
        tm_contracts,
        fbref_performance,
        salaries,
        overrides,
        CanonicalBuildConfig { minimum_identity_score, ambiguity_margin, ..CanonicalBuildConfig::default() },
    )?;
    write_canonical_dataset(output, &dataset)?;
    println!(
        "canonicalized dataset={} players={} identities={} unresolved={} issues={} fm_snapshots={} market_values={} transfers={} contracts={} performance={} salaries={} metrics={} output={}",
        dataset.dataset_version.0,
        dataset.players.len(),
        dataset.identities.len(),
        dataset.unresolved_identities.len(),
        dataset.issues.len(),
        dataset.fm_snapshots.len(),
        dataset.market_values.len(),
        dataset.transfers.len(),
        dataset.contracts.len(),
        dataset.performance.len(),
        dataset.salaries.len(),
        dataset.metric_registry.len(),
        output
    );
    Ok(())
}

fn verify_manifest(manifest: &DatasetManifest) -> Result<(), Box<dyn Error>> {
    if !manifest.complete { return Err("dataset manifest is not complete".into()); }
    for source in &manifest.sources {
        let (byte_len, checksum) = checksum_file(&source.input_path)?;
        if byte_len != source.byte_len || checksum != source.checksum { return Err(format!("source drift detected: {}", source.input_path).into()); }
        println!("verified source={:?} schema={} bytes={} records={} checksum={}", source.source, source.schema_version, byte_len, source.record_count, checksum);
    }
    Ok(())
}

struct ManifestIndex {
    entries: HashMap<String, (SourceKind, u16, ProvenanceId)>,
    provenances: Vec<CanonicalProvenance>,
}

impl ManifestIndex {
    fn new(manifest: &DatasetManifest) -> Result<Self, Box<dyn Error>> {
        let mut entries = HashMap::new();
        let mut provenances = Vec::with_capacity(manifest.sources.len());
        for (index, source) in manifest.sources.iter().enumerate() {
            let raw_id = u64::try_from(index + 1)?;
            let provenance_id = ProvenanceId(raw_id);
            if entries.insert(source.input_path.clone(), (source.source, source.schema_version, provenance_id)).is_some() {
                return Err(format!("duplicate path in source manifest: {}", source.input_path).into());
            }
            provenances.push(CanonicalProvenance { id: provenance_id, source: core_source(source.source), input_path: source.input_path.clone(), source_checksum: source.checksum });
        }
        Ok(Self { entries, provenances })
    }

    fn require(&self, path: &str, expected: SourceKind) -> Result<(ProvenanceId, u16), Box<dyn Error>> {
        let (kind, schema, provenance) = self.entries.get(path).copied().ok_or_else(|| format!("input is not registered in source manifest: {path}"))?;
        if kind != expected { return Err(format!("source kind mismatch for {path}: expected {expected:?}, got {kind:?}").into()); }
        Ok((provenance, schema))
    }

    fn provenances(&self) -> Vec<CanonicalProvenance> { self.provenances.clone() }
}

fn core_source(kind: SourceKind) -> Source {
    match kind {
        SourceKind::FootballManager => Source::FootballManager,
        SourceKind::Transfermarkt => Source::Transfermarkt,
        SourceKind::Fbref => Source::Fbref,
        SourceKind::Salary => Source::Salary,
        SourceKind::Manual => Source::Manual,
    }
}

fn parse_external_player_spec(spec: &str) -> Result<(Source, SourceKind, &str), Box<dyn Error>> {
    let mut parts = spec.splitn(2, ',');
    let raw = parts.next().unwrap_or_default();
    let path = parts.next().ok_or("external-player spec must be source,path")?.trim();
    let (source, kind) = match raw.trim().to_ascii_lowercase().as_str() {
        "tm" | "transfermarkt" => (Source::Transfermarkt, SourceKind::Transfermarkt),
        "fbref" => (Source::Fbref, SourceKind::Fbref),
        "salary" | "wage" => (Source::Salary, SourceKind::Salary),
        _ => return Err(format!("unsupported external-player source: {raw}").into()),
    };
    Ok((source, kind, path))
}

fn fail_parse_report(path: &str, issues: &[footy_dataset::ParseIssue]) -> Result<(), Box<dyn Error>> {
    if let Some(first) = issues.first() { Err(format!("parse failed for {path} with {} issue(s); first row={} column={} message={}", issues.len(), first.row, first.column, first.message).into()) } else { Ok(()) }
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
    if path.is_empty() || !Path::new(path).is_file() { return Err(format!("source path is not a file: {path}").into()); }
    Ok((kind, schema_version, path))
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    optional_value(args, flag).ok_or_else(|| format!("missing {flag}").into())
}

fn optional_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1).map(String::as_str)
}

fn required_u64(args: &[String], flag: &str) -> Result<u64, Box<dyn Error>> { Ok(required_value(args, flag)?.parse::<u64>()?) }

fn repeated_values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.iter().enumerate().filter_map(|(index, arg)| (arg == flag).then(|| args.get(index + 1).map(String::as_str)).flatten()).collect()
}
