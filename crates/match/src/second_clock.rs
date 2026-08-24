use footy_forecast::Rng64;

pub const REGULATION_MATCH_SECONDS: u32 = 90 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MatchSecond(pub u32);

impl MatchSecond {
    pub const KICKOFF: Self = Self(0);
    pub const REGULATION_END: Self = Self(REGULATION_MATCH_SECONDS);

    pub fn from_minute(minute: f64) -> Option<Self> {
        if !minute.is_finite() || minute < 0.0 { return None; }
        let seconds = (minute * 60.0).round();
        (seconds <= f64::from(u32::MAX)).then_some(Self(seconds as u32))
    }

    pub fn as_minutes(self) -> f64 { f64::from(self.0) / 60.0 }
}

/// Total ordering for events whose provider/model timestamp is only known to one-second precision.
/// `within_second` is a sampled latent ordering key, not a claim that the source observed a true
/// sub-second timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventTime {
    pub second: MatchSecond,
    pub within_second: u32,
}

impl EventTime {
    pub fn at_second(second: MatchSecond, rng: &mut Rng64) -> Self {
        Self { second, within_second: random_u32(rng) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondClockConfig {
    /// Regulation is 5,400 seconds. Added time is explicit because treating every fixture as exactly
    /// 90:00 creates a systematic tail bias once second-level ordering exists.
    pub regulation_seconds: u32,
    pub first_half_added_seconds: u16,
    pub second_half_added_seconds: u16,
    /// Hard guard against a pathological event cascade at the same second.
    pub max_events_same_second: u16,
}

impl Default for SecondClockConfig {
    fn default() -> Self {
        Self {
            regulation_seconds: REGULATION_MATCH_SECONDS,
            first_half_added_seconds: 0,
            second_half_added_seconds: 0,
            max_events_same_second: 64,
        }
    }
}

impl SecondClockConfig {
    pub fn validate(self) -> bool {
        self.regulation_seconds >= 2 * 45 * 60
            && self.regulation_seconds <= 3 * 60 * 60
            && self.first_half_added_seconds <= 20 * 60
            && self.second_half_added_seconds <= 30 * 60
            && self.max_events_same_second > 0
    }

    pub fn first_half_end(self) -> MatchSecond {
        MatchSecond(self.regulation_seconds / 2 + u32::from(self.first_half_added_seconds))
    }

    pub fn match_end(self) -> MatchSecond {
        MatchSecond(self.regulation_seconds
            + u32::from(self.first_half_added_seconds)
            + u32::from(self.second_half_added_seconds))
    }
}

/// Convert a per-90 count intensity to a one-second Poisson intensity.
pub fn per90_rate_to_per_second(rate_per90: f64) -> Option<f64> {
    (rate_per90.is_finite() && rate_per90 >= 0.0)
        .then_some(rate_per90 / f64::from(REGULATION_MATCH_SECONDS))
}

/// Convert a per-90 probability into a one-second conditional hazard while preserving the exact
/// 90-minute survival probability.
pub fn per90_probability_to_per_second(probability_per90: f64) -> Option<f64> {
    if !probability_per90.is_finite() || !(0.0..=1.0).contains(&probability_per90) { return None; }
    if probability_per90 <= 0.0 { return Some(0.0); }
    if probability_per90 >= 1.0 { return Some(1.0); }
    Some(1.0 - (1.0 - probability_per90).powf(1.0 / f64::from(REGULATION_MATCH_SECONDS)))
}

/// Sample the next integer match second from a constant per-second hazard. The returned timestamp
/// is strictly after `now`; callers recompute hazards immediately after every state-changing event.
pub fn sample_waiting_second(
    now: MatchSecond,
    end: MatchSecond,
    hazard_per_second: f64,
    rng: &mut Rng64,
) -> Option<MatchSecond> {
    if !hazard_per_second.is_finite() || hazard_per_second <= 0.0 || now >= end { return None; }
    let p = hazard_per_second.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let u = rng.uniform().clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let wait = (u.ln() / (1.0 - p).ln()).floor().max(0.0) as u32 + 1;
    let second = now.0.saturating_add(wait);
    (second <= end.0).then_some(MatchSecond(second))
}

/// Exact time-scaling for a discrete mean-reversion fraction calibrated over `reference_seconds`.
/// This prevents a 15-minute coefficient from being applied 900 times when the clock is refined to
/// one-second resolution.
pub fn scaled_reversion_fraction(reference_fraction: f64, elapsed_seconds: u32, reference_seconds: u32) -> Option<f64> {
    if !reference_fraction.is_finite() || !(0.0..=1.0).contains(&reference_fraction)
        || reference_seconds == 0
    { return None; }
    if elapsed_seconds == 0 || reference_fraction == 0.0 { return Some(0.0); }
    if reference_fraction == 1.0 { return Some(1.0); }
    Some(1.0 - (1.0 - reference_fraction).powf(f64::from(elapsed_seconds) / f64::from(reference_seconds)))
}

/// Brownian/innovation sigma scales with sqrt(time), not linearly with the number of one-second
/// updates. This avoids exploding psychological volatility when moving from coarse segments to a
/// second-resolution clock.
pub fn scaled_innovation_sigma(reference_sigma: f64, elapsed_seconds: u32, reference_seconds: u32) -> Option<f64> {
    if !reference_sigma.is_finite() || reference_sigma < 0.0 || reference_seconds == 0 { return None; }
    Some(reference_sigma * (f64::from(elapsed_seconds) / f64::from(reference_seconds)).sqrt())
}

/// Same scaling for convex-combination/blending coefficients such as active-lineup social pull.
pub fn scaled_blend_fraction(reference_fraction: f64, elapsed_seconds: u32, reference_seconds: u32) -> Option<f64> {
    scaled_reversion_fraction(reference_fraction, elapsed_seconds, reference_seconds)
}

fn random_u32(rng: &mut Rng64) -> u32 {
    (rng.uniform() * (f64::from(u32::MAX) + 1.0)).floor().min(f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per90_probability_round_trips_through_5400_seconds() {
        let p90 = 0.12;
        let p1 = per90_probability_to_per_second(p90).unwrap();
        let rebuilt = 1.0 - (1.0 - p1).powf(f64::from(REGULATION_MATCH_SECONDS));
        assert!((rebuilt - p90).abs() < 1e-10);
    }

    #[test]
    fn second_level_reversion_matches_reference_horizon() {
        let reference = 0.08;
        let per_second = scaled_reversion_fraction(reference, 1, 15 * 60).unwrap();
        let rebuilt = 1.0 - (1.0 - per_second).powf(900.0);
        assert!((rebuilt - reference).abs() < 1e-10);
    }

    #[test]
    fn innovation_scales_with_square_root_of_time() {
        let sigma = 0.03;
        let per_second = scaled_innovation_sigma(sigma, 1, 900).unwrap();
        assert!((per_second * 900.0_f64.sqrt() - sigma).abs() < 1e-12);
    }

    #[test]
    fn same_second_events_have_reproducible_latent_order() {
        let mut first = Rng64::new(42);
        let mut second = Rng64::new(42);
        let a = EventTime::at_second(MatchSecond(100), &mut first);
        let b = EventTime::at_second(MatchSecond(100), &mut second);
        assert_eq!(a, b);
    }
}
