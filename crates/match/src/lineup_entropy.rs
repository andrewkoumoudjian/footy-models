//! Fixed-cardinality maximum-entropy subset sampler for role-neutral research inputs.
//! This is deliberately separate from the role-aware production lineup sampler.

use footy_forecast::Rng64;

#[derive(Debug, Clone, PartialEq)]
pub struct ExactLineupWeights {
    /// Dual log weights in the same order as the input candidates; infinities denote hard 0/1.
    pub log_weights: Vec<f64>,
    /// Euclidean projection onto the uniform-matroid base polytope, preserving hard 0/1.
    pub projected_marginals: Vec<f64>,
    pub projection_l1: f64,
    pub maximum_marginal_error: f64,
}

fn logaddexp(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY { return b; }
    if b == f64::NEG_INFINITY { return a; }
    let maximum = a.max(b);
    maximum + ((a - maximum).exp() + (b - maximum).exp()).ln()
}

fn log_partition(weights: &[f64], count: usize) -> f64 {
    if count > weights.len() { return f64::NEG_INFINITY; }
    let mut terms = vec![f64::NEG_INFINITY; count + 1];
    terms[0] = 0.0;
    for (index, &weight) in weights.iter().enumerate() {
        for selected in (1..=count.min(index + 1)).rev() {
            terms[selected] = logaddexp(terms[selected], terms[selected - 1] + weight);
        }
    }
    terms[count]
}

/// Prepare the unique (up to an additive constant) dual of an exactly-`count` subset law:
/// P(S) ∝ exp(sum theta[i]), |S|=count. This preserves probability boundaries exactly.
/// Infeasible pre-match marginals are projected onto sum(p)=count by a common clipped shift.
/// The projection is a *declared model change*, not a calibration claim.
pub fn fit_exact_lineup_weights(priors: &[f64], count: usize) -> Option<ExactLineupWeights> {
    if priors.len() < count || priors.is_empty()
        || priors.iter().any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
    { return None; }
    let forced = priors.iter().filter(|&&p| p == 1.0).count();
    let adjustable: Vec<usize> = priors.iter().enumerate()
        .filter_map(|(i, &p)| (p > 0.0 && p < 1.0).then_some(i)).collect();
    if forced > count || forced + adjustable.len() < count { return None; }
    let to_fill = count - forced;
    let mut projected = priors.to_vec();
    if to_fill == 0 || to_fill == adjustable.len() {
        for &index in &adjustable { projected[index] = if to_fill != 0 { 1.0 } else { 0.0 }; }
    } else {
        let mut low = -1.0;
        let mut high = 1.0;
        for _ in 0..65 {
            let shift = 0.5 * (low + high);
            let total: f64 = adjustable.iter().map(|&i| (priors[i] + shift).clamp(0.0, 1.0)).sum();
            if total < to_fill as f64 { low = shift; } else { high = shift; }
        }
        for &i in &adjustable { projected[i] = (priors[i] + 0.5 * (low + high)).clamp(0.0, 1.0); }
    }

    let mut log_weights = projected.iter().map(|&p| {
        if p == 0.0 { f64::NEG_INFINITY }
        else if p == 1.0 { f64::INFINITY }
        else { (p / (1.0 - p)).ln() }
    }).collect::<Vec<_>>();
    let interior: Vec<usize> = projected.iter().enumerate()
        .filter_map(|(i, &p)| (p > 0.0 && p < 1.0).then_some(i)).collect();
    let fixed = projected.iter().filter(|&&p| p == 1.0).count();
    let target_count = count - fixed;
    let mut maximum_marginal_error = 0.0_f64;
    if target_count > 0 && target_count < interior.len() {
        let mut theta: Vec<f64> = interior.iter().map(|&i| log_weights[i]).collect();
        let mut converged = false;
        for _ in 0..400 {
            let log_z = log_partition(&theta, target_count);
            if !log_z.is_finite() { return None; }
            maximum_marginal_error = 0.0;
            let mut corrections = vec![0.0; theta.len()];
            for (j, &i) in interior.iter().enumerate() {
                let mut without = theta.clone();
                without.remove(j);
                let actual = (theta[j] + log_partition(&without, target_count - 1) - log_z).exp();
                maximum_marginal_error = maximum_marginal_error.max((actual - projected[i]).abs());
                if !(actual > 0.0 && actual < 1.0) { return None; }
                let desired = projected[i];
                corrections[j] = ((desired / (1.0 - desired)).ln()
                    - (actual / (1.0 - actual)).ln()).clamp(-4.0, 4.0);
            }
            if maximum_marginal_error < 1e-8 { converged = true; break; }
            for (value, correction) in theta.iter_mut().zip(corrections) {
                *value += 0.7 * correction;
            }
            let offset = theta.iter().sum::<f64>() / theta.len() as f64;
            for value in &mut theta { *value -= offset; }
        }
        if !converged { return None; }
        for (j, &i) in interior.iter().enumerate() { log_weights[i] = theta[j]; }
    }
    let projection_l1 = priors.iter().zip(&projected).map(|(a, b)| (a - b).abs()).sum();
    Some(ExactLineupWeights {
        log_weights, projected_marginals: projected, projection_l1, maximum_marginal_error,
    })
}

