use footy_core::Date;

use crate::{AsOf, Interval, TemporalError};

/// A bitemporal snapshot. `valid` is football reality; `observed_at` is when the
/// system learned this version of the fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot<K, T> {
    pub entity: K,
    pub valid: Interval,
    pub observed_at: Date,
    pub value: T,
}

/// Compact snapshot store sorted by `(entity, valid.start, observed_at)`.
#[derive(Debug, Clone, Default)]
pub struct SnapshotStore<K, T> {
    entries: Vec<Snapshot<K, T>>,
}

impl<K: Ord, T> SnapshotStore<K, T> {
    pub fn try_from_snapshots(mut entries: Vec<Snapshot<K, T>>) -> Result<Self, TemporalError> {
        entries.sort_by(|left, right| {
            left.entity
                .cmp(&right.entity)
                .then(left.valid.start.cmp(&right.valid.start))
                .then(left.valid.end_exclusive.cmp(&right.valid.end_exclusive))
                .then(left.observed_at.cmp(&right.observed_at))
        });

        if entries.windows(2).any(|pair| {
            pair[0].entity == pair[1].entity
                && pair[0].valid == pair[1].valid
                && pair[0].observed_at == pair[1].observed_at
        }) {
            return Err(TemporalError::DuplicateTemporalKey);
        }

        Ok(Self { entries })
    }

    #[inline]
    pub fn entries(&self) -> &[Snapshot<K, T>] {
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

impl<K: Ord + Copy, T> AsOf<K, T> for SnapshotStore<K, T> {
    fn as_of(&self, entity: K, valid_on: Date, known_by: Date) -> Option<&T> {
        let (start, end) = entity_bounds(&self.entries, entity);
        let mut best: Option<&Snapshot<K, T>> = None;

        for candidate in &self.entries[start..end] {
            if candidate.valid.start > valid_on {
                break;
            }
            if candidate.observed_at > known_by || !candidate.valid.contains(valid_on) {
                continue;
            }

            // Latest knowledge wins; ties prefer the most recently starting validity interval.
            let replace = match best {
                None => true,
                Some(current) => {
                    candidate.observed_at > current.observed_at
                        || (candidate.observed_at == current.observed_at
                            && candidate.valid.start > current.valid.start)
                }
            };
            if replace {
                best = Some(candidate);
            }
        }

        best.map(|snapshot| &snapshot.value)
    }
}

fn entity_bounds<K: Ord, T>(entries: &[Snapshot<K, T>], entity: K) -> (usize, usize) {
    let start = lower_bound_by(entries.len(), |index| entries[index].entity < entity);
    let end = lower_bound_by(entries.len(), |index| entries[index].entity <= entity);
    (start, end)
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

    fn interval(start: i32, end: Option<i32>) -> Interval {
        Interval::new(Date(start), end.map(Date)).unwrap()
    }

    #[test]
    fn validity_end_is_exclusive() {
        let store = SnapshotStore::try_from_snapshots(vec![Snapshot {
            entity: PlayerId(1),
            valid: interval(10, Some(20)),
            observed_at: Date(10),
            value: 7,
        }])
        .unwrap();
        assert_eq!(store.as_of(PlayerId(1), Date(19), Date(19)), Some(&7));
        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(20)), None);
    }

    #[test]
    fn later_revision_is_hidden_before_knowledge_cutoff() {
        let store = SnapshotStore::try_from_snapshots(vec![
            Snapshot {
                entity: PlayerId(1),
                valid: interval(10, Some(30)),
                observed_at: Date(10),
                value: 100,
            },
            Snapshot {
                entity: PlayerId(1),
                valid: interval(10, Some(30)),
                observed_at: Date(25),
                value: 120,
            },
        ])
        .unwrap();
        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(20)), Some(&100));
        assert_eq!(store.as_of(PlayerId(1), Date(20), Date(25)), Some(&120));
    }

    #[test]
    fn overlapping_snapshots_resolve_deterministically() {
        let store = SnapshotStore::try_from_snapshots(vec![
            Snapshot {
                entity: PlayerId(1),
                valid: interval(10, Some(40)),
                observed_at: Date(15),
                value: 100,
            },
            Snapshot {
                entity: PlayerId(1),
                valid: interval(20, Some(30)),
                observed_at: Date(15),
                value: 200,
            },
        ])
        .unwrap();
        assert_eq!(store.as_of(PlayerId(1), Date(25), Date(25)), Some(&200));
    }

    #[test]
    fn duplicate_snapshot_versions_are_rejected() {
        let result = SnapshotStore::try_from_snapshots(vec![
            Snapshot { entity: PlayerId(1), valid: interval(10, Some(20)), observed_at: Date(10), value: 1 },
            Snapshot { entity: PlayerId(1), valid: interval(10, Some(20)), observed_at: Date(10), value: 2 },
        ]);
        assert_eq!(result.unwrap_err(), TemporalError::DuplicateTemporalKey);
    }
}
