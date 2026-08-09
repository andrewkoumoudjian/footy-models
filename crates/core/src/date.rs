use crate::CoreError;

/// A timezone-free civil date represented as days since 1970-01-01.
///
/// `Date` deliberately has day precision. Time-of-day and timezone semantics belong
/// at source boundaries and must be resolved before constructing this type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Date(pub i32);

impl Date {
    pub const UNIX_EPOCH: Self = Self(0);

    /// Construct a date from a Gregorian calendar date.
    pub fn from_ymd(year: i32, month: u8, day: u8) -> Result<Self, CoreError> {
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return Err(CoreError::InvalidDate { year, month, day });
        }

        let days = days_from_civil(year, month, day);
        let days = i32::try_from(days).map_err(|_| CoreError::DateOverflow)?;
        Ok(Self(days))
    }

    /// Convert to `(year, month, day)` in the proleptic Gregorian calendar.
    pub fn to_ymd(self) -> (i32, u8, u8) {
        civil_from_days(i64::from(self.0))
    }

    #[inline]
    pub const fn days_since_unix_epoch(self) -> i32 {
        self.0
    }

    pub fn checked_add_days(self, days: i32) -> Result<Self, CoreError> {
        self.0
            .checked_add(days)
            .map(Self)
            .ok_or(CoreError::DateOverflow)
    }

    pub fn checked_sub_days(self, days: i32) -> Result<Self, CoreError> {
        self.0
            .checked_sub(days)
            .map(Self)
            .ok_or(CoreError::DateOverflow)
    }

    /// Signed number of whole days from `earlier` to `self`.
    #[inline]
    pub fn days_since(self, earlier: Self) -> i64 {
        i64::from(self.0) - i64::from(earlier.0)
    }
}

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

// Howard Hinnant's civil-calendar algorithms, adapted to a 1970-01-01 epoch.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    y -= if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += if m <= 2 { 1 } else { 0 };
    (y as i32, m as u8, d as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_round_trips() {
        let epoch = Date::from_ymd(1970, 1, 1).unwrap();
        assert_eq!(epoch, Date::UNIX_EPOCH);
        assert_eq!(epoch.to_ymd(), (1970, 1, 1));
    }

    #[test]
    fn leap_day_round_trips() {
        let date = Date::from_ymd(2024, 2, 29).unwrap();
        assert_eq!(date.to_ymd(), (2024, 2, 29));
        assert!(Date::from_ymd(2023, 2, 29).is_err());
    }

    #[test]
    fn supports_pre_epoch_dates() {
        let date = Date::from_ymd(1960, 6, 7).unwrap();
        assert!(date.days_since_unix_epoch() < 0);
        assert_eq!(date.to_ymd(), (1960, 6, 7));
    }

    #[test]
    fn checked_arithmetic_detects_overflow() {
        assert!(Date(i32::MAX).checked_add_days(1).is_err());
        assert!(Date(i32::MIN).checked_sub_days(1).is_err());
    }
}
