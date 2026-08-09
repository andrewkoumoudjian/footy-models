use crate::CoreError;

macro_rules! bounded_unit_float {
    ($name:ident, $error:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(f32);

        impl $name {
            pub const ZERO: Self = Self(0.0);
            pub const ONE: Self = Self(1.0);

            pub fn new(value: f32) -> Result<Self, CoreError> {
                if value.is_finite() && (0.0..=1.0).contains(&value) {
                    Ok(Self(value))
                } else {
                    Err(CoreError::$error(value))
                }
            }

            #[inline]
            pub fn clamped(value: f32) -> Self {
                if value.is_nan() {
                    Self::ZERO
                } else {
                    Self(value.clamp(0.0, 1.0))
                }
            }

            #[inline]
            pub const fn get(self) -> f32 {
                self.0
            }
        }

        impl TryFrom<f32> for $name {
            type Error = CoreError;

            fn try_from(value: f32) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for f32 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }
    };
}

bounded_unit_float!(Probability, ProbabilityOutOfRange);
bounded_unit_float!(Confidence, ConfidenceOutOfRange);

/// Explicit observation/missingness state. A missing metric is never equivalent to zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationState {
    Measured,
    NotApplicable,
    SourceUnavailable,
    PlayerUnobserved,
    LowSample,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_values_reject_invalid_inputs() {
        assert!(Probability::new(-0.01).is_err());
        assert!(Probability::new(1.01).is_err());
        assert!(Probability::new(f32::NAN).is_err());
        assert_eq!(Confidence::new(0.75).unwrap().get(), 0.75);
    }
}
