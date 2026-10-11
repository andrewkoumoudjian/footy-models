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

/// Huber IRLS is robust to response/residual outliers. It is not a high-leverage
/// robust-regression method; callers should validate feature leverage separately.
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
        let center = median(&residuals).unwrap_or(0.0);
        let deviations: Vec<f64> = residuals.iter().map(|value| (value - center).abs()).collect();
        scale = (1.4826 * median(&deviations).unwrap_or(0.0)).max(1e-9);
        let threshold = config.delta * scale;
        for (index, residual) in residuals.iter().enumerate() {
            let centered = (residual - center).abs();
            let robust_weight = if centered <= threshold { 1.0 } else { threshold / centered };
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

/// Deterministic standardization + missingness encoding for raw feature rows.
///
/// For each raw feature, the mean and standard deviation are computed over the
/// non-missing training values. Missing values are encoded twice: the standardized
/// value slot is filled with `0.0` and a companion binary missingness indicator is
/// set to `1.0`. The transformed vector therefore has `2 * width` entries. The fit
/// is a pure function of the input rows: identical input produces identical output.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureTransform {
    pub means: Vec<f64>,
    pub stds: Vec<f64>,
    pub missing_rates: Vec<f64>,
}

impl FeatureTransform {
    pub fn fit(rows: &[Vec<f64>]) -> Result<Self, FitError> {
        if rows.is_empty() {
            return Err(FitError::EmptyData);
        }
        let width = rows[0].len();
        if width == 0 {
            return Err(FitError::DimensionMismatch);
        }
        if rows.iter().any(|row| row.len() != width) {
            return Err(FitError::DimensionMismatch);
        }
        let mut sums = vec![0.0; width];
        let mut counts = vec![0_u64; width];
        for row in rows {
            for (index, value) in row.iter().enumerate() {
                if value.is_finite() {
                    sums[index] += value;
                    counts[index] += 1;
                }
            }
        }
        let mut means = vec![0.0; width];
        for index in 0..width {
            means[index] = if counts[index] > 0 { sums[index] / counts[index] as f64 } else { 0.0 };
        }
        let mut squared = vec![0.0; width];
        for row in rows {
            for (index, value) in row.iter().enumerate() {
                if value.is_finite() {
                    let deviation = value - means[index];
                    squared[index] += deviation * deviation;
                }
            }
        }
        let mut stds = vec![1.0; width];
        for index in 0..width {
            if counts[index] > 1 {
                let variance = squared[index] / counts[index] as f64;
                stds[index] = variance.sqrt().max(1e-9);
            }
        }
        let missing_rates: Vec<f64> = (0..width)
            .map(|index| 1.0 - counts[index] as f64 / rows.len() as f64)
            .collect();
        Ok(Self { means, stds, missing_rates })
    }

    /// Number of raw features this transform accepts.
    pub fn width(&self) -> usize {
        self.means.len()
    }

    /// Transform one raw row into `2 * width` model features. `NaN` entries are
    /// treated as missing (standardized slot `0.0`, missingness indicator `1.0`).
    pub fn apply(&self, row: &[f64]) -> Result<Vec<f64>, FitError> {
        if row.len() != self.means.len() {
            return Err(FitError::DimensionMismatch);
        }
        let mut out = Vec::with_capacity(row.len() * 2);
        for (index, value) in row.iter().enumerate() {
            if value.is_finite() {
                out.push((value - self.means[index]) / self.stds[index]);
                out.push(0.0);
            } else {
                out.push(0.0);
                out.push(1.0);
            }
        }
        Ok(out)
    }
}

