use crate::{DatasetError, ExternalPlayerRecord, IdentityOverride};
use footy_core::{Confidence, Date, ProvenanceId, Source};
use footy_source_fbref::{PerformanceSourceRecord, SourceMetric};
use footy_source_fm::{
    FmTsvSchema, NamedAttributeColumn, NamedPositionColumn,
};
use footy_source_salary::{
    CompensationKind, CurrencyCode, CurrencyRange, Periodicity, SalaryObservation, WageBasis,
};
use footy_source_transfermarkt::{
    parse_eur_amount, parse_transfer_kind, ContractSourceRecord, MarketValueSourceRecord,
    TransferSourceRecord,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIssue {
    pub row: usize,
    pub column: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseReport<T> {
    pub records: Vec<T>,
    pub issues: Vec<ParseIssue>,
}

impl<T> ParseReport<T> {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Infer the normalized FM TSV convention used by the orchestration pipeline.
/// Required columns are `uid`, `date`, and `name`. Optional scalar columns are
/// `dob`, `ca`, `pa`, and `reputation`. Dynamic columns are `attr:<u16>`,
/// `position:<u16>`, `personality:<u16>`, and `hidden:<u16>`.
pub fn infer_fm_schema(input: &str, version: u16) -> Result<FmTsvSchema, DatasetError> {
    let header = input.lines().next().ok_or_else(|| DatasetError::new("FM TSV is empty"))?;
    let headers: Vec<&str> = header.split('\t').collect();
    for required in ["uid", "date", "name"] {
        if !headers.contains(&required) {
            return Err(DatasetError::new(format!("FM TSV missing required column {required}")));
        }
    }

    let mut attributes = Vec::new();
    let mut positions = Vec::new();
    let mut personality_attributes = Vec::new();
    let mut hidden_attributes = Vec::new();
    let mut seen_attribute_ids = HashSet::new();
    let mut seen_position_ids = HashSet::new();

    for header in headers {
        if let Some(raw) = header.strip_prefix("attr:") {
            let id = raw.parse::<u16>().map_err(|_| DatasetError::new(format!("invalid FM attribute header {header}")))?;
            if !seen_attribute_ids.insert((0_u8, id)) {
                return Err(DatasetError::new(format!("duplicate FM attribute id {id}")));
            }
            attributes.push(NamedAttributeColumn { header: header.to_owned(), id: footy_core::AttributeId(id) });
        } else if let Some(raw) = header.strip_prefix("position:") {
            let id = raw.parse::<u16>().map_err(|_| DatasetError::new(format!("invalid FM position header {header}")))?;
            if !seen_position_ids.insert(id) {
                return Err(DatasetError::new(format!("duplicate FM position id {id}")));
            }
            positions.push(NamedPositionColumn { header: header.to_owned(), position_id: id });
        } else if let Some(raw) = header.strip_prefix("personality:") {
            let id = raw.parse::<u16>().map_err(|_| DatasetError::new(format!("invalid FM personality header {header}")))?;
            if !seen_attribute_ids.insert((1_u8, id)) {
                return Err(DatasetError::new(format!("duplicate FM personality id {id}")));
            }
            personality_attributes.push(NamedAttributeColumn { header: header.to_owned(), id: footy_core::AttributeId(id) });
        } else if let Some(raw) = header.strip_prefix("hidden:") {
            let id = raw.parse::<u16>().map_err(|_| DatasetError::new(format!("invalid FM hidden header {header}")))?;
            if !seen_attribute_ids.insert((2_u8, id)) {
                return Err(DatasetError::new(format!("duplicate FM hidden attribute id {id}")));
            }
            hidden_attributes.push(NamedAttributeColumn { header: header.to_owned(), id: footy_core::AttributeId(id) });
        }
    }

    attributes.sort_by_key(|column| column.id);
    positions.sort_by_key(|column| column.position_id);
    personality_attributes.sort_by_key(|column| column.id);
    hidden_attributes.sort_by_key(|column| column.id);

    Ok(FmTsvSchema {
        version,
        external_player_id: "uid".into(),
        snapshot_date: "date".into(),
        name: "name".into(),
        birth_date: header_present(input, "dob").then(|| "dob".into()),
        current_ability: header_present(input, "ca").then(|| "ca".into()),
        potential_ability: header_present(input, "pa").then(|| "pa".into()),
        reputation: header_present(input, "reputation").then(|| "reputation".into()),
        attributes,
        positions,
        personality_attributes,
        hidden_attributes,
    })
}

fn header_present(input: &str, name: &str) -> bool {
    input.lines().next().is_some_and(|line| line.split('\t').any(|header| header == name))
}

/// Parse generic player metadata used for Transfermarkt, FBref, and salary identity resolution.
/// Required columns: `external_id`, `name`. Optional: `birth_date`, `height_cm`, `nationality`.
pub fn parse_external_players_tsv(
    input: &str,
    source: Source,
    provenance_id: ProvenanceId,
) -> ParseReport<ExternalPlayerRecord> {
    let Some((headers, lines)) = split_table(input) else {
        return ParseReport { records: Vec::new(), issues: vec![issue(1, "", "empty player metadata TSV")] };
    };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["external_id", "name"]);
    if !missing.is_empty() {
        return ParseReport { records: Vec::new(), issues: missing.into_iter().map(|column| issue(1, column, "required column missing")).collect() };
    }

    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<ExternalPlayerRecord, ParseIssue> {
            let external_id = required(&fields, &index, "external_id", row)?.to_owned();
            let name = required(&fields, &index, "name", row)?.to_owned();
            let birth_date = optional_date(&fields, &index, "birth_date", row)?;
            let height_cm = optional_u16(&fields, &index, "height_cm", row)?;
            let nationality = optional(&fields, &index, "nationality").map(str::to_owned);
            Ok(ExternalPlayerRecord { source, external_id, name, birth_date, height_cm, nationality, provenance_id })
        })();
        match result {
            Ok(record) => report.records.push(record),
            Err(error) => report.issues.push(error),
        }
    }
    report
}

