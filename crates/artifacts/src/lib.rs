#![forbid(unsafe_code)]
//! Immutable, checksummed artifact contracts for orchestration boundaries.

use footy_core::{
    CalibrationVersion, Confidence, DatasetVersion, Date, EuroCents, FeatureId,
    FeatureSetVersion, ModelVersion, PlayerId,
};
use footy_market::PriceDistribution;
use footy_models::{ConformalCalibrator, FeatureTransform, LinearModel, ModelMetadata};
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
    SurgeFeatureMatrix = 5,
    SurgeModel = 6,
    ForecastFeatureMatrix = 7,
    ForecastModel = 8,
    TailForecastModel = 9,
}

impl ArtifactKind {
    fn from_u16(value: u16) -> Result<Self, ArtifactError> {
        match value {
            1 => Ok(Self::DatasetManifest),
            2 => Ok(Self::FeatureMatrix),
            3 => Ok(Self::LinearPriceModel),
            4 => Ok(Self::PredictionBatch),
            5 => Ok(Self::SurgeFeatureMatrix),
            6 => Ok(Self::SurgeModel),
            7 => Ok(Self::ForecastFeatureMatrix),
            8 => Ok(Self::ForecastModel),
            9 => Ok(Self::TailForecastModel),
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
    fn surge_feature_matrix_round_trip() {
        let path = temp_path("surge-matrix");
        let artifact = SurgeFeatureMatrixArtifact {
            dataset_version: DatasetVersion(7),
            source_manifest_checksum: 42,
            feature_set_version: FeatureSetVersion(3),
            feature_names: vec!["a".into(), "b".into()],
            rows: vec![
                SurgeFeatureRow { player: PlayerId(1), as_of: Date(19000), mv_t: 500_000.0, target: Some(1.0), realized_return: Some(0.7), weight: 1.0, features: vec![1.0, f32::NAN] },
                SurgeFeatureRow { player: PlayerId(2), as_of: Date(19001), mv_t: 600_000.0, target: None, realized_return: None, weight: 1.0, features: vec![f32::NAN, 2.0] },
            ],
        };
        write_surge_feature_matrix(&path, &artifact).unwrap();
        let loaded = read_surge_feature_matrix(&path).unwrap();
        assert_eq!(loaded.dataset_version, artifact.dataset_version);
        assert_eq!(loaded.feature_names, artifact.feature_names);
        assert_eq!(loaded.rows.len(), artifact.rows.len());
        for (left, right) in loaded.rows.iter().zip(&artifact.rows) {
            assert_eq!(left.player, right.player);
            assert_eq!(left.as_of, right.as_of);
            assert_eq!(left.mv_t, right.mv_t);
            assert_eq!(left.target, right.target);
            assert_eq!(left.realized_return, right.realized_return);
            for (left_value, right_value) in left.features.iter().zip(&right.features) {
                let same = (left_value.is_nan() && right_value.is_nan()) || left_value == right_value;
                assert!(same, "feature mismatch: {left_value} vs {right_value}");
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn surge_model_round_trip_and_scoring() {
        let path = temp_path("surge-model");
        let transform = FeatureTransform {
            means: vec![0.5, -1.0],
            stds: vec![1.0, 2.0],
            missing_rates: vec![0.1, 0.2],
        };
        let artifact = SurgeModelArtifact {
            metadata: ModelMetadata {
                trained_through: Date(20000),
                model_version: ModelVersion(1),
                dataset_version: DatasetVersion(7),
                feature_set_version: FeatureSetVersion(3),
                seed: 42,
            },
            source_manifest_checksum: 42,
            calibration_version: CalibrationVersion(1),
            ridge: 1.0,
            feature_names: vec!["a".into(), "b".into()],
            transform,
            coefficients: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            walkforward: SurgeWalkForwardMetrics {
                periods: vec![SurgePeriodMetrics { cutoff: Date(19000), train_rows: 100, test_rows: 50, auc: 0.75 }],
                pooled_auc: 0.74,
                auc_without_fm: 0.72,
                ece: 0.02,
                deciles: vec![SurgeDecileSnapshot { decile: 0, count: 5, mean_score: 0.1, realized_rate: 0.2, mean_return: 0.3 }],
                conformal_radius: 0.2,
                calibration_coverage: 0.8,
                calibration_size: 500,
            },
        };
        write_surge_model(&path, &artifact).unwrap();
        let loaded = read_surge_model(&path).unwrap();
        assert_eq!(loaded, artifact);
        let (p, lower, upper) = loaded.predict_surge(&[1.0, f64::NAN]).unwrap();
        assert!(p.is_finite() && lower >= 0.0 && upper <= 1.0 && lower <= upper);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn forecast_feature_matrix_round_trip() {
        let path = temp_path("forecast-matrix");
        let artifact = ForecastFeatureMatrixArtifact {
            dataset_version: DatasetVersion(7),
            source_manifest_checksum: 42,
            feature_set_version: FeatureSetVersion(3),
            feature_names: vec!["a".into(), "b".into()],
            rows: vec![
                ForecastFeatureRow { player: PlayerId(1), as_of: Date(19000), mv_t: 500_000.0, target: Some(0.7), realized_return: Some(1.0), weight: 1.0, features: vec![1.0, f32::NAN] },
                ForecastFeatureRow { player: PlayerId(2), as_of: Date(19001), mv_t: 600_000.0, target: None, realized_return: None, weight: 1.0, features: vec![f32::NAN, 2.0] },
            ],
        };
        write_forecast_feature_matrix(&path, &artifact).unwrap();
        let loaded = read_forecast_feature_matrix(&path).unwrap();
        assert_eq!(loaded.dataset_version, artifact.dataset_version);
        assert_eq!(loaded.feature_names, artifact.feature_names);
        assert_eq!(loaded.rows.len(), artifact.rows.len());
        for (left, right) in loaded.rows.iter().zip(&artifact.rows) {
            assert_eq!(left.player, right.player);
            assert_eq!(left.as_of, right.as_of);
            assert_eq!(left.mv_t, right.mv_t);
            assert_eq!(left.target, right.target);
            assert_eq!(left.realized_return, right.realized_return);
            for (left_value, right_value) in left.features.iter().zip(&right.features) {
                let same = (left_value.is_nan() && right_value.is_nan()) || left_value == right_value;
                assert!(same, "feature mismatch: {left_value} vs {right_value}");
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn forecast_model_round_trip_and_scoring() {
        let path = temp_path("forecast-model");
        let transform = FeatureTransform {
            means: vec![0.5, -1.0],
            stds: vec![1.0, 2.0],
            missing_rates: vec![0.1, 0.2],
        };
        let artifact = ForecastModelArtifact {
            metadata: ModelMetadata {
                trained_through: Date(20000),
                model_version: ModelVersion(1),
                dataset_version: DatasetVersion(7),
                feature_set_version: FeatureSetVersion(3),
                seed: 42,
            },
            source_manifest_checksum: 42,
            calibration_version: CalibrationVersion(1),
            ridge: 1.0,
            feature_names: vec!["a".into(), "b".into()],
            transform,
            coefficients: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            walkforward: ForecastWalkForwardMetrics {
                periods: vec![ForecastPeriodMetrics { cutoff: Date(19000), train_rows: 100, test_rows: 50, rmse: 0.5, mae: 0.4, spearman_ic: Some(0.25) }],
                pooled_rmse: 0.5,
                pooled_mae: 0.4,
                pooled_spearman_ic: Some(0.25),
                deciles: vec![ForecastDecileSnapshot { decile: 0, count: 5, mean_predicted: 0.1, mean_realized: 0.2, mean_return: 0.3 }],
                conformal_radius: 0.5,
                calibration_coverage: 0.8,
                calibration_size: 500,
            },
        };
        write_forecast_model(&path, &artifact).unwrap();
        let loaded = read_forecast_model(&path).unwrap();
        assert_eq!(loaded, artifact);
        let (prediction, lower, upper) = loaded.predict_log_ratio(&[1.0, f64::NAN]).unwrap();
        assert!(prediction.is_finite() && lower <= prediction && prediction <= upper);
        let _ = std::fs::remove_file(&path);
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

// ---------------------------------------------------------------------------
// Value-surge (future-gem) artifacts
// ---------------------------------------------------------------------------

/// One raw (untransformed) feature row. `features` deliberately permits `NaN`
/// entries: missingness is a first-class signal encoded by the model transform.
#[derive(Debug, Clone, PartialEq)]
pub struct SurgeFeatureRow {
    pub player: PlayerId,
    pub as_of: Date,
    pub mv_t: f64,
    pub target: Option<f64>,
    pub realized_return: Option<f64>,
    pub weight: f64,
    pub features: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurgeFeatureMatrixArtifact {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub feature_set_version: FeatureSetVersion,
    pub feature_names: Vec<String>,
    pub rows: Vec<SurgeFeatureRow>,
}

impl SurgeFeatureMatrixArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.feature_names.is_empty() {
            return Err(ArtifactError::Invalid("surge matrix has no features"));
        }
        if self.feature_names.iter().any(|name| name.is_empty()) {
            return Err(ArtifactError::Invalid("empty surge feature name"));
        }
        let mut row_keys = BTreeSet::new();
        for row in &self.rows {
            if row.features.len() != self.feature_names.len() {
                return Err(ArtifactError::Invalid("surge feature row width mismatch"));
            }
            if row.features.iter().any(|value| !value.is_finite() && !value.is_nan()) {
                return Err(ArtifactError::Invalid("infinite surge feature"));
            }
            if !row.mv_t.is_finite() || row.mv_t <= 0.0 {
                return Err(ArtifactError::Invalid("invalid surge reference value"));
            }
            if row.target.is_some_and(|value| !value.is_finite() || (value != 0.0 && value != 1.0)) {
                return Err(ArtifactError::Invalid("surge target must be 0 or 1"));
            }
            if row.realized_return.is_some_and(|value| !value.is_finite()) {
                return Err(ArtifactError::Invalid("non-finite realized return"));
            }
            if !row.weight.is_finite() || row.weight <= 0.0 {
                return Err(ArtifactError::Invalid("invalid surge row weight"));
            }
            if !row_keys.insert((row.player, row.as_of)) {
                return Err(ArtifactError::Invalid("duplicate surge player/date row"));
            }
        }
        Ok(())
    }
}

/// Persisted walk-forward evaluation summary for the surge model.
#[derive(Debug, Clone, PartialEq)]
pub struct SurgePeriodMetrics {
    pub cutoff: Date,
    pub train_rows: u32,
    pub test_rows: u32,
    pub auc: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurgeDecileSnapshot {
    pub decile: u8,
    pub count: u32,
    pub mean_score: f64,
    pub realized_rate: f64,
    pub mean_return: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurgeWalkForwardMetrics {
    pub periods: Vec<SurgePeriodMetrics>,
    pub pooled_auc: f64,
    pub auc_without_fm: f64,
    pub ece: f64,
    pub deciles: Vec<SurgeDecileSnapshot>,
    pub conformal_radius: f64,
    pub calibration_coverage: f64,
    pub calibration_size: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurgeModelArtifact {
    pub metadata: ModelMetadata,
    pub source_manifest_checksum: u64,
    pub calibration_version: CalibrationVersion,
    pub ridge: f64,
    pub feature_names: Vec<String>,
    pub transform: FeatureTransform,
    /// Intercept first, followed by `2 * feature_names.len()` coefficients
    /// (standardized value slot, then missingness-indicator slot, per feature).
    pub coefficients: Vec<f64>,
    pub walkforward: SurgeWalkForwardMetrics,
}

impl SurgeModelArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if !self.ridge.is_finite() || self.ridge < 0.0 {
            return Err(ArtifactError::Invalid("invalid surge ridge parameter"));
        }
        if self.feature_names.is_empty() {
            return Err(ArtifactError::Invalid("surge model has no features"));
        }
        if self.transform.width() != self.feature_names.len() {
            return Err(ArtifactError::Invalid("surge transform/feature mismatch"));
        }
        let expected_coefficients = self.feature_names.len() * 2 + 1;
        if self.coefficients.len() != expected_coefficients {
            return Err(ArtifactError::Invalid("surge model coefficient/feature mismatch"));
        }
        if self.coefficients.iter().any(|value| !value.is_finite()) {
            return Err(ArtifactError::Invalid("non-finite surge model coefficient"));
        }
        if !self.walkforward.pooled_auc.is_finite()
            || !self.walkforward.ece.is_finite()
            || !self.walkforward.conformal_radius.is_finite()
            || self.walkforward.conformal_radius < 0.0
            || self.walkforward.calibration_size == 0
        {
            return Err(ArtifactError::Invalid("invalid surge walk-forward metrics"));
        }
        Ok(())
    }

    /// Score one raw feature row: returns (surge probability, 80% interval).
    pub fn predict_surge(&self, raw: &[f64]) -> Result<(f64, f64, f64), ArtifactError> {
        self.validate()?;
        let transformed = self.transform.apply(raw).map_err(|_| ArtifactError::Invalid("surge scoring transform failed"))?;
        let score = self.coefficients[0]
            + self.coefficients[1..].iter().zip(&transformed).map(|(coefficient, value)| coefficient * value).sum::<f64>();
        let probability = 1.0 / (1.0 + (-score).exp());
        if !probability.is_finite() {
            return Err(ArtifactError::Invalid("non-finite surge probability"));
        }
        let radius = self.walkforward.conformal_radius;
        let lower = (probability - radius).clamp(0.0, 1.0);
        let upper = (probability + radius).clamp(0.0, 1.0);
        Ok((probability, lower, upper))
    }
}

pub fn write_surge_feature_matrix(
    path: impl AsRef<Path>,
    artifact: &SurgeFeatureMatrixArtifact,
) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u32(artifact.feature_names.len().try_into().map_err(|_| ArtifactError::Invalid("too many surge features"))?);
    for name in &artifact.feature_names {
        encoder.string(name)?;
    }
    encoder.u64(artifact.rows.len().try_into().map_err(|_| ArtifactError::Invalid("too many surge rows"))?);
    for row in &artifact.rows {
        encoder.u64(row.player.0);
        encoder.i32(row.as_of.0);
        encoder.f64(row.mv_t);
        encoder.f64(row.weight);
        match row.target {
            Some(target) => {
                encoder.u8(1);
                encoder.f64(target);
            }
            None => encoder.u8(0),
        }
        match row.realized_return {
            Some(return_value) => {
                encoder.u8(1);
                encoder.f64(return_value);
            }
            None => encoder.u8(0),
        }
        for value in &row.features {
            encoder.f32(*value);
        }
    }
    write_envelope(path, ArtifactKind::SurgeFeatureMatrix, ArtifactVersions {
        dataset: artifact.dataset_version,
        feature_set: Some(artifact.feature_set_version),
        model: None,
        calibration: None,
        cutoff: None,
    }, &encoder.finish())
}

pub fn read_surge_feature_matrix(path: impl AsRef<Path>) -> Result<SurgeFeatureMatrixArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::SurgeFeatureMatrix)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("surge matrix missing feature version"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let feature_count = decoder.u32()? as usize;
    if feature_count == 0 {
        return Err(ArtifactError::Invalid("surge matrix has zero width"));
    }
    let mut feature_names = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        feature_names.push(decoder.string()?);
    }
    let row_count: usize = decoder.u64()?.try_into().map_err(|_| ArtifactError::Invalid("too many surge rows"))?;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let player = PlayerId(decoder.u64()?);
        let as_of = Date(decoder.i32()?);
        let mv_t = decoder.f64()?;
        let weight = decoder.f64()?;
        let target = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid surge target flag")),
        };
        let realized_return = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid surge return flag")),
        };
        let mut features = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            features.push(decoder.f32()?);
        }
        rows.push(SurgeFeatureRow { player, as_of, mv_t, target, realized_return, weight, features });
    }
    decoder.finish()?;
    let artifact = SurgeFeatureMatrixArtifact {
        dataset_version: header.versions.dataset,
        source_manifest_checksum,
        feature_set_version,
        feature_names,
        rows,
    };
    artifact.validate()?;
    Ok(artifact)
}

fn encode_surge_deciles(encoder: &mut Encoder, deciles: &[SurgeDecileSnapshot]) -> Result<(), ArtifactError> {
    encoder.u32(deciles.len().try_into().map_err(|_| ArtifactError::Invalid("too many deciles"))?);
    for decile in deciles {
        encoder.u8(decile.decile);
        encoder.u32(decile.count);
        encoder.f64(decile.mean_score);
        encoder.f64(decile.realized_rate);
        encoder.f64(decile.mean_return);
    }
    Ok(())
}

fn decode_surge_deciles(decoder: &mut Decoder<'_>) -> Result<Vec<SurgeDecileSnapshot>, ArtifactError> {
    let count = decoder.u32()? as usize;
    let mut deciles = Vec::with_capacity(count);
    for _ in 0..count {
        deciles.push(SurgeDecileSnapshot {
            decile: decoder.u8()?,
            count: decoder.u32()?,
            mean_score: decoder.f64()?,
            realized_rate: decoder.f64()?,
            mean_return: decoder.f64()?,
        });
    }
    Ok(deciles)
}

pub fn write_surge_model(path: impl AsRef<Path>, artifact: &SurgeModelArtifact) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u64(artifact.metadata.seed);
    encoder.f64(artifact.ridge);
    encoder.u32(artifact.feature_names.len().try_into().map_err(|_| ArtifactError::Invalid("too many surge features"))?);
    for name in &artifact.feature_names {
        encoder.string(name)?;
    }
    encoder.u32(artifact.transform.width().try_into().map_err(|_| ArtifactError::Invalid("too many transform features"))?);
    for value in &artifact.transform.means {
        encoder.f64(*value);
    }
    for value in &artifact.transform.stds {
        encoder.f64(*value);
    }
    for value in &artifact.transform.missing_rates {
        encoder.f64(*value);
    }
    encoder.u32(artifact.coefficients.len().try_into().map_err(|_| ArtifactError::Invalid("too many surge coefficients"))?);
    for coefficient in &artifact.coefficients {
        encoder.f64(*coefficient);
    }
    encoder.u32(artifact.walkforward.periods.len().try_into().map_err(|_| ArtifactError::Invalid("too many periods"))?);
    for period in &artifact.walkforward.periods {
        encoder.i32(period.cutoff.0);
        encoder.u32(period.train_rows);
        encoder.u32(period.test_rows);
        encoder.f64(period.auc);
    }
    encoder.f64(artifact.walkforward.pooled_auc);
    encoder.f64(artifact.walkforward.auc_without_fm);
    encoder.f64(artifact.walkforward.ece);
    encode_surge_deciles(&mut encoder, &artifact.walkforward.deciles)?;
    encoder.f64(artifact.walkforward.conformal_radius);
    encoder.f64(artifact.walkforward.calibration_coverage);
    encoder.u32(artifact.walkforward.calibration_size);
    write_envelope(path, ArtifactKind::SurgeModel, ArtifactVersions {
        dataset: artifact.metadata.dataset_version,
        feature_set: Some(artifact.metadata.feature_set_version),
        model: Some(artifact.metadata.model_version),
        calibration: Some(artifact.calibration_version),
        cutoff: Some(artifact.metadata.trained_through),
    }, &encoder.finish())
}

pub fn read_surge_model(path: impl AsRef<Path>) -> Result<SurgeModelArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::SurgeModel)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("surge model missing feature version"))?;
    let model_version = header.versions.model.ok_or(ArtifactError::Invalid("surge model missing model version"))?;
    let calibration_version = header.versions.calibration.ok_or(ArtifactError::Invalid("surge model missing calibration version"))?;
    let trained_through = header.versions.cutoff.ok_or(ArtifactError::Invalid("surge model missing training cutoff"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let seed = decoder.u64()?;
    let ridge = decoder.f64()?;
    let feature_count = decoder.u32()? as usize;
    let mut feature_names = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        feature_names.push(decoder.string()?);
    }
    let transform_width = decoder.u32()? as usize;
    if transform_width != feature_count {
        return Err(ArtifactError::Invalid("surge transform width mismatch"));
    }
    let mut means = Vec::with_capacity(transform_width);
    let mut stds = Vec::with_capacity(transform_width);
    let mut missing_rates = Vec::with_capacity(transform_width);
    for _ in 0..transform_width {
        means.push(decoder.f64()?);
    }
    for _ in 0..transform_width {
        stds.push(decoder.f64()?);
    }
    for _ in 0..transform_width {
        missing_rates.push(decoder.f64()?);
    }
    let coefficient_count = decoder.u32()? as usize;
    let mut coefficients = Vec::with_capacity(coefficient_count);
    for _ in 0..coefficient_count {
        coefficients.push(decoder.f64()?);
    }
    let period_count = decoder.u32()? as usize;
    let mut periods = Vec::with_capacity(period_count);
    for _ in 0..period_count {
        periods.push(SurgePeriodMetrics {
            cutoff: Date(decoder.i32()?),
            train_rows: decoder.u32()?,
            test_rows: decoder.u32()?,
            auc: decoder.f64()?,
        });
    }
    let pooled_auc = decoder.f64()?;
    let auc_without_fm = decoder.f64()?;
    let ece = decoder.f64()?;
    let deciles = decode_surge_deciles(&mut decoder)?;
    let conformal_radius = decoder.f64()?;
    let calibration_coverage = decoder.f64()?;
    let calibration_size = decoder.u32()?;
    decoder.finish()?;
    let artifact = SurgeModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version,
            dataset_version: header.versions.dataset,
            feature_set_version,
            seed,
        },
        source_manifest_checksum,
        calibration_version,
        ridge,
        feature_names,
        transform: FeatureTransform { means, stds, missing_rates },
        coefficients,
        walkforward: SurgeWalkForwardMetrics {
            periods,
            pooled_auc,
            auc_without_fm,
            ece,
            deciles,
            conformal_radius,
            calibration_coverage,
            calibration_size,
        },
    };
    artifact.validate()?;
    Ok(artifact)
}

