#![forbid(unsafe_code)]
//! Market-price calibration, transaction context, price distributions, and transfer hazard.

use footy_core::{
    ClubId, Confidence, Date, EuroCents, PlayerId, Probability, ProvenanceId, TransferId,
};
use footy_models::{fit_huber, HuberConfig, LogisticModel};

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

impl TransferKind {
    pub const fn is_paid_permanent(self) -> bool {
        matches!(self, Self::Permanent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketValueObservation {
    pub player: PlayerId,
    pub date: Date,
    pub value: EuroCents,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferEvent {
    pub id: TransferId,
    pub player: PlayerId,
    pub date: Date,
    pub from: ClubId,
    pub to: ClubId,
    pub kind: TransferKind,
    pub fee: Option<EuroCents>,
    pub loan_fee: Option<EuroCents>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractSnapshot {
    pub player: PlayerId,
    pub club: ClubId,
    pub observed_on: Date,
    pub expiry: Option<Date>,
    pub option_years: Option<u8>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClubMarketSnapshot {
    pub club: ClubId,
    pub as_of: Date,
    pub squad_value: EuroCents,
    pub trailing_buys: EuroCents,
    pub trailing_sales: EuroCents,
    pub financial_strength: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignedPaidTransfer {
    pub transfer_id: TransferId,
    pub player: PlayerId,
    pub date: Date,
    pub market_value_date: Date,
    pub market_value: EuroCents,
    pub fee: EuroCents,
}

/// Join paid permanent transfers to the latest positive market-value observation strictly before
/// the transaction date. Same-day or future market-value revisions are deliberately excluded.
pub fn align_paid_transfers(
    transfers: &[TransferEvent],
    values: &[MarketValueObservation],
) -> Vec<AlignedPaidTransfer> {
    let mut aligned = Vec::new();
    for transfer in transfers {
        if !transfer.kind.is_paid_permanent() {
            continue;
        }
        let Some(fee) = transfer.fee.filter(|fee| fee.cents() > 0) else {
            continue;
        };
        let latest = values
            .iter()
            .filter(|value| {
                value.player == transfer.player && value.date < transfer.date && value.value.cents() > 0
            })
            .max_by_key(|value| value.date);
        if let Some(value) = latest {
            aligned.push(AlignedPaidTransfer {
                transfer_id: transfer.id,
                player: transfer.player,
                date: transfer.date,
                market_value_date: value.date,
                market_value: value.value,
                fee,
            });
        }
    }
    aligned.sort_by_key(|row| (row.date, row.transfer_id));
    aligned
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationConfig {
    pub lookback_days: u32,
    pub half_life_days: f64,
    pub min_observations: u32,
    pub huber: HuberConfig,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            lookback_days: 730,
            half_life_days: 365.0,
            min_observations: 30,
            huber: HuberConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketCalibration {
    pub as_of: Date,
    pub alpha: f64,
    pub beta: f64,
    pub residual_sigma: f64,
    pub observations: u32,
    pub effective_samples: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    InsufficientObservations,
    InvalidConfiguration,
    FitFailed,
    InvalidPrice,
}

pub fn fit_market_calibration(
    rows: &[AlignedPaidTransfer],
    as_of: Date,
    config: CalibrationConfig,
) -> Result<MarketCalibration, CalibrationError> {
    if config.min_observations < 2
        || !config.half_life_days.is_finite()
        || config.half_life_days <= 0.0
        || config.lookback_days == 0
    {
        return Err(CalibrationError::InvalidConfiguration);
    }
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut weights = Vec::new();
    for row in rows {
        let age = as_of.days_since(row.date);
        if age < 0 || age > i64::from(config.lookback_days) {
            continue;
        }
        if row.market_value.cents() <= 0 || row.fee.cents() <= 0 {
            continue;
        }
        x.push((row.market_value.cents() as f64).ln());
        y.push((row.fee.cents() as f64).ln());
        weights.push((-std::f64::consts::LN_2 * age as f64 / config.half_life_days).exp());
    }
    if x.len() < config.min_observations as usize {
        return Err(CalibrationError::InsufficientObservations);
    }
    let matrix = footy_models::DenseMatrix::new(x.len(), 1, x).map_err(|_| CalibrationError::FitFailed)?;
    let fit = fit_huber(&matrix, &y, Some(&weights), config.huber).map_err(|_| CalibrationError::FitFailed)?;
    let alpha = fit.model.coefficients[0];
    let beta = fit.model.coefficients[1];
    if !alpha.is_finite() || !beta.is_finite() || !fit.residual_scale.is_finite() {
        return Err(CalibrationError::FitFailed);
    }
    Ok(MarketCalibration {
        as_of,
        alpha,
        beta,
        residual_sigma: fit.residual_scale.max(1e-9),
        observations: y.len().min(u32::MAX as usize) as u32,
        effective_samples: weights.iter().sum(),
    })
}

impl MarketCalibration {
    pub fn perceived_price(self, market_value: EuroCents) -> Result<PriceDistribution, CalibrationError> {
        if market_value.cents() <= 0 {
            return Err(CalibrationError::InvalidPrice);
        }
        let center = self.alpha + self.beta * (market_value.cents() as f64).ln();
        PriceDistribution::from_log_normal(center, self.residual_sigma, confidence_from_samples(self.effective_samples))
            .ok_or(CalibrationError::InvalidPrice)
    }

    pub fn shrink_toward(self, parent: Self, shrinkage_k: f64) -> Self {
        let k = if shrinkage_k.is_finite() && shrinkage_k >= 0.0 { shrinkage_k } else { 0.0 };
        let n = self.effective_samples.max(0.0);
        let weight = if n + k > 0.0 { n / (n + k) } else { 0.0 };
        Self {
            as_of: self.as_of,
            alpha: weight * self.alpha + (1.0 - weight) * parent.alpha,
            beta: weight * self.beta + (1.0 - weight) * parent.beta,
            residual_sigma: (weight * self.residual_sigma.powi(2) + (1.0 - weight) * parent.residual_sigma.powi(2)).sqrt(),
            observations: self.observations,
            effective_samples: self.effective_samples,
        }
    }
}

fn confidence_from_samples(effective_samples: f64) -> Confidence {
    let score = 1.0 - (-effective_samples.max(0.0) / 30.0).exp();
    Confidence::clamped(score as f32)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceDistribution {
    pub p10: EuroCents,
    pub p25: EuroCents,
    pub p50: EuroCents,
    pub p75: EuroCents,
    pub p90: EuroCents,
    pub confidence: Confidence,
}

impl PriceDistribution {
    pub fn new(
        p10: EuroCents,
        p25: EuroCents,
        p50: EuroCents,
        p75: EuroCents,
        p90: EuroCents,
        confidence: Confidence,
    ) -> Option<Self> {
        let ordered = p10 <= p25 && p25 <= p50 && p50 <= p75 && p75 <= p90 && p10.cents() >= 0;
        ordered.then_some(Self { p10, p25, p50, p75, p90, confidence })
    }

    pub fn point(value: EuroCents, confidence: Confidence) -> Option<Self> {
        (value.cents() >= 0).then_some(Self { p10: value, p25: value, p50: value, p75: value, p90: value, confidence })
    }

    pub fn from_log_normal(log_median: f64, sigma: f64, confidence: Confidence) -> Option<Self> {
        if !log_median.is_finite() || !sigma.is_finite() || sigma < 0.0 {
            return None;
        }
        const Z10: f64 = -1.281_551_565_544_600_4;
        const Z25: f64 = -0.674_489_750_196_081_7;
        const Z75: f64 = 0.674_489_750_196_081_7;
        const Z90: f64 = 1.281_551_565_544_600_4;
        Self::new(
            euro_from_log(log_median + Z10 * sigma)?,
            euro_from_log(log_median + Z25 * sigma)?,
            euro_from_log(log_median)?,
            euro_from_log(log_median + Z75 * sigma)?,
            euro_from_log(log_median + Z90 * sigma)?,
            confidence,
        )
    }

    pub fn from_samples(samples: &[EuroCents], confidence: Confidence) -> Option<Self> {
        let values: Vec<f64> = samples.iter().filter(|value| value.cents() >= 0).map(|value| value.cents() as f64).collect();
        if values.is_empty() { return None; }
        let q = |p| footy_models::quantile(&values, p).map(|value| EuroCents(value.round() as i64));
        Self::new(q(0.10)?, q(0.25)?, q(0.50)?, q(0.75)?, q(0.90)?, confidence)
    }
}

fn euro_from_log(value: f64) -> Option<EuroCents> {
    let cents = value.exp();
    if !cents.is_finite() || cents < 0.0 || cents > i64::MAX as f64 { None } else { Some(EuroCents(cents.round() as i64)) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketPerceivedValue(pub PriceDistribution);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FundamentalValue(pub PriceDistribution);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcquisitionPrice(pub PriceDistribution);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FutureValue(pub PriceDistribution);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FairWage(pub PriceDistribution);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceTargetKind {
    ActualTransfer,
    CalibratedMarketValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceTarget {
    pub player: PlayerId,
    pub date: Date,
    pub price: EuroCents,
    pub variance_log: f64,
    pub kind: PriceTargetKind,
}

impl PriceTarget {
    pub fn actual_transfer(player: PlayerId, date: Date, fee: EuroCents) -> Option<Self> {
        (fee.cents() > 0).then_some(Self { player, date, price: fee, variance_log: 0.0, kind: PriceTargetKind::ActualTransfer })
    }

    pub fn calibrated_market_value(player: PlayerId, date: Date, value: MarketPerceivedValue, residual_sigma: f64) -> Option<Self> {
        if !residual_sigma.is_finite() || residual_sigma < 0.0 { return None; }
        Some(Self { player, date, price: value.0.p50, variance_log: residual_sigma.powi(2), kind: PriceTargetKind::CalibratedMarketValue })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketIndexObservation {
    pub date: Date,
    pub observed_price: EuroCents,
    pub constant_quality_price: EuroCents,
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketIndex {
    pub as_of: Date,
    pub level: f64,
    pub residual_sigma: f64,
    pub observations: u32,
}

pub fn fit_market_index(observations: &[MarketIndexObservation], as_of: Date) -> Option<MarketIndex> {
    let mut weighted_logs = Vec::new();
    let mut weights = Vec::new();
    for observation in observations.iter().filter(|row| row.date <= as_of) {
        if observation.observed_price.cents() <= 0 || observation.constant_quality_price.cents() <= 0 || !observation.weight.is_finite() || observation.weight <= 0.0 { continue; }
        weighted_logs.push((observation.observed_price.cents() as f64 / observation.constant_quality_price.cents() as f64).ln());
        weights.push(observation.weight);
    }
    if weighted_logs.is_empty() { return None; }
    let weight_sum = weights.iter().sum::<f64>();
    if weight_sum <= 0.0 { return None; }
    let center = weighted_logs.iter().zip(&weights).map(|(value, weight)| value * weight).sum::<f64>() / weight_sum;
    let variance = weighted_logs.iter().zip(&weights).map(|(value, weight)| weight * (value - center).powi(2)).sum::<f64>() / weight_sum;
    Some(MarketIndex { as_of, level: center.exp(), residual_sigma: variance.sqrt(), observations: weighted_logs.len().min(u32::MAX as usize) as u32 })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcquisitionContext {
    pub contract_remaining_days: i32,
    pub buyer: ClubId,
    pub seller: ClubId,
    pub buyer_power: f64,
    pub seller_power: f64,
    pub route_log_premium: f64,
    pub window_log_premium: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcquisitionCoefficients {
    pub intercept_log: f64,
    pub contract_years: f64,
    pub buyer_power: f64,
    pub seller_power: f64,
    pub residual_sigma: f64,
}

impl Default for AcquisitionCoefficients {
    fn default() -> Self {
        Self { intercept_log: 0.0, contract_years: 0.0, buyer_power: 0.0, seller_power: 0.0, residual_sigma: 0.15 }
    }
}

pub fn expected_acquisition_price(
    fundamental: FundamentalValue,
    context: AcquisitionContext,
    coefficients: AcquisitionCoefficients,
) -> Option<AcquisitionPrice> {
    if fundamental.0.p50.cents() <= 0
        || !context.buyer_power.is_finite()
        || !context.seller_power.is_finite()
        || !context.route_log_premium.is_finite()
        || !context.window_log_premium.is_finite()
        || !coefficients.residual_sigma.is_finite()
        || coefficients.residual_sigma < 0.0
    { return None; }
    let years = f64::from(context.contract_remaining_days.max(0)) / 365.2425;
    let adjustment = coefficients.intercept_log
        + coefficients.contract_years * years
        + coefficients.buyer_power * context.buyer_power
        + coefficients.seller_power * context.seller_power
        + context.route_log_premium
        + context.window_log_premium;
    let center = (fundamental.0.p50.cents() as f64).ln() + adjustment;
    PriceDistribution::from_log_normal(center, coefficients.residual_sigma, fundamental.0.confidence).map(AcquisitionPrice)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferHazard {
    pub p_6m: Probability,
    pub p_12m: Probability,
    pub p_24m: Probability,
}

impl TransferHazard {
    pub fn new(p_6m: f64, p_12m: f64, p_24m: f64) -> Self {
        let six = p_6m.clamp(0.0, 1.0);
        let twelve = p_12m.clamp(six, 1.0);
        let twenty_four = p_24m.clamp(twelve, 1.0);
        Self {
            p_6m: Probability::clamped(six as f32),
            p_12m: Probability::clamped(twelve as f32),
            p_24m: Probability::clamped(twenty_four as f32),
        }
    }
}

pub trait HazardModel {
    fn predict(&self, features: &[f64]) -> Option<TransferHazard>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogisticHazardModel {
    pub six_month: LogisticModel,
    pub twelve_month: LogisticModel,
    pub twenty_four_month: LogisticModel,
}

impl HazardModel for LogisticHazardModel {
    fn predict(&self, features: &[f64]) -> Option<TransferHazard> {
        Some(TransferHazard::new(
            self.six_month.predict_probability(features).ok()?,
            self.twelve_month.predict_probability(features).ok()?,
            self.twenty_four_month.predict_probability(features).ok()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_is_strictly_pre_transfer_and_excludes_free_moves() {
        let values = [
            MarketValueObservation { player: PlayerId(1), date: Date(9), value: EuroCents(1_000), provenance_id: ProvenanceId(1) },
            MarketValueObservation { player: PlayerId(1), date: Date(10), value: EuroCents(9_999), provenance_id: ProvenanceId(1) },
        ];
        let transfers = [
            TransferEvent { id: TransferId(1), player: PlayerId(1), date: Date(10), from: ClubId(1), to: ClubId(2), kind: TransferKind::Permanent, fee: Some(EuroCents(1_200)), loan_fee: None, provenance_id: ProvenanceId(2) },
            TransferEvent { id: TransferId(2), player: PlayerId(1), date: Date(20), from: ClubId(2), to: ClubId(3), kind: TransferKind::Free, fee: None, loan_fee: None, provenance_id: ProvenanceId(2) },
        ];
        let aligned = align_paid_transfers(&transfers, &values);
        assert_eq!(aligned.len(), 1);
        assert_eq!(aligned[0].market_value, EuroCents(1_000));
    }

    #[test]
    fn calibration_recovers_near_one_to_one_price_surface() {
        let mut rows = Vec::new();
        for index in 1..=40_i64 {
            rows.push(AlignedPaidTransfer {
                transfer_id: TransferId(index as u64),
                player: PlayerId(index as u64),
                date: Date(index as i32),
                market_value_date: Date(index as i32 - 1),
                market_value: EuroCents(index * 100_000),
                fee: EuroCents(index * 110_000),
            });
        }
        let calibration = fit_market_calibration(&rows, Date(50), CalibrationConfig { min_observations: 20, ..CalibrationConfig::default() }).unwrap();
        assert!((calibration.beta - 1.0).abs() < 0.05);
        let price = calibration.perceived_price(EuroCents(10_000_000)).unwrap();
        assert!(price.p50 > EuroCents(10_000_000));
    }

    #[test]
    fn price_distribution_preserves_quantile_order() {
        let distribution = PriceDistribution::from_log_normal((1_000_000_f64).ln(), 0.2, Confidence::ONE).unwrap();
        assert!(distribution.p10 < distribution.p25 && distribution.p25 < distribution.p50 && distribution.p50 < distribution.p75 && distribution.p75 < distribution.p90);
    }

    #[test]
    fn transfer_hazard_is_monotonic() {
        let hazard = TransferHazard::new(0.4, 0.2, 0.3);
        assert!(hazard.p_6m.get() <= hazard.p_12m.get());
        assert!(hazard.p_12m.get() <= hazard.p_24m.get());
    }

    #[test]
    fn actual_price_target_rejects_zero_fee() {
        assert!(PriceTarget::actual_transfer(PlayerId(1), Date(1), EuroCents::ZERO).is_none());
    }
}
