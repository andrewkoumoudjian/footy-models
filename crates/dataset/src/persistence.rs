use crate::canonical::{
    source_from_tag, source_tag, CanonicalContract, CanonicalDataset, CanonicalFmSnapshot,
    CanonicalIdentity, CanonicalIssue, CanonicalMarketValue, CanonicalPerformanceObservation,
    CanonicalPlayer, CanonicalProvenance, CanonicalSalaryObservation, CanonicalTransfer,
    CanonicalUnresolvedCandidate, CanonicalUnresolvedIdentity, MetricDefinition,
    SourceClubRecord, SourceCompetitionRecord, UnresolvedReason,
};
use crate::DatasetError;
use footy_core::{
    ClubId, CompetitionId, Confidence, DatasetVersion, Date, MetricId, ObservationState, PlayerId,
    ProvenanceId, Source, TransferId,
};
use footy_entities::LinkMethod;
use footy_performance::MetricValue;
use footy_source_fm::{FmAttribute, PositionRating};
use footy_source_salary::{CompensationKind, CurrencyCode, CurrencyRange, Periodicity, WageBasis};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"FTYCAN01";
const SCHEMA_VERSION: u16 = 1;
const HEADER_SIZE: usize = 42;

pub fn write_canonical_dataset(path: impl AsRef<Path>, dataset: &CanonicalDataset) -> Result<(), DatasetError> {
    dataset.validate()?;
    let payload = encode_dataset(dataset)?;
    let checksum = checksum(&payload);
    let mut bytes = Vec::with_capacity(HEADER_SIZE + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    bytes.extend_from_slice(&dataset.dataset_version.0.to_le_bytes());
    bytes.extend_from_slice(&dataset.source_manifest_checksum.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(&payload);
    atomic_create(path.as_ref(), &bytes)
}

pub fn read_canonical_dataset(path: impl AsRef<Path>) -> Result<CanonicalDataset, DatasetError> {
    let bytes = fs::read(path.as_ref()).map_err(|error| DatasetError::new(format!("canonical dataset read failed: {error}")))?;
    if bytes.len() < HEADER_SIZE || &bytes[..8] != MAGIC { return Err(DatasetError::new("invalid canonical dataset magic/header")); }
    let schema = u16::from_le_bytes(bytes[8..10].try_into().map_err(|_| DatasetError::new("invalid canonical schema bytes"))?);
    if schema != SCHEMA_VERSION { return Err(DatasetError::new("unsupported canonical dataset schema")); }
    let dataset_version = DatasetVersion(u64::from_le_bytes(bytes[10..18].try_into().map_err(|_| DatasetError::new("invalid dataset version"))?));
    let source_manifest_checksum = u64::from_le_bytes(bytes[18..26].try_into().map_err(|_| DatasetError::new("invalid source manifest checksum"))?);
    let payload_len = u64::from_le_bytes(bytes[26..34].try_into().map_err(|_| DatasetError::new("invalid payload length"))?) as usize;
    let expected_checksum = u64::from_le_bytes(bytes[34..42].try_into().map_err(|_| DatasetError::new("invalid payload checksum"))?);
    if bytes.len() != HEADER_SIZE.checked_add(payload_len).ok_or_else(|| DatasetError::new("canonical payload length overflow"))? {
        return Err(DatasetError::new("canonical payload length mismatch"));
    }
    let payload = &bytes[HEADER_SIZE..];
    if checksum(payload) != expected_checksum { return Err(DatasetError::new("canonical dataset checksum mismatch")); }
    let mut dataset = decode_dataset(payload)?;
    dataset.dataset_version = dataset_version;
    dataset.source_manifest_checksum = source_manifest_checksum;
    dataset.validate()?;
    Ok(dataset)
}

fn atomic_create(path: &Path, bytes: &[u8]) -> Result<(), DatasetError> {
    if path.exists() { return Err(DatasetError::new(format!("immutable canonical dataset already exists: {}", path.display()))); }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(io_error)?;
    let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("canonical");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| DatasetError::new("system clock before Unix epoch"))?.as_nanos();
    let temporary: PathBuf = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), nanos));
    let result = (|| -> Result<(), DatasetError> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        fs::rename(&temporary, path).map_err(io_error)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

fn io_error(error: std::io::Error) -> DatasetError { DatasetError::new(format!("canonical persistence I/O error: {error}")) }

fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes { hash ^= u64::from(*byte); hash = hash.wrapping_mul(0x0000_0100_0000_01b3); }
    hash
}