// ---------------------------------------------------------------------------
// 12-month value-forecast artifacts
// ---------------------------------------------------------------------------

/// One raw (untransformed) forecast feature row. `features` deliberately permits
/// `NaN` entries: missingness is encoded by the model transform, never as zero.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastFeatureRow {
    pub player: PlayerId,
    pub as_of: Date,
    pub mv_t: f64,
    /// 12-month log value ratio `log(mv_12m / mv_t)`; `None` for candidate rows.
    pub target: Option<f64>,
    pub realized_return: Option<f64>,
    pub weight: f64,
    pub features: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForecastFeatureMatrixArtifact {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub feature_set_version: FeatureSetVersion,
    pub feature_names: Vec<String>,
    pub rows: Vec<ForecastFeatureRow>,
}

impl ForecastFeatureMatrixArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.feature_names.is_empty() {
            return Err(ArtifactError::Invalid("forecast matrix has no features"));
        }
        if self.feature_names.iter().any(|name| name.is_empty()) {
            return Err(ArtifactError::Invalid("empty forecast feature name"));
        }
        let mut row_keys = BTreeSet::new();
        for row in &self.rows {
            if row.features.len() != self.feature_names.len() {
                return Err(ArtifactError::Invalid("forecast feature row width mismatch"));
            }
            if row.features.iter().any(|value| !value.is_finite() && !value.is_nan()) {
                return Err(ArtifactError::Invalid("infinite forecast feature"));
            }
            if !row.mv_t.is_finite() || row.mv_t <= 0.0 {
                return Err(ArtifactError::Invalid("invalid forecast reference value"));
            }
            if row.target.is_some_and(|value| !value.is_finite()) {
                return Err(ArtifactError::Invalid("non-finite forecast target"));
            }
            if row.realized_return.is_some_and(|value| !value.is_finite()) {
                return Err(ArtifactError::Invalid("non-finite realized return"));
            }
            if !row.weight.is_finite() || row.weight <= 0.0 {
                return Err(ArtifactError::Invalid("invalid forecast row weight"));
            }
            if !row_keys.insert((row.player, row.as_of)) {
                return Err(ArtifactError::Invalid("duplicate forecast player/date row"));
            }
        }
        Ok(())
    }
}

