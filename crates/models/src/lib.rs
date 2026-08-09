#![forbid(unsafe_code)]
//! Dependency-light generic statistical algorithms used by football domain crates.

use footy_core::{DatasetVersion, Date, FeatureSetVersion, ModelVersion};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum FitError {
    EmptyData,
    DimensionMismatch,
    NonFiniteInput,
    SingularSystem,
    InvalidParameter(&'static str),
}

impl fmt::Display for FitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FitError {}

#[derive(Debug, Clone, PartialEq)]
pub struct DenseMatrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl DenseMatrix {
    pub fn new(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self, FitError> {
        if rows == 0 || cols == 0 {
            return Err(FitError::EmptyData);
        }
        if rows.checked_mul(cols) != Some(data.len()) {
            return Err(FitError::DimensionMismatch);
        }
        if !data.iter().all(|value| value.is_finite()) {
            return Err(FitError::NonFiniteInput);
        }
        Ok(Self { rows, cols, data })
    }

    pub const fn rows(&self) -> usize { self.rows }
    pub const fn cols(&self) -> usize { self.cols }
    pub fn row(&self, row: usize) -> &[f64] {
        let start = row * self.cols;
        &self.data[start..start + self.cols]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearModel {
    /// Intercept first, followed by feature coefficients.
    pub coefficients: Vec<f64>,
}

impl LinearModel {
    pub fn predict(&self, features: &[f64]) -> Result<f64, FitError> {
        if self.coefficients.len() != features.len() + 1 || !features.iter().all(|value| value.is_finite()) {
            return Err(FitError::DimensionMismatch);
        }
        Ok(self.coefficients[0] + self.coefficients[1..].iter().zip(features).map(|(coefficient, value)| coefficient * value).sum::<f64>())
    }
}

pub fn fit_weighted_linear(
    x: &DenseMatrix,
    y: &[f64],
    weights: Option<&[f64]>,
    ridge: f64,
) -> Result<LinearModel, FitError> {
    if y.len() != x.rows() || y.is_empty() {
        return Err(FitError::DimensionMismatch);
    }
    if !y.iter().all(|value| value.is_finite()) || !ridge.is_finite() || ridge < 0.0 {
        return Err(FitError::NonFiniteInput);
    }
    if let Some(weights) = weights {
        if weights.len() != y.len() || !weights.iter().all(|weight| weight.is_finite() && *weight >= 0.0) {
            return Err(FitError::DimensionMismatch);
        }
    }

    let parameters = x.cols() + 1;
    let mut normal = vec![0.0; parameters * parameters];
    let mut rhs = vec![0.0; parameters];
    for row_index in 0..x.rows() {
        let weight = weights.map_or(1.0, |values| values[row_index]);
        if weight == 0.0 { continue; }
        let row = x.row(row_index);
        for i in 0..parameters {
            let xi = if i == 0 { 1.0 } else { row[i - 1] };
            rhs[i] += weight * xi * y[row_index];
            for j in 0..parameters {
                let xj = if j == 0 { 1.0 } else { row[j - 1] };
                normal[i * parameters + j] += weight * xi * xj;
            }
        }
    }
    for index in 1..parameters {
        normal[index * parameters + index] += ridge;
    }
    let coefficients = solve_linear_system(normal, rhs, parameters)?;
    Ok(LinearModel { coefficients })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HuberConfig {
    pub delta: f64,
    pub max_iterations: u32,
    pub tolerance: f64,
    pub ridge: f64,
}

impl Default for HuberConfig {
    fn default() -> Self {
        Self { delta: 1.345, max_iterations: 50, tolerance: 1e-8, ridge: 1e-8 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HuberFit {
    pub model: LinearModel,
    pub iterations: u32,
    pub residual_scale: f64,
}

pub fn fit_huber(
    x: &DenseMatrix,
    y: &[f64],
    base_weights: Option<&[f64]>,
    config: HuberConfig,
) -> Result<HuberFit, FitError> {
    if !config.delta.is_finite() || config.delta <= 0.0 || !config.tolerance.is_finite() || config.tolerance <= 0.0 || !config.ridge.is_finite() || config.ridge < 0.0 {
        return Err(FitError::InvalidParameter("invalid Huber configuration"));
    }
    let base = match base_weights {
        Some(weights) if weights.len() == y.len() && weights.iter().all(|weight| weight.is_finite() && *weight >= 0.0) => weights.to_vec(),
        Some(_) => return Err(FitError::DimensionMismatch),
        None => vec![1.0; y.len()],
    };
    let mut weights = base.clone();
    let mut model = fit_weighted_linear(x, y, Some(&weights), config.ridge)?;
    let mut scale = 1.0;

    for iteration in 1..=config.max_iterations.max(1) {
        let residuals = residuals(x, y, &model)?;
        let absolute: Vec<f64> = residuals.iter().map(|value| value.abs()).collect();
        scale = (1.4826 * median(&absolute).unwrap_or(0.0)).max(1e-9);
        let threshold = config.delta * scale;
        for (index, residual) in residuals.iter().enumerate() {
            let robust_weight = if residual.abs() <= threshold { 1.0 } else { threshold / residual.abs() };
            weights[index] = base[index] * robust_weight;
        }
        let next = fit_weighted_linear(x, y, Some(&weights), config.ridge)?;
        let max_change = model.coefficients.iter().zip(&next.coefficients).map(|(left, right)| (left - right).abs()).fold(0.0_f64, f64::max);
        model = next;
        if max_change <= config.tolerance {
            return Ok(HuberFit { model, iterations: iteration, residual_scale: scale });
        }
    }
    Ok(HuberFit { model, iterations: config.max_iterations.max(1), residual_scale: scale })
}

fn residuals(x: &DenseMatrix, y: &[f64], model: &LinearModel) -> Result<Vec<f64>, FitError> {
    if y.len() != x.rows() { return Err(FitError::DimensionMismatch); }
    (0..x.rows()).map(|row| Ok(y[row] - model.predict(x.row(row))?)).collect()
}

fn solve_linear_system(mut matrix: Vec<f64>, mut rhs: Vec<f64>, n: usize) -> Result<Vec<f64>, FitError> {
    for pivot in 0..n {
        let mut best = pivot;
        let mut best_value = matrix[pivot * n + pivot].abs();
        for row in pivot + 1..n {
            let value = matrix[row * n + pivot].abs();
            if value > best_value { best = row; best_value = value; }
        }
        if best_value <= 1e-14 { return Err(FitError::SingularSystem); }
        if best != pivot {
            for column in 0..n { matrix.swap(pivot * n + column, best * n + column); }
            rhs.swap(pivot, best);
        }
        let diagonal = matrix[pivot * n + pivot];
        for column in pivot..n { matrix[pivot * n + column] /= diagonal; }
        rhs[pivot] /= diagonal;
        for row in 0..n {
            if row == pivot { continue; }
            let factor = matrix[row * n + pivot];
            if factor == 0.0 { continue; }
            for column in pivot..n { matrix[row * n + column] -= factor * matrix[pivot * n + column]; }
            rhs[row] -= factor * rhs[pivot];
        }
    }
    Ok(rhs)
}

pub fn quantile(values: &[f64], probability: f64) -> Option<f64> {
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) { return None; }
    let mut values: Vec<f64> = values.iter().copied().filter(|value| value.is_finite()).collect();
    if values.is_empty() { return None; }
    values.sort_by(f64::total_cmp);
    let index = probability * (values.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    if lower == upper { Some(values[lower]) } else {
        let fraction = index - lower as f64;
        Some(values[lower] * (1.0 - fraction) + values[upper] * fraction)
    }
}

pub fn median(values: &[f64]) -> Option<f64> { quantile(values, 0.5) }

pub fn mad(values: &[f64]) -> Option<f64> {
    let center = median(values)?;
    let deviations: Vec<f64> = values.iter().copied().filter(|value| value.is_finite()).map(|value| (value - center).abs()).collect();
    median(&deviations)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConformalCalibrator {
    pub radius: f64,
    pub coverage: f64,
    pub calibration_size: u32,
}

impl ConformalCalibrator {
    pub fn fit(residuals: &[f64], coverage: f64) -> Result<Self, FitError> {
        if !coverage.is_finite() || coverage <= 0.0 || coverage >= 1.0 || residuals.is_empty() {
            return Err(FitError::InvalidParameter("coverage must be inside (0, 1) and residuals non-empty"));
        }
        let mut absolute: Vec<f64> = residuals.iter().copied().filter(|value| value.is_finite()).map(f64::abs).collect();
        if absolute.is_empty() { return Err(FitError::NonFiniteInput); }
        absolute.sort_by(f64::total_cmp);
        let rank = (((absolute.len() + 1) as f64) * coverage).ceil() as usize;
        let index = rank.saturating_sub(1).min(absolute.len() - 1);
        Ok(Self { radius: absolute[index], coverage, calibration_size: absolute.len().min(u32::MAX as usize) as u32 })
    }

    pub fn interval(self, prediction: f64) -> (f64, f64) {
        (prediction - self.radius, prediction + self.radius)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogisticModel {
    /// Intercept first, followed by feature coefficients.
    pub coefficients: Vec<f64>,
}

impl LogisticModel {
    pub fn predict_probability(&self, features: &[f64]) -> Result<f64, FitError> {
        if self.coefficients.len() != features.len() + 1 || !features.iter().all(|value| value.is_finite()) {
            return Err(FitError::DimensionMismatch);
        }
        let score = self.coefficients[0] + self.coefficients[1..].iter().zip(features).map(|(coefficient, value)| coefficient * value).sum::<f64>();
        Ok(sigmoid(score))
    }
}

pub fn fit_logistic(
    x: &DenseMatrix,
    y: &[f64],
    l2: f64,
    learning_rate: f64,
    iterations: u32,
) -> Result<LogisticModel, FitError> {
    if y.len() != x.rows() || y.is_empty() { return Err(FitError::DimensionMismatch); }
    if !y.iter().all(|value| value.is_finite() && (0.0..=1.0).contains(value)) || !l2.is_finite() || l2 < 0.0 || !learning_rate.is_finite() || learning_rate <= 0.0 {
        return Err(FitError::InvalidParameter("invalid logistic configuration"));
    }
    let mut coefficients = vec![0.0; x.cols() + 1];
    for _ in 0..iterations.max(1) {
        let mut gradient = vec![0.0; coefficients.len()];
        for row_index in 0..x.rows() {
            let row = x.row(row_index);
            let score = coefficients[0] + coefficients[1..].iter().zip(row).map(|(coefficient, value)| coefficient * value).sum::<f64>();
            let error = sigmoid(score) - y[row_index];
            gradient[0] += error;
            for feature in 0..x.cols() { gradient[feature + 1] += error * row[feature]; }
        }
        let n = x.rows() as f64;
        coefficients[0] -= learning_rate * gradient[0] / n;
        for index in 1..coefficients.len() {
            coefficients[index] -= learning_rate * (gradient[index] / n + l2 * coefficients[index]);
        }
    }
    Ok(LogisticModel { coefficients })
}

fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exponential = value.exp();
        exponential / (1.0 + exponential)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelMetadata {
    pub trained_through: Date,
    pub model_version: ModelVersion,
    pub dataset_version: DatasetVersion,
    pub feature_set_version: FeatureSetVersion,
    pub seed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_linear_recovers_simple_relation() {
        let x = DenseMatrix::new(4, 1, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
        let model = fit_weighted_linear(&x, &[1.0, 3.0, 5.0, 7.0], None, 1e-12).unwrap();
        assert!((model.coefficients[0] - 1.0).abs() < 1e-6);
        assert!((model.coefficients[1] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn huber_reduces_outlier_influence() {
        let x = DenseMatrix::new(5, 1, vec![0.0, 1.0, 2.0, 3.0, 4.0]).unwrap();
        let fit = fit_huber(&x, &[1.0, 3.0, 5.0, 7.0, 100.0], None, HuberConfig::default()).unwrap();
        assert!(fit.model.coefficients[1] < 10.0);
    }

    #[test]
    fn conformal_interval_uses_empirical_residual_quantile() {
        let calibrator = ConformalCalibrator::fit(&[-1.0, 0.5, 2.0, -0.25], 0.8).unwrap();
        let interval = calibrator.interval(10.0);
        assert!(interval.0 <= 8.0 && interval.1 >= 12.0);
    }

    #[test]
    fn logistic_separates_simple_data() {
        let x = DenseMatrix::new(4, 1, vec![-2.0, -1.0, 1.0, 2.0]).unwrap();
        let model = fit_logistic(&x, &[0.0, 0.0, 1.0, 1.0], 0.0, 0.2, 200).unwrap();
        assert!(model.predict_probability(&[2.0]).unwrap() > 0.8);
        assert!(model.predict_probability(&[-2.0]).unwrap() < 0.2);
    }
}
