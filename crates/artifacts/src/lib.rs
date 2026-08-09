#![forbid(unsafe_code)]
//! Immutable, checksummed artifact contracts for orchestration boundaries.

use footy_core::{
    CalibrationVersion, Confidence, DatasetVersion, Date, EuroCents, FeatureId,
    FeatureSetVersion, ModelVersion, PlayerId,
};
use footy_market::PriceDistribution;
use footy_models::{ConformalCalibrator, LinearModel, ModelMetadata};
use std::collections::BTreeSet;
use std::convert::TryInto;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"FTYART01";
const HEADER_SIZE: usize = 64;
const SCHEMA_VERSION: u16 = 1;
const NONE_U64: u64 = u64::MAX;
const NONE_I32: i32 = i32::MIN;

#[derive(Debug)]
pub enum ArtifactError {
    Io(io::Error),
    Invalid(&'static str),
    InvalidOwned(String),
    WrongKind { expected: ArtifactKind, actual: ArtifactKind },
    ChecksumMismatch,
    AlreadyExists(PathBuf),
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "artifact I/O error: {error}"),
            Self::Invalid(message) => write!(formatter, "invalid artifact: {message}"),
            Self::InvalidOwned(message) => write!(formatter, "invalid artifact: {message}"),
            Self::WrongKind { expected, actual } => {
                write!(formatter, "wrong artifact kind: expected {expected:?}, got {actual:?}")
            }
            Self::ChecksumMismatch => write!(formatter, "artifact payload checksum mismatch"),
            Self::AlreadyExists(path) => write!(formatter, "immutable artifact already exists: {}", path.display()),
        }
    }
}

impl std::error::Error for ArtifactError {}

impl From<io::Error> for ArtifactError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ArtifactKind {
    DatasetManifest = 1,
    FeatureMatrix = 2,
    LinearPriceModel = 3,
    PredictionBatch = 4,
}