/// Persisted walk-forward evaluation summary for the forecast model.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastPeriodMetrics {
    pub cutoff: Date,
    pub train_rows: u32,
    pub test_rows: u32,
    pub rmse: f64,
    pub mae: f64,
    pub spearman_ic: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForecastDecileSnapshot {
    pub decile: u8,
    pub count: u32,
    pub mean_predicted: f64,
    pub mean_realized: f64,
    pub mean_return: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForecastWalkForwardMetrics {
    pub periods: Vec<ForecastPeriodMetrics>,
    pub pooled_rmse: f64,
    pub pooled_mae: f64,
    pub pooled_spearman_ic: Option<f64>,
    pub deciles: Vec<ForecastDecileSnapshot>,
    pub conformal_radius: f64,
    pub calibration_coverage: f64,
    pub calibration_size: u32,
}

/// Forecast model artifact: standardized-feature ridge OLS on the 12-month log
/// value ratio, plus walk-forward evaluation and conformal interval radius.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastModelArtifact {
    pub metadata: ModelMetadata,
    pub source_manifest_checksum: u64,
    pub calibration_version: CalibrationVersion,
    pub ridge: f64,
    pub feature_names: Vec<String>,
    pub transform: FeatureTransform,
    /// Intercept first, followed by `2 * feature_names.len()` coefficients
    /// (standardized value slot, then missingness-indicator slot, per feature).
    pub coefficients: Vec<f64>,
    pub walkforward: ForecastWalkForwardMetrics,
}

