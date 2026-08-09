#![forbid(unsafe_code)]
//! Lowest-level, source-neutral primitives for `footy-models`.

mod date;
mod error;
mod ids;
mod interner;
mod money;
mod provenance;
mod quality;

pub use date::Date;
pub use error::CoreError;
pub use ids::*;
pub use interner::StringInterner;
pub use money::EuroCents;
pub use provenance::{Coverage, CoverageStatus, Observed, Provenance, Source};
pub use quality::{Confidence, ObservationState, Probability};