impl ArtifactKind {
    fn from_u16(value: u16) -> Result<Self, ArtifactError> {
        match value {
            1 => Ok(Self::DatasetManifest),
            2 => Ok(Self::FeatureMatrix),
            3 => Ok(Self::LinearPriceModel),
            4 => Ok(Self::PredictionBatch),
            _ => Err(ArtifactError::Invalid("unknown artifact kind")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactVersions {
    pub dataset: DatasetVersion,
    pub feature_set: Option<FeatureSetVersion>,
    pub model: Option<ModelVersion>,
    pub calibration: Option<CalibrationVersion>,
    pub cutoff: Option<Date>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactHeader {
    pub kind: ArtifactKind,
    pub schema_version: u16,
    pub versions: ArtifactVersions,
    pub payload_len: u64,
    pub payload_checksum: u64,
}

pub fn checksum_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn checksum_file(path: impl AsRef<Path>) -> Result<(u64, u64), ArtifactError> {
    let bytes = fs::read(path)?;
    Ok((bytes.len() as u64, checksum_bytes(&bytes)))
}

fn encode_header(header: ArtifactHeader) -> [u8; HEADER_SIZE] {
    let mut bytes = [0_u8; HEADER_SIZE];
    bytes[0..8].copy_from_slice(MAGIC);
    bytes[8..10].copy_from_slice(&(header.kind as u16).to_le_bytes());
    bytes[10..12].copy_from_slice(&header.schema_version.to_le_bytes());
    bytes[12..20].copy_from_slice(&header.versions.dataset.0.to_le_bytes());
    bytes[20..28].copy_from_slice(&header.versions.feature_set.map_or(NONE_U64, |value| value.0).to_le_bytes());
    bytes[28..36].copy_from_slice(&header.versions.model.map_or(NONE_U64, |value| value.0).to_le_bytes());
    bytes[36..44].copy_from_slice(&header.versions.calibration.map_or(NONE_U64, |value| value.0).to_le_bytes());
    bytes[44..48].copy_from_slice(&header.versions.cutoff.map_or(NONE_I32, |value| value.0).to_le_bytes());
    bytes[48..56].copy_from_slice(&header.payload_len.to_le_bytes());
    bytes[56..64].copy_from_slice(&header.payload_checksum.to_le_bytes());
    bytes
}

fn decode_header(bytes: &[u8]) -> Result<ArtifactHeader, ArtifactError> {
    if bytes.len() < HEADER_SIZE || &bytes[0..8] != MAGIC {
        return Err(ArtifactError::Invalid("bad artifact magic or truncated header"));
    }
    let kind = ArtifactKind::from_u16(u16::from_le_bytes(bytes[8..10].try_into().map_err(|_| ArtifactError::Invalid("kind"))?))?;
    let schema_version = u16::from_le_bytes(bytes[10..12].try_into().map_err(|_| ArtifactError::Invalid("schema version"))?);
    if schema_version != SCHEMA_VERSION {
        return Err(ArtifactError::Invalid("unsupported artifact schema version"));
    }
    let dataset = DatasetVersion(u64::from_le_bytes(bytes[12..20].try_into().map_err(|_| ArtifactError::Invalid("dataset version"))?));
    let feature_raw = u64::from_le_bytes(bytes[20..28].try_into().map_err(|_| ArtifactError::Invalid("feature version"))?);
    let model_raw = u64::from_le_bytes(bytes[28..36].try_into().map_err(|_| ArtifactError::Invalid("model version"))?);
    let calibration_raw = u64::from_le_bytes(bytes[36..44].try_into().map_err(|_| ArtifactError::Invalid("calibration version"))?);
    let cutoff_raw = i32::from_le_bytes(bytes[44..48].try_into().map_err(|_| ArtifactError::Invalid("cutoff"))?);
    let payload_len = u64::from_le_bytes(bytes[48..56].try_into().map_err(|_| ArtifactError::Invalid("payload length"))?);
    let payload_checksum = u64::from_le_bytes(bytes[56..64].try_into().map_err(|_| ArtifactError::Invalid("payload checksum"))?);
    Ok(ArtifactHeader {
        kind,
        schema_version,
        versions: ArtifactVersions {
            dataset,
            feature_set: (feature_raw != NONE_U64).then_some(FeatureSetVersion(feature_raw)),
            model: (model_raw != NONE_U64).then_some(ModelVersion(model_raw)),
            calibration: (calibration_raw != NONE_U64).then_some(CalibrationVersion(calibration_raw)),
            cutoff: (cutoff_raw != NONE_I32).then_some(Date(cutoff_raw)),
        },
        payload_len,
        payload_checksum,
    })
}

pub fn read_header(path: impl AsRef<Path>) -> Result<ArtifactHeader, ArtifactError> {
    let mut file = File::open(path)?;
    let mut bytes = [0_u8; HEADER_SIZE];
    file.read_exact(&mut bytes)?;
    decode_header(&bytes)
}

fn write_envelope(
    path: impl AsRef<Path>,
    kind: ArtifactKind,
    versions: ArtifactVersions,
    payload: &[u8],
) -> Result<ArtifactHeader, ArtifactError> {
    let path = path.as_ref();
    if path.exists() {
        return Err(ArtifactError::AlreadyExists(path.to_path_buf()));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("artifact");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ArtifactError::Invalid("system clock before Unix epoch"))?.as_nanos();
    let temp = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), nanos));
    let header = ArtifactHeader {
        kind,
        schema_version: SCHEMA_VERSION,
        versions,
        payload_len: payload.len() as u64,
        payload_checksum: checksum_bytes(payload),
    };
    let result = (|| -> Result<(), ArtifactError> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
        file.write_all(&encode_header(header))?;
        file.write_all(payload)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok(header)
}

fn read_envelope(path: impl AsRef<Path>, expected: ArtifactKind) -> Result<(ArtifactHeader, Vec<u8>), ArtifactError> {
    let bytes = fs::read(path)?;
    if bytes.len() < HEADER_SIZE {
        return Err(ArtifactError::Invalid("truncated artifact"));
    }
    let header = decode_header(&bytes[..HEADER_SIZE])?;
    if header.kind != expected {
        return Err(ArtifactError::WrongKind { expected, actual: header.kind });
    }
    let payload_len: usize = header.payload_len.try_into().map_err(|_| ArtifactError::Invalid("payload too large"))?;
    if bytes.len() != HEADER_SIZE.checked_add(payload_len).ok_or(ArtifactError::Invalid("payload length overflow"))? {
        return Err(ArtifactError::Invalid("payload length mismatch"));
    }
    let payload = bytes[HEADER_SIZE..].to_vec();
    if checksum_bytes(&payload) != header.payload_checksum {
        return Err(ArtifactError::ChecksumMismatch);
    }
    Ok((header, payload))
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
    fn string(&mut self, value: &str) -> Result<(), ArtifactError> {
        let len: u32 = value.len().try_into().map_err(|_| ArtifactError::Invalid("string too long"))?;
        self.u32(len);
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, pos: 0 } }
    fn take(&mut self, count: usize) -> Result<&'a [u8], ArtifactError> {
        let end = self.pos.checked_add(count).ok_or(ArtifactError::Invalid("decode overflow"))?;
        if end > self.bytes.len() { return Err(ArtifactError::Invalid("truncated payload")); }
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, ArtifactError> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, ArtifactError> { Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(|_| ArtifactError::Invalid("u16"))?)) }
    fn u32(&mut self) -> Result<u32, ArtifactError> { Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| ArtifactError::Invalid("u32"))?)) }
    fn u64(&mut self) -> Result<u64, ArtifactError> { Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(|_| ArtifactError::Invalid("u64"))?)) }
    fn i32(&mut self) -> Result<i32, ArtifactError> { Ok(i32::from_le_bytes(self.take(4)?.try_into().map_err(|_| ArtifactError::Invalid("i32"))?)) }
    fn i64(&mut self) -> Result<i64, ArtifactError> { Ok(i64::from_le_bytes(self.take(8)?.try_into().map_err(|_| ArtifactError::Invalid("i64"))?)) }
    fn f32(&mut self) -> Result<f32, ArtifactError> { Ok(f32::from_le_bytes(self.take(4)?.try_into().map_err(|_| ArtifactError::Invalid("f32"))?)) }
    fn f64(&mut self) -> Result<f64, ArtifactError> { Ok(f64::from_le_bytes(self.take(8)?.try_into().map_err(|_| ArtifactError::Invalid("f64"))?)) }
    fn string(&mut self) -> Result<String, ArtifactError> {
        let len: usize = self.u32()?.try_into().map_err(|_| ArtifactError::Invalid("string length"))?;
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| ArtifactError::Invalid("invalid UTF-8 string"))
    }
    fn finish(self) -> Result<(), ArtifactError> {
        if self.pos == self.bytes.len() { Ok(()) } else { Err(ArtifactError::Invalid("trailing payload bytes")) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    FootballManager,
    Transfermarkt,
    Fbref,
    Salary,
    Manual,
}

impl SourceKind {
    fn encode(self) -> u8 {
        match self {
            Self::FootballManager => 1,
            Self::Transfermarkt => 2,
            Self::Fbref => 3,
            Self::Salary => 4,
            Self::Manual => 5,
        }
    }
    fn decode(value: u8) -> Result<Self, ArtifactError> {
        match value {
            1 => Ok(Self::FootballManager),
            2 => Ok(Self::Transfermarkt),
            3 => Ok(Self::Fbref),
            4 => Ok(Self::Salary),
            5 => Ok(Self::Manual),
            _ => Err(ArtifactError::Invalid("unknown source kind")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManifest {
    pub source: SourceKind,
    pub input_path: String,
    pub schema_version: u16,
    pub byte_len: u64,
    pub checksum: u64,
    pub record_count: u64,
    pub issue_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetManifest {
    pub dataset_version: DatasetVersion,
    pub complete: bool,
    pub sources: Vec<SourceManifest>,
}

impl DatasetManifest {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.sources.is_empty() { return Err(ArtifactError::Invalid("dataset manifest has no sources")); }
        if self.complete && self.sources.iter().any(|source| source.issue_count > 0) {
            return Err(ArtifactError::Invalid("complete dataset cannot contain source issues"));
        }
        let mut identities = BTreeSet::new();
        for source in &self.sources {
            if source.input_path.is_empty() || source.byte_len == 0 {
                return Err(ArtifactError::Invalid("source manifest has empty path or file"));
            }
            if !identities.insert((source.source.encode(), source.input_path.as_str())) {
                return Err(ArtifactError::Invalid("duplicate source manifest entry"));
            }
        }
        Ok(())
    }
}

pub fn write_dataset_manifest(path: impl AsRef<Path>, manifest: &DatasetManifest) -> Result<ArtifactHeader, ArtifactError> {
    manifest.validate()?;
    let mut encoder = Encoder::new();
    encoder.u8(u8::from(manifest.complete));
    encoder.u32(manifest.sources.len().try_into().map_err(|_| ArtifactError::Invalid("too many sources"))?);
    for source in &manifest.sources {
        encoder.u8(source.source.encode());
        encoder.string(&source.input_path)?;
        encoder.u16(source.schema_version);
        encoder.u64(source.byte_len);
        encoder.u64(source.checksum);
        encoder.u64(source.record_count);
        encoder.u64(source.issue_count);
    }
    write_envelope(path, ArtifactKind::DatasetManifest, ArtifactVersions {
        dataset: manifest.dataset_version,
        feature_set: None,
        model: None,
        calibration: None,
        cutoff: None,
    }, &encoder.finish())
}

pub fn read_dataset_manifest(path: impl AsRef<Path>) -> Result<DatasetManifest, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::DatasetManifest)?;
    let mut decoder = Decoder::new(&payload);
    let complete = match decoder.u8()? { 0 => false, 1 => true, _ => return Err(ArtifactError::Invalid("invalid complete flag")) };
    let count = decoder.u32()? as usize;
    let mut sources = Vec::with_capacity(count);
    for _ in 0..count {
        sources.push(SourceManifest {
            source: SourceKind::decode(decoder.u8()?)?,
            input_path: decoder.string()?,
            schema_version: decoder.u16()?,
            byte_len: decoder.u64()?,
            checksum: decoder.u64()?,
            record_count: decoder.u64()?,
            issue_count: decoder.u64()?,
        });
    }
    decoder.finish()?;
    let manifest = DatasetManifest { dataset_version: header.versions.dataset, complete, sources };
    manifest.validate()?;
    Ok(manifest)
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureRow {
    pub player: PlayerId,
    pub as_of: Date,
    pub features: Vec<f32>,
    pub target_log_price: Option<f64>,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureMatrixArtifact {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub feature_set_version: FeatureSetVersion,
    pub feature_ids: Vec<FeatureId>,
    pub rows: Vec<FeatureRow>,
}

impl FeatureMatrixArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.feature_ids.is_empty() { return Err(ArtifactError::Invalid("feature matrix has no features")); }
        let mut feature_ids = BTreeSet::new();
        if self.feature_ids.iter().any(|id| !feature_ids.insert(id.0)) {
            return Err(ArtifactError::Invalid("duplicate feature id"));
        }
        let mut row_keys = BTreeSet::new();
        for row in &self.rows {
            if row.features.len() != self.feature_ids.len() { return Err(ArtifactError::Invalid("feature row width mismatch")); }
            if row.features.iter().any(|value| !value.is_finite()) { return Err(ArtifactError::Invalid("non-finite feature")); }
            if row.target_log_price.is_some_and(|value| !value.is_finite()) { return Err(ArtifactError::Invalid("non-finite target")); }
            if !row.weight.is_finite() || row.weight <= 0.0 { return Err(ArtifactError::Invalid("invalid row weight")); }
            if !row_keys.insert((row.player, row.as_of)) { return Err(ArtifactError::Invalid("duplicate player/date feature row")); }
        }
        Ok(())
    }
}

pub fn write_feature_matrix(path: impl AsRef<Path>, artifact: &FeatureMatrixArtifact) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u32(artifact.feature_ids.len().try_into().map_err(|_| ArtifactError::Invalid("too many features"))?);
    for id in &artifact.feature_ids { encoder.u32(id.0); }
    encoder.u64(artifact.rows.len().try_into().map_err(|_| ArtifactError::Invalid("too many rows"))?);
    for row in &artifact.rows {
        encoder.u64(row.player.0);
        encoder.i32(row.as_of.0);
        encoder.f64(row.weight);
        match row.target_log_price {
            Some(target) => { encoder.u8(1); encoder.f64(target); }
            None => encoder.u8(0),
        }
        for value in &row.features { encoder.f32(*value); }
    }
    write_envelope(path, ArtifactKind::FeatureMatrix, ArtifactVersions {
        dataset: artifact.dataset_version,
        feature_set: Some(artifact.feature_set_version),
        model: None,
        calibration: None,
        cutoff: None,
    }, &encoder.finish())
}

pub fn read_feature_matrix(path: impl AsRef<Path>) -> Result<FeatureMatrixArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::FeatureMatrix)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("feature matrix missing feature version"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let feature_count = decoder.u32()? as usize;
    if feature_count == 0 { return Err(ArtifactError::Invalid("feature matrix has zero width")); }
    let mut feature_ids = Vec::with_capacity(feature_count);
    for _ in 0..feature_count { feature_ids.push(FeatureId(decoder.u32()?)); }
    let row_count: usize = decoder.u64()?.try_into().map_err(|_| ArtifactError::Invalid("too many rows"))?;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let player = PlayerId(decoder.u64()?);
        let as_of = Date(decoder.i32()?);
        let weight = decoder.f64()?;
        let target_log_price = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid target flag")),
        };
        let mut features = Vec::with_capacity(feature_count);
        for _ in 0..feature_count { features.push(decoder.f32()?); }
        rows.push(FeatureRow { player, as_of, features, target_log_price, weight });
    }
    decoder.finish()?;
    let artifact = FeatureMatrixArtifact {
        dataset_version: header.versions.dataset,
        source_manifest_checksum,
        feature_set_version,
        feature_ids,
        rows,
    };
    artifact.validate()?;
    Ok(artifact)
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearPriceModelArtifact {
    pub metadata: ModelMetadata,
    pub source_manifest_checksum: u64,
    pub calibration_version: CalibrationVersion,
    pub calibration_cutoff: Date,
    pub ridge: f64,
    pub feature_ids: Vec<FeatureId>,
    pub model: LinearModel,
    pub p50_calibrator: ConformalCalibrator,
    pub p80_calibrator: ConformalCalibrator,
}

