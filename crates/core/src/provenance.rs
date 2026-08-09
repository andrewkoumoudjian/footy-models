use crate::{
    CompetitionId, Confidence, Date, MetricGroupId, ProvenanceId, SeasonId, StringId,
};

/// Canonical origin category for an imported observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    FootballManager,
    Transfermarkt,
    Fbref,
    Salary,
    Manual,
    Other(StringId),
}

/// Immutable metadata describing where and when an observation came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Provenance {
    pub source: Source,
    pub external_id: StringId,
    pub retrieved_at_unix: i64,
    pub source_date: Date,
    pub confidence: Confidence,
}

/// A value linked to a provenance record stored elsewhere.
#[derive(Debug, Clone, PartialEq)]
pub struct Observed<T> {
    pub value: T,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverageStatus {
    Complete,
    Partial,
    Missing,
    Unknown,
}

/// Coverage is date/competition/source metadata, not an implicit zero value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coverage {
    pub source: Source,
    pub competition: CompetitionId,
    pub season: SeasonId,
    pub metric_group: MetricGroupId,
    pub status: CoverageStatus,
}