/// Newton-Raphson (IRLS) ridge logistic regression. Deterministic: the fit is a
/// pure function of the inputs with a fixed iteration budget and convergence test.
/// The ridge penalty is added to the feature diagonal of the Hessian (intercept is
/// unpenalized); a small floor keeps the system solvable when a column is constant.
pub fn fit_logistic_irls(
    x: &DenseMatrix,
    y: &[f64],
    l2: f64,
    max_iterations: u32,
) -> Result<LogisticModel, FitError> {
    if y.len() != x.rows() || y.is_empty() {
        return Err(FitError::DimensionMismatch);
    }
    if !y.iter().all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        || !l2.is_finite()
        || l2 < 0.0
    {
        return Err(FitError::InvalidParameter("invalid logistic configuration"));
    }
    let rows = x.rows();
    let features = x.cols();
    let parameters = features + 1;
    let mut coefficients = vec![0.0; parameters];
    let mut gradient = vec![0.0; parameters];
    let mut hessian = vec![0.0; parameters * parameters];
    for _ in 0..max_iterations.max(1) {
        gradient.fill(0.0);
        hessian.fill(0.0);
        for row_index in 0..rows {
            let row = x.row(row_index);
            let score = coefficients[0]
                + coefficients[1..].iter().zip(row).map(|(coefficient, value)| coefficient * value).sum::<f64>();
            let probability = sigmoid(score);
            let error = probability - y[row_index];
            let weight = probability * (1.0 - probability);
            gradient[0] += error;
            hessian[0] += weight;
            for feature in 0..features {
                let value = row[feature];
                gradient[feature + 1] += error * value;
                hessian[feature + 1] += weight * value;
                hessian[(feature + 1) * parameters] += weight * value;
                for other in 0..features {
                    hessian[(feature + 1) * parameters + other + 1] += weight * value * row[other];
                }
            }
        }
        for index in 0..parameters {
            let ridge = if index == 0 { 1e-12 } else { l2 + 1e-12 };
            hessian[index * parameters + index] += ridge;
        }
        let step = solve_linear_system(hessian.clone(), gradient.clone(), parameters)?;
        let mut max_change = 0.0_f64;
        for index in 0..parameters {
            coefficients[index] -= step[index];
            max_change = max_change.max(step[index].abs());
        }
        if max_change <= 1e-9 {
            break;
        }
    }
    Ok(LogisticModel { coefficients })
}

/// Ridge OLS on standardized raw features with paired missingness indicators.
///
/// Fits a `FeatureTransform` (mean/std on the raw rows, missing -> 0 slot + 1.0
/// indicator slot) and then solves the ridge normal equations in closed form on
/// the resulting `2 * width` design matrix. The whole fit is a pure function of
/// its inputs: identical rows and targets produce byte-identical coefficients,
/// and the transform statistics are part of the artifact so scoring never needs
/// the training data again.
#[derive(Debug, Clone, PartialEq)]
pub struct RidgeOls {
    pub transform: FeatureTransform,
    pub model: LinearModel,
}

impl RidgeOls {
    /// Fit ridge OLS on raw feature rows. `y` is the response (e.g. 12-month
    /// log value ratio). `ridge` penalizes all feature coefficients; the
    /// intercept is unpenalized (handled inside `fit_weighted_linear`).
    pub fn fit(rows: &[Vec<f64>], y: &[f64], ridge: f64) -> Result<Self, FitError> {
        if rows.is_empty() {
            return Err(FitError::EmptyData);
        }
        if y.len() != rows.len() {
            return Err(FitError::DimensionMismatch);
        }
        let transform = FeatureTransform::fit(rows)?;
        let width = transform.width();
        let mut data = Vec::with_capacity(rows.len() * width * 2);
        for row in rows {
            data.extend(transform.apply(row)?);
        }
        let x = DenseMatrix::new(rows.len(), width * 2, data)?;
        let model = fit_weighted_linear(&x, y, None, ridge)?;
        Ok(Self { transform, model })
    }

    /// Number of model inputs after standardization + missingness encoding.
    pub fn model_inputs(&self) -> usize {
        self.transform.width() * 2
    }

    /// Predict the response for one raw row (`NaN` = missing).
    pub fn predict(&self, row: &[f64]) -> Result<f64, FitError> {
        self.model.predict(&self.transform.apply(row)?)
    }
}


