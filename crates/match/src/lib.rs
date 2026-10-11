#![forbid(unsafe_code)]
//! Specific-match causal graph and Monte Carlo primitives.
//!
//! The crate is intentionally source-neutral. Upstream adapters/calibration construct player state,
//! selection histories, tactical templates, matchup rules, mental state, and context factors; this
//! crate owns the stochastic graph from lineup uncertainty through player interactions and outcome.
//! A fixture forecast is produced from complete independent Monte Carlo match paths; recursive
//! mental/game state belongs to each path and is never shared between paths.

mod calibration;
mod channels;
mod counterfactual;
mod dynamic_mental_mc;
mod explain;
mod lineup;
mod lineup_entropy;
mod matchup;
mod psychology;
mod psychology_explain;
mod second_clock;
mod second_resolved_api;
mod second_resolved_mc;
mod simulation;
mod specific;
mod specific_explain;
mod team_goals;

pub use calibration::*;
pub use channels::*;
pub use counterfactual::*;
pub use dynamic_mental_mc::*;
pub use explain::*;
pub use lineup::*;
pub use lineup_entropy::*;
pub use matchup::*;
pub use psychology::*;
pub use psychology_explain::*;
pub use second_clock::*;
pub use second_resolved_api::*;
pub use simulation::*;
pub use specific::*;
pub use specific_explain::*;
pub use team_goals::*;