impl LinearPriceModelArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.calibration_cutoff <= self.metadata.trained_through { return Err(ArtifactError::Invalid("calibration cutoff must follow training cutoff")); }
        if !self.ridge.is_finite() || self.ridge < 0.0 { return Err(ArtifactError::Invalid("invalid ridge parameter")); }
        if self.feature_ids.is_empty() || self.model.coefficients.len() != self.feature_ids.len() + 1 {
            return Err(ArtifactError::Invalid("model coefficient/feature mismatch"));
        }
        if self.model.coefficients.iter().any(|value| !value.is_finite()) { return Err(ArtifactError::Invalid("non-finite model coefficient")); }
        validate_calibrator(self.p50_calibrator)?;
        validate_calibrator(self.p80_calibrator)?;
        if self.p50_calibrator.coverage >= self.p80_calibrator.coverage || self.p50_calibrator.radius > self.p80_calibrator.radius {
            return Err(ArtifactError::Invalid("calibration intervals are not nested"));
        }
        Ok(())
    }

    pub fn predict_distribution(&self, features: &[f32]) -> Result<PriceDistribution, ArtifactError> {
        self.validate()?;
        if features.len() != self.feature_ids.len() || features.iter().any(|value| !value.is_finite()) {
            return Err(ArtifactError::Invalid("scoring feature mismatch"));
        }
        let features64: Vec<f64> = features.iter().map(|value| f64::from(*value)).collect();
        let center = self.model.predict(&features64).map_err(|_| ArtifactError::Invalid("model scoring failed"))?;
        let p10 = euro_from_log(center - self.p80_calibrator.radius)?;
        let p25 = euro_from_log(center - self.p50_calibrator.radius)?;
        let p50 = euro_from_log(center)?;
        let p75 = euro_from_log(center + self.p50_calibrator.radius)?;
        let p90 = euro_from_log(center + self.p80_calibrator.radius)?;
        let samples = self.p50_calibrator.calibration_size.min(self.p80_calibrator.calibration_size) as f32;
        let confidence = Confidence::clamped(samples / (samples + 30.0));
        PriceDistribution::new(p10, p25, p50, p75, p90, confidence).ok_or(ArtifactError::Invalid("invalid scored price distribution"))
    }
}

