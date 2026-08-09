use crate::{canonical::stable_hash64, CanonicalDataset, CanonicalFmSnapshot, DatasetError};
use footy_core::{Date, FeatureId, FeatureSetVersion, ObservationState, PlayerId};
use footy_market::{
    align_paid_transfers, fit_market_calibration, CalibrationConfig, MarketPerceivedValue,
    MarketValueObservation, PriceTargetKind, TransferEvent,
};
use footy_performance::{build_window, PlayerMatch};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const FEATURE_AGE_YEARS: FeatureId = FeatureId(1);
pub const FEATURE_AGE_OBSERVED: FeatureId = FeatureId(2);
pub const FEATURE_CA_VALUE: FeatureId = FeatureId(3);
pub const FEATURE_CA_OBSERVED: FeatureId = FeatureId(4);
pub const FEATURE_PA_VALUE: FeatureId = FeatureId(5);
pub const FEATURE_PA_OBSERVED: FeatureId = FeatureId(6);
pub const FEATURE_REPUTATION_VALUE: FeatureId = FeatureId(7);
pub const FEATURE_REPUTATION_OBSERVED: FeatureId = FeatureId(8);
pub const FEATURE_FM_AGE_DAYS: FeatureId = FeatureId(9);
pub const FEATURE_FM_PRESENT: FeatureId = FeatureId(10);
pub const FEATURE_CA_VELOCITY: FeatureId = FeatureId(11);
pub const FEATURE_CA_VELOCITY_OBSERVED: FeatureId = FeatureId(12);
pub const FEATURE_CA_REALIZATION: FeatureId = FeatureId(13);
pub const FEATURE_CA_REALIZATION_OBSERVED: FeatureId = FeatureId(14);
const FEATURE_ATTRIBUTE_MEAN_VELOCITY: FeatureId = FeatureId(15);
const FEATURE_ATTRIBUTE_MEAN_VELOCITY_OBSERVED: FeatureId = FeatureId(16);
const FEATURE_MINUTES_30: FeatureId = FeatureId(101);
const FEATURE_MINUTES_30_OBSERVED: FeatureId = FeatureId(102);
const FEATURE_MINUTES_90: FeatureId = FeatureId(103);
const FEATURE_MINUTES_90_OBSERVED: FeatureId = FeatureId(104);
const FEATURE_MINUTES_365: FeatureId = FeatureId(105);
const FEATURE_MINUTES_365_OBSERVED: FeatureId = FeatureId(106);
const FM_ATTRIBUTE_BASE: u32 = 1_000;
const PERFORMANCE_NAMESPACE: u32 = 0x4000_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterializationConfig {
    pub feature_set_version: FeatureSetVersion,
    pub through: Option<Date>,
    pub performance_half_life_days: [f64; 3],
    pub low_sample_minutes: u32,
    pub market_calibration: CalibrationConfig,
}

