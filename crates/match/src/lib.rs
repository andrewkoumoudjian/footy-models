#![forbid(unsafe_code)]
//! Specific-match causal graph and Monte Carlo primitives.
//!
//! The crate is intentionally source-neutral. Upstream adapters/calibration construct player state,
//! selection histories, tactical templates, matchup rules, mental state, and context factors; this
//! crate owns the stochastic graph from lineup uncertainty through player interactions and outcome.

mod calibration;
mod channels;
mod counterfactual;
mod explain;
mod lineup;
mod matchup;
mod psychology;
mod psychology_explain;
mod simulation;
mod specific;
mod specific_explain;

pub use calibration::*;
pub use channels::*;
pub use counterfactual::*;
pub use explain::*;
pub use lineup::*;
pub use matchup::*;
pub use psychology::*;
pub use psychology_explain::*;
pub use simulation::*;
pub use specific::*;
pub use specific_explain::*;