impl ForecastModelArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if !self.ridge.is_finite() || self.ridge < 0.0 {
            return Err(ArtifactError::Invalid("invalid forecast ridge parameter"));
        }
        if self.feature_names.is_empty() {
            return Err(ArtifactError::Invalid("forecast model has no features"));
        }
        if self.transform.width() != self.feature_names.len() {
            return Err(ArtifactError::Invalid("forecast transform/feature mismatch"));
        }
        let expected_coefficients = self.feature_names.len() * 2 + 1;
        if self.coefficients.len() != expected_coefficients {
            return Err(ArtifactError::Invalid("forecast model coefficient/feature mismatch"));
        }
        if self.coefficients.iter().any(|value| !value.is_finite()) {
            return Err(ArtifactError::Invalid("non-finite forecast model coefficient"));
        }
        if !self.walkforward.pooled_rmse.is_finite()
            || !self.walkforward.pooled_mae.is_finite()
            || !self.walkforward.conformal_radius.is_finite()
            || self.walkforward.conformal_radius < 0.0
            || self.walkforward.calibration_size == 0
        {
            return Err(ArtifactError::Invalid("invalid forecast walk-forward metrics"));
        }
        Ok(())
    }

    /// Score one raw feature row: returns (predicted log_ratio, 80% interval
    /// lower, 80% interval upper) on the log scale.
    pub fn predict_log_ratio(&self, raw: &[f64]) -> Result<(f64, f64, f64), ArtifactError> {
        self.validate()?;
        let transformed = self.transform
            .apply(raw)
            .map_err(|_| ArtifactError::Invalid("forecast scoring transform failed"))?;
        let prediction = self.coefficients[0]
            + self.coefficients[1..]
                .iter()
                .zip(&transformed)
                .map(|(coefficient, value)| coefficient * value)
                .sum::<f64>();
        if !prediction.is_finite() {
            return Err(ArtifactError::Invalid("non-finite forecast prediction"));
        }
        let radius = self.walkforward.conformal_radius;
        Ok((prediction, prediction - radius, prediction + radius))
    }
}

