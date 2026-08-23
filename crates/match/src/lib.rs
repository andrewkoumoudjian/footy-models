#![forbid(unsafe_code)]
//! Specific-match causal graph and Monte Carlo primitives.
//!
//! The crate is intentionally source-neutral. Upstream adapters/calibration construct player state,
//! selection histories, tactical templates, matchup rules, and context factors; this crate owns the
//! stochastic graph from lineup uncertainty through player interactions and match outcome.

mod calibration;
mod counterfactual;
mod explain;
mod lineup;
mod matchup;
mod simulation;

pub use calibration::*;
pub use counterfactual::*;
pub use explain::*;
pub use lineup::*;
pub use matchup::*;
pub use simulation::*;