fn validate_calibrator(calibrator: ConformalCalibrator) -> Result<(), ArtifactError> {
    if !calibrator.radius.is_finite() || calibrator.radius < 0.0 || !calibrator.coverage.is_finite() || calibrator.coverage <= 0.0 || calibrator.coverage >= 1.0 || calibrator.calibration_size == 0 {
        Err(ArtifactError::Invalid("invalid conformal calibrator"))
    } else {
        Ok(())
    }
}

fn euro_from_log(value: f64) -> Result<EuroCents, ArtifactError> {
    let cents = value.exp();
    if !cents.is_finite() || cents < 0.0 || cents > i64::MAX as f64 {
        Err(ArtifactError::Invalid("predicted price out of range"))
    } else {
        Ok(EuroCents(cents.round() as i64))
    }
}

fn encode_calibrator(encoder: &mut Encoder, calibrator: ConformalCalibrator) {
    encoder.f64(calibrator.radius);
    encoder.f64(calibrator.coverage);
    encoder.u32(calibrator.calibration_size);
}

fn decode_calibrator(decoder: &mut Decoder<'_>) -> Result<ConformalCalibrator, ArtifactError> {
    Ok(ConformalCalibrator { radius: decoder.f64()?, coverage: decoder.f64()?, calibration_size: decoder.u32()? })
}