/// Pinball-loss (quantile) ridge regression on standardized raw features with
/// paired missingness indicators.
///
/// Minimizes `sum_i rho_tau(y_i - x_i'beta) + ridge * ||beta||^2` with the
/// pinball loss `rho_tau(u) = u * (tau - 1{u < 0})`, solved by deterministic
/// iteratively-reweighted least squares: at each iteration the asymmetric
/// weights are `tau / |r_i|` for positive residuals and `(1 - tau) / |r_i|`
/// for negative residuals (clamped to keep the system stable), and the next
/// iterate is the weighted ridge normal-equations solution. Identical inputs
/// produce byte-identical coefficients (fixed iteration budget + fixed
/// tolerance); the transform statistics are part of the returned fit.
pub fn fit_quantile_ridge(
    rows: &[Vec<f64>],
    y: &[f64],
    tau: f64,
    ridge: f64,
    max_iterations: u32,
    tolerance: f64,
) -> Result<RidgeOls, FitError> {
    if !tau.is_finite() || !(0.0..=1.0).contains(&tau) || tau == 0.0 || tau == 1.0 {
        return Err(FitError::InvalidParameter("tau must be strictly inside (0, 1)"));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(FitError::InvalidParameter("tolerance must be positive"));
    }
    fit_irls(rows, y, ridge, max_iterations.max(1), tolerance, |residual| {
        let side = if residual > 0.0 { tau } else { 1.0 - tau };
        (side / residual.abs()).clamp(1e-12, 1e9)
    })
}

/// Asymmetric-squared-loss (expectile) ridge regression on standardized raw
/// features with paired missingness indicators.
///
/// Minimizes `sum_i w_i * (y_i - x_i'beta)^2 + ridge * ||beta||^2` where
/// `w_i = 2*omega` for positive residuals and `2*(1-omega)` for negative
/// residuals. Solved by the same deterministic IRLS machinery as
/// [`fit_quantile_ridge`]; the weights are constant given the residual sign,
/// so the iteration converges quickly to the (unique) expectile fit.
pub fn fit_expectile_ridge(
    rows: &[Vec<f64>],
    y: &[f64],
    omega: f64,
    ridge: f64,
    max_iterations: u32,
    tolerance: f64,
) -> Result<RidgeOls, FitError> {
    if !omega.is_finite() || !(0.0..=1.0).contains(&omega) || omega == 0.0 || omega == 1.0 {
        return Err(FitError::InvalidParameter("omega must be strictly inside (0, 1)"));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(FitError::InvalidParameter("tolerance must be positive"));
    }
    fit_irls(rows, y, ridge, max_iterations.max(1), tolerance, |residual| {
        if residual > 0.0 { 2.0 * omega } else { 2.0 * (1.0 - omega) }
    })
}

/// Shared deterministic IRLS driver for asymmetric-loss ridge regression.
/// `weight_fn(residual)` returns the per-row IRLS weight for the current
/// residual. The iteration starts from the unweighted ridge (conditional-mean)
/// solution, which is a stable initializer for both the quantile and the
/// expectile problems.
fn fit_irls(
    rows: &[Vec<f64>],
    y: &[f64],
    ridge: f64,
    max_iterations: u32,
    tolerance: f64,
    weight_fn: impl Fn(f64) -> f64,
) -> Result<RidgeOls, FitError> {
    if rows.is_empty() {
        return Err(FitError::EmptyData);
    }
    if y.len() != rows.len() {
        return Err(FitError::DimensionMismatch);
    }
    if !y.iter().all(|value| value.is_finite()) || !ridge.is_finite() || ridge < 0.0 {
        return Err(FitError::NonFiniteInput);
    }
    let transform = FeatureTransform::fit(rows)?;
    let width = transform.width();
    let mut data = Vec::with_capacity(rows.len() * width * 2);
    for row in rows {
        data.extend(transform.apply(row)?);
    }
    let x = DenseMatrix::new(rows.len(), width * 2, data)?;
    let mut model = fit_weighted_linear(&x, y, None, ridge)?;
    for _ in 0..max_iterations {
        let mut weights = Vec::with_capacity(rows.len());
        for row_index in 0..rows.len() {
            let residual = y[row_index] - model.predict(x.row(row_index))?;
            weights.push(weight_fn(residual));
        }
        let next = fit_weighted_linear(&x, y, Some(&weights), ridge)?;
        let max_change = model
            .coefficients
            .iter()
            .zip(&next.coefficients)
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f64, f64::max);
        model = next;
        if max_change <= tolerance {
            break;
        }
    }
    Ok(RidgeOls { transform, model })
}