struct Encoder(Vec<u8>);
impl Encoder {
    fn new() -> Self { Self(Vec::new()) }
    fn finish(self) -> Vec<u8> { self.0 }
    fn u8(&mut self, value: u8) { self.0.push(value); }
    fn u16(&mut self, value: u16) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn u32(&mut self, value: u32) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn u64(&mut self, value: u64) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn i32(&mut self, value: i32) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn i64(&mut self, value: i64) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn f32(&mut self, value: f32) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn f64(&mut self, value: f64) { self.0.extend_from_slice(&value.to_le_bytes()); }
    fn string(&mut self, value: &str) -> Result<(), DatasetError> {
        let length = u32::try_from(value.len()).map_err(|_| DatasetError::new("canonical string too long"))?;
        self.u32(length); self.0.extend_from_slice(value.as_bytes()); Ok(())
    }
    fn optional_string(&mut self, value: Option<&str>) -> Result<(), DatasetError> { match value { Some(value) => { self.u8(1); self.string(value)?; }, None => self.u8(0) } Ok(()) }
    fn optional_date(&mut self, value: Option<Date>) { match value { Some(value) => { self.u8(1); self.i32(value.0); }, None => self.u8(0) } }
    fn optional_u16(&mut self, value: Option<u16>) { match value { Some(value) => { self.u8(1); self.u16(value); }, None => self.u8(0) } }
    fn optional_u8(&mut self, value: Option<u8>) { match value { Some(value) => { self.u8(1); self.u8(value); }, None => self.u8(0) } }
    fn optional_money(&mut self, value: Option<footy_core::EuroCents>) { match value { Some(value) => { self.u8(1); self.i64(value.cents()); }, None => self.u8(0) } }
}

struct Decoder<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, offset: 0 } }
    fn take(&mut self, length: usize) -> Result<&'a [u8], DatasetError> {
        let end = self.offset.checked_add(length).ok_or_else(|| DatasetError::new("canonical decode overflow"))?;
        if end > self.bytes.len() { return Err(DatasetError::new("truncated canonical dataset")); }
        let out = &self.bytes[self.offset..end]; self.offset = end; Ok(out)
    }
    fn finish(&self) -> Result<(), DatasetError> { if self.offset == self.bytes.len() { Ok(()) } else { Err(DatasetError::new("canonical dataset has trailing bytes")) } }
    fn u8(&mut self) -> Result<u8, DatasetError> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, DatasetError> { Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(|_| DatasetError::new("u16 decode"))?)) }
    fn u32(&mut self) -> Result<u32, DatasetError> { Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| DatasetError::new("u32 decode"))?)) }
    fn u64(&mut self) -> Result<u64, DatasetError> { Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(|_| DatasetError::new("u64 decode"))?)) }
    fn i32(&mut self) -> Result<i32, DatasetError> { Ok(i32::from_le_bytes(self.take(4)?.try_into().map_err(|_| DatasetError::new("i32 decode"))?)) }
    fn i64(&mut self) -> Result<i64, DatasetError> { Ok(i64::from_le_bytes(self.take(8)?.try_into().map_err(|_| DatasetError::new("i64 decode"))?)) }
    fn f32(&mut self) -> Result<f32, DatasetError> { Ok(f32::from_le_bytes(self.take(4)?.try_into().map_err(|_| DatasetError::new("f32 decode"))?)) }
    fn f64(&mut self) -> Result<f64, DatasetError> { Ok(f64::from_le_bytes(self.take(8)?.try_into().map_err(|_| DatasetError::new("f64 decode"))?)) }
    fn string(&mut self) -> Result<String, DatasetError> { let length = self.u32()? as usize; String::from_utf8(self.take(length)?.to_vec()).map_err(|_| DatasetError::new("canonical string is not UTF-8")) }
    fn optional_string(&mut self) -> Result<Option<String>, DatasetError> { match self.u8()? { 0 => Ok(None), 1 => Ok(Some(self.string()?)), _ => Err(DatasetError::new("invalid optional string flag")) } }
    fn optional_date(&mut self) -> Result<Option<Date>, DatasetError> { match self.u8()? { 0 => Ok(None), 1 => Ok(Some(Date(self.i32()?))), _ => Err(DatasetError::new("invalid optional date flag")) } }
    fn optional_u16(&mut self) -> Result<Option<u16>, DatasetError> { match self.u8()? { 0 => Ok(None), 1 => Ok(Some(self.u16()?)), _ => Err(DatasetError::new("invalid optional u16 flag")) } }
    fn optional_u8(&mut self) -> Result<Option<u8>, DatasetError> { match self.u8()? { 0 => Ok(None), 1 => Ok(Some(self.u8()?)), _ => Err(DatasetError::new("invalid optional u8 flag")) } }
    fn optional_money(&mut self) -> Result<Option<footy_core::EuroCents>, DatasetError> { match self.u8()? { 0 => Ok(None), 1 => Ok(Some(footy_core::EuroCents(self.i64()?))), _ => Err(DatasetError::new("invalid optional money flag")) } }
}