pub fn write_linear_price_model(path: impl AsRef<Path>, artifact: &LinearPriceModelArtifact) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u64(artifact.metadata.seed);
    encoder.i32(artifact.calibration_cutoff.0);
    encoder.f64(artifact.ridge);
    encoder.u32(artifact.feature_ids.len().try_into().map_err(|_| ArtifactError::Invalid("too many model features"))?);
    for id in &artifact.feature_ids { encoder.u32(id.0); }
    encoder.u32(artifact.model.coefficients.len().try_into().map_err(|_| ArtifactError::Invalid("too many coefficients"))?);
    for coefficient in &artifact.model.coefficients { encoder.f64(*coefficient); }
    encode_calibrator(&mut encoder, artifact.p50_calibrator);
    encode_calibrator(&mut encoder, artifact.p80_calibrator);
    write_envelope(path, ArtifactKind::LinearPriceModel, ArtifactVersions {
        dataset: artifact.metadata.dataset_version,
        feature_set: Some(artifact.metadata.feature_set_version),
        model: Some(artifact.metadata.model_version),
        calibration: Some(artifact.calibration_version),
        cutoff: Some(artifact.metadata.trained_through),
    }, &encoder.finish())
}

pub fn read_linear_price_model(path: impl AsRef<Path>) -> Result<LinearPriceModelArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::LinearPriceModel)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("model missing feature version"))?;
    let model_version = header.versions.model.ok_or(ArtifactError::Invalid("model missing model version"))?;
    let calibration_version = header.versions.calibration.ok_or(ArtifactError::Invalid("model missing calibration version"))?;
    let trained_through = header.versions.cutoff.ok_or(ArtifactError::Invalid("model missing training cutoff"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let seed = decoder.u64()?;
    let calibration_cutoff = Date(decoder.i32()?);
    let ridge = decoder.f64()?;
    let feature_count = decoder.u32()? as usize;
    let mut feature_ids = Vec::with_capacity(feature_count);
    for _ in 0..feature_count { feature_ids.push(FeatureId(decoder.u32()?)); }
    let coefficient_count = decoder.u32()? as usize;
    let mut coefficients = Vec::with_capacity(coefficient_count);
    for _ in 0..coefficient_count { coefficients.push(decoder.f64()?); }
    let p50_calibrator = decode_calibrator(&mut decoder)?;
    let p80_calibrator = decode_calibrator(&mut decoder)?;
    decoder.finish()?;
    let artifact = LinearPriceModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version,
            dataset_version: header.versions.dataset,
            feature_set_version,
            seed,
        },
        source_manifest_checksum,
        calibration_version,
        calibration_cutoff,
        ridge,
        feature_ids,
        model: LinearModel { coefficients },
        p50_calibrator,
        p80_calibrator,
    };
    artifact.validate()?;
    Ok(artifact)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FundamentalPredictionRow {
    pub player: PlayerId,
    pub as_of: Date,
    pub value: PriceDistribution,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PredictionBatchArtifact {
    pub scoring_dataset: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub training_dataset: DatasetVersion,
    pub feature_set_version: FeatureSetVersion,
    pub model_version: ModelVersion,
    pub calibration_version: CalibrationVersion,
    pub trained_through: Date,
    pub calibration_cutoff: Date,
    pub rows: Vec<FundamentalPredictionRow>,
}

impl PredictionBatchArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.calibration_cutoff <= self.trained_through { return Err(ArtifactError::Invalid("prediction lineage has invalid cutoffs")); }
        let mut keys = BTreeSet::new();
        for row in &self.rows {
            if row.as_of <= self.calibration_cutoff { return Err(ArtifactError::Invalid("prediction predates model knowledge cutoff")); }
            if !keys.insert((row.player, row.as_of)) { return Err(ArtifactError::Invalid("duplicate prediction row")); }
        }
        Ok(())
    }
}

