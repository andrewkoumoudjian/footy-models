use crate::CoreError;

/// Canonical euro amount in integer cents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EuroCents(pub i64);

impl EuroCents {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub const fn cents(self) -> i64 {
        self.0
    }

    pub fn from_whole_euros(euros: i64) -> Result<Self, CoreError> {
        euros
            .checked_mul(100)
            .map(Self)
            .ok_or(CoreError::MoneyOverflow)
    }

    pub fn checked_add(self, other: Self) -> Result<Self, CoreError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(CoreError::MoneyOverflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, CoreError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(CoreError::MoneyOverflow)
    }

    pub fn checked_mul(self, factor: i64) -> Result<Self, CoreError> {
        self.0
            .checked_mul(factor)
            .map(Self)
            .ok_or(CoreError::MoneyOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_euros_are_exact() {
        assert_eq!(EuroCents::from_whole_euros(15).unwrap(), EuroCents(1_500));
    }

    #[test]
    fn detects_overflow() {
        assert!(EuroCents(i64::MAX).checked_add(EuroCents(1)).is_err());
    }
}