pub fn write_forecast_feature_matrix(
    path: impl AsRef<Path>,
    artifact: &ForecastFeatureMatrixArtifact,
) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u32(artifact.feature_names.len().try_into().map_err(|_| ArtifactError::Invalid("too many forecast features"))?);
    for name in &artifact.feature_names {
        encoder.string(name)?;
    }
    encoder.u64(artifact.rows.len().try_into().map_err(|_| ArtifactError::Invalid("too many forecast rows"))?);
    for row in &artifact.rows {
        encoder.u64(row.player.0);
        encoder.i32(row.as_of.0);
        encoder.f64(row.mv_t);
        encoder.f64(row.weight);
        match row.target {
            Some(target) => {
                encoder.u8(1);
                encoder.f64(target);
            }
            None => encoder.u8(0),
        }
        match row.realized_return {
            Some(return_value) => {
                encoder.u8(1);
                encoder.f64(return_value);
            }
            None => encoder.u8(0),
        }
        for value in &row.features {
            encoder.f32(*value);
        }
    }
    write_envelope(path, ArtifactKind::ForecastFeatureMatrix, ArtifactVersions {
        dataset: artifact.dataset_version,
        feature_set: Some(artifact.feature_set_version),
        model: None,
        calibration: None,
        cutoff: None,
    }, &encoder.finish())
}

pub fn read_forecast_feature_matrix(path: impl AsRef<Path>) -> Result<ForecastFeatureMatrixArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::ForecastFeatureMatrix)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("forecast matrix missing feature version"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let feature_count = decoder.u32()? as usize;
    if feature_count == 0 {
        return Err(ArtifactError::Invalid("forecast matrix has zero width"));
    }
    let mut feature_names = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        feature_names.push(decoder.string()?);
    }
    let row_count: usize = decoder.u64()?.try_into().map_err(|_| ArtifactError::Invalid("too many forecast rows"))?;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let player = PlayerId(decoder.u64()?);
        let as_of = Date(decoder.i32()?);
        let mv_t = decoder.f64()?;
        let weight = decoder.f64()?;
        let target = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid forecast target flag")),
        };
        let realized_return = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid forecast return flag")),
        };
        let mut features = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            features.push(decoder.f32()?);
        }
        rows.push(ForecastFeatureRow { player, as_of, mv_t, target, realized_return, weight, features });
    }
    decoder.finish()?;
    let artifact = ForecastFeatureMatrixArtifact {
        dataset_version: header.versions.dataset,
        source_manifest_checksum,
        feature_set_version,
        feature_names,
        rows,
    };
    artifact.validate()?;
    Ok(artifact)
}

fn encode_forecast_deciles(encoder: &mut Encoder, deciles: &[ForecastDecileSnapshot]) -> Result<(), ArtifactError> {
    encoder.u32(deciles.len().try_into().map_err(|_| ArtifactError::Invalid("too many forecast deciles"))?);
    for decile in deciles {
        encoder.u8(decile.decile);
        encoder.u32(decile.count);
        encoder.f64(decile.mean_predicted);
        encoder.f64(decile.mean_realized);
        encoder.f64(decile.mean_return);
    }
    Ok(())
}

fn decode_forecast_deciles(decoder: &mut Decoder<'_>) -> Result<Vec<ForecastDecileSnapshot>, ArtifactError> {
    let count = decoder.u32()? as usize;
    let mut deciles = Vec::with_capacity(count);
    for _ in 0..count {
        deciles.push(ForecastDecileSnapshot {
            decile: decoder.u8()?,
            count: decoder.u32()?,
            mean_predicted: decoder.f64()?,
            mean_realized: decoder.f64()?,
            mean_return: decoder.f64()?,
        });
    }
    Ok(deciles)
}

pub fn write_forecast_model(path: impl AsRef<Path>, artifact: &ForecastModelArtifact) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u64(artifact.metadata.seed);
    encoder.f64(artifact.ridge);
    encoder.u32(artifact.feature_names.len().try_into().map_err(|_| ArtifactError::Invalid("too many forecast features"))?);
    for name in &artifact.feature_names {
        encoder.string(name)?;
    }
    encoder.u32(artifact.transform.width().try_into().map_err(|_| ArtifactError::Invalid("too many transform features"))?);
    for value in &artifact.transform.means {
        encoder.f64(*value);
    }
    for value in &artifact.transform.stds {
        encoder.f64(*value);
    }
    for value in &artifact.transform.missing_rates {
        encoder.f64(*value);
    }
    encoder.u32(artifact.coefficients.len().try_into().map_err(|_| ArtifactError::Invalid("too many forecast coefficients"))?);
    for coefficient in &artifact.coefficients {
        encoder.f64(*coefficient);
    }
    encoder.u32(artifact.walkforward.periods.len().try_into().map_err(|_| ArtifactError::Invalid("too many forecast periods"))?);
    for period in &artifact.walkforward.periods {
        encoder.i32(period.cutoff.0);
        encoder.u32(period.train_rows);
        encoder.u32(period.test_rows);
        encoder.f64(period.rmse);
        encoder.f64(period.mae);
        match period.spearman_ic {
            Some(ic) => {
                encoder.u8(1);
                encoder.f64(ic);
            }
            None => encoder.u8(0),
        }
    }
    encoder.f64(artifact.walkforward.pooled_rmse);
    encoder.f64(artifact.walkforward.pooled_mae);
    match artifact.walkforward.pooled_spearman_ic {
        Some(ic) => {
            encoder.u8(1);
            encoder.f64(ic);
        }
        None => encoder.u8(0),
    }
    encode_forecast_deciles(&mut encoder, &artifact.walkforward.deciles)?;
    encoder.f64(artifact.walkforward.conformal_radius);
    encoder.f64(artifact.walkforward.calibration_coverage);
    encoder.u32(artifact.walkforward.calibration_size);
    write_envelope(path, ArtifactKind::ForecastModel, ArtifactVersions {
        dataset: artifact.metadata.dataset_version,
        feature_set: Some(artifact.metadata.feature_set_version),
        model: Some(artifact.metadata.model_version),
        calibration: Some(artifact.calibration_version),
        cutoff: Some(artifact.metadata.trained_through),
    }, &encoder.finish())
}