fn encode_dataset(dataset: &CanonicalDataset) -> Result<Vec<u8>, DatasetError> {
    let mut encoder = Encoder::new();
    encode_vec_len(&mut encoder, dataset.provenances.len())?;
    for value in &dataset.provenances { encoder.u64(value.id.0); encoder.u8(source_tag(value.source)); encoder.string(&value.input_path)?; encoder.u64(value.source_checksum); }
    encode_vec_len(&mut encoder, dataset.players.len())?;
    for value in &dataset.players { encoder.u64(value.player.0); encoder.string(&value.fm_external_id)?; encoder.string(&value.name)?; encoder.optional_date(value.birth_date); }
    encode_vec_len(&mut encoder, dataset.identities.len())?;
    for value in &dataset.identities { encoder.u64(value.player.0); encoder.u8(source_tag(value.source)); encoder.string(&value.external_id)?; encoder.f32(value.score.get()); encoder.u8(link_method_tag(value.method)); encoder.u8(u8::from(value.verified)); encoder.u64(value.provenance_id.0); }
    encode_vec_len(&mut encoder, dataset.unresolved_identities.len())?;
    for value in &dataset.unresolved_identities {
        encoder.u8(source_tag(value.source)); encoder.string(&value.external_id)?; encoder.string(&value.name)?; encoder.optional_date(value.birth_date); encoder.u8(unresolved_tag(value.reason)); encoder.u64(value.provenance_id.0); encode_vec_len(&mut encoder, value.candidates.len())?;
        for candidate in &value.candidates { encoder.u64(candidate.player.0); encoder.f32(candidate.score.get()); encoder.u8(link_method_tag(candidate.method)); }
    }
    encode_vec_len(&mut encoder, dataset.issues.len())?;
    for value in &dataset.issues { encoder.u8(source_tag(value.source)); encoder.string(&value.external_id)?; encoder.string(&value.message)?; }
    encode_vec_len(&mut encoder, dataset.fm_snapshots.len())?;
    for value in &dataset.fm_snapshots {
        encoder.u64(value.player.0); encoder.i32(value.snapshot_date.0); encoder.optional_u16(value.current_ability); encoder.optional_u16(value.potential_ability); encoder.optional_u16(value.reputation); encoder.u64(value.provenance_id.0); encoder.u16(value.schema_version);
        encode_attributes(&mut encoder, &value.attributes)?; encode_positions(&mut encoder, &value.positions)?; encode_attributes(&mut encoder, &value.personality_attributes)?; encode_attributes(&mut encoder, &value.hidden_attributes)?;
    }
    encode_vec_len(&mut encoder, dataset.clubs.len())?;
    for value in &dataset.clubs { encoder.u32(value.id.0); encoder.u8(source_tag(value.source)); encoder.string(&value.external_id)?; }
    encode_vec_len(&mut encoder, dataset.competitions.len())?;
    for value in &dataset.competitions { encoder.u32(value.id.0); encoder.u8(source_tag(value.source)); encoder.string(&value.external_id)?; }
    encode_vec_len(&mut encoder, dataset.metric_registry.len())?;
    for value in &dataset.metric_registry { encoder.u16(value.metric.0); encoder.string(&value.name)?; }
    encode_vec_len(&mut encoder, dataset.market_values.len())?;
    for value in &dataset.market_values { encoder.u64(value.player.0); encoder.i32(value.date.0); encoder.i64(value.value.cents()); encoder.u64(value.provenance_id.0); }
    encode_vec_len(&mut encoder, dataset.transfers.len())?;
    for value in &dataset.transfers { encoder.u64(value.id.0); encoder.u64(value.player.0); encoder.i32(value.date.0); encoder.u32(value.from.0); encoder.u32(value.to.0); encoder.u8(market_transfer_tag(value.kind)); encoder.optional_money(value.fee); encoder.optional_money(value.loan_fee); encoder.u64(value.provenance_id.0); }
    encode_vec_len(&mut encoder, dataset.contracts.len())?;
    for value in &dataset.contracts { encoder.u64(value.player.0); encoder.u32(value.club.0); encoder.i32(value.observed_on.0); encoder.optional_date(value.expiry); encoder.optional_u8(value.option_years); encoder.u64(value.provenance_id.0); }
    encode_vec_len(&mut encoder, dataset.performance.len())?;
    for value in &dataset.performance {
        encoder.u64(value.player.0); encoder.u32(value.club.0); encoder.u32(value.competition.0); encoder.string(&value.season_external_id)?; encoder.optional_string(value.match_external_id.as_deref())?; encoder.i32(value.start_date.0); encoder.i32(value.end_date.0); encoder.u32(value.minutes); encoder.u8(observation_tag(value.observation_state)); encoder.u64(value.provenance_id.0); encode_vec_len(&mut encoder, value.metrics.len())?;
        for metric in &value.metrics { encoder.u16(metric.metric.0); encoder.f64(metric.value); }
    }
    encode_vec_len(&mut encoder, dataset.salaries.len())?;
    for value in &dataset.salaries {
        encoder.u64(value.player.0); encoder.u32(value.club.0); encoder.i32(value.effective_date.0); encoder.0.extend_from_slice(&value.amount.currency.bytes()); encoder.i64(value.amount.low_minor_units); encoder.i64(value.amount.high_minor_units); encoder.u8(wage_basis_tag(value.basis)); encoder.u8(periodicity_tag(value.periodicity)); encoder.u8(compensation_tag(value.kind)); encoder.u64(value.provenance_id.0); encoder.f32(value.confidence.get());
    }
    Ok(encoder.finish())
}

