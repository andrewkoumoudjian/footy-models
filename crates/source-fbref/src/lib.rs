#![forbid(unsafe_code)]
//! Permitted/local FBref/Sports Reference import primitives.

use footy_core::{CoverageStatus, Date, ProvenanceId};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceMetric {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceSourceRecord {
    pub player_external_id: String,
    pub club_external_id: String,
    pub competition_external_id: String,
    pub season_external_id: String,
    pub match_external_id: Option<String>,
    pub start_date: Date,
    pub end_date: Date,
    pub minutes: u32,
    pub metrics: Vec<SourceMetric>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageSourceRecord {
    pub competition_external_id: String,
    pub season_external_id: String,
    pub metric_group: String,
    pub status: CoverageStatus,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    InvertedDateRange,
    DuplicateMetric(String),
    NonFiniteMetric(String),
}

/// Parse a source numeric cell. Empty and source-missing markers are `None`, never zero.
/// Percentages are returned as fractions (e.g. `73.5% -> 0.735`).
pub fn parse_numeric(value: &str) -> Result<Option<f64>, &'static str> {
    let trimmed = value.trim();
    if trimmed.is_empty() || matches!(trimmed, "-" | "—" | "NA" | "N/A" | "n/a") {
        return Ok(None);
    }
    let is_percent = trimmed.ends_with('%');
    let number = trimmed.trim_end_matches('%').replace(',', "");
    let mut parsed = number.parse::<f64>().map_err(|_| "invalid numeric cell")?;
    if !parsed.is_finite() {
        return Err("non-finite numeric cell");
    }
    if is_percent {
        parsed /= 100.0;
    }
    Ok(Some(parsed))
}

pub fn metrics_from_cells<'a, I>(cells: I) -> Result<Vec<SourceMetric>, &'static str>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut metrics = Vec::new();
    for (name, value) in cells {
        if let Some(value) = parse_numeric(value)? {
            metrics.push(SourceMetric { name: name.to_owned(), value });
        }
    }
    Ok(metrics)
}

pub fn validate_record(record: &PerformanceSourceRecord) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if record.end_date < record.start_date {
        issues.push(ValidationIssue::InvertedDateRange);
    }
    let mut names = std::collections::HashSet::new();
    for metric in &record.metrics {
        if !names.insert(metric.name.as_str()) {
            issues.push(ValidationIssue::DuplicateMetric(metric.name.clone()));
        }
        if !metric.value.is_finite() {
            issues.push(ValidationIssue::NonFiniteMetric(metric.name.clone()));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_cells_remain_missing() {
        assert_eq!(parse_numeric("—").unwrap(), None);
        assert_eq!(parse_numeric("").unwrap(), None);
    }

    #[test]
    fn parses_percentages_and_numbers() {
        assert_eq!(parse_numeric("73.5%").unwrap(), Some(0.735));
        assert_eq!(parse_numeric("1,234").unwrap(), Some(1234.0));
    }

    #[test]
    fn duplicate_metrics_are_reported() {
        let record = PerformanceSourceRecord {
            player_external_id: "p".into(),
            club_external_id: "c".into(),
            competition_external_id: "l".into(),
            season_external_id: "s".into(),
            match_external_id: None,
            start_date: Date(1),
            end_date: Date(2),
            minutes: 90,
            metrics: vec![SourceMetric { name: "goals".into(), value: 1.0 }, SourceMetric { name: "goals".into(), value: 1.0 }],
            provenance_id: ProvenanceId(1),
        };
        assert_eq!(validate_record(&record), vec![ValidationIssue::DuplicateMetric("goals".into())]);
    }
}