/// Identity override TSV: `source`, `external_id`, `fm_external_id`.
pub fn parse_identity_overrides_tsv(input: &str) -> ParseReport<IdentityOverride> {
    let Some((headers, lines)) = split_table(input) else {
        return ParseReport { records: Vec::new(), issues: vec![issue(1, "", "empty identity override TSV")] };
    };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["source", "external_id", "fm_external_id"]);
    if !missing.is_empty() {
        return ParseReport { records: Vec::new(), issues: missing.into_iter().map(|column| issue(1, column, "required column missing")).collect() };
    }
    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    let mut seen = HashSet::new();
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<IdentityOverride, ParseIssue> {
            let source = parse_source(required(&fields, &index, "source", row)?).ok_or_else(|| issue(row, "source", "unsupported source"))?;
            let external_id = required(&fields, &index, "external_id", row)?.to_owned();
            let fm_external_id = required(&fields, &index, "fm_external_id", row)?.to_owned();
            if !seen.insert((source_key(source), external_id.clone())) {
                return Err(issue(row, "external_id", "duplicate identity override"));
            }
            Ok(IdentityOverride { source, external_id, fm_external_id })
        })();
        match result { Ok(record) => report.records.push(record), Err(error) => report.issues.push(error) }
    }
    report
}

/// Transfermarkt market value TSV: `player_id`, `date`, `value_eur`.
pub fn parse_tm_market_values_tsv(input: &str, provenance_id: ProvenanceId) -> ParseReport<MarketValueSourceRecord> {
    let Some((headers, lines)) = split_table(input) else { return empty_report("empty Transfermarkt market-value TSV"); };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["player_id", "date", "value_eur"]);
    if !missing.is_empty() { return required_report(missing); }
    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<MarketValueSourceRecord, ParseIssue> {
            let player_external_id = required(&fields, &index, "player_id", row)?.to_owned();
            let date = parse_date(required(&fields, &index, "date", row)?, row, "date")?;
            let raw = required(&fields, &index, "value_eur", row)?;
            let value = parse_eur_amount(raw).map_err(|message| issue(row, "value_eur", message))?
                .ok_or_else(|| issue(row, "value_eur", "market value cannot be missing"))?;
            if value.cents() <= 0 { return Err(issue(row, "value_eur", "market value must be positive")); }
            Ok(MarketValueSourceRecord { player_external_id, date, value, provenance_id })
        })();
        match result { Ok(record) => report.records.push(record), Err(error) => report.issues.push(error) }
    }
    report
}