pub fn read_forecast_model(path: impl AsRef<Path>) -> Result<ForecastModelArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::ForecastModel)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("forecast model missing feature version"))?;
    let model_version = header.versions.model.ok_or(ArtifactError::Invalid("forecast model missing model version"))?;
    let calibration_version = header.versions.calibration.ok_or(ArtifactError::Invalid("forecast model missing calibration version"))?;
    let trained_through = header.versions.cutoff.ok_or(ArtifactError::Invalid("forecast model missing training cutoff"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let seed = decoder.u64()?;
    let ridge = decoder.f64()?;
    let feature_count = decoder.u32()? as usize;
    let mut feature_names = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        feature_names.push(decoder.string()?);
    }
    let transform_width = decoder.u32()? as usize;
    if transform_width != feature_count {
        return Err(ArtifactError::Invalid("forecast transform width mismatch"));
    }
    let mut means = Vec::with_capacity(transform_width);
    let mut stds = Vec::with_capacity(transform_width);
    let mut missing_rates = Vec::with_capacity(transform_width);
    for _ in 0..transform_width {
        means.push(decoder.f64()?);
    }
    for _ in 0..transform_width {
        stds.push(decoder.f64()?);
    }
    for _ in 0..transform_width {
        missing_rates.push(decoder.f64()?);
    }
    let coefficient_count = decoder.u32()? as usize;
    let mut coefficients = Vec::with_capacity(coefficient_count);
    for _ in 0..coefficient_count {
        coefficients.push(decoder.f64()?);
    }
    let period_count = decoder.u32()? as usize;
    let mut periods = Vec::with_capacity(period_count);
    for _ in 0..period_count {
        let cutoff = Date(decoder.i32()?);
        let train_rows = decoder.u32()?;
        let test_rows = decoder.u32()?;
        let rmse = decoder.f64()?;
        let mae = decoder.f64()?;
        let spearman_ic = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid forecast IC flag")),
        };
        periods.push(ForecastPeriodMetrics { cutoff, train_rows, test_rows, rmse, mae, spearman_ic });
    }
    let pooled_rmse = decoder.f64()?;
    let pooled_mae = decoder.f64()?;
    let pooled_spearman_ic = match decoder.u8()? {
        0 => None,
        1 => Some(decoder.f64()?),
        _ => return Err(ArtifactError::Invalid("invalid forecast pooled IC flag")),
    };
    let deciles = decode_forecast_deciles(&mut decoder)?;
    let conformal_radius = decoder.f64()?;
    let calibration_coverage = decoder.f64()?;
    let calibration_size = decoder.u32()?;
    decoder.finish()?;
    let artifact = ForecastModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version,
            dataset_version: header.versions.dataset,
            feature_set_version,
            seed,
        },
        source_manifest_checksum,
        calibration_version,
        ridge,
        feature_names,
        transform: FeatureTransform { means, stds, missing_rates },
        coefficients,
        walkforward: ForecastWalkForwardMetrics {
            periods,
            pooled_rmse,
            pooled_mae,
            pooled_spearman_ic,
            deciles,
            conformal_radius,
            calibration_coverage,
            calibration_size,
        },
    };
    artifact.validate()?;
    Ok(artifact)
}

// ---------------------------------------------------------------------------
// Tail-aware forecast model artifact (spike refinement run)
// ---------------------------------------------------------------------------

/// The tail objective the model was fit with. `Quantile` minimizes the pinball
/// loss at `tau` (the fitted value is the conditional `tau`-quantile of the
/// 12-month log value ratio); `Expectile` minimizes the asymmetric squared loss
/// at `omega` (the fitted value is the conditional `omega`-expectile).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TailObjective {
    Quantile { tau: f64 },
    Expectile { omega: f64 },
}

impl TailObjective {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Quantile { .. } => "quantile",
            Self::Expectile { .. } => "expectile",
        }
    }
}

/// One walk-forward period of the tail-aware forecast evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailPeriodMetrics {
    pub cutoff: Date,
    pub train_rows: u32,
    pub test_rows: u32,
    pub rank_ic: Option<f64>,
    pub top_decile_spike50_rate: f64,
    pub top_decile_spike100_rate: f64,
}

/// Pooled out-of-fold decile snapshot ranked by the tail score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailDecileSnapshot {
    pub decile: u8,
    pub count: u32,
    pub mean_predicted: f64,
    pub mean_realized: f64,
    pub mean_return: f64,
    pub median_return: f64,
    pub spike50_rate: f64,
    pub spike100_rate: f64,
}

/// Walk-forward evaluation metrics for the tail-aware forecast model.
#[derive(Debug, Clone, PartialEq)]
pub struct TailWalkForwardMetrics {
    pub periods: Vec<TailPeriodMetrics>,
    pub pooled_rank_ic: Option<f64>,
    pub deciles: Vec<TailDecileSnapshot>,
    /// Empirical fraction of out-of-fold rows with `actual <= predicted`; for a
    /// well-calibrated quantile model this is close to `tau`.
    pub q_coverage: f64,
    pub q_coverage_size: u32,
    pub pooled_spike50_rate: f64,
    pub pooled_spike100_rate: f64,
}

/// Tail-aware forecast model artifact: ridge regression with a tail objective
/// (high quantile / expectile) on the 12-month log value ratio, plus
/// walk-forward evaluation. Same 47-feature causal schema as the forecast and
/// surge artifacts (standardized values + paired missingness indicators).
#[derive(Debug, Clone, PartialEq)]
pub struct TailForecastModelArtifact {
    pub metadata: ModelMetadata,
    pub source_manifest_checksum: u64,
    pub calibration_version: CalibrationVersion,
    pub ridge: f64,
    pub objective: TailObjective,
    pub feature_names: Vec<String>,
    pub transform: FeatureTransform,
    /// Intercept first, followed by `2 * feature_names.len()` coefficients
    /// (standardized value slot, then missingness-indicator slot, per feature).
    pub coefficients: Vec<f64>,
    pub walkforward: TailWalkForwardMetrics,
}