pub fn write_prediction_batch(path: impl AsRef<Path>, artifact: &PredictionBatchArtifact) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u64(artifact.training_dataset.0);
    encoder.i32(artifact.trained_through.0);
    encoder.i32(artifact.calibration_cutoff.0);
    encoder.u64(artifact.rows.len().try_into().map_err(|_| ArtifactError::Invalid("too many predictions"))?);
    for row in &artifact.rows {
        encoder.u64(row.player.0);
        encoder.i32(row.as_of.0);
        encoder.i64(row.value.p10.cents());
        encoder.i64(row.value.p25.cents());
        encoder.i64(row.value.p50.cents());
        encoder.i64(row.value.p75.cents());
        encoder.i64(row.value.p90.cents());
        encoder.f32(row.value.confidence.get());
    }
    write_envelope(path, ArtifactKind::PredictionBatch, ArtifactVersions {
        dataset: artifact.scoring_dataset,
        feature_set: Some(artifact.feature_set_version),
        model: Some(artifact.model_version),
        calibration: Some(artifact.calibration_version),
        cutoff: Some(artifact.calibration_cutoff),
    }, &encoder.finish())
}

pub fn read_prediction_batch(path: impl AsRef<Path>) -> Result<PredictionBatchArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::PredictionBatch)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("predictions missing feature version"))?;
    let model_version = header.versions.model.ok_or(ArtifactError::Invalid("predictions missing model version"))?;
    let calibration_version = header.versions.calibration.ok_or(ArtifactError::Invalid("predictions missing calibration version"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let training_dataset = DatasetVersion(decoder.u64()?);
    let trained_through = Date(decoder.i32()?);
    let calibration_cutoff = Date(decoder.i32()?);
    let row_count: usize = decoder.u64()?.try_into().map_err(|_| ArtifactError::Invalid("too many predictions"))?;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let player = PlayerId(decoder.u64()?);
        let as_of = Date(decoder.i32()?);
        let p10 = EuroCents(decoder.i64()?);
        let p25 = EuroCents(decoder.i64()?);
        let p50 = EuroCents(decoder.i64()?);
        let p75 = EuroCents(decoder.i64()?);
        let p90 = EuroCents(decoder.i64()?);
        let confidence = Confidence::new(decoder.f32()?).map_err(|_| ArtifactError::Invalid("prediction confidence out of range"))?;
        let value = PriceDistribution::new(p10, p25, p50, p75, p90, confidence).ok_or(ArtifactError::Invalid("invalid prediction distribution"))?;
        rows.push(FundamentalPredictionRow { player, as_of, value });
    }
    decoder.finish()?;
    let artifact = PredictionBatchArtifact {
        scoring_dataset: header.versions.dataset,
        source_manifest_checksum,
        training_dataset,
        feature_set_version,
        model_version,
        calibration_version,
        trained_through,
        calibration_cutoff,
        rows,
    };
    artifact.validate()?;
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("footy-artifacts-{name}-{}-{nanos}.bin", std::process::id()))
    }

    #[test]
    fn dataset_manifest_round_trips_with_checksum() {
        let path = temp_path("manifest");
        let manifest = DatasetManifest {
            dataset_version: DatasetVersion(7),
            complete: true,
            sources: vec![SourceManifest {
                source: SourceKind::FootballManager,
                input_path: "fm.tsv".into(),
                schema_version: 2,
                byte_len: 10,
                checksum: 99,
                record_count: 3,
                issue_count: 0,
            }],
        };
        write_dataset_manifest(&path, &manifest).unwrap();
        assert_eq!(read_dataset_manifest(&path).unwrap(), manifest);
        assert!(matches!(write_dataset_manifest(&path, &manifest), Err(ArtifactError::AlreadyExists(_))));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn feature_matrix_rejects_duplicate_player_dates() {
        let row = FeatureRow { player: PlayerId(1), as_of: Date(2), features: vec![1.0], target_log_price: Some(3.0), weight: 1.0 };
        let artifact = FeatureMatrixArtifact {
            dataset_version: DatasetVersion(1),
            source_manifest_checksum: 3,
            feature_set_version: FeatureSetVersion(1),
            feature_ids: vec![FeatureId(4)],
            rows: vec![row.clone(), row],
        };
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn linear_model_round_trips_and_scores_nested_intervals() {
        let path = temp_path("model");
        let artifact = LinearPriceModelArtifact {
            metadata: ModelMetadata {
                trained_through: Date(10),
                model_version: ModelVersion(2),
                dataset_version: DatasetVersion(3),
                feature_set_version: FeatureSetVersion(4),
                seed: 5,
            },
            source_manifest_checksum: 6,
            calibration_version: CalibrationVersion(7),
            calibration_cutoff: Date(20),
            ridge: 1e-8,
            feature_ids: vec![FeatureId(1)],
            model: LinearModel { coefficients: vec![(1_000_000_f64).ln(), 0.1] },
            p50_calibrator: ConformalCalibrator { radius: 0.2, coverage: 0.5, calibration_size: 20 },
            p80_calibrator: ConformalCalibrator { radius: 0.5, coverage: 0.8, calibration_size: 20 },
        };
        write_linear_price_model(&path, &artifact).unwrap();
        let decoded = read_linear_price_model(&path).unwrap();
        let distribution = decoded.predict_distribution(&[1.0]).unwrap();
        assert!(distribution.p10 < distribution.p25 && distribution.p25 < distribution.p50);
        assert!(distribution.p50 < distribution.p75 && distribution.p75 < distribution.p90);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn prediction_batch_rejects_rows_before_calibration_cutoff() {
        let value = PriceDistribution::point(EuroCents(10), Confidence::ONE).unwrap();
        let artifact = PredictionBatchArtifact {
            scoring_dataset: DatasetVersion(2),
            source_manifest_checksum: 3,
            training_dataset: DatasetVersion(1),
            feature_set_version: FeatureSetVersion(1),
            model_version: ModelVersion(1),
            calibration_version: CalibrationVersion(1),
            trained_through: Date(10),
            calibration_cutoff: Date(20),
            rows: vec![FundamentalPredictionRow { player: PlayerId(1), as_of: Date(20), value }],
        };
        assert!(artifact.validate().is_err());
    }
}