/// Transfermarkt transfer TSV columns: `transfer_id`, `player_id`, `date`, `from_club_id`,
/// `to_club_id`, `kind`, `fee_eur`, `loan_fee_eur`. `transfer_id`, fees are optional cells.
pub fn parse_tm_transfers_tsv(input: &str, provenance_id: ProvenanceId) -> ParseReport<TransferSourceRecord> {
    let Some((headers, lines)) = split_table(input) else { return empty_report("empty Transfermarkt transfer TSV"); };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["player_id", "date", "from_club_id", "to_club_id", "kind"]);
    if !missing.is_empty() { return required_report(missing); }
    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<TransferSourceRecord, ParseIssue> {
            let external_transfer_id = optional(&fields, &index, "transfer_id").map(str::to_owned);
            let player_external_id = required(&fields, &index, "player_id", row)?.to_owned();
            let date = parse_date(required(&fields, &index, "date", row)?, row, "date")?;
            let from_club_external_id = required(&fields, &index, "from_club_id", row)?.to_owned();
            let to_club_external_id = required(&fields, &index, "to_club_id", row)?.to_owned();
            let kind_raw = required(&fields, &index, "kind", row)?;
            let kind = parse_transfer_kind(kind_raw);
            if matches!(kind, footy_source_transfermarkt::TransferKind::Unknown) {
                return Err(issue(row, "kind", "unknown transfer kind"));
            }
            let fee = parse_optional_eur(&fields, &index, "fee_eur", row)?;
            let loan_fee = parse_optional_eur(&fields, &index, "loan_fee_eur", row)?;
            Ok(TransferSourceRecord { external_transfer_id, player_external_id, date, from_club_external_id, to_club_external_id, kind, fee, loan_fee, provenance_id })
        })();
        match result { Ok(record) => report.records.push(record), Err(error) => report.issues.push(error) }
    }
    for validation in footy_source_transfermarkt::validate_transfers(&report.records) {
        report.issues.push(issue(validation.index + 2, "", &validation.message));
    }
    report
}

/// Transfermarkt contract TSV: `player_id`, `club_id`, `observed_on`, optional `expiry`, `option_years`.
pub fn parse_tm_contracts_tsv(input: &str, provenance_id: ProvenanceId) -> ParseReport<ContractSourceRecord> {
    let Some((headers, lines)) = split_table(input) else { return empty_report("empty Transfermarkt contract TSV"); };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["player_id", "club_id", "observed_on"]);
    if !missing.is_empty() { return required_report(missing); }
    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<ContractSourceRecord, ParseIssue> {
            let player_external_id = required(&fields, &index, "player_id", row)?.to_owned();
            let club_external_id = required(&fields, &index, "club_id", row)?.to_owned();
            let observed_on = parse_date(required(&fields, &index, "observed_on", row)?, row, "observed_on")?;
            let expiry = optional_date(&fields, &index, "expiry", row)?;
            let option_years = optional_u8(&fields, &index, "option_years", row)?;
            Ok(ContractSourceRecord { player_external_id, club_external_id, observed_on, expiry, option_years, provenance_id })
        })();
        match result { Ok(record) => report.records.push(record), Err(error) => report.issues.push(error) }
    }
    report
}

