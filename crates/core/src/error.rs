use core::fmt;

/// Errors raised by dependency-free core primitives.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    InvalidDate { year: i32, month: u8, day: u8 },
    DateOverflow,
    MoneyOverflow,
    ProbabilityOutOfRange(f32),
    ConfidenceOutOfRange(f32),
    StringTableFull,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDate { year, month, day } => {
                write!(f, "invalid calendar date: {year:04}-{month:02}-{day:02}")
            }
            Self::DateOverflow => f.write_str("date is outside the supported i32 day range"),
            Self::MoneyOverflow => f.write_str("money arithmetic overflow"),
            Self::ProbabilityOutOfRange(value) => {
                write!(f, "probability must be finite and in [0, 1], got {value}")
            }
            Self::ConfidenceOutOfRange(value) => {
                write!(f, "confidence must be finite and in [0, 1], got {value}")
            }
            Self::StringTableFull => f.write_str("string interner exhausted u32 identifiers"),
        }
    }
}

impl std::error::Error for CoreError {}