impl Default for MaterializationConfig {
    fn default() -> Self {
        Self {
            feature_set_version: FeatureSetVersion(1),
            through: None,
            performance_half_life_days: [15.0, 45.0, 182.5],
            low_sample_minutes: 450,
            market_calibration: CalibrationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatasetFeatureRow {
    pub player: PlayerId,
    pub as_of: Date,
    pub features: Vec<f32>,
    pub target_log_price: Option<f64>,
    pub target_kind: Option<PriceTargetKind>,
    pub target_variance_log: Option<f64>,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureMaterialization {
    pub feature_set_version: FeatureSetVersion,
    pub feature_ids: Vec<FeatureId>,
    pub rows: Vec<DatasetFeatureRow>,
    pub actual_target_rows: u64,
    pub calibrated_target_rows: u64,
    pub skipped_targets_without_state: u64,
    pub skipped_uncalibrated_market_values: u64,
}

pub fn materialize_training_features(
    dataset: &CanonicalDataset,
    config: MaterializationConfig,
) -> Result<FeatureMaterialization, DatasetError> {
    dataset.validate()?;
    validate_config(config)?;
    let feature_ids = build_feature_registry(dataset)?;
    let indexes = FeatureIndexes::new(dataset);
    let targets = build_targets(dataset, config)?;
    let mut rows = Vec::with_capacity(targets.targets.len());
    let mut actual_target_rows = 0_u64;
    let mut calibrated_target_rows = 0_u64;
    let mut skipped_targets_without_state = 0_u64;

    for ((player, target_date), target) in targets.targets {
        if config.through.is_some_and(|through| target_date > through) { continue; }
        let feature_date = target_date.checked_sub_days(1).map_err(|_| DatasetError::new("target date underflow"))?;
        let Some(features) = build_player_features(dataset, &indexes, player, target_date, feature_date, &feature_ids, config)? else {
            skipped_targets_without_state = skipped_targets_without_state.saturating_add(1);
            continue;
        };
        let target_log_price = (target.price_cents as f64).ln();
        let weight = match target.kind {
            PriceTargetKind::ActualTransfer => 1.0,
            PriceTargetKind::CalibratedMarketValue => 1.0 / (1.0 + target.variance_log.max(0.0)),
        };
        match target.kind {
            PriceTargetKind::ActualTransfer => actual_target_rows = actual_target_rows.saturating_add(1),
            PriceTargetKind::CalibratedMarketValue => calibrated_target_rows = calibrated_target_rows.saturating_add(1),
        }
        rows.push(DatasetFeatureRow {
            player,
            as_of: target_date,
            features,
            target_log_price: Some(target_log_price),
            target_kind: Some(target.kind),
            target_variance_log: Some(target.variance_log),
            weight,
        });
    }
    rows.sort_by_key(|row| (row.as_of, row.player));
    Ok(FeatureMaterialization {
        feature_set_version: config.feature_set_version,
        feature_ids,
        rows,
        actual_target_rows,
        calibrated_target_rows,
        skipped_targets_without_state,
        skipped_uncalibrated_market_values: targets.skipped_uncalibrated_market_values,
    })
}

pub fn materialize_scoring_features(
    dataset: &CanonicalDataset,
    as_of: Date,
    feature_set_version: FeatureSetVersion,
    expected_feature_ids: Option<&[FeatureId]>,
) -> Result<FeatureMaterialization, DatasetError> {
    dataset.validate()?;
    let default_config = MaterializationConfig { feature_set_version, through: Some(as_of), ..MaterializationConfig::default() };
    let generated = build_feature_registry(dataset)?;
    let feature_ids = expected_feature_ids.map_or(generated, |ids| ids.to_vec());
    if feature_ids.is_empty() { return Err(DatasetError::new("scoring feature registry cannot be empty")); }
    let mut unique = BTreeSet::new();
    if feature_ids.iter().any(|id| !unique.insert(id.0)) { return Err(DatasetError::new("duplicate scoring feature id")); }
    let indexes = FeatureIndexes::new(dataset);
    let mut rows = Vec::new();
    for player in &dataset.players {
        if let Some(features) = build_player_features(dataset, &indexes, player.player, as_of, as_of, &feature_ids, default_config)? {
            rows.push(DatasetFeatureRow { player: player.player, as_of, features, target_log_price: None, target_kind: None, target_variance_log: None, weight: 1.0 });
        }
    }
    rows.sort_by_key(|row| row.player);
    Ok(FeatureMaterialization { feature_set_version, feature_ids, rows, actual_target_rows: 0, calibrated_target_rows: 0, skipped_targets_without_state: 0, skipped_uncalibrated_market_values: 0 })
}

pub fn feature_name(dataset: &CanonicalDataset, feature: FeatureId) -> Option<String> {
    let fixed = match feature {
        FEATURE_AGE_YEARS => Some("age_years"), FEATURE_AGE_OBSERVED => Some("age_observed"),
        FEATURE_CA_VALUE => Some("fm_ca"), FEATURE_CA_OBSERVED => Some("fm_ca_observed"),
        FEATURE_PA_VALUE => Some("fm_pa"), FEATURE_PA_OBSERVED => Some("fm_pa_observed"),
        FEATURE_REPUTATION_VALUE => Some("fm_reputation"), FEATURE_REPUTATION_OBSERVED => Some("fm_reputation_observed"),
        FEATURE_FM_AGE_DAYS => Some("fm_snapshot_age_days"), FEATURE_FM_PRESENT => Some("fm_snapshot_present"),
        FEATURE_CA_VELOCITY => Some("fm_ca_velocity_per_year"), FEATURE_CA_VELOCITY_OBSERVED => Some("fm_ca_velocity_observed"),
        FEATURE_CA_REALIZATION => Some("fm_ca_realization"), FEATURE_CA_REALIZATION_OBSERVED => Some("fm_ca_realization_observed"),
        FEATURE_ATTRIBUTE_MEAN_VELOCITY => Some("fm_attribute_mean_velocity"), FEATURE_ATTRIBUTE_MEAN_VELOCITY_OBSERVED => Some("fm_attribute_mean_velocity_observed"),
        FEATURE_MINUTES_30 => Some("perf_minutes_30"), FEATURE_MINUTES_30_OBSERVED => Some("perf_minutes_30_observed"),
        FEATURE_MINUTES_90 => Some("perf_minutes_90"), FEATURE_MINUTES_90_OBSERVED => Some("perf_minutes_90_observed"),
        FEATURE_MINUTES_365 => Some("perf_minutes_365"), FEATURE_MINUTES_365_OBSERVED => Some("perf_minutes_365_observed"),
        _ => None,
    };
    if let Some(name) = fixed { return Some(name.to_owned()); }
    if feature.0 >= FM_ATTRIBUTE_BASE && feature.0 < PERFORMANCE_NAMESPACE {
        let offset = feature.0 - FM_ATTRIBUTE_BASE;
        let attribute = offset / 2;
        let suffix = if offset % 2 == 0 { "value" } else { "observed" };
        if attribute <= u16::MAX as u32 { return Some(format!("fm_attr:{attribute}:{suffix}")); }
    }
    for metric in &dataset.metric_registry {
        for horizon in [30_u16, 90, 365] {
            for statistic in [PerformanceStatistic::Per90, PerformanceStatistic::EwmaPer90, PerformanceStatistic::TrendPer90] {
                for observed in [false, true] {
                    if performance_feature_id(&metric.name, horizon, statistic, observed) == feature {
                        return Some(format!("perf:{}:{}:{}", metric.name, horizon, if observed { statistic.observed_name() } else { statistic.value_name() }));
                    }
                }
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
enum PerformanceStatistic { Per90, EwmaPer90, TrendPer90 }
impl PerformanceStatistic {
    fn value_name(self) -> &'static str { match self { Self::Per90 => "per90", Self::EwmaPer90 => "ewma_per90", Self::TrendPer90 => "trend_per90" } }
    fn observed_name(self) -> &'static str { match self { Self::Per90 => "per90_observed", Self::EwmaPer90 => "ewma_per90_observed", Self::TrendPer90 => "trend_per90_observed" } }
}

fn validate_config(config: MaterializationConfig) -> Result<(), DatasetError> {
    if config.performance_half_life_days.iter().any(|value| !value.is_finite() || *value <= 0.0) {
        return Err(DatasetError::new("performance half lives must be positive and finite"));
    }
    Ok(())
}

fn build_feature_registry(dataset: &CanonicalDataset) -> Result<Vec<FeatureId>, DatasetError> {
    let mut ids = BTreeSet::new();
    for fixed in [
        FEATURE_AGE_YEARS, FEATURE_AGE_OBSERVED, FEATURE_CA_VALUE, FEATURE_CA_OBSERVED,
        FEATURE_PA_VALUE, FEATURE_PA_OBSERVED, FEATURE_REPUTATION_VALUE, FEATURE_REPUTATION_OBSERVED,
        FEATURE_FM_AGE_DAYS, FEATURE_FM_PRESENT, FEATURE_CA_VELOCITY, FEATURE_CA_VELOCITY_OBSERVED,
        FEATURE_CA_REALIZATION, FEATURE_CA_REALIZATION_OBSERVED, FEATURE_ATTRIBUTE_MEAN_VELOCITY,
        FEATURE_ATTRIBUTE_MEAN_VELOCITY_OBSERVED, FEATURE_MINUTES_30, FEATURE_MINUTES_30_OBSERVED,
        FEATURE_MINUTES_90, FEATURE_MINUTES_90_OBSERVED, FEATURE_MINUTES_365, FEATURE_MINUTES_365_OBSERVED,
    ] { ids.insert(fixed.0); }
    let attribute_ids: BTreeSet<u16> = dataset.fm_snapshots.iter().flat_map(|snapshot| snapshot.attributes.iter().map(|attribute| attribute.id.0)).collect();
    for attribute in attribute_ids {
        ids.insert(fm_attribute_feature(attribute, false).0);
        ids.insert(fm_attribute_feature(attribute, true).0);
    }
    for metric in &dataset.metric_registry {
        for horizon in [30_u16, 90, 365] {
            for statistic in [PerformanceStatistic::Per90, PerformanceStatistic::EwmaPer90, PerformanceStatistic::TrendPer90] {
                for observed in [false, true] {
                    let id = performance_feature_id(&metric.name, horizon, statistic, observed);
                    if !ids.insert(id.0) { return Err(DatasetError::new(format!("feature id collision for performance metric {}", metric.name))); }
                }
            }
        }
    }
    Ok(ids.into_iter().map(FeatureId).collect())
}

fn fm_attribute_feature(attribute: u16, observed: bool) -> FeatureId {
    FeatureId(FM_ATTRIBUTE_BASE + u32::from(attribute) * 2 + u32::from(observed))
}

fn performance_feature_id(name: &str, horizon: u16, statistic: PerformanceStatistic, observed: bool) -> FeatureId {
    let descriptor = format!("{name}:{horizon}:{}:{}", statistic.value_name(), if observed { 1 } else { 0 });
    let hash = stable_hash64(b"feature:performance:", descriptor.as_bytes());
    FeatureId(PERFORMANCE_NAMESPACE | ((hash as u32) & 0x0fff_ffff))
}

struct FeatureIndexes<'a> {
    fm: HashMap<PlayerId, Vec<&'a CanonicalFmSnapshot>>,
    matches: HashMap<PlayerId, Vec<PlayerMatch>>,
}

impl<'a> FeatureIndexes<'a> {
    fn new(dataset: &'a CanonicalDataset) -> Self {
        let mut fm: HashMap<PlayerId, Vec<&CanonicalFmSnapshot>> = HashMap::new();
        for snapshot in &dataset.fm_snapshots { fm.entry(snapshot.player).or_default().push(snapshot); }
        for snapshots in fm.values_mut() { snapshots.sort_by_key(|snapshot| snapshot.snapshot_date); }
        let mut matches: HashMap<PlayerId, Vec<PlayerMatch>> = HashMap::new();
        for observation in &dataset.performance {
            if !observation.is_match_level() { continue; }
            let minutes = observation.minutes.min(u32::from(u16::MAX)) as u16;
            matches.entry(observation.player).or_default().push(PlayerMatch { player: observation.player, club: observation.club, competition: observation.competition, date: observation.end_date, minutes, metrics: observation.metrics.clone(), observation_state: observation.observation_state });
        }
        for values in matches.values_mut() { values.sort_by_key(|value| value.date); }
        Self { fm, matches }
    }
}

fn build_player_features(
    dataset: &CanonicalDataset,
    indexes: &FeatureIndexes<'_>,
    player: PlayerId,
    state_date: Date,
    data_cutoff: Date,
    feature_ids: &[FeatureId],
    config: MaterializationConfig,
) -> Result<Option<Vec<f32>>, DatasetError> {
    let Some(snapshots) = indexes.fm.get(&player) else { return Ok(None); };
    let Some((latest_index, latest)) = snapshots.iter().enumerate().rev().find(|(_, snapshot)| snapshot.snapshot_date <= data_cutoff) else { return Ok(None); };
    let canonical_player = dataset.player(player).ok_or_else(|| DatasetError::new("feature player missing from canonical registry"))?;
    let mut values = HashMap::<FeatureId, f32>::new();
    values.insert(FEATURE_FM_PRESENT, 1.0);
    values.insert(FEATURE_FM_AGE_DAYS, state_date.days_since(latest.snapshot_date).max(0) as f32);
    if let Some(birth_date) = canonical_player.birth_date {
        values.insert(FEATURE_AGE_YEARS, (state_date.days_since(birth_date) as f32 / 365.2425).max(0.0));
        values.insert(FEATURE_AGE_OBSERVED, 1.0);
    }
    if let Some(ca) = latest.current_ability { values.insert(FEATURE_CA_VALUE, ca as f32); values.insert(FEATURE_CA_OBSERVED, 1.0); }
    if let Some(pa) = latest.potential_ability { values.insert(FEATURE_PA_VALUE, pa as f32); values.insert(FEATURE_PA_OBSERVED, 1.0); }
    if let Some(reputation) = latest.reputation { values.insert(FEATURE_REPUTATION_VALUE, reputation as f32); values.insert(FEATURE_REPUTATION_OBSERVED, 1.0); }
    if let (Some(ca), Some(pa)) = (latest.current_ability, latest.potential_ability) {
        if pa > 0 { values.insert(FEATURE_CA_REALIZATION, ca as f32 / pa as f32); values.insert(FEATURE_CA_REALIZATION_OBSERVED, 1.0); }
    }
    for attribute in &latest.attributes {
        values.insert(fm_attribute_feature(attribute.id.0, false), attribute.value as f32);
        values.insert(fm_attribute_feature(attribute.id.0, true), 1.0);
    }
    if latest_index > 0 {
        let previous = snapshots[latest_index - 1];
        let years = latest.snapshot_date.days_since(previous.snapshot_date) as f32 / 365.2425;
        if years > 0.0 {
            if let (Some(current), Some(prior)) = (latest.current_ability, previous.current_ability) {
                values.insert(FEATURE_CA_VELOCITY, (current as f32 - prior as f32) / years);
                values.insert(FEATURE_CA_VELOCITY_OBSERVED, 1.0);
            }
            let prior_attributes: HashMap<u16, u8> = previous.attributes.iter().map(|attribute| (attribute.id.0, attribute.value)).collect();
            let deltas: Vec<f32> = latest.attributes.iter().filter_map(|attribute| prior_attributes.get(&attribute.id.0).map(|prior| attribute.value as f32 - *prior as f32)).collect();
            if !deltas.is_empty() {
                values.insert(FEATURE_ATTRIBUTE_MEAN_VELOCITY, deltas.iter().sum::<f32>() / deltas.len() as f32 / years);
                values.insert(FEATURE_ATTRIBUTE_MEAN_VELOCITY_OBSERVED, 1.0);
            }
        }
    }

    let player_matches = indexes.matches.get(&player).map(Vec::as_slice).unwrap_or(&[]);
    let horizons = [(30_u16, config.performance_half_life_days[0], FEATURE_MINUTES_30, FEATURE_MINUTES_30_OBSERVED), (90, config.performance_half_life_days[1], FEATURE_MINUTES_90, FEATURE_MINUTES_90_OBSERVED), (365, config.performance_half_life_days[2], FEATURE_MINUTES_365, FEATURE_MINUTES_365_OBSERVED)];
    for (horizon, half_life, minutes_feature, minutes_observed) in horizons {
        let window = build_window(player_matches, player, data_cutoff, horizon, half_life, config.low_sample_minutes);
        if window.coverage.matches > 0 {
            values.insert(minutes_feature, window.coverage.minutes as f32);
            values.insert(minutes_observed, 1.0);
        }
        for metric in &window.metrics {
            let Some(name) = dataset.metric_name(metric.metric) else { continue; };
            for (statistic, value) in [(PerformanceStatistic::Per90, metric.per90), (PerformanceStatistic::EwmaPer90, metric.ewma_per90), (PerformanceStatistic::TrendPer90, metric.trend_per90)] {
                if let Some(value) = value.filter(|value| value.is_finite()) {
                    values.insert(performance_feature_id(name, horizon, statistic, false), value as f32);
                    values.insert(performance_feature_id(name, horizon, statistic, true), 1.0);
                }
            }
        }
    }
    let features = feature_ids.iter().map(|id| values.get(id).copied().unwrap_or(0.0)).collect();
    Ok(Some(features))
}

struct TargetValue { price_cents: i64, kind: PriceTargetKind, variance_log: f64 }
struct TargetBuild { targets: BTreeMap<(PlayerId, Date), TargetValue>, skipped_uncalibrated_market_values: u64 }

fn build_targets(dataset: &CanonicalDataset, config: MaterializationConfig) -> Result<TargetBuild, DatasetError> {
    let values: Vec<MarketValueObservation> = dataset.market_values.iter().map(|value| MarketValueObservation { player: value.player, date: value.date, value: value.value, provenance_id: value.provenance_id }).collect();
    let transfers: Vec<TransferEvent> = dataset.transfers.iter().map(|transfer| TransferEvent { id: transfer.id, player: transfer.player, date: transfer.date, from: transfer.from, to: transfer.to, kind: transfer.kind, fee: transfer.fee, loan_fee: transfer.loan_fee, provenance_id: transfer.provenance_id }).collect();
    let aligned = align_paid_transfers(&transfers, &values);
    let mut targets = BTreeMap::new();
    for transfer in &transfers {
        if transfer.kind.is_paid_permanent() {
            if let Some(fee) = transfer.fee.filter(|fee| fee.cents() > 0) {
                targets.insert((transfer.player, transfer.date), TargetValue { price_cents: fee.cents(), kind: PriceTargetKind::ActualTransfer, variance_log: 0.0 });
            }
        }
    }
    let mut calibration_cache: HashMap<Date, Option<footy_market::MarketCalibration>> = HashMap::new();
    let mut skipped = 0_u64;
    for value in &values {
        if config.through.is_some_and(|through| value.date > through) || targets.contains_key(&(value.player, value.date)) { continue; }
        let calibration_date = value.date.checked_sub_days(1).map_err(|_| DatasetError::new("market-value date underflow"))?;
        let calibration = *calibration_cache.entry(calibration_date).or_insert_with(|| fit_market_calibration(&aligned, calibration_date, config.market_calibration).ok());
        let Some(calibration) = calibration else { skipped = skipped.saturating_add(1); continue; };
        let distribution = calibration.perceived_price(value.value).map_err(|_| DatasetError::new("market calibration produced invalid price"))?;
        let perceived = MarketPerceivedValue(distribution);
        let variance_log = calibration.residual_sigma.powi(2);
        targets.insert((value.player, value.date), TargetValue { price_cents: perceived.0.p50.cents(), kind: PriceTargetKind::CalibratedMarketValue, variance_log });
    }
    Ok(TargetBuild { targets, skipped_uncalibrated_market_values: skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalPlayer, CanonicalTransfer};
    use footy_core::{Confidence, DatasetVersion, ProvenanceId};
    use footy_source_fm::FmAttribute;

    fn dataset() -> CanonicalDataset {
        let player = PlayerId(1);
        CanonicalDataset {
            dataset_version: DatasetVersion(1), source_manifest_checksum: 1, provenances: Vec::new(),
            players: vec![CanonicalPlayer { player, fm_external_id: "fm1".into(), name: "A".into(), birth_date: Some(Date(0)) }],
            identities: Vec::new(), unresolved_identities: Vec::new(), issues: Vec::new(),
            fm_snapshots: vec![
                CanonicalFmSnapshot { player, snapshot_date: Date(10), current_ability: Some(100), potential_ability: Some(150), reputation: None, attributes: vec![FmAttribute { id: footy_core::AttributeId(1), value: 10 }], positions: Vec::new(), personality_attributes: Vec::new(), hidden_attributes: Vec::new(), provenance_id: ProvenanceId(1), schema_version: 1 },
                CanonicalFmSnapshot { player, snapshot_date: Date(30), current_ability: Some(180), potential_ability: Some(190), reputation: None, attributes: vec![FmAttribute { id: footy_core::AttributeId(1), value: 20 }], positions: Vec::new(), personality_attributes: Vec::new(), hidden_attributes: Vec::new(), provenance_id: ProvenanceId(1), schema_version: 1 },
            ],
            clubs: Vec::new(), competitions: Vec::new(), metric_registry: Vec::new(), market_values: Vec::new(),
            transfers: vec![CanonicalTransfer { id: footy_core::TransferId(1), player, date: Date(20), from: footy_core::ClubId(1), to: footy_core::ClubId(2), kind: footy_market::TransferKind::Permanent, fee: Some(footy_core::EuroCents(1_000_000)), loan_fee: None, provenance_id: ProvenanceId(1) }],
            contracts: Vec::new(), performance: Vec::new(), salaries: Vec::new(),
        }
    }

    #[test]
    fn training_features_use_only_pre_target_fm_snapshot() {
        let materialized = materialize_training_features(&dataset(), MaterializationConfig { market_calibration: CalibrationConfig { min_observations: 2, ..CalibrationConfig::default() }, ..MaterializationConfig::default() }).unwrap();
        assert_eq!(materialized.rows.len(), 1);
        let attr_id = fm_attribute_feature(1, false);
        let position = materialized.feature_ids.iter().position(|id| *id == attr_id).unwrap();
        assert_eq!(materialized.rows[0].features[position], 10.0);
        assert_eq!(materialized.rows[0].target_kind, Some(PriceTargetKind::ActualTransfer));
    }

    #[test]
    fn scoring_can_project_exact_model_feature_order() {
        let expected = vec![FEATURE_CA_VALUE, FEATURE_AGE_YEARS];
        let scored = materialize_scoring_features(&dataset(), Date(40), FeatureSetVersion(1), Some(&expected)).unwrap();
        assert_eq!(scored.feature_ids, expected);
        assert_eq!(scored.rows.len(), 1);
        assert_eq!(scored.rows[0].features[0], 180.0);
    }

    #[test]
    fn performance_feature_hashes_are_stable() {
        let a = performance_feature_id("goals", 365, PerformanceStatistic::Per90, false);
        let b = performance_feature_id("goals", 365, PerformanceStatistic::Per90, false);
        assert_eq!(a, b);
        assert_ne!(a, performance_feature_id("assists", 365, PerformanceStatistic::Per90, false));
    }
}