/// FBref performance TSV fixed columns: `player_id`, `club_id`, `competition_id`, `season_id`,
/// `start_date`, `end_date`, `minutes`, optional `match_id`; metrics are `metric:<name>` columns.
pub fn parse_fbref_performance_tsv(input: &str, provenance_id: ProvenanceId) -> ParseReport<PerformanceSourceRecord> {
    let Some((headers, lines)) = split_table(input) else { return empty_report("empty FBref performance TSV"); };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["player_id", "club_id", "competition_id", "season_id", "start_date", "end_date", "minutes"]);
    if !missing.is_empty() { return required_report(missing); }
    let mut metric_columns = Vec::new();
    let mut metric_names = HashSet::new();
    for (position, header) in headers.iter().enumerate() {
        if let Some(name) = header.strip_prefix("metric:") {
            if name.trim().is_empty() || !metric_names.insert(name.to_owned()) {
                return ParseReport { records: Vec::new(), issues: vec![issue(1, header, "empty or duplicate metric name")] };
            }
            metric_columns.push((position, name.to_owned()));
        }
    }
    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<PerformanceSourceRecord, ParseIssue> {
            let player_external_id = required(&fields, &index, "player_id", row)?.to_owned();
            let club_external_id = required(&fields, &index, "club_id", row)?.to_owned();
            let competition_external_id = required(&fields, &index, "competition_id", row)?.to_owned();
            let season_external_id = required(&fields, &index, "season_id", row)?.to_owned();
            let match_external_id = optional(&fields, &index, "match_id").map(str::to_owned);
            let start_date = parse_date(required(&fields, &index, "start_date", row)?, row, "start_date")?;
            let end_date = parse_date(required(&fields, &index, "end_date", row)?, row, "end_date")?;
            if end_date < start_date { return Err(issue(row, "end_date", "end date precedes start date")); }
            let minutes = required(&fields, &index, "minutes", row)?.parse::<u32>().map_err(|_| issue(row, "minutes", "invalid minutes"))?;
            let mut metrics = Vec::new();
            for (position, name) in &metric_columns {
                let raw = fields.get(*position).copied().unwrap_or("");
                match footy_source_fbref::parse_numeric(raw) {
                    Ok(Some(value)) => metrics.push(SourceMetric { name: name.clone(), value }),
                    Ok(None) => {}
                    Err(message) => return Err(issue(row, &format!("metric:{name}"), message)),
                }
            }
            let record = PerformanceSourceRecord { player_external_id, club_external_id, competition_external_id, season_external_id, match_external_id, start_date, end_date, minutes, metrics, provenance_id };
            if let Some(problem) = footy_source_fbref::validate_record(&record).first() {
                return Err(issue(row, "", &format!("FBref validation failed: {problem:?}")));
            }
            Ok(record)
        })();
        match result { Ok(record) => report.records.push(record), Err(error) => report.issues.push(error) }
    }
    report
}

