#![forbid(unsafe_code)]
//! Salary/wage import primitives with explicit source uncertainty.

use footy_core::{Confidence, Date, EuroCents, ProvenanceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrencyCode([u8; 3]);

impl CurrencyCode {
    pub fn new(code: &str) -> Result<Self, &'static str> {
        let bytes = code.as_bytes();
        if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_alphabetic) {
            return Err("currency code must be three ASCII letters");
        }
        Ok(Self([bytes[0].to_ascii_uppercase(), bytes[1].to_ascii_uppercase(), bytes[2].to_ascii_uppercase()]))
    }

    pub const fn bytes(self) -> [u8; 3] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WageBasis {
    Gross,
    Net,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Periodicity {
    Daily,
    Weekly,
    Monthly,
    Annual,
    OneOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompensationKind {
    BaseWage,
    PerformanceBonus,
    SigningFee,
    AgentFee,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyRange {
    pub currency: CurrencyCode,
    pub low_minor_units: i64,
    pub high_minor_units: i64,
}

impl CurrencyRange {
    pub fn new(currency: CurrencyCode, low_minor_units: i64, high_minor_units: i64) -> Result<Self, &'static str> {
        if low_minor_units < 0 || high_minor_units < low_minor_units {
            return Err("invalid monetary range");
        }
        Ok(Self { currency, low_minor_units, high_minor_units })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SalaryObservation {
    pub player_external_id: String,
    pub club_external_id: String,
    pub effective_date: Date,
    pub amount: CurrencyRange,
    pub basis: WageBasis,
    pub periodicity: Periodicity,
    pub kind: CompensationKind,
    pub provenance_id: ProvenanceId,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnualizedRange {
    pub currency: CurrencyCode,
    pub low_minor_units: i64,
    pub high_minor_units: i64,
}

pub fn annualize(range: CurrencyRange, periodicity: Periodicity) -> Result<AnnualizedRange, &'static str> {
    let factor = match periodicity {
        Periodicity::Daily => 365,
        Periodicity::Weekly => 52,
        Periodicity::Monthly => 12,
        Periodicity::Annual => 1,
        Periodicity::OneOff => return Err("one-off compensation cannot be annualized as wage"),
    };
    let low = range.low_minor_units.checked_mul(factor).ok_or("annualization overflow")?;
    let high = range.high_minor_units.checked_mul(factor).ok_or("annualization overflow")?;
    Ok(AnnualizedRange { currency: range.currency, low_minor_units: low, high_minor_units: high })
}

/// Convert currency minor-units to euro cents using an explicit EUR-per-currency-unit FX rate.
/// The caller owns the historical FX-date semantics.
pub fn to_euro_cents(range: AnnualizedRange, euros_per_unit: f64) -> Result<(EuroCents, EuroCents), &'static str> {
    if !euros_per_unit.is_finite() || euros_per_unit <= 0.0 {
        return Err("invalid FX rate");
    }
    fn convert(value: i64, rate: f64) -> Result<EuroCents, &'static str> {
        let euro_cents = (value as f64) * rate;
        if !euro_cents.is_finite() || euro_cents > i64::MAX as f64 {
            return Err("FX conversion overflow");
        }
        Ok(EuroCents(euro_cents.round() as i64))
    }
    Ok((convert(range.low_minor_units, euros_per_unit)?, convert(range.high_minor_units, euros_per_unit)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_codes_are_normalized() {
        assert_eq!(CurrencyCode::new("gbp").unwrap().bytes(), *b"GBP");
    }

    #[test]
    fn weekly_salary_annualizes_without_collapsing_range() {
        let gbp = CurrencyCode::new("GBP").unwrap();
        let range = CurrencyRange::new(gbp, 100_00, 120_00).unwrap();
        let annual = annualize(range, Periodicity::Weekly).unwrap();
        assert_eq!(annual.low_minor_units, 520_000);
        assert_eq!(annual.high_minor_units, 624_000);
    }

    #[test]
    fn one_off_fees_are_not_misrepresented_as_wages() {
        let eur = CurrencyCode::new("EUR").unwrap();
        let range = CurrencyRange::new(eur, 100, 100).unwrap();
        assert!(annualize(range, Periodicity::OneOff).is_err());
    }
}
