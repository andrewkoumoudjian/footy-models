#![forbid(unsafe_code)]
//! Causal temporal primitives and bitemporal as-of lookup.

mod event;
mod interval;
mod snapshot;

pub use event::{Event, EventStore, ObservedEvent};
pub use interval::{Interval, TemporalError};
pub use snapshot::{Snapshot, SnapshotStore};

use footy_core::Date;

/// Bitemporal lookup: `valid_on` is football reality; `known_by` is the information cutoff.
pub trait AsOf<K, T> {
    fn as_of(&self, entity: K, valid_on: Date, known_by: Date) -> Option<&T>;
}