fn decode_dataset(payload: &[u8]) -> Result<CanonicalDataset, DatasetError> {
    let mut decoder = Decoder::new(payload);
    let provenances = decode_count(&mut decoder)?.map(|_| Ok(CanonicalProvenance { id: ProvenanceId(decoder.u64()?), source: source_from_tag(decoder.u8()?)?, input_path: decoder.string()?, source_checksum: decoder.u64()? })).collect::<Result<Vec<_>, DatasetError>>()?;
    let players = decode_count(&mut decoder)?.map(|_| Ok(CanonicalPlayer { player: PlayerId(decoder.u64()?), fm_external_id: decoder.string()?, name: decoder.string()?, birth_date: decoder.optional_date()? })).collect::<Result<Vec<_>, DatasetError>>()?;
    let identities = decode_count(&mut decoder)?.map(|_| Ok(CanonicalIdentity { player: PlayerId(decoder.u64()?), source: source_from_tag(decoder.u8()?)?, external_id: decoder.string()?, score: decode_confidence(decoder.f32()?)?, method: link_method_from_tag(decoder.u8()?)?, verified: decode_bool(decoder.u8()?)?, provenance_id: ProvenanceId(decoder.u64()?) })).collect::<Result<Vec<_>, DatasetError>>()?;
    let mut unresolved_identities = Vec::new();
    for _ in decode_count(&mut decoder)? {
        let source = source_from_tag(decoder.u8()?)?; let external_id = decoder.string()?; let name = decoder.string()?; let birth_date = decoder.optional_date()?; let reason = unresolved_from_tag(decoder.u8()?)?; let provenance_id = ProvenanceId(decoder.u64()?); let mut candidates = Vec::new();
        for _ in decode_count(&mut decoder)? { candidates.push(CanonicalUnresolvedCandidate { player: PlayerId(decoder.u64()?), score: decode_confidence(decoder.f32()?)?, method: link_method_from_tag(decoder.u8()?)? }); }
        unresolved_identities.push(CanonicalUnresolvedIdentity { source, external_id, name, birth_date, reason, candidates, provenance_id });
    }
    let issues = decode_count(&mut decoder)?.map(|_| Ok(CanonicalIssue { source: source_from_tag(decoder.u8()?)?, external_id: decoder.string()?, message: decoder.string()? })).collect::<Result<Vec<_>, DatasetError>>()?;
    let mut fm_snapshots = Vec::new();
    for _ in decode_count(&mut decoder)? { fm_snapshots.push(CanonicalFmSnapshot { player: PlayerId(decoder.u64()?), snapshot_date: Date(decoder.i32()?), current_ability: decoder.optional_u16()?, potential_ability: decoder.optional_u16()?, reputation: decoder.optional_u16()?, provenance_id: ProvenanceId(decoder.u64()?), schema_version: decoder.u16()?, attributes: decode_attributes(&mut decoder)?, positions: decode_positions(&mut decoder)?, personality_attributes: decode_attributes(&mut decoder)?, hidden_attributes: decode_attributes(&mut decoder)? }); }
    let clubs = decode_count(&mut decoder)?.map(|_| Ok(SourceClubRecord { id: ClubId(decoder.u32()?), source: source_from_tag(decoder.u8()?)?, external_id: decoder.string()? })).collect::<Result<Vec<_>, DatasetError>>()?;
    let competitions = decode_count(&mut decoder)?.map(|_| Ok(SourceCompetitionRecord { id: CompetitionId(decoder.u32()?), source: source_from_tag(decoder.u8()?)?, external_id: decoder.string()? })).collect::<Result<Vec<_>, DatasetError>>()?;
    let metric_registry = decode_count(&mut decoder)?.map(|_| Ok(MetricDefinition { metric: MetricId(decoder.u16()?), name: decoder.string()? })).collect::<Result<Vec<_>, DatasetError>>()?;
    let market_values = decode_count(&mut decoder)?.map(|_| Ok(CanonicalMarketValue { player: PlayerId(decoder.u64()?), date: Date(decoder.i32()?), value: footy_core::EuroCents(decoder.i64()?), provenance_id: ProvenanceId(decoder.u64()?) })).collect::<Result<Vec<_>, DatasetError>>()?;
    let transfers = decode_count(&mut decoder)?.map(|_| Ok(CanonicalTransfer { id: TransferId(decoder.u64()?), player: PlayerId(decoder.u64()?), date: Date(decoder.i32()?), from: ClubId(decoder.u32()?), to: ClubId(decoder.u32()?), kind: market_transfer_from_tag(decoder.u8()?)?, fee: decoder.optional_money()?, loan_fee: decoder.optional_money()?, provenance_id: ProvenanceId(decoder.u64()?) })).collect::<Result<Vec<_>, DatasetError>>()?;
    let contracts = decode_count(&mut decoder)?.map(|_| Ok(CanonicalContract { player: PlayerId(decoder.u64()?), club: ClubId(decoder.u32()?), observed_on: Date(decoder.i32()?), expiry: decoder.optional_date()?, option_years: decoder.optional_u8()?, provenance_id: ProvenanceId(decoder.u64()?) })).collect::<Result<Vec<_>, DatasetError>>()?;
    let mut performance = Vec::new();
    for _ in decode_count(&mut decoder)? {
        let player = PlayerId(decoder.u64()?); let club = ClubId(decoder.u32()?); let competition = CompetitionId(decoder.u32()?); let season_external_id = decoder.string()?; let match_external_id = decoder.optional_string()?; let start_date = Date(decoder.i32()?); let end_date = Date(decoder.i32()?); let minutes = decoder.u32()?; let observation_state = observation_from_tag(decoder.u8()?)?; let provenance_id = ProvenanceId(decoder.u64()?); let mut metrics = Vec::new();
        for _ in decode_count(&mut decoder)? { metrics.push(MetricValue { metric: MetricId(decoder.u16()?), value: decoder.f64()? }); }
        performance.push(CanonicalPerformanceObservation { player, club, competition, season_external_id, match_external_id, start_date, end_date, minutes, metrics, observation_state, provenance_id });
    }
    let mut salaries = Vec::new();
    for _ in decode_count(&mut decoder)? {
        let player = PlayerId(decoder.u64()?); let club = ClubId(decoder.u32()?); let effective_date = Date(decoder.i32()?); let currency_bytes = decoder.take(3)?; let currency_text = std::str::from_utf8(currency_bytes).map_err(|_| DatasetError::new("invalid persisted currency"))?; let currency = CurrencyCode::new(currency_text).map_err(DatasetError::new)?; let low = decoder.i64()?; let high = decoder.i64()?; let amount = CurrencyRange::new(currency, low, high).map_err(DatasetError::new)?; let basis = wage_basis_from_tag(decoder.u8()?)?; let periodicity = periodicity_from_tag(decoder.u8()?)?; let kind = compensation_from_tag(decoder.u8()?)?; let provenance_id = ProvenanceId(decoder.u64()?); let confidence = decode_confidence(decoder.f32()?)?;
        salaries.push(CanonicalSalaryObservation { player, club, effective_date, amount, basis, periodicity, kind, provenance_id, confidence });
    }
    decoder.finish()?;
    Ok(CanonicalDataset { dataset_version: DatasetVersion(0), source_manifest_checksum: 0, provenances, players, identities, unresolved_identities, issues, fm_snapshots, clubs, competitions, metric_registry, market_values, transfers, contracts, performance, salaries })
}