/// Exact backwards draw conditional on an available roster, with no rejection or ordering bias.
/// Index order is stable; caller must map indices back to its original candidate ordering.
pub fn draw_exact_lineup(weights: &[f64], count: usize, rng: &mut Rng64) -> Option<Vec<usize>> {
    let forced: Vec<usize> = weights.iter().enumerate()
        .filter_map(|(i, &w)| (w == f64::INFINITY).then_some(i)).collect();
    if weights.iter().any(|w| w.is_nan()) || forced.len() > count { return None; }
    let candidates: Vec<usize> = weights.iter().enumerate()
        .filter_map(|(i, &w)| w.is_finite().then_some(i)).collect();
    let to_fill = count - forced.len();
    if candidates.len() < to_fill { return None; }
    let mut dp = vec![vec![f64::NEG_INFINITY; to_fill + 1]; candidates.len() + 1];
    dp[0][0] = 0.0;
    for (step, &index) in candidates.iter().enumerate() {
        for k in 0..=to_fill.min(step + 1) {
            dp[step + 1][k] = logaddexp(dp[step][k],
                if k > 0 { dp[step][k - 1] + weights[index] } else { f64::NEG_INFINITY });
        }
    }
    if !dp[candidates.len()][to_fill].is_finite() { return None; }
    let mut selected = forced;
    let mut remaining = to_fill;
    for step in (1..=candidates.len()).rev() {
        if remaining == 0 { break; }
        let index = candidates[step - 1];
        let probability = (dp[step - 1][remaining - 1] + weights[index]
            - dp[step][remaining]).exp().clamp(0.0, 1.0);
        if rng.bernoulli(probability) {
            selected.push(index);
            remaining -= 1;
        }
    }
    if remaining != 0 { return None; }
    selected.sort_unstable();
    Some(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infeasible_boundaries_fail_closed() {
        assert!(fit_exact_lineup_weights(&[1.0, 1.0, 0.5], 1).is_none());
        assert!(fit_exact_lineup_weights(&[0.0, 0.0, 0.5], 2).is_none());
        assert!(fit_exact_lineup_weights(&[f64::NAN, 0.2], 1).is_none());
    }

    #[test]
    fn preserves_boundaries_and_cardinality() {
        let weights = fit_exact_lineup_weights(&[1.0, 0.0, 0.8, 0.3, 0.2, 0.4], 3).unwrap();
        assert_eq!(weights.projected_marginals[0], 1.0);
        assert_eq!(weights.projected_marginals[1], 0.0);
        assert!((weights.projected_marginals.iter().sum::<f64>() - 3.0).abs() < 1e-8);
        let mut rng = Rng64::new(42);
        for _ in 0..200 {
            let selected = draw_exact_lineup(&weights.log_weights, 3, &mut rng).unwrap();
            assert_eq!(selected.len(), 3);
            assert!(selected.contains(&0));
            assert!(!selected.contains(&1));
        }
    }

    #[test]
    fn exact_distribution_recovers_projected_marginals() {
        let priors = [0.8, 0.75, 0.6, 0.45, 0.4, 0.25, 0.15];
        let model = fit_exact_lineup_weights(&priors, 3).unwrap();
        assert!(model.maximum_marginal_error < 1e-8);
        let mut rng = Rng64::new(77);
        let mut counts = [0_u32; 7];
        for _ in 0..30_000 {
            for index in draw_exact_lineup(&model.log_weights, 3, &mut rng).unwrap() {
                counts[index] += 1;
            }
        }
        for (i, target) in model.projected_marginals.iter().enumerate() {
            assert!((f64::from(counts[i]) / 30_000.0 - target).abs() < 0.017);
        }
    }

    #[test]
    fn fixed_seed_is_reproducible() {
        let model = fit_exact_lineup_weights(&[0.95, 0.8, 0.7, 0.6, 0.3, 0.2], 3).unwrap();
        let mut a = Rng64::new(19);
        let mut b = Rng64::new(19);
        for _ in 0..100 {
            assert_eq!(draw_exact_lineup(&model.log_weights, 3, &mut a),
                       draw_exact_lineup(&model.log_weights, 3, &mut b));
        }
    }
}