/// Area under the ROC curve via average-rank ties. Labels must be 0/1; non-finite
/// scores are ignored. Returns `None` when either class is absent.
pub fn auc(labels: &[f64], scores: &[f64]) -> Option<f64> {
    if labels.len() != scores.len() || labels.is_empty() {
        return None;
    }
    let mut pairs: Vec<(f64, f64)> = labels
        .iter()
        .zip(scores)
        .filter(|(label, score)| {
            label.is_finite() && (**label == 0.0 || **label == 1.0) && score.is_finite()
        })
        .map(|(label, score)| (*label, *score))
        .collect();
    if pairs.is_empty() {
        return None;
    }
    pairs.sort_by(|left, right| left.1.total_cmp(&right.1).then_with(|| left.0.total_cmp(&right.0)));
    let n = pairs.len();
    let mut ranks = vec![0.0_f64; n];
    let mut index = 0;
    while index < n {
        let mut end = index + 1;
        while end < n && pairs[end].1 == pairs[index].1 {
            end += 1;
        }
        let average_rank = (index + 1 + end) as f64 / 2.0;
        for position in index..end {
            ranks[position] = average_rank;
        }
        index = end;
    }
    let positives = pairs.iter().filter(|(label, _)| *label == 1.0).count();
    let negatives = n - positives;
    if positives == 0 || negatives == 0 {
        return None;
    }
    let rank_sum: f64 = pairs
        .iter()
        .zip(&ranks)
        .filter(|((label, _), _)| *label == 1.0)
        .map(|(_, rank)| rank)
        .sum();
    Some((rank_sum - positives as f64 * (positives as f64 + 1.0) / 2.0) / (positives as f64 * negatives as f64))
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
    fn huber_reduces_response_outlier_influence() {
        let x = DenseMatrix::new(5, 1, vec![0.0, 1.0, 2.0, 3.0, 4.0]).unwrap();
        let fit = fit_huber(&x, &[1.0, 3.0, 100.0, 7.0, 9.0], None, HuberConfig::default()).unwrap();
        assert!((fit.model.coefficients[1] - 2.0).abs() < 0.5);
    }

    #[test]
    fn conformal_interval_uses_empirical_residual_quantile() {
        let calibrator = ConformalCalibrator::fit(&[-1.0, 0.5, 2.0, -0.25], 0.8).unwrap();
        let interval = calibrator.interval(10.0);
        assert!(interval.0 <= 8.0 && interval.1 >= 12.0);
    }

    #[test]
    fn feature_transform_encodes_missingness_deterministically() {
        let rows = vec![
            vec![1.0, 2.0, f64::NAN],
            vec![3.0, 4.0, 5.0],
            vec![5.0, 6.0, 7.0],
        ];
        let transform = FeatureTransform::fit(&rows).unwrap();
        let first = transform.apply(&rows[0]).unwrap();
        let second = transform.apply(&rows[1]).unwrap();
        assert_eq!(first.len(), 6);
        // missing feature is standardized to 0 with indicator 1
        assert_eq!(first[4], 0.0);
        assert_eq!(first[5], 1.0);
        // present feature is standardized with indicator 0
        assert_eq!(second[5], 0.0);
        // refitting produces identical output (determinism)
        let transform2 = FeatureTransform::fit(&rows).unwrap();
        assert_eq!(transform, transform2);
    }

    #[test]
    fn logistic_irls_separates_simple_data() {
        let x = DenseMatrix::new(4, 1, vec![-2.0, -1.0, 1.0, 2.0]).unwrap();
        let model = fit_logistic_irls(&x, &[0.0, 0.0, 1.0, 1.0], 0.1, 30).unwrap();
        assert!(model.predict_probability(&[2.0]).unwrap() > 0.8);
        assert!(model.predict_probability(&[-2.0]).unwrap() < 0.2);
    }

    #[test]
    fn logistic_irls_accepts_constant_columns() {
        // a constant (fully-missing indicator) column must not break the solve
        let x = DenseMatrix::new(4, 2, vec![-2.0, 1.0, -1.0, 1.0, 1.0, 1.0, 2.0, 1.0]).unwrap();
        let model = fit_logistic_irls(&x, &[0.0, 0.0, 1.0, 1.0], 0.1, 30).unwrap();
        assert!(model.coefficients.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn auc_matches_expected_value() {
        // perfect separation: all positives score above all negatives -> AUC 1
        assert_eq!(auc(&[0.0, 0.0, 1.0, 1.0], &[0.1, 0.2, 0.9, 0.8]), Some(1.0));
        // reversed -> 0
        assert_eq!(auc(&[0.0, 0.0, 1.0, 1.0], &[0.9, 0.8, 0.1, 0.2]), Some(0.0));
        // tied scores use average ranks
        let value = auc(&[0.0, 1.0], &[0.5, 0.5]).unwrap();
        assert!((value - 0.5).abs() < 1e-12);
        // single class -> None
        assert_eq!(auc(&[1.0, 1.0], &[0.5, 0.6]), None);
    }

    #[test]
    fn ridge_ols_recovers_linear_relation_with_missingness() {
        // y = 2*x + 1 with a missing column that must not poison the fit
        let rows = vec![
            vec![0.0, 1.0],
            vec![1.0, f64::NAN],
            vec![2.0, 3.0],
            vec![3.0, f64::NAN],
        ];
        let y = [1.0, 3.0, 5.0, 7.0];
        let model = RidgeOls::fit(&rows, &y, 1e-9).unwrap();
        assert!(model.model_inputs() == 4);
        for (row, expected) in rows.iter().zip(y) {
            let predicted = model.predict(row).unwrap();
            assert!((predicted - expected).abs() < 1e-6, "predicted {predicted} != {expected}");
        }
    }

    #[test]
    fn ridge_ols_is_deterministic_and_ridges_toward_zero() {
        let rows: Vec<Vec<f64>> = (0..200).map(|i| vec![i as f64, (i % 7) as f64]).collect();
        let y: Vec<f64> = rows.iter().map(|row| 3.0 * row[0] + 0.5 * row[1] + 10.0).collect();
        let first = RidgeOls::fit(&rows, &y, 0.01).unwrap();
        let second = RidgeOls::fit(&rows, &y, 0.01).unwrap();
        assert_eq!(first, second, "identical inputs must produce identical fits");
        // strong ridge shrinks feature coefficients toward zero vs. weak ridge
        let strong = RidgeOls::fit(&rows, &y, 100.0).unwrap();
        let weak = RidgeOls::fit(&rows, &y, 1e-6).unwrap();
        let strong_norm: f64 = strong.model.coefficients[1..].iter().map(|v| v * v).sum();
        let weak_norm: f64 = weak.model.coefficients[1..].iter().map(|v| v * v).sum();
        assert!(strong_norm < weak_norm, "ridge must shrink coefficients");
    }

    #[test]
    fn quantile_ridge_recovers_upper_tail_of_skewed_data() {
        // y = 2x + skew: ~1/3 of rows at each x get +4, the rest get -0.5, so the
        // conditional 0.90 quantile sits well above the conditional mean.
        let rows: Vec<Vec<f64>> = (0..400).map(|i| vec![(i % 20) as f64]).collect();
        let mut y = Vec::with_capacity(400);
        for (i, row) in rows.iter().enumerate() {
            let base = 2.0 * row[0];
            let skew = if i % 3 == 0 { 4.0 } else { -0.5 };
            y.push(base + skew + (i % 5) as f64 * 0.1);
        }
        let mean = fit_quantile_ridge(&rows, &y, 0.5, 1e-3, 200, 1e-10).unwrap();
        let upper = fit_quantile_ridge(&rows, &y, 0.90, 1e-3, 200, 1e-10).unwrap();
        for (row, x) in rows.iter().take(10).zip((0..10).map(|x| x as f64)) {
            let p50 = mean.predict(row).unwrap();
            let p90 = upper.predict(row).unwrap();
            assert!(p90 > p50, "q90 {p90} must exceed q50 {p50} for x={x}");
            // q90 tracks 2x plus the upper-mass offset (~4.4) within tolerance
            let expected = 2.0 * x + 4.4;
            assert!(
                (p90 - expected).abs() < 1.0,
                "q90 {p90} far from expected {expected} for x={x}"
            );
        }
    }

    #[test]
    fn quantile_and_expectile_fits_are_deterministic() {
        let rows: Vec<Vec<f64>> = (0..150).map(|i| vec![i as f64, (i % 7) as f64]).collect();
        let y: Vec<f64> = rows.iter().map(|row| 3.0 * row[0] + (if row[1] > 3.0 { 2.0 } else { 0.0 }) + (row[0] * 0.1).sin()).collect();
        let first = fit_quantile_ridge(&rows, &y, 0.9, 1.0, 200, 1e-10).unwrap();
        let second = fit_quantile_ridge(&rows, &y, 0.9, 1.0, 200, 1e-10).unwrap();
        assert_eq!(first, second, "quantile fit must be deterministic");
        let e1 = fit_expectile_ridge(&rows, &y, 0.9, 1.0, 200, 1e-10).unwrap();
        let e2 = fit_expectile_ridge(&rows, &y, 0.9, 1.0, 200, 1e-10).unwrap();
        assert_eq!(e1, e2, "expectile fit must be deterministic");
        // high-tau quantile sits above the conditional mean on right-skewed data
        let mean_pred = RidgeOls::fit(&rows, &y, 1.0).unwrap();
        assert!(first.predict(&rows[10]).unwrap() > mean_pred.predict(&rows[10]).unwrap());
    }

    #[test]
    fn quantile_ridge_rejects_invalid_tau() {
        let rows = vec![vec![1.0], vec![2.0], vec![3.0]];
        let y = [1.0, 2.0, 3.0];
        assert!(fit_quantile_ridge(&rows, &y, 0.0, 1.0, 50, 1e-10).is_err());
        assert!(fit_quantile_ridge(&rows, &y, 1.0, 1.0, 50, 1e-10).is_err());
        assert!(fit_quantile_ridge(&rows, &y, f64::NAN, 1.0, 50, 1e-10).is_err());
        assert!(fit_expectile_ridge(&rows, &y, 0.0, 1.0, 50, 1e-10).is_err());
        assert!(fit_expectile_ridge(&rows, &y, 1.0, 1.0, 50, 1e-10).is_err());
    }

    #[test]
    fn logistic_separates_simple_data() {
        let x = DenseMatrix::new(4, 1, vec![-2.0, -1.0, 1.0, 2.0]).unwrap();
        let model = fit_logistic(&x, &[0.0, 0.0, 1.0, 1.0], 0.0, 0.2, 200).unwrap();
        assert!(model.predict_probability(&[2.0]).unwrap() > 0.8);
        assert!(model.predict_probability(&[-2.0]).unwrap() < 0.2);
    }
}
