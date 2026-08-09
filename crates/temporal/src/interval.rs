use core::fmt;
use footy_core::Date;

/// Half-open validity interval `[start, end_exclusive)`. `None` means unbounded future.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interval {
    pub start: Date,
    pub end_exclusive: Option<Date>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalError {
    EmptyOrReversedInterval { start: Date, end_exclusive: Date },
    DuplicateTemporalKey,
}

impl Interval {
    pub fn new(start: Date, end_exclusive: Option<Date>) -> Result<Self, TemporalError> {
        if let Some(end) = end_exclusive {
            if end <= start {
                return Err(TemporalError::EmptyOrReversedInterval {
                    start,
                    end_exclusive: end,
                });
            }
        }
        Ok(Self {
            start,
            end_exclusive,
        })
    }

    #[inline]
    pub fn open_ended(start: Date) -> Self {
        Self {
            start,
            end_exclusive: None,
        }
    }

    #[inline]
    pub fn contains(self, date: Date) -> bool {
        date >= self.start && self.end_exclusive.map_or(true, |end| date < end)
    }

    pub fn overlaps(self, other: Self) -> bool {
        let self_before_other = self.end_exclusive.is_some_and(|end| end <= other.start);
        let other_before_self = other.end_exclusive.is_some_and(|end| end <= self.start);
        !(self_before_other || other_before_self)
    }
}

impl fmt::Display for TemporalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOrReversedInterval { start, end_exclusive } => write!(
                f,
                "validity interval must have end > start, got {:?}..{:?}",
                start, end_exclusive
            ),
            Self::DuplicateTemporalKey => {
                f.write_str("duplicate entity/effective-or-valid/observed temporal key")
            }
        }
    }
}

impl std::error::Error for TemporalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_are_half_open() {
        let interval = Interval::new(Date(10), Some(Date(20))).unwrap();
        assert!(interval.contains(Date(10)));
        assert!(interval.contains(Date(19)));
        assert!(!interval.contains(Date(20)));
    }

    #[test]
    fn adjacent_intervals_do_not_overlap() {
        let first = Interval::new(Date(10), Some(Date(20))).unwrap();
        let second = Interval::new(Date(20), Some(Date(30))).unwrap();
        assert!(!first.overlaps(second));
    }
}
