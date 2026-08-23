#![forbid(unsafe_code)]
//! Deterministic Monte Carlo and market-factor volatility primitives.

mod stats;
pub use stats::*;

use footy_core::{Confidence, EuroCents, Probability};
use footy_market::PriceDistribution;

/// Small deterministic SplitMix64 generator. This is for reproducible simulation, not cryptography.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    pub const fn new(seed: u64) -> Self { Self { state: seed } }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub fn uniform(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        bits as f64 * (1.0 / ((1_u64 << 53) as f64))
    }

    pub fn normal(&mut self) -> f64 {
        let u1 = self.uniform().max(f64::MIN_POSITIVE);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    pub fn bernoulli(&mut self, probability: f64) -> bool {
        self.uniform() < probability.clamp(0.0, 1.0)
    }
}

pub trait VolatilityModel {
    fn variance(&self) -> f64;
    fn update(&mut self, return_value: f64, long_run_signal: Option<f64>);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EwmaVolatility {
    lambda: f64,
    variance: f64,
}

impl EwmaVolatility {
    pub fn new(lambda: f64, initial_variance: f64) -> Option<Self> {
        (lambda.is_finite() && (0.0..1.0).contains(&lambda) && initial_variance.is_finite() && initial_variance > 0.0)
            .then_some(Self { lambda, variance: initial_variance })
    }
}

impl VolatilityModel for EwmaVolatility {
    fn variance(&self) -> f64 { self.variance }
    fn update(&mut self, return_value: f64, _long_run_signal: Option<f64>) {
        if return_value.is_finite() {
            self.variance = self.lambda * self.variance + (1.0 - self.lambda) * return_value.powi(2);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Garch11 {
    omega: f64,
    alpha: f64,
    beta: f64,
    variance: f64,
}

impl Garch11 {
    pub fn new(omega: f64, alpha: f64, beta: f64, initial_variance: f64) -> Option<Self> {
        (omega.is_finite() && omega > 0.0 && alpha.is_finite() && beta.is_finite() && alpha >= 0.0 && beta >= 0.0 && alpha + beta < 1.0 && initial_variance.is_finite() && initial_variance > 0.0)
            .then_some(Self { omega, alpha, beta, variance: initial_variance })
    }
}

impl VolatilityModel for Garch11 {
    fn variance(&self) -> f64 { self.variance }
    fn update(&mut self, return_value: f64, _long_run_signal: Option<f64>) {
        if return_value.is_finite() {
            self.variance = (self.omega + self.alpha * return_value.powi(2) + self.beta * self.variance).max(f64::MIN_POSITIVE);
        }
    }
}

/// Two-frequency market-factor volatility primitive inspired by MF2-GARCH. The long component
/// is explicitly supplied/updated from a lower-frequency market signal; this type is intended for
/// dense transfer-market indices, not individual player prices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mf2Garch {
    short_alpha: f64,
    short_beta: f64,
    long_weight: f64,
    short_variance: f64,
    long_variance: f64,
}

impl Mf2Garch {
    pub fn new(short_alpha: f64, short_beta: f64, long_weight: f64, short_variance: f64, long_variance: f64) -> Option<Self> {
        (short_alpha.is_finite() && short_alpha >= 0.0 && short_beta.is_finite() && short_beta >= 0.0 && short_alpha + short_beta < 1.0 && long_weight.is_finite() && (0.0..=1.0).contains(&long_weight) && short_variance.is_finite() && short_variance > 0.0 && long_variance.is_finite() && long_variance > 0.0)
            .then_some(Self { short_alpha, short_beta, long_weight, short_variance, long_variance })
    }
}

impl VolatilityModel for Mf2Garch {
    fn variance(&self) -> f64 {
        (self.short_variance + self.long_variance).max(f64::MIN_POSITIVE)
    }

    fn update(&mut self, return_value: f64, long_run_signal: Option<f64>) {
        if return_value.is_finite() {
            let intercept = (1.0 - self.short_alpha - self.short_beta) * self.long_variance;
            self.short_variance = (intercept + self.short_alpha * return_value.powi(2) + self.short_beta * self.short_variance).max(f64::MIN_POSITIVE);
        }
        if let Some(signal) = long_run_signal.filter(|value| value.is_finite() && *value > 0.0) {
            self.long_variance = self.long_weight * self.long_variance + (1.0 - self.long_weight) * signal;
        }
    }
}

pub trait StochasticProcess {
    type State: Clone;

    fn step(&self, state: &Self::State, step_days: u16, rng: &mut Rng64) -> Self::State;
    fn terminal_price(&self, state: &Self::State) -> EuroCents;
    fn transferred(&self, _state: &Self::State) -> bool { false }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonteCarloConfig {
    pub horizon_days: u16,
    pub step_days: u16,
    pub paths: u32,
    pub seed: u64,
    pub downside_fraction: f64,
}

impl Default for MonteCarloConfig {
    fn default() -> Self {
        Self { horizon_days: 730, step_days: 30, paths: 1_000, seed: 42, downside_fraction: 0.5 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonteCarloSummary {
    pub price: PriceDistribution,
    pub mean_price: EuroCents,
    pub standard_error_cents: f64,
    pub transfer_probability: Probability,
    pub downside_probability: Probability,
    pub paths: u32,
}

pub fn monte_carlo<P: StochasticProcess>(
    process: &P,
    initial: &P::State,
    config: MonteCarloConfig,
) -> Option<MonteCarloSummary> {
    if config.paths == 0 || config.step_days == 0 || config.horizon_days == 0 || !config.downside_fraction.is_finite() || !(0.0..=1.0).contains(&config.downside_fraction) {
        return None;
    }
    let initial_price = process.terminal_price(initial);
    if initial_price.cents() < 0 { return None; }
    let mut rng = Rng64::new(config.seed);
    let mut prices = Vec::with_capacity(config.paths as usize);
    let mut transferred = 0_u32;
    let mut downside = 0_u32;
    for _ in 0..config.paths {
        let mut state = initial.clone();
        let mut elapsed = 0_u16;
        while elapsed < config.horizon_days {
            let remaining = config.horizon_days - elapsed;
            let step = config.step_days.min(remaining);
            state = process.step(&state, step, &mut rng);
            elapsed = elapsed.saturating_add(step);
        }
        let price = process.terminal_price(&state);
        if process.transferred(&state) { transferred = transferred.saturating_add(1); }
        if price.cents() as f64 <= initial_price.cents() as f64 * config.downside_fraction { downside = downside.saturating_add(1); }
        prices.push(price);
    }
    let confidence = Confidence::clamped((1.0 - (-(config.paths as f64) / 1_000.0).exp()) as f32);
    let distribution = PriceDistribution::from_samples(&prices, confidence)?;
    let mean = prices.iter().map(|value| value.cents() as f64).sum::<f64>() / prices.len() as f64;
    let variance = if prices.len() > 1 {
        prices.iter().map(|value| (value.cents() as f64 - mean).powi(2)).sum::<f64>() / (prices.len() - 1) as f64
    } else { 0.0 };
    Some(MonteCarloSummary {
        price: distribution,
        mean_price: EuroCents(mean.round().clamp(0.0, i64::MAX as f64) as i64),
        standard_error_cents: (variance / prices.len() as f64).sqrt(),
        transfer_probability: Probability::clamped(transferred as f32 / config.paths as f32),
        downside_probability: Probability::clamped(downside as f32 / config.paths as f32),
        paths: config.paths,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulatedPlayerState {
    pub ability_index: f64,
    pub performance_index: f64,
    pub availability: f64,
    pub contract_remaining_days: i32,
    pub market_log_factor: f64,
    pub price: EuroCents,
    pub transferred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootballPathParameters {
    pub annual_development_mean: f64,
    pub annual_development_sigma: f64,
    pub performance_reversion: f64,
    pub annual_performance_sigma: f64,
    pub daily_unavailability_probability: f64,
    pub unavailable_performance_factor: f64,
    pub annual_market_drift: f64,
    pub annual_market_sigma: f64,
    pub annual_transfer_hazard: f64,
    pub ability_value_elasticity: f64,
    pub performance_value_elasticity: f64,
    pub contract_value_decay_per_year: f64,
}

impl Default for FootballPathParameters {
    fn default() -> Self {
        Self {
            annual_development_mean: 0.0,
            annual_development_sigma: 0.05,
            performance_reversion: 0.4,
            annual_performance_sigma: 0.10,
            daily_unavailability_probability: 0.0005,
            unavailable_performance_factor: 0.4,
            annual_market_drift: 0.0,
            annual_market_sigma: 0.10,
            annual_transfer_hazard: 0.25,
            ability_value_elasticity: 1.0,
            performance_value_elasticity: 0.5,
            contract_value_decay_per_year: 0.05,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootballPathModel {
    pub parameters: FootballPathParameters,
}

impl StochasticProcess for FootballPathModel {
    type State = SimulatedPlayerState;

    fn step(&self, state: &Self::State, step_days: u16, rng: &mut Rng64) -> Self::State {
        let p = self.parameters;
        let dt = f64::from(step_days) / 365.2425;
        let sqrt_dt = dt.sqrt();

        // 1) Development changes underlying capability.
        let ability_delta = p.annual_development_mean * dt + p.annual_development_sigma.max(0.0) * sqrt_dt * rng.normal();
        let ability = (state.ability_index + ability_delta).max(0.0);

        // 2) Availability modifies realized usage/output opportunity.
        let daily_probability = p.daily_unavailability_probability.clamp(0.0, 1.0);
        let unavailable_probability = 1.0 - (1.0 - daily_probability).powf(f64::from(step_days));
        let unavailable = rng.bernoulli(unavailable_probability);
        let availability = if unavailable { p.unavailable_performance_factor.clamp(0.0, 1.0) } else { 1.0 };

        // 3) Performance mean-reverts toward available capability plus idiosyncratic noise.
        let target_performance = ability * availability;
        let reversion = (p.performance_reversion.max(0.0) * dt).clamp(0.0, 1.0);
        let performance = (state.performance_index + reversion * (target_performance - state.performance_index) + p.annual_performance_sigma.max(0.0) * sqrt_dt * rng.normal()).max(0.0);

        // 4) Contract and market states evolve.
        let contract_remaining_days = state.contract_remaining_days.saturating_sub(i32::from(step_days)).max(0);
        let market_delta = p.annual_market_drift * dt + p.annual_market_sigma.max(0.0) * sqrt_dt * rng.normal();
        let market_log_factor = state.market_log_factor + market_delta;

        // 5) Transfer event is sampled after the sporting/contract state for the period exists.
        let hazard = p.annual_transfer_hazard.max(0.0);
        let base_transfer_probability = 1.0 - (-hazard * dt).exp();
        let contract_boost = if contract_remaining_days <= 365 { 1.5 } else { 1.0 };
        let transferred = state.transferred || rng.bernoulli((base_transfer_probability * contract_boost).clamp(0.0, 1.0));

        // 6) Value reacts to the causal driver changes, rather than being simulated directly.
        let performance_delta = performance - state.performance_index;
        let contract_decay = p.contract_value_decay_per_year.max(0.0) * dt;
        let log_price_change = p.ability_value_elasticity * ability_delta + p.performance_value_elasticity * performance_delta + market_delta - contract_decay;
        let price = scale_price(state.price, log_price_change);

        Self::State { ability_index: ability, performance_index: performance, availability, contract_remaining_days, market_log_factor, price, transferred }
    }

    fn terminal_price(&self, state: &Self::State) -> EuroCents { state.price }
    fn transferred(&self, state: &Self::State) -> bool { state.transferred }
}

fn scale_price(price: EuroCents, log_change: f64) -> EuroCents {
    if price.cents() <= 0 || !log_change.is_finite() { return EuroCents::ZERO; }
    let scaled = price.cents() as f64 * log_change.exp();
    EuroCents(scaled.round().clamp(0.0, i64::MAX as f64) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_reproducible() {
        let mut first = Rng64::new(7);
        let mut second = Rng64::new(7);
        for _ in 0..20 { assert_eq!(first.next_u64(), second.next_u64()); }
    }

    #[test]
    fn volatility_models_remain_positive() {
        let mut garch = Garch11::new(0.01, 0.1, 0.8, 0.2).unwrap();
        for value in [0.1, -0.2, 0.3, -0.05] { garch.update(value, None); }
        assert!(garch.variance() > 0.0);
        let mut mf2 = Mf2Garch::new(0.1, 0.8, 0.9, 0.1, 0.2).unwrap();
        mf2.update(0.1, Some(0.25));
        assert!(mf2.variance() > 0.0);
    }

    #[test]
    fn monte_carlo_is_deterministic_for_fixed_seed() {
        let process = FootballPathModel { parameters: FootballPathParameters::default() };
        let initial = SimulatedPlayerState { ability_index: 1.0, performance_index: 1.0, availability: 1.0, contract_remaining_days: 1_095, market_log_factor: 0.0, price: EuroCents(1_000_000), transferred: false };
        let config = MonteCarloConfig { paths: 200, horizon_days: 365, step_days: 30, seed: 99, ..MonteCarloConfig::default() };
        let first = monte_carlo(&process, &initial, config).unwrap();
        let second = monte_carlo(&process, &initial, config).unwrap();
        assert_eq!(first, second);
        assert!(first.price.p10 <= first.price.p50 && first.price.p50 <= first.price.p90);
    }

    #[test]
    fn development_changes_value_through_driver_path() {
        let process = FootballPathModel { parameters: FootballPathParameters { annual_development_mean: 0.2, annual_development_sigma: 0.0, annual_performance_sigma: 0.0, annual_market_sigma: 0.0, annual_transfer_hazard: 0.0, daily_unavailability_probability: 0.0, contract_value_decay_per_year: 0.0, ..FootballPathParameters::default() } };
        let initial = SimulatedPlayerState { ability_index: 1.0, performance_index: 1.0, availability: 1.0, contract_remaining_days: 1_095, market_log_factor: 0.0, price: EuroCents(1_000_000), transferred: false };
        let mut rng = Rng64::new(1);
        let next = process.step(&initial, 365, &mut rng);
        assert!(next.ability_index > initial.ability_index);
        assert!(next.price > initial.price);
    }
}