impl TailForecastModelArtifact {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if !self.ridge.is_finite() || self.ridge < 0.0 {
            return Err(ArtifactError::Invalid("invalid tail ridge parameter"));
        }
        match self.objective {
            TailObjective::Quantile { tau } => {
                if !tau.is_finite() || tau <= 0.0 || tau >= 1.0 {
                    return Err(ArtifactError::Invalid("invalid tail quantile tau"));
                }
            }
            TailObjective::Expectile { omega } => {
                if !omega.is_finite() || omega <= 0.0 || omega >= 1.0 {
                    return Err(ArtifactError::Invalid("invalid tail expectile omega"));
                }
            }
        }
        if self.feature_names.is_empty() {
            return Err(ArtifactError::Invalid("tail model has no features"));
        }
        if self.transform.width() != self.feature_names.len() {
            return Err(ArtifactError::Invalid("tail transform/feature mismatch"));
        }
        let expected_coefficients = self.feature_names.len() * 2 + 1;
        if self.coefficients.len() != expected_coefficients {
            return Err(ArtifactError::Invalid("tail model coefficient/feature mismatch"));
        }
        if self.coefficients.iter().any(|value| !value.is_finite()) {
            return Err(ArtifactError::Invalid("non-finite tail model coefficient"));
        }
        if !self.walkforward.q_coverage.is_finite()
            || !self.walkforward.pooled_spike50_rate.is_finite()
            || !self.walkforward.pooled_spike100_rate.is_finite()
            || self.walkforward.q_coverage_size == 0
        {
            return Err(ArtifactError::Invalid("invalid tail walk-forward metrics"));
        }
        Ok(())
    }

    /// Score one raw feature row: returns the fitted tail score (the predicted
    /// high quantile / expectile of the 12-month log value ratio). Higher is a
    /// stronger spike signal.
    pub fn predict_tail(&self, raw: &[f64]) -> Result<f64, ArtifactError> {
        self.validate()?;
        let transformed = self
            .transform
            .apply(raw)
            .map_err(|_| ArtifactError::Invalid("tail scoring transform failed"))?;
        let prediction = self.coefficients[0]
            + self.coefficients[1..]
                .iter()
                .zip(&transformed)
                .map(|(coefficient, value)| coefficient * value)
                .sum::<f64>();
        if !prediction.is_finite() {
            return Err(ArtifactError::Invalid("non-finite tail prediction"));
        }
        Ok(prediction)
    }
}

pub fn write_tail_forecast_model(
    path: impl AsRef<Path>,
    artifact: &TailForecastModelArtifact,
) -> Result<ArtifactHeader, ArtifactError> {
    artifact.validate()?;
    let mut encoder = Encoder::new();
    encoder.u64(artifact.source_manifest_checksum);
    encoder.u64(artifact.metadata.seed);
    encoder.f64(artifact.ridge);
    match artifact.objective {
        TailObjective::Quantile { tau } => {
            encoder.u8(0);
            encoder.f64(tau);
        }
        TailObjective::Expectile { omega } => {
            encoder.u8(1);
            encoder.f64(omega);
        }
    }
    encoder.u32(artifact.feature_names.len().try_into().map_err(|_| ArtifactError::Invalid("too many tail features"))?);
    for name in &artifact.feature_names {
        encoder.string(name)?;
    }
    encoder.u32(artifact.transform.width().try_into().map_err(|_| ArtifactError::Invalid("too many transform features"))?);
    for value in &artifact.transform.means {
        encoder.f64(*value);
    }
    for value in &artifact.transform.stds {
        encoder.f64(*value);
    }
    for value in &artifact.transform.missing_rates {
        encoder.f64(*value);
    }
    encoder.u32(artifact.coefficients.len().try_into().map_err(|_| ArtifactError::Invalid("too many tail coefficients"))?);
    for coefficient in &artifact.coefficients {
        encoder.f64(*coefficient);
    }
    encoder.u32(artifact.walkforward.periods.len().try_into().map_err(|_| ArtifactError::Invalid("too many tail periods"))?);
    for period in &artifact.walkforward.periods {
        encoder.i32(period.cutoff.0);
        encoder.u32(period.train_rows);
        encoder.u32(period.test_rows);
        match period.rank_ic {
            Some(ic) => {
                encoder.u8(1);
                encoder.f64(ic);
            }
            None => encoder.u8(0),
        }
        encoder.f64(period.top_decile_spike50_rate);
        encoder.f64(period.top_decile_spike100_rate);
    }
    match artifact.walkforward.pooled_rank_ic {
        Some(ic) => {
            encoder.u8(1);
            encoder.f64(ic);
        }
        None => encoder.u8(0),
    }
    encoder.u32(artifact.walkforward.deciles.len().try_into().map_err(|_| ArtifactError::Invalid("too many tail deciles"))?);
    for decile in &artifact.walkforward.deciles {
        encoder.u8(decile.decile);
        encoder.u32(decile.count);
        encoder.f64(decile.mean_predicted);
        encoder.f64(decile.mean_realized);
        encoder.f64(decile.mean_return);
        encoder.f64(decile.median_return);
        encoder.f64(decile.spike50_rate);
        encoder.f64(decile.spike100_rate);
    }
    encoder.f64(artifact.walkforward.q_coverage);
    encoder.u32(artifact.walkforward.q_coverage_size);
    encoder.f64(artifact.walkforward.pooled_spike50_rate);
    encoder.f64(artifact.walkforward.pooled_spike100_rate);
    write_envelope(path, ArtifactKind::TailForecastModel, ArtifactVersions {
        dataset: artifact.metadata.dataset_version,
        feature_set: Some(artifact.metadata.feature_set_version),
        model: Some(artifact.metadata.model_version),
        calibration: Some(artifact.calibration_version),
        cutoff: Some(artifact.metadata.trained_through),
    }, &encoder.finish())
}