fn encode_vec_len(encoder: &mut Encoder, length: usize) -> Result<(), DatasetError> { encoder.u64(u64::try_from(length).map_err(|_| DatasetError::new("canonical vector too large"))?); Ok(()) }
fn decode_count<'a>(decoder: &mut Decoder<'a>) -> Result<std::ops::Range<u64>, DatasetError> { Ok(0..decoder.u64()?) }
fn encode_attributes(encoder: &mut Encoder, values: &[FmAttribute]) -> Result<(), DatasetError> { encode_vec_len(encoder, values.len())?; for value in values { encoder.u16(value.id.0); encoder.u8(value.value); } Ok(()) }
fn decode_attributes(decoder: &mut Decoder<'_>) -> Result<Vec<FmAttribute>, DatasetError> { decode_count(decoder)?.map(|_| Ok(FmAttribute { id: footy_core::AttributeId(decoder.u16()?), value: decoder.u8()? })).collect() }
fn encode_positions(encoder: &mut Encoder, values: &[PositionRating]) -> Result<(), DatasetError> { encode_vec_len(encoder, values.len())?; for value in values { encoder.u16(value.position_id); encoder.u8(value.value); } Ok(()) }
fn decode_positions(decoder: &mut Decoder<'_>) -> Result<Vec<PositionRating>, DatasetError> { decode_count(decoder)?.map(|_| Ok(PositionRating { position_id: decoder.u16()?, value: decoder.u8()? })).collect() }
fn decode_confidence(value: f32) -> Result<Confidence, DatasetError> { Confidence::new(value).map_err(|_| DatasetError::new("invalid persisted confidence")) }
fn decode_bool(value: u8) -> Result<bool, DatasetError> { match value { 0 => Ok(false), 1 => Ok(true), _ => Err(DatasetError::new("invalid persisted boolean")) } }
fn link_method_tag(value: LinkMethod) -> u8 { match value { LinkMethod::Explicit => 1, LinkMethod::ExactMetadata => 2, LinkMethod::WeightedMetadata => 3, LinkMethod::Manual => 4 } }
fn link_method_from_tag(value: u8) -> Result<LinkMethod, DatasetError> { match value { 1 => Ok(LinkMethod::Explicit), 2 => Ok(LinkMethod::ExactMetadata), 3 => Ok(LinkMethod::WeightedMetadata), 4 => Ok(LinkMethod::Manual), _ => Err(DatasetError::new("invalid link method")) } }
fn unresolved_tag(value: UnresolvedReason) -> u8 { match value { UnresolvedReason::NoCandidate => 1, UnresolvedReason::BelowThreshold => 2, UnresolvedReason::Ambiguous => 3, UnresolvedReason::MissingMetadata => 4 } }
fn unresolved_from_tag(value: u8) -> Result<UnresolvedReason, DatasetError> { match value { 1 => Ok(UnresolvedReason::NoCandidate), 2 => Ok(UnresolvedReason::BelowThreshold), 3 => Ok(UnresolvedReason::Ambiguous), 4 => Ok(UnresolvedReason::MissingMetadata), _ => Err(DatasetError::new("invalid unresolved reason")) } }
fn observation_tag(value: ObservationState) -> u8 { match value { ObservationState::Measured => 1, ObservationState::NotApplicable => 2, ObservationState::SourceUnavailable => 3, ObservationState::PlayerUnobserved => 4, ObservationState::LowSample => 5 } }
fn observation_from_tag(value: u8) -> Result<ObservationState, DatasetError> { match value { 1 => Ok(ObservationState::Measured), 2 => Ok(ObservationState::NotApplicable), 3 => Ok(ObservationState::SourceUnavailable), 4 => Ok(ObservationState::PlayerUnobserved), 5 => Ok(ObservationState::LowSample), _ => Err(DatasetError::new("invalid observation state")) } }
fn market_transfer_tag(value: footy_market::TransferKind) -> u8 { match value { footy_market::TransferKind::Permanent => 1, footy_market::TransferKind::Free => 2, footy_market::TransferKind::Loan => 3, footy_market::TransferKind::LoanWithOption => 4, footy_market::TransferKind::LoanWithObligation => 5, footy_market::TransferKind::Youth => 6, footy_market::TransferKind::ReturnFromLoan => 7, footy_market::TransferKind::Internal => 8, footy_market::TransferKind::Unknown => 9 } }
fn market_transfer_from_tag(value: u8) -> Result<footy_market::TransferKind, DatasetError> { match value { 1 => Ok(footy_market::TransferKind::Permanent), 2 => Ok(footy_market::TransferKind::Free), 3 => Ok(footy_market::TransferKind::Loan), 4 => Ok(footy_market::TransferKind::LoanWithOption), 5 => Ok(footy_market::TransferKind::LoanWithObligation), 6 => Ok(footy_market::TransferKind::Youth), 7 => Ok(footy_market::TransferKind::ReturnFromLoan), 8 => Ok(footy_market::TransferKind::Internal), 9 => Ok(footy_market::TransferKind::Unknown), _ => Err(DatasetError::new("invalid transfer kind")) } }
fn wage_basis_tag(value: WageBasis) -> u8 { match value { WageBasis::Gross => 1, WageBasis::Net => 2, WageBasis::Unknown => 3 } }
fn wage_basis_from_tag(value: u8) -> Result<WageBasis, DatasetError> { match value { 1 => Ok(WageBasis::Gross), 2 => Ok(WageBasis::Net), 3 => Ok(WageBasis::Unknown), _ => Err(DatasetError::new("invalid wage basis")) } }
fn periodicity_tag(value: Periodicity) -> u8 { match value { Periodicity::Daily => 1, Periodicity::Weekly => 2, Periodicity::Monthly => 3, Periodicity::Annual => 4, Periodicity::OneOff => 5 } }
fn periodicity_from_tag(value: u8) -> Result<Periodicity, DatasetError> { match value { 1 => Ok(Periodicity::Daily), 2 => Ok(Periodicity::Weekly), 3 => Ok(Periodicity::Monthly), 4 => Ok(Periodicity::Annual), 5 => Ok(Periodicity::OneOff), _ => Err(DatasetError::new("invalid periodicity")) } }
fn compensation_tag(value: CompensationKind) -> u8 { match value { CompensationKind::BaseWage => 1, CompensationKind::PerformanceBonus => 2, CompensationKind::SigningFee => 3, CompensationKind::AgentFee => 4, CompensationKind::Other => 5 } }
fn compensation_from_tag(value: u8) -> Result<CompensationKind, DatasetError> { match value { 1 => Ok(CompensationKind::BaseWage), 2 => Ok(CompensationKind::PerformanceBonus), 3 => Ok(CompensationKind::SigningFee), 4 => Ok(CompensationKind::AgentFee), 5 => Ok(CompensationKind::Other), _ => Err(DatasetError::new("invalid compensation kind")) } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn canonical_dataset_round_trips_and_is_immutable() {
        let dataset = CanonicalDataset { dataset_version: DatasetVersion(7), source_manifest_checksum: 99, provenances: Vec::new(), players: vec![CanonicalPlayer { player: PlayerId(1), fm_external_id: "fm1".into(), name: "Test".into(), birth_date: Some(Date(1)) }], identities: Vec::new(), unresolved_identities: Vec::new(), issues: Vec::new(), fm_snapshots: Vec::new(), clubs: Vec::new(), competitions: Vec::new(), metric_registry: Vec::new(), market_values: Vec::new(), transfers: Vec::new(), contracts: Vec::new(), performance: Vec::new(), salaries: Vec::new() };
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("footy-canonical-{}-{suffix}.bin", std::process::id()));
        write_canonical_dataset(&path, &dataset).unwrap();
        let loaded = read_canonical_dataset(&path).unwrap();
        assert_eq!(loaded, dataset);
        assert!(write_canonical_dataset(&path, &dataset).is_err());
        let _ = fs::remove_file(path);
    }
}