/// Salary TSV columns: `player_id`, `club_id`, `effective_date`, `currency`, `low_minor_units`,
/// `high_minor_units`, `basis`, `periodicity`, `kind`, `confidence`.
pub fn parse_salary_tsv(input: &str, provenance_id: ProvenanceId) -> ParseReport<SalaryObservation> {
    let Some((headers, lines)) = split_table(input) else { return empty_report("empty salary TSV"); };
    let index = header_index(&headers);
    let missing = missing_required(&index, &["player_id", "club_id", "effective_date", "currency", "low_minor_units", "high_minor_units", "basis", "periodicity", "kind", "confidence"]);
    if !missing.is_empty() { return required_report(missing); }
    let mut report = ParseReport { records: Vec::new(), issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row = offset + 2;
        if line.trim().is_empty() { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        let result = (|| -> Result<SalaryObservation, ParseIssue> {
            let player_external_id = required(&fields, &index, "player_id", row)?.to_owned();
            let club_external_id = required(&fields, &index, "club_id", row)?.to_owned();
            let effective_date = parse_date(required(&fields, &index, "effective_date", row)?, row, "effective_date")?;
            let currency = CurrencyCode::new(required(&fields, &index, "currency", row)?).map_err(|message| issue(row, "currency", message))?;
            let low = required(&fields, &index, "low_minor_units", row)?.parse::<i64>().map_err(|_| issue(row, "low_minor_units", "invalid amount"))?;
            let high = required(&fields, &index, "high_minor_units", row)?.parse::<i64>().map_err(|_| issue(row, "high_minor_units", "invalid amount"))?;
            let amount = CurrencyRange::new(currency, low, high).map_err(|message| issue(row, "low_minor_units", message))?;
            let basis = match required(&fields, &index, "basis", row)?.trim().to_ascii_lowercase().as_str() {
                "gross" => WageBasis::Gross, "net" => WageBasis::Net, "unknown" => WageBasis::Unknown,
                _ => return Err(issue(row, "basis", "unknown wage basis")),
            };
            let periodicity = match required(&fields, &index, "periodicity", row)?.trim().to_ascii_lowercase().as_str() {
                "daily" => Periodicity::Daily, "weekly" => Periodicity::Weekly, "monthly" => Periodicity::Monthly,
                "annual" => Periodicity::Annual, "oneoff" | "one_off" | "one-off" => Periodicity::OneOff,
                _ => return Err(issue(row, "periodicity", "unknown periodicity")),
            };
            let kind = match required(&fields, &index, "kind", row)?.trim().to_ascii_lowercase().as_str() {
                "base" | "base_wage" => CompensationKind::BaseWage,
                "performance_bonus" | "bonus" => CompensationKind::PerformanceBonus,
                "signing_fee" => CompensationKind::SigningFee,
                "agent_fee" => CompensationKind::AgentFee,
                "other" => CompensationKind::Other,
                _ => return Err(issue(row, "kind", "unknown compensation kind")),
            };
            let confidence_value = required(&fields, &index, "confidence", row)?.parse::<f32>().map_err(|_| issue(row, "confidence", "invalid confidence"))?;
            let confidence = Confidence::new(confidence_value).map_err(|_| issue(row, "confidence", "confidence must be in 0..=1"))?;
            Ok(SalaryObservation { player_external_id, club_external_id, effective_date, amount, basis, periodicity, kind, provenance_id, confidence })
        })();
        match result { Ok(record) => report.records.push(record), Err(error) => report.issues.push(error) }
    }
    report
}

fn parse_optional_eur(fields: &[&str], index: &HashMap<&str, usize>, column: &str, row: usize) -> Result<Option<footy_core::EuroCents>, ParseIssue> {
    let Some(raw) = optional(fields, index, column) else { return Ok(None); };
    parse_eur_amount(raw).map_err(|message| issue(row, column, message))
}

fn parse_source(value: &str) -> Option<Source> {
    match value.trim().to_ascii_lowercase().as_str() {
        "fm" | "footballmanager" => Some(Source::FootballManager),
        "tm" | "transfermarkt" => Some(Source::Transfermarkt),
        "fbref" => Some(Source::Fbref),
        "salary" | "wage" => Some(Source::Salary),
        "manual" => Some(Source::Manual),
        _ => None,
    }
}

fn source_key(source: Source) -> u8 {
    match source {
        Source::FootballManager => 1,
        Source::Transfermarkt => 2,
        Source::Fbref => 3,
        Source::Salary => 4,
        Source::Manual => 5,
        Source::Other(_) => 255,
    }
}

fn split_table(input: &str) -> Option<(Vec<&str>, std::str::Lines<'_>)> {
    let mut lines = input.lines();
    let header = lines.next()?;
    Some((header.split('\t').collect(), lines))
}

fn header_index<'a>(headers: &'a [&'a str]) -> HashMap<&'a str, usize> {
    headers.iter().enumerate().map(|(index, value)| (*value, index)).collect()
}

fn missing_required<'a>(index: &HashMap<&str, usize>, required_columns: &'a [&'a str]) -> Vec<&'a str> {
    required_columns.iter().copied().filter(|column| !index.contains_key(column)).collect()
}

fn required<'a>(fields: &'a [&str], index: &HashMap<&str, usize>, column: &str, row: usize) -> Result<&'a str, ParseIssue> {
    let position = *index.get(column).ok_or_else(|| issue(row, column, "required column missing"))?;
    let value = fields.get(position).copied().unwrap_or("").trim();
    if value.is_empty() { Err(issue(row, column, "required value missing")) } else { Ok(value) }
}

fn optional<'a>(fields: &'a [&str], index: &HashMap<&str, usize>, column: &str) -> Option<&'a str> {
    let position = *index.get(column)?;
    let value = fields.get(position)?.trim();
    (!value.is_empty()).then_some(value)
}

