use footy_core::Date;

use crate::{AsOf, TemporalError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event<K, T> {
    pub entity: K,
    pub date: Date,
    pub value: T,
}

/// Event whose effective time and system-observation time are explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedEvent<K, T> {
    pub entity: K,
    pub effective_at: Date,
    pub observed_at: Date,
    pub value: T,
}

/// Compact bitemporal event store sorted by `(entity, effective_at, observed_at)`.
#[derive(Debug, Clone, Default)]
pub struct EventStore<K, T> {
    entries: Vec<ObservedEvent<K, T>>,
}

impl<K: Ord, T> EventStore<K, T> {
    pub fn try_from_observed(
        mut entries: Vec<ObservedEvent<K, T>>,
    ) -> Result<Self, TemporalError> {
        entries.sort_by(|left, right| {
            left.entity
                .cmp(&right.entity)
                .then(left.effective_at.cmp(&right.effective_at))
                .then(left.observed_at.cmp(&right.observed_at))
        });

        if entries.windows(2).any(|pair| {
            pair[0].entity == pair[1].entity
                && pair[0].effective_at == pair[1].effective_at
                && pair[0].observed_at == pair[1].observed_at
        }) {
            return Err(TemporalError::DuplicateTemporalKey);
        }

        Ok(Self { entries })
    }

    pub fn try_from_events(entries: Vec<Event<K, T>>) -> Result<Self, TemporalError> {
        Self::try_from_observed(
            entries
                .into_iter()
                .map(|event| ObservedEvent {
                    entity: event.entity,
                    effective_at: event.date,
                    observed_at: event.date,
                    value: event.value,
                })
                .collect(),
        )
    }

    #[inline]
    pub fn entries(&self) -> &[ObservedEvent<K, T>] {
        &self.entries
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: Ord + Copy, T> AsOf<K, T> for EventStore<K, T> {
    fn as_of(&self, entity: K, valid_on: Date, known_by: Date) -> Option<&T> {
        let (start, end) = entity_bounds(&self.entries, entity);
        if start == end {
            return None;
        }

        // Walk backwards from the last potentially effective event. For each effective
        // timestamp, the latest revision known by the cutoff wins.
        let mut index = upper_bound_effective(&self.entries[start..end], valid_on) + start;
        while index > start {
            index -= 1;
            let effective_at = self.entries[index].effective_at;

            let mut revision = index + 1;
            while revision > start {
                revision -= 1;
                let candidate = &self.entries[revision];
                if candidate.effective_at != effective_at {
                    break;
                }
                if candidate.observed_at <= known_by {
                    return Some(&candidate.value);
                }
            }

            while index > start && self.entries[index - 1].effective_at == effective_at {
                index -= 1;
            }
        }

        None
    }
}

fn entity_bounds<K: Ord, T>(entries: &[ObservedEvent<K, T>], entity: K) -> (usize, usize) {
    let start = lower_bound_by(entries.len(), |index| entries[index].entity < entity);
    let end = lower_bound_by(entries.len(), |index| entries[index].entity <= entity);
    (start, end)
}

fn upper_bound_effective<K, T>(entries: &[ObservedEvent<K, T>], cutoff: Date) -> usize {
    lower_bound_by(entries.len(), |index| entries[index].effective_at <= cutoff)
}

fn lower_bound_by(mut len: usize, mut predicate: impl FnMut(usize) -> bool) -> usize {
    let mut base = 0;
    while len > 0 {
        let half = len / 2;
        let mid = base + half;
        if predicate(mid) {
            base = mid + 1;
            len -= half + 1;
        } else {
            len = half;
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::PlayerId;

    #[test]
    fn future_observation_is_not_visible_to_past_query() {
        let store = EventStore::try_from_observed(vec![ObservedEvent {
            entity: PlayerId(1),
            effective_at: Date(10),
            observed_at: Date(30),
            value: 42,
        }])
        .unwrap();

        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(20)), None);
        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(30)), Some(&42));
    }

    #[test]
    fn latest_known_revision_wins_without_leaking_later_revision() {
        let store = EventStore::try_from_observed(vec![
            ObservedEvent {
                entity: PlayerId(1),
                effective_at: Date(10),
                observed_at: Date(10),
                value: 100,
            },
            ObservedEvent {
                entity: PlayerId(1),
                effective_at: Date(10),
                observed_at: Date(40),
                value: 120,
            },
        ])
        .unwrap();

        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(20)), Some(&100));
        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(40)), Some(&120));
    }

    #[test]
    fn latest_effective_event_wins() {
        let store = EventStore::try_from_events(vec![
            Event { entity: PlayerId(1), date: Date(10), value: 100 },
            Event { entity: PlayerId(1), date: Date(20), value: 200 },
            Event { entity: PlayerId(2), date: Date(15), value: 999 },
        ])
        .unwrap();
        assert_eq!(store.as_of(PlayerId(1), Date(25), Date(25)), Some(&200));
        assert_eq!(store.as_of(PlayerId(1), Date(15), Date(15)), Some(&100));
        assert_eq!(store.as_of(PlayerId(3), Date(25), Date(25)), None);
    }

    #[test]
    fn duplicate_temporal_keys_are_rejected() {
        let result = EventStore::try_from_observed(vec![
            ObservedEvent { entity: PlayerId(1), effective_at: Date(10), observed_at: Date(10), value: 1 },
            ObservedEvent { entity: PlayerId(1), effective_at: Date(10), observed_at: Date(10), value: 2 },
        ]);
        assert_eq!(result.unwrap_err(), TemporalError::DuplicateTemporalKey);
    }
}