pub fn read_tail_forecast_model(path: impl AsRef<Path>) -> Result<TailForecastModelArtifact, ArtifactError> {
    let (header, payload) = read_envelope(path, ArtifactKind::TailForecastModel)?;
    let feature_set_version = header.versions.feature_set.ok_or(ArtifactError::Invalid("tail model missing feature version"))?;
    let model_version = header.versions.model.ok_or(ArtifactError::Invalid("tail model missing model version"))?;
    let calibration_version = header.versions.calibration.ok_or(ArtifactError::Invalid("tail model missing calibration version"))?;
    let trained_through = header.versions.cutoff.ok_or(ArtifactError::Invalid("tail model missing training cutoff"))?;
    let mut decoder = Decoder::new(&payload);
    let source_manifest_checksum = decoder.u64()?;
    let seed = decoder.u64()?;
    let ridge = decoder.f64()?;
    let objective = match decoder.u8()? {
        0 => TailObjective::Quantile { tau: decoder.f64()? },
        1 => TailObjective::Expectile { omega: decoder.f64()? },
        _ => return Err(ArtifactError::Invalid("invalid tail objective flag")),
    };
    let feature_count = decoder.u32()? as usize;
    let mut feature_names = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        feature_names.push(decoder.string()?);
    }
    let transform_width = decoder.u32()? as usize;
    if transform_width != feature_count {
        return Err(ArtifactError::Invalid("tail transform width mismatch"));
    }
    let mut means = Vec::with_capacity(transform_width);
    let mut stds = Vec::with_capacity(transform_width);
    let mut missing_rates = Vec::with_capacity(transform_width);
    for _ in 0..transform_width {
        means.push(decoder.f64()?);
    }
    for _ in 0..transform_width {
        stds.push(decoder.f64()?);
    }
    for _ in 0..transform_width {
        missing_rates.push(decoder.f64()?);
    }
    let coefficient_count = decoder.u32()? as usize;
    let mut coefficients = Vec::with_capacity(coefficient_count);
    for _ in 0..coefficient_count {
        coefficients.push(decoder.f64()?);
    }
    let period_count = decoder.u32()? as usize;
    let mut periods = Vec::with_capacity(period_count);
    for _ in 0..period_count {
        let cutoff = Date(decoder.i32()?);
        let train_rows = decoder.u32()?;
        let test_rows = decoder.u32()?;
        let rank_ic = match decoder.u8()? {
            0 => None,
            1 => Some(decoder.f64()?),
            _ => return Err(ArtifactError::Invalid("invalid tail IC flag")),
        };
        let top_decile_spike50_rate = decoder.f64()?;
        let top_decile_spike100_rate = decoder.f64()?;
        periods.push(TailPeriodMetrics { cutoff, train_rows, test_rows, rank_ic, top_decile_spike50_rate, top_decile_spike100_rate });
    }
    let pooled_rank_ic = match decoder.u8()? {
        0 => None,
        1 => Some(decoder.f64()?),
        _ => return Err(ArtifactError::Invalid("invalid tail pooled IC flag")),
    };
    let decile_count = decoder.u32()? as usize;
    let mut deciles = Vec::with_capacity(decile_count);
    for _ in 0..decile_count {
        let decile = decoder.u8()?;
        let count = decoder.u32()?;
        let mean_predicted = decoder.f64()?;
        let mean_realized = decoder.f64()?;
        let mean_return = decoder.f64()?;
        let median_return = decoder.f64()?;
        let spike50_rate = decoder.f64()?;
        let spike100_rate = decoder.f64()?;
        deciles.push(TailDecileSnapshot { decile, count, mean_predicted, mean_realized, mean_return, median_return, spike50_rate, spike100_rate });
    }
    let q_coverage = decoder.f64()?;
    let q_coverage_size = decoder.u32()?;
    let pooled_spike50_rate = decoder.f64()?;
    let pooled_spike100_rate = decoder.f64()?;
    decoder.finish()?;
    let artifact = TailForecastModelArtifact {
        metadata: ModelMetadata {
            trained_through,
            model_version,
            dataset_version: header.versions.dataset,
            feature_set_version,
            seed,
        },
        source_manifest_checksum,
        calibration_version,
        ridge,
        objective,
        feature_names,
        transform: FeatureTransform { means, stds, missing_rates },
        coefficients,
        walkforward: TailWalkForwardMetrics {
            periods,
            pooled_rank_ic,
            deciles,
            q_coverage,
            q_coverage_size,
            pooled_spike50_rate,
            pooled_spike100_rate,
        },
    };
    artifact.validate()?;
    Ok(artifact)
}

#[cfg(test)]
mod tail_tests {
    use super::*;

    fn sample_tail_artifact() -> TailForecastModelArtifact {
        let width = 47;
        let transform = FeatureTransform {
            means: vec![0.0; width],
            stds: vec![1.0; width],
            missing_rates: vec![0.0; width],
        };
        let coefficients = vec![0.1; width * 2 + 1];
        TailForecastModelArtifact {
            metadata: ModelMetadata {
                trained_through: Date(20000),
                model_version: ModelVersion(3),
                dataset_version: DatasetVersion(1),
                feature_set_version: FeatureSetVersion(1),
                seed: 42,
            },
            source_manifest_checksum: 7,
            calibration_version: CalibrationVersion(1),
            ridge: 1.0,
            objective: TailObjective::Quantile { tau: 0.9 },
            feature_names: (0..width).map(|i| format!("f{i}")).collect(),
            transform,
            coefficients,
            walkforward: TailWalkForwardMetrics {
                periods: vec![TailPeriodMetrics {
                    cutoff: Date(20000),
                    train_rows: 100,
                    test_rows: 50,
                    rank_ic: Some(0.3),
                    top_decile_spike50_rate: 0.4,
                    top_decile_spike100_rate: 0.2,
                }],
                pooled_rank_ic: Some(0.3),
                deciles: vec![TailDecileSnapshot {
                    decile: 9,
                    count: 50,
                    mean_predicted: 0.5,
                    mean_realized: 0.4,
                    mean_return: 0.6,
                    median_return: 0.3,
                    spike50_rate: 0.4,
                    spike100_rate: 0.2,
                }],
                q_coverage: 0.9,
                q_coverage_size: 500,
                pooled_spike50_rate: 0.3,
                pooled_spike100_rate: 0.15,
            },
        }
    }

    #[test]
    fn tail_artifact_round_trips() {
        let dir = std::env::temp_dir().join(format!("footy-tail-artifact-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tail.bin");
        let artifact = sample_tail_artifact();
        write_tail_forecast_model(&path, &artifact).unwrap();
        let loaded = read_tail_forecast_model(&path).unwrap();
        assert_eq!(loaded, artifact);
        let raw = vec![0.0_f64; 47];
        let score = loaded.predict_tail(&raw).unwrap();
        assert!(score.is_finite());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_artifact_rejects_bad_tau() {
        let mut artifact = sample_tail_artifact();
        artifact.objective = TailObjective::Quantile { tau: 1.5 };
        assert!(artifact.validate().is_err());
        let mut artifact = sample_tail_artifact();
        artifact.objective = TailObjective::Expectile { omega: 0.0 };
        assert!(artifact.validate().is_err());
    }
}