fn optional_date(fields: &[&str], index: &HashMap<&str, usize>, column: &str, row: usize) -> Result<Option<Date>, ParseIssue> {
    optional(fields, index, column).map(|value| parse_date(value, row, column)).transpose()
}

fn optional_u16(fields: &[&str], index: &HashMap<&str, usize>, column: &str, row: usize) -> Result<Option<u16>, ParseIssue> {
    optional(fields, index, column).map(|value| value.parse::<u16>().map_err(|_| issue(row, column, "invalid unsigned integer"))).transpose()
}

fn optional_u8(fields: &[&str], index: &HashMap<&str, usize>, column: &str, row: usize) -> Result<Option<u8>, ParseIssue> {
    optional(fields, index, column).map(|value| value.parse::<u8>().map_err(|_| issue(row, column, "invalid unsigned integer"))).transpose()
}

fn parse_date(value: &str, row: usize, column: &str) -> Result<Date, ParseIssue> {
    let mut parts = value.split('-');
    let year = parts.next().and_then(|value| value.parse::<i32>().ok());
    let month = parts.next().and_then(|value| value.parse::<u8>().ok());
    let day = parts.next().and_then(|value| value.parse::<u8>().ok());
    if parts.next().is_some() { return Err(issue(row, column, "date must be YYYY-MM-DD")); }
    match (year, month, day) {
        (Some(year), Some(month), Some(day)) => Date::from_ymd(year, month, day).map_err(|_| issue(row, column, "invalid date")),
        _ => Err(issue(row, column, "date must be YYYY-MM-DD")),
    }
}

fn issue(row: usize, column: &str, message: &str) -> ParseIssue {
    ParseIssue { row, column: column.to_owned(), message: message.to_owned() }
}

fn empty_report<T>(message: &str) -> ParseReport<T> {
    ParseReport { records: Vec::new(), issues: vec![issue(1, "", message)] }
}

fn required_report<T>(columns: Vec<&str>) -> ParseReport<T> {
    ParseReport { records: Vec::new(), issues: columns.into_iter().map(|column| issue(1, column, "required column missing")).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_dynamic_fm_columns() {
        let input = "uid\tdate\tname\tdob\tca\tattr:1\thidden:9\n1\t2024-01-01\tA\t2000-01-01\t120\t15\t12\n";
        let schema = infer_fm_schema(input, 3).unwrap();
        assert_eq!(schema.version, 3);
        assert_eq!(schema.attributes.len(), 1);
        assert_eq!(schema.hidden_attributes.len(), 1);
    }

    #[test]
    fn tm_missing_values_do_not_become_zero() {
        let input = "player_id\tdate\tvalue_eur\np1\t2024-01-01\tundisclosed\n";
        let report = parse_tm_market_values_tsv(input, ProvenanceId(1));
        assert!(report.records.is_empty());
        assert_eq!(report.issues.len(), 1);
    }

    #[test]
    fn fbref_metrics_are_dynamic_and_missing_cells_are_absent() {
        let input = "player_id\tclub_id\tcompetition_id\tseason_id\tmatch_id\tstart_date\tend_date\tminutes\tmetric:goals\tmetric:assists\np1\tc1\tl1\ts1\tm1\t2024-01-01\t2024-01-01\t90\t1\t\n";
        let report = parse_fbref_performance_tsv(input, ProvenanceId(1));
        assert!(report.is_clean());
        assert_eq!(report.records[0].metrics.len(), 1);
        assert_eq!(report.records[0].metrics[0].name, "goals");
    }
}
