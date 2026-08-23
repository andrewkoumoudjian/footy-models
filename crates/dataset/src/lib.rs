#![forbid(unsafe_code)]
//! Canonical cross-source dataset construction, causal feature materialization, and persistence.

mod canonical;
#[allow(unused_imports)]
mod features;
#[allow(unused_imports)]
mod persistence;
mod sporting;
mod tabular;

pub use canonical::{
    build_canonical_dataset, CanonicalBuildConfig, CanonicalContract, CanonicalDataset,
    CanonicalFmSnapshot, CanonicalIdentity, CanonicalIssue, CanonicalMarketValue,
    CanonicalPerformanceObservation, CanonicalPlayer, CanonicalProvenance, CanonicalSalaryObservation,
    CanonicalTransfer, CanonicalUnresolvedCandidate, CanonicalUnresolvedIdentity, ExternalPlayerRecord,
    IdentityOverride, MetricDefinition, SourceClubRef, SourceCompetitionRef, UnresolvedReason,
};
pub use features::{
    feature_name, materialize_scoring_features, materialize_training_features, DatasetFeatureRow,
    FeatureMaterialization, MaterializationConfig, FEATURE_AGE_OBSERVED, FEATURE_AGE_YEARS,
    FEATURE_CA_OBSERVED, FEATURE_CA_REALIZATION, FEATURE_CA_REALIZATION_OBSERVED,
    FEATURE_CA_VALUE, FEATURE_CA_VELOCITY, FEATURE_CA_VELOCITY_OBSERVED,
    FEATURE_FM_AGE_DAYS, FEATURE_FM_PRESENT, FEATURE_PA_OBSERVED, FEATURE_PA_VALUE,
    FEATURE_REPUTATION_OBSERVED, FEATURE_REPUTATION_VALUE,
};
pub use persistence::{read_canonical_dataset, write_canonical_dataset};
pub use sporting::{
    build_sporting_dataset, resolve_sporting_player_links, CanonicalMatchResult,
    CanonicalPlayerStatObservation, CanonicalTeamStatObservation, SportingClubRecord,
    SportingCompetitionRecord, SportingDataset, SportingExternalPlayer, SportingIdentityResolution,
    SportingIssue, SportingPlayerLink, SportingProvenance,
};
pub use tabular::{
    infer_fm_schema, parse_external_players_tsv, parse_fbref_performance_tsv,
    parse_identity_overrides_tsv, parse_salary_tsv, parse_tm_contracts_tsv,
    parse_tm_market_values_tsv, parse_tm_transfers_tsv, ParseIssue, ParseReport,
};

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetError(pub String);

impl DatasetError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DatasetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for DatasetError {}

impl From<&str> for DatasetError {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for DatasetError {
    fn from(value: String) -> Self {
        Self(value)
    }
}
