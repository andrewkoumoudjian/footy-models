#![forbid(unsafe_code)]
//! Football Manager local/import boundary.

use footy_core::{AttributeId, Date, ProvenanceId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FmAttribute {
    pub id: AttributeId,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionRating {
    pub position_id: u16,
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FmSnapshotRecord {
    pub external_player_id: String,
    pub snapshot_date: Date,
    pub name: String,
    pub birth_date: Option<Date>,
    pub current_ability: Option<u16>,
    pub potential_ability: Option<u16>,
    pub reputation: Option<u16>,
    pub attributes: Vec<FmAttribute>,
    pub positions: Vec<PositionRating>,
    pub personality_attributes: Vec<FmAttribute>,
    pub hidden_attributes: Vec<FmAttribute>,
    pub provenance_id: ProvenanceId,
    pub schema_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedAttributeColumn {
    pub header: String,
    pub id: AttributeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPositionColumn {
    pub header: String,
    pub position_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmTsvSchema {
    pub version: u16,
    pub external_player_id: String,
    pub snapshot_date: String,
    pub name: String,
    pub birth_date: Option<String>,
    pub current_ability: Option<String>,
    pub potential_ability: Option<String>,
    pub reputation: Option<String>,
    pub attributes: Vec<NamedAttributeColumn>,
    pub positions: Vec<NamedPositionColumn>,
    pub personality_attributes: Vec<NamedAttributeColumn>,
    pub hidden_attributes: Vec<NamedAttributeColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIssue {
    pub row: usize,
    pub column: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseReport {
    pub records: Vec<FmSnapshotRecord>,
    pub unknown_columns: Vec<String>,
    pub issues: Vec<ParseIssue>,
}

impl ParseReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Parse a dependency-free TSV export. Tabs/newlines inside field values are intentionally
/// unsupported; richer CSV/JSON adapters belong at the application boundary.
pub fn parse_tsv(input: &str, schema: &FmTsvSchema, provenance_id: ProvenanceId) -> ParseReport {
    let mut lines = input.lines();
    let Some(header_line) = lines.next() else {
        return ParseReport { records: Vec::new(), unknown_columns: Vec::new(), issues: Vec::new() };
    };

    let headers: Vec<&str> = header_line.split('\t').collect();
    let index: HashMap<&str, usize> = headers.iter().enumerate().map(|(i, h)| (*h, i)).collect();
    let known = known_headers(schema);
    let unknown_columns = headers
        .iter()
        .filter(|header| !known.contains(**header))
        .map(|header| (*header).to_owned())
        .collect();

    let mut report = ParseReport { records: Vec::new(), unknown_columns, issues: Vec::new() };
    for (offset, line) in lines.enumerate() {
        let row_number = offset + 2;
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match parse_row(&fields, &index, schema, provenance_id, row_number) {
            Ok(record) => report.records.push(record),
            Err(issue) => report.issues.push(issue),
        }
    }
    report
}

fn parse_row(
    fields: &[&str],
    index: &HashMap<&str, usize>,
    schema: &FmTsvSchema,
    provenance_id: ProvenanceId,
    row: usize,
) -> Result<FmSnapshotRecord, ParseIssue> {
    let external_player_id = required(fields, index, &schema.external_player_id, row)?.to_owned();
    let name = required(fields, index, &schema.name, row)?.to_owned();
    let snapshot_date = parse_date(required(fields, index, &schema.snapshot_date, row)?, &schema.snapshot_date, row)?;
    let birth_date = optional_date(fields, index, schema.birth_date.as_deref(), row)?;
    let current_ability = optional_u16(fields, index, schema.current_ability.as_deref(), row)?;
    let potential_ability = optional_u16(fields, index, schema.potential_ability.as_deref(), row)?;
    let reputation = optional_u16(fields, index, schema.reputation.as_deref(), row)?;

    Ok(FmSnapshotRecord {
        external_player_id,
        snapshot_date,
        name,
        birth_date,
        current_ability,
        potential_ability,
        reputation,
        attributes: parse_attributes(fields, index, &schema.attributes, row)?,
        positions: parse_positions(fields, index, &schema.positions, row)?,
        personality_attributes: parse_attributes(fields, index, &schema.personality_attributes, row)?,
        hidden_attributes: parse_attributes(fields, index, &schema.hidden_attributes, row)?,
        provenance_id,
        schema_version: schema.version,
    })
}

fn known_headers(schema: &FmTsvSchema) -> HashSet<&str> {
    let mut set = HashSet::new();
    set.insert(schema.external_player_id.as_str());
    set.insert(schema.snapshot_date.as_str());
    set.insert(schema.name.as_str());
    for header in [schema.birth_date.as_deref(), schema.current_ability.as_deref(), schema.potential_ability.as_deref(), schema.reputation.as_deref()].into_iter().flatten() {
        set.insert(header);
    }
    for item in schema.attributes.iter().chain(schema.personality_attributes.iter()).chain(schema.hidden_attributes.iter()) {
        set.insert(item.header.as_str());
    }
    for item in &schema.positions {
        set.insert(item.header.as_str());
    }
    set
}

fn required<'a>(fields: &'a [&str], index: &HashMap<&str, usize>, column: &str, row: usize) -> Result<&'a str, ParseIssue> {
    let Some(&position) = index.get(column) else {
        return Err(issue(row, column, "required column is missing"));
    };
    let value = fields.get(position).copied().unwrap_or("").trim();
    if value.is_empty() {
        Err(issue(row, column, "required value is empty"))
    } else {
        Ok(value)
    }
}

fn optional<'a>(fields: &'a [&str], index: &HashMap<&str, usize>, column: Option<&str>) -> Option<&'a str> {
    let column = column?;
    let position = *index.get(column)?;
    let value = fields.get(position)?.trim();
    (!value.is_empty()).then_some(value)
}

fn optional_u16(fields: &[&str], index: &HashMap<&str, usize>, column: Option<&str>, row: usize) -> Result<Option<u16>, ParseIssue> {
    let Some(value) = optional(fields, index, column) else { return Ok(None); };
    value.parse::<u16>().map(Some).map_err(|_| issue(row, column.unwrap_or_default(), "invalid unsigned integer"))
}

fn optional_date(fields: &[&str], index: &HashMap<&str, usize>, column: Option<&str>, row: usize) -> Result<Option<Date>, ParseIssue> {
    let Some(value) = optional(fields, index, column) else { return Ok(None); };
    parse_date(value, column.unwrap_or_default(), row).map(Some)
}

fn parse_attributes(fields: &[&str], index: &HashMap<&str, usize>, columns: &[NamedAttributeColumn], row: usize) -> Result<Vec<FmAttribute>, ParseIssue> {
    let mut out = Vec::with_capacity(columns.len());
    for column in columns {
        let Some(value) = optional(fields, index, Some(&column.header)) else { continue; };
        let parsed = value.parse::<u8>().map_err(|_| issue(row, &column.header, "invalid attribute"))?;
        if parsed > 20 {
            return Err(issue(row, &column.header, "attribute must be in 0..=20"));
        }
        out.push(FmAttribute { id: column.id, value: parsed });
    }
    Ok(out)
}

fn parse_positions(fields: &[&str], index: &HashMap<&str, usize>, columns: &[NamedPositionColumn], row: usize) -> Result<Vec<PositionRating>, ParseIssue> {
    let mut out = Vec::with_capacity(columns.len());
    for column in columns {
        let Some(value) = optional(fields, index, Some(&column.header)) else { continue; };
        let parsed = value.parse::<u8>().map_err(|_| issue(row, &column.header, "invalid position rating"))?;
        if parsed > 20 {
            return Err(issue(row, &column.header, "position rating must be in 0..=20"));
        }
        out.push(PositionRating { position_id: column.position_id, value: parsed });
    }
    Ok(out)
}

fn parse_date(value: &str, column: &str, row: usize) -> Result<Date, ParseIssue> {
    let mut parts = value.split('-');
    let year = parts.next().and_then(|v| v.parse::<i32>().ok());
    let month = parts.next().and_then(|v| v.parse::<u8>().ok());
    let day = parts.next().and_then(|v| v.parse::<u8>().ok());
    if parts.next().is_some() {
        return Err(issue(row, column, "date must be YYYY-MM-DD"));
    }
    match (year, month, day) {
        (Some(y), Some(m), Some(d)) => Date::from_ymd(y, m, d).map_err(|_| issue(row, column, "invalid calendar date")),
        _ => Err(issue(row, column, "date must be YYYY-MM-DD")),
    }
}

fn issue(row: usize, column: &str, message: &str) -> ParseIssue {
    ParseIssue { row, column: column.to_owned(), message: message.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> FmTsvSchema {
        FmTsvSchema {
            version: 1,
            external_player_id: "uid".into(),
            snapshot_date: "date".into(),
            name: "name".into(),
            birth_date: Some("dob".into()),
            current_ability: Some("ca".into()),
            potential_ability: Some("pa".into()),
            reputation: None,
            attributes: vec![NamedAttributeColumn { header: "pace".into(), id: AttributeId(1) }],
            positions: Vec::new(),
            personality_attributes: Vec::new(),
            hidden_attributes: Vec::new(),
        }
    }

    #[test]
    fn parses_historical_snapshot_and_reports_unknown_columns() {
        let input = "uid\tdate\tname\tdob\tca\tpa\tpace\textra\n10\t2024-01-01\tPlayer A\t2004-02-03\t120\t170\t16\tx";
        let report = parse_tsv(input, &schema(), ProvenanceId(9));
        assert!(report.is_clean());
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.records[0].attributes[0].value, 16);
        assert_eq!(report.unknown_columns, vec!["extra"]);
    }

    #[test]
    fn rejects_out_of_range_attributes() {
        let input = "uid\tdate\tname\tdob\tca\tpa\tpace\n10\t2024-01-01\tPlayer A\t2004-02-03\t120\t170\t24";
        let report = parse_tsv(input, &schema(), ProvenanceId(9));
        assert_eq!(report.records.len(), 0);
        assert_eq!(report.issues.len(), 1);
    }
}
