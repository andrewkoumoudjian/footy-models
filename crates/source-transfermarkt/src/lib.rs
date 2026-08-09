#![forbid(unsafe_code)]
//! Authorized/local Transfermarkt-derived import primitives.

use footy_core::{Date, EuroCents, ProvenanceId};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransferKind {
    Permanent,
    Free,
    Loan,
    LoanWithOption,
    LoanWithObligation,
    Youth,
    ReturnFromLoan,
    Internal,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSourceRecord {
    pub external_transfer_id: Option<String>,
    pub player_external_id: String,
    pub date: Date,
    pub from_club_external_id: String,
    pub to_club_external_id: String,
    pub kind: TransferKind,
    pub fee: Option<EuroCents>,
    pub loan_fee: Option<EuroCents>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketValueSourceRecord {
    pub player_external_id: String,
    pub date: Date,
    pub value: EuroCents,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractSourceRecord {
    pub player_external_id: String,
    pub club_external_id: String,
    pub observed_on: Date,
    pub expiry: Option<Date>,
    pub option_years: Option<u8>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub index: usize,
    pub message: String,
}

pub fn parse_transfer_kind(value: &str) -> TransferKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "permanent" | "transfer" | "paid" => TransferKind::Permanent,
        "free" | "free transfer" => TransferKind::Free,
        "loan" => TransferKind::Loan,
        "loan with option" | "loan_option" => TransferKind::LoanWithOption,
        "loan with obligation" | "loan_obligation" => TransferKind::LoanWithObligation,
        "youth" => TransferKind::Youth,
        "return from loan" | "loan return" => TransferKind::ReturnFromLoan,
        "internal" => TransferKind::Internal,
        _ => TransferKind::Unknown,
    }
}

/// Parse an explicitly euro-denominated value such as `€1.5m`, `750k`, or `1,500,000`.
/// Empty/undisclosed markers become `None`; they never become zero.
pub fn parse_eur_amount(value: &str) -> Result<Option<EuroCents>, &'static str> {
    let trimmed = value.trim();
    if trimmed.is_empty() || matches!(trimmed, "-" | "—" | "?" | "n/a" | "N/A" | "undisclosed") {
        return Ok(None);
    }

    let mut normalized = trimmed.replace('€', "").replace(' ', "").replace('_', "");
    let multiplier = match normalized.chars().last().map(|c| c.to_ascii_lowercase()) {
        Some('m') => { normalized.pop(); 1_000_000.0 }
        Some('k') => { normalized.pop(); 1_000.0 }
        _ => 1.0,
    };

    if multiplier != 1.0 {
        if normalized.contains(',') && !normalized.contains('.') {
            normalized = normalized.replace(',', ".");
        } else {
            normalized = normalized.replace(',', "");
        }
    } else {
        normalized = normalized.replace(',', "");
    }

    let euros = normalized.parse::<f64>().map_err(|_| "invalid euro amount")? * multiplier;
    if !euros.is_finite() || euros < 0.0 || euros > (i64::MAX as f64) / 100.0 {
        return Err("euro amount out of range");
    }
    Ok(Some(EuroCents((euros * 100.0).round() as i64)))
}

pub fn latest_value_before<'a>(
    player_external_id: &str,
    date: Date,
    values: &'a [MarketValueSourceRecord],
) -> Option<&'a MarketValueSourceRecord> {
    values
        .iter()
        .filter(|value| value.player_external_id == player_external_id && value.date < date)
        .max_by_key(|value| value.date)
}

pub fn validate_transfers(records: &[TransferSourceRecord]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for (index, transfer) in records.iter().enumerate() {
        let key = (
            transfer.player_external_id.as_str(),
            transfer.date,
            transfer.from_club_external_id.as_str(),
            transfer.to_club_external_id.as_str(),
            transfer.kind,
        );
        if !seen.insert(key) {
            issues.push(ValidationIssue { index, message: "duplicate transfer identity".into() });
        }
        if transfer.kind == TransferKind::Free && transfer.fee.is_some() {
            issues.push(ValidationIssue { index, message: "free transfer should not carry a paid-transfer fee".into() });
        }
        if matches!(transfer.fee, Some(value) if value.cents() < 0) {
            issues.push(ValidationIssue { index, message: "negative transfer fee".into() });
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scaled_euro_amounts_without_turning_missing_into_zero() {
        assert_eq!(parse_eur_amount("€1.5m").unwrap(), Some(EuroCents(150_000_000)));
        assert_eq!(parse_eur_amount("750k").unwrap(), Some(EuroCents(75_000_000)));
        assert_eq!(parse_eur_amount("undisclosed").unwrap(), None);
    }

    #[test]
    fn latest_value_is_strictly_before_transfer() {
        let values = vec![
            MarketValueSourceRecord { player_external_id: "p1".into(), date: Date(10), value: EuroCents(100), provenance_id: ProvenanceId(1) },
            MarketValueSourceRecord { player_external_id: "p1".into(), date: Date(20), value: EuroCents(200), provenance_id: ProvenanceId(1) },
        ];
        assert_eq!(latest_value_before("p1", Date(20), &values).unwrap().value, EuroCents(100));
    }

    #[test]
    fn duplicate_transfers_are_observable() {
        let transfer = TransferSourceRecord {
            external_transfer_id: None,
            player_external_id: "p1".into(),
            date: Date(1),
            from_club_external_id: "a".into(),
            to_club_external_id: "b".into(),
            kind: TransferKind::Permanent,
            fee: Some(EuroCents(1)),
            loan_fee: None,
            provenance_id: ProvenanceId(1),
        };
        assert_eq!(validate_transfers(&[transfer.clone(), transfer]).len(), 1);
    }
}
