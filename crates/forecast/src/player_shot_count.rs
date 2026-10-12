//! Minimal exposure-offset Poisson shot-rate challenger.
//!
//! The model conditions on player minutes for fitting, but consumers must
//! integrate over an **as-of** participation/minutes distribution to predict
//! next-match unconditional counts. Actual target-match minutes are labels,
//! never inputs at forecast time.

const WIDTH: usize = 6;

/// Fixed, prior-specified role intensity; not fitted on the evaluation window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotRole { Goalkeeper, Defender, Midfielder, Forward, Unknown }

impl ShotRole {
    pub fn prior_per90(self) -> f64 {
        match self {
            Self::Goalkeeper => 0.02,
            Self::Defender => 0.40,
            Self::Midfielder => 0.90,
            Self::Forward => 2.20,
            Self::Unknown => 0.95,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotRateFeatures {
    pub role: ShotRole,
    pub prior_shots: f64,
    pub prior_minutes: f64,
}

impl ShotRateFeatures {
    pub fn validate(self) -> bool {
        [self.prior_shots, self.prior_minutes].iter().all(|v| v.is_finite() && *v >= 0.0)
            && (self.prior_minutes > 0.0 || self.prior_shots == 0.0)
    }

    /// Gamma-Poisson posterior-mean rate, with five full-match prior exposures.
    pub fn empirical_per90(self) -> Option<f64> {
        self.validate().then(|| {
            (self.prior_shots + 5.0 * self.role.prior_per90()) / (self.prior_minutes / 90.0 + 5.0)
        })
    }

    fn vector(self) -> Option<[f64; WIDTH]> {
        let rate = self.empirical_per90()?;
        Some([
            1.0,
            rate.max(1e-8).ln(),
            (self.prior_minutes / 90.0).ln_1p(),
            f64::from(self.role == ShotRole::Defender),
            f64::from(self.role == ShotRole::Midfielder),
            f64::from(self.role == ShotRole::Forward),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservedShotCount {
    pub features: ShotRateFeatures,
    pub shots: u32,
    pub minutes: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExposurePoissonShotModel {
    coefficients: [f64; WIDTH],
    pub observations: usize,
}

impl ExposurePoissonShotModel {
    /// Deterministic Newton-IRLS with quadratic regularization around the
    /// empirical-Bayes log-rate predictor and a backtracking line search.
    pub fn fit(samples: &[ObservedShotCount], ridge: f64) -> Option<Self> {
        if samples.len() < 30 || !ridge.is_finite() || ridge < 0.0 { return None; }
        let mut design = Vec::with_capacity(samples.len());
        for sample in samples {
            if !sample.minutes.is_finite() || sample.minutes <= 0.0 || sample.minutes > 130.0 { return None; }
            design.push((sample.features.vector()?, f64::from(sample.shots), sample.minutes / 90.0));
        }
        let center = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let mut beta = center;
        let objective = |b: &[f64; WIDTH]| -> f64 {
            let likelihood: f64 = design.iter().map(|(x, count, exposure)| {
                let rate = linear(b, x).clamp(-12.0, 6.0).exp();
                count * rate.ln() - exposure * rate
            }).sum();
            let penalty: f64 = (1..WIDTH).map(|j| (b[j] - center[j]).powi(2)).sum();
            likelihood - 0.5 * ridge * penalty
        };
        for _ in 0..40 {
            let mut gradient = [0.0; WIDTH];
            let mut hessian = [[0.0; WIDTH]; WIDTH];
            for (x, count, exposure) in &design {
                let mu = exposure * linear(&beta, x).clamp(-12.0, 6.0).exp();
                for j in 0..WIDTH {
                    gradient[j] += (count - mu) * x[j];
                    for k in 0..WIDTH { hessian[j][k] += mu * x[j] * x[k]; }
                }
            }
            for j in 0..WIDTH {
                let penalty = if j == 0 { 1e-6 } else { ridge };
                gradient[j] -= penalty * (beta[j] - center[j]);
                hessian[j][j] += penalty + 1e-8;
            }
            let step = solve(hessian, gradient)?;
            let old = objective(&beta);
            let mut scale = 1.0_f64;
            let mut accepted = false;
            while scale >= 1.0 / 65536.0 {
                let next = std::array::from_fn(|j| beta[j] + scale * step[j]);
                if objective(&next) >= old - 1e-9 {
                    beta = next;
                    accepted = true;
                    break;
                }
                scale /= 2.0;
            }
            if !accepted || step.iter().map(|v| (scale * v).abs()).fold(0.0, f64::max) < 1e-7 {
                break;
            }
        }
        if !beta.iter().all(|v| v.is_finite()) { return None; }
        Some(Self { coefficients: beta, observations: samples.len() })
    }

    pub fn rate_per90(&self, features: ShotRateFeatures) -> Option<f64> {
        Some(linear(&self.coefficients, &features.vector()?).clamp(-12.0, 6.0).exp())
    }
}

fn linear(beta: &[f64; WIDTH], x: &[f64; WIDTH]) -> f64 {
    (0..WIDTH).map(|j| beta[j] * x[j]).sum()
}

fn solve(mut a: [[f64; WIDTH]; WIDTH], mut b: [f64; WIDTH]) -> Option<[f64; WIDTH]> {
    for i in 0..WIDTH {
        let pivot = (i..WIDTH).max_by(|&left, &right| a[left][i].abs().total_cmp(&a[right][i].abs()))?;
        if a[pivot][i].abs() < 1e-12 { return None; }
        a.swap(i, pivot);
        b.swap(i, pivot);
        let inv = 1.0 / a[i][i];
        for j in i..WIDTH { a[i][j] *= inv; }
        b[i] *= inv;
        for row in 0..WIDTH {
            if row == i { continue; }
            let ratio = a[row][i];
            for col in i..WIDTH { a[row][col] -= ratio * a[i][col]; }
            b[row] -= ratio * b[i];
        }
    }
    b.iter().all(|value| value.is_finite()).then_some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empirical_rate_uses_actual_past_exposure() {
        let feature = ShotRateFeatures { role: ShotRole::Forward, prior_shots: 4.0, prior_minutes: 180.0 };
        assert!((feature.empirical_per90().unwrap() - 15.0 / 7.0).abs() < 1e-12);
        assert!(ShotRateFeatures { role: ShotRole::Forward, prior_shots: 1.0, prior_minutes: 0.0 }.empirical_per90().is_none());
    }

    #[test]
    fn offset_poisson_is_deterministic_and_minute_invariant() {
        let observations: Vec<_> = (0..120).map(|index| {
            let role = if index % 2 == 0 { ShotRole::Forward } else { ShotRole::Defender };
            ObservedShotCount {
                features: ShotRateFeatures { role, prior_shots: (index % 10) as f64, prior_minutes: 450.0 },
                shots: if role == ShotRole::Forward { 2 + (index % 4) as u32 } else { (index % 3 == 0) as u32 },
                minutes: if index % 3 == 0 { 45.0 } else { 90.0 },
            }
        }).collect();
        let first = ExposurePoissonShotModel::fit(&observations, 10.0).unwrap();
        let second = ExposurePoissonShotModel::fit(&observations, 10.0).unwrap();
        assert_eq!(first, second);
        let forward = ShotRateFeatures { role: ShotRole::Forward, prior_shots: 7.0, prior_minutes: 450.0 };
        assert!(first.rate_per90(forward).unwrap().is_finite());
        assert!(first.rate_per90(forward).unwrap() > 0.0);
    }
}
