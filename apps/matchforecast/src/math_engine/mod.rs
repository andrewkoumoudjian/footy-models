//! Research-only, coherent shot -> goal point process.
//!
//! A goal is a mark of a sampled shot, not an independent Poisson clock.
//! This module deliberately does not modify the production second-resolved model.
use footy_forecast::Rng64;
use footy_match::{draw_exact_lineup, fit_exact_lineup_weights};
use std::collections::{BTreeMap, BTreeSet};

const SECONDS: f64 = 5400.0;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: u64,
    pub club: String,
    pub minutes_prior: f64,
    pub shots_per90: f64,
    pub xg_per90: f64,
    pub is_goalkeeper: bool,
    pub fm_source: String,
    pub fm_finishing: Option<f64>,
    /// Empirical, dated selection marginal; None means there is no measured history.
    pub start_probability: Option<f64>,
    /// Mean playing minutes among historically observed starts, including completed substitutions.
    pub expected_minutes_if_start: Option<f64>,
    /// Empirical chance of coming on, conditional on not starting.
    pub bench_appearance_probability: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchParameters {
    pub paths: u32,
    pub seed: u64,
    /// Exponential competing-hazard per-player dismissal rate.
    pub red_card_probability_per90: f64,
    /// All tactical adjustments are off by default pending event-level OOS fitting.
    pub trailing_shot_log_effect: f64,
    pub numerical_advantage_log_effect: f64,
    /// Dated FM attribute effects require an as-of supervised fit; default zero.
    pub fm_finishing_log_effect: f64,
    /// Research ablation using an exact-cardinality maximum-entropy XI law.
    pub use_selection_marginals: bool,
    /// Research ablation. 3 subs before 2020, 5 in modern leagues.
    /// Zero preserves the previously frozen no-substitution mechanism.
    pub maximum_substitutions_per_team: u8,
}

impl Default for MatchParameters {
    fn default() -> Self {
        Self {
            paths: 2500,
            seed: 42,
            red_card_probability_per90: 0.0,
            trailing_shot_log_effect: 0.0,
            numerical_advantage_log_effect: 0.0,
            fm_finishing_log_effect: 0.0,
            use_selection_marginals: false,
            maximum_substitutions_per_team: 0,
        }
    }
}

impl MatchParameters {
    fn valid(self) -> bool {
        self.paths > 0
            && self.paths <= 1_000_000
            && self.red_card_probability_per90.is_finite()
            && (0.0..1.0).contains(&self.red_card_probability_per90)
            && self.maximum_substitutions_per_team <= 5
            && [
                self.trailing_shot_log_effect,
                self.numerical_advantage_log_effect,
                self.fm_finishing_log_effect,
            ]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerCounts {
    pub shots: u64,
    pub goals: u64,
    pub starts: u32,
    /// Complete paths with at least one shot, not a Poisson approximation.
    pub shot_occurrences: u32,
    /// Complete paths with at least one scored goal.
    pub goal_occurrences: u32,
    pub appearances: u32,
    pub substitutions_on: u32,
    /// Actual seconds played across sampled paths, accounting for red cards and substitutions.
    pub seconds_played: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchResult {
    pub home_win: f64,
    pub draw: f64,
    pub away_win: f64,
    pub expected_home_goals: f64,
    pub expected_away_goals: f64,
    pub expected_home_shots: f64,
    pub expected_away_shots: f64,
    pub mean_red_cards: f64,
    pub mean_home_substitutions: f64,
    pub mean_away_substitutions: f64,
    pub scorelines: BTreeMap<(u32, u32), u32>,
    pub players: BTreeMap<u64, PlayerCounts>,
    pub home_window_shots: [u64; 6],
    pub away_window_shots: [u64; 6],
    pub home_window_goals: [u64; 6],
    pub away_window_goals: [u64; 6],
    pub paths: u32,
}

fn selection_weights(pool: &[Candidate], enabled: bool) -> Result<Option<[Vec<f64>; 2]>, String> {
    if !enabled {
        return Ok(None);
    }
    let mut groups = [Vec::new(), Vec::new()];
    for (group, goalkeepers) in groups.iter_mut().zip([true, false]) {
        let probabilities = pool
            .iter()
            .filter(|p| p.is_goalkeeper == goalkeepers)
            .map(|p| {
                p.start_probability
                    .ok_or("measured pre-match start history required for marginal-fit XI")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let selected = if goalkeepers { 1 } else { 10 };
        *group = fit_exact_lineup_weights(&probabilities, selected)
            .ok_or("infeasible or unconverged max-entropy lineup fit")?
            .log_weights;
    }
    Ok(Some(groups))
}

fn starting_eleven(
    pool: &[Candidate],
    rng: &mut Rng64,
    fitted_weights: Option<&[Vec<f64>; 2]>,
) -> Result<Vec<usize>, String> {
    // The two disjoint fixed-cardinality subsets are a partition-matroid special case.
    // The DP samples exactly from P(S) proportional to product_i(weight_i), |S|=k.
    // This is NOT a claim that the resulting start marginals are calibrated.
    let mut selected = Vec::with_capacity(11);
    for (group, (keeper, count)) in [(true, 1_usize), (false, 10_usize)].into_iter().enumerate() {
        let candidates: Vec<usize> = pool
            .iter()
            .enumerate()
            .filter_map(|(i, p)| (p.is_goalkeeper == keeper).then_some(i))
            .collect();
        if candidates.len() < count {
            return Err(format!(
                "insufficient {} candidates: {} < {}",
                if keeper { "goalkeeper" } else { "outfield" },
                candidates.len(),
                count
            ));
        }
        let log_weights: Vec<f64> = if let Some(weights) = fitted_weights {
            weights[group].clone()
        } else {
            candidates
                .iter()
                .map(|&i| (pool[i].minutes_prior.max(1.0)).ln())
                .collect()
        };
        let draw = draw_exact_lineup(&log_weights, count, rng)
            .ok_or("exact-cardinality dynamic program could not draw lineup")?;
        selected.extend(draw.into_iter().map(|i| candidates[i]));
    }
    Ok(selected)
}

/// Constant per-minute completed-substitution hazard after half time, implied
/// by a *previously observed* conditional starter-mean playing duration.
/// A non-zero rate is defined only when the input has actual start histories.
/// E[duration] = 45 + (1 - exp(-45λ))/λ. No same-match data are used.
fn substitution_rate_per_second(mean_start_minutes: Option<f64>) -> f64 {
    let Some(mean) = mean_start_minutes else {
        return 0.0;
    };
    if mean >= 89.99 {
        return 0.0;
    }
    let target = mean.clamp(45.01, 89.99);
    let mut lower = 1e-10_f64;
    let mut upper = 10.0_f64;
    for _ in 0..72 {
        let lambda = (lower + upper) * 0.5;
        let expectation = 45.0 - (-lambda * 45.0).exp_m1() / lambda;
        if expectation > target {
            lower = lambda;
        } else {
            upper = lambda;
        }
    }
    0.5 * (lower + upper) / 60.0
}

fn logistic(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

fn conversion_probabilities(
    pool: &[Candidate],
    lineup: &[usize],
    team_goals: f64,
    team_shots: f64,
    fm_coefficient: f64,
) -> Result<Vec<f64>, String> {
    if team_goals <= 0.0
        || team_shots <= team_goals
        || !team_goals.is_finite()
        || !team_shots.is_finite()
    {
        return Err("team shot/goal anchor must be positive, with shots > goals".into());
    }
    let target = team_goals / team_shots;
    let mut weights = Vec::with_capacity(lineup.len());
    let mut logits = Vec::with_capacity(lineup.len());
    for &j in lineup {
        let p = &pool[j];
        let weight = p.shots_per90.max(0.005);
        let raw = (p.xg_per90.max(0.001) / weight).clamp(0.005, 0.70);
        // Synthetic FM fields never enter an attribute effect. The coefficient
        // defaults to zero for every source until fitted on dated player events.
        let fm = if p.fm_source == "db" {
            p.fm_finishing
                .map(|v| fm_coefficient * ((v - 10.0) / 10.0))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        weights.push(weight);
        logits.push((raw / (1.0 - raw)).ln() + fm);
    }
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return Err("lineup has no positive shot rate".into());
    }
    // Solve the one-dimensional logit intercept to preserve the fitted team
    // goal expectation exactly under the player-weighted conversion model.
    let mut lo = -50.0;
    let mut hi = 50.0;
    for _ in 0..90 {
        let shift = (lo + hi) * 0.5;
        let mean: f64 = logits
            .iter()
            .zip(&weights)
            .map(|(z, w)| w * logistic(z + shift))
            .sum::<f64>()
            / total;
        if mean < target {
            lo = shift;
        } else {
            hi = shift;
        }
    }
    let probs: Vec<f64> = logits
        .iter()
        .map(|z| logistic(z + (lo + hi) * 0.5))
        .collect();
    let got = probs.iter().zip(&weights).map(|(p, w)| p * w).sum::<f64>() / total;
    if (got - target).abs() > 1e-8 {
        return Err("conversion calibration failed".into());
    }
    Ok(probs)
}

fn sample_weighted(weights: &[f64], rng: &mut Rng64) -> usize {
    let mut target = rng.uniform() * weights.iter().sum::<f64>();
    for (i, w) in weights.iter().enumerate() {
        target -= w;
        if target <= 0.0 {
            return i;
        }
    }
    weights.len() - 1
}

/// Simulate complete independent paths. Team rates are *external causal fits*;
/// input player statistics are explicitly FM-derived priors, not observed labels.
pub fn simulate(
    home: &[Candidate],
    away: &[Candidate],
    goal_rates: [f64; 2],
    shot_rates: [f64; 2],
    p: MatchParameters,
) -> Result<MatchResult, String> {
    if !p.valid()
        || goal_rates
            .iter()
            .chain(&shot_rates)
            .any(|x| !x.is_finite() || *x <= 0.0)
        || goal_rates.iter().zip(shot_rates).any(|(g, s)| *g >= s)
        || home.iter().any(|c| c.club != home[0].club)
        || away.iter().any(|c| c.club != away[0].club)
        || home.is_empty()
        || away.is_empty()
        || home.iter().chain(away).any(|c| {
            !c.minutes_prior.is_finite()
                || c.minutes_prior < 0.0
                || !c.shots_per90.is_finite()
                || c.shots_per90 < 0.0
                || !c.xg_per90.is_finite()
                || c.xg_per90 < 0.0
                || c.start_probability
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                || c.expected_minutes_if_start
                    .is_some_and(|v| !v.is_finite() || !(0.0..=90.0).contains(&v))
                || c.bench_appearance_probability
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        })
    {
        return Err("invalid player pool or match-rate parameters".into());
    }
    let mut rng = Rng64::new(p.seed);
    let weights = [
        selection_weights(home, p.use_selection_marginals)?,
        selection_weights(away, p.use_selection_marginals)?,
    ];
    if p.maximum_substitutions_per_team > 0
        && home.iter().chain(away).any(|c| {
            c.expected_minutes_if_start.is_none() || c.bench_appearance_probability.is_none()
        })
    {
        return Err("cannot enable learned substitutions without dated player minutes and bench appearance priors".into());
    }
    let mut result = MatchResult {
        home_win: 0.0,
        draw: 0.0,
        away_win: 0.0,
        expected_home_goals: 0.0,
        expected_away_goals: 0.0,
        expected_home_shots: 0.0,
        expected_away_shots: 0.0,
        mean_red_cards: 0.0,
        mean_home_substitutions: 0.0,
        mean_away_substitutions: 0.0,
        scorelines: BTreeMap::new(),
        players: BTreeMap::new(),
        home_window_shots: [0; 6],
        away_window_shots: [0; 6],
        home_window_goals: [0; 6],
        away_window_goals: [0; 6],
        paths: p.paths,
    };
    let teams = [home, away];
    // Zero-probability candidates must still have explicit output rows:
    // omission is not the same statistical statement as a measured zero.
    for candidate in home.iter().chain(away.iter()) {
        result.players.entry(candidate.id).or_default();
    }
    for _ in 0..p.paths {
        let mut active = [
            starting_eleven(home, &mut rng, weights[0].as_ref())?,
            starting_eleven(away, &mut rng, weights[1].as_ref())?,
        ];
        let mut entered = [BTreeSet::new(), BTreeSet::new()];
        let mut on_since = [BTreeMap::new(), BTreeMap::new()];
        for (side, starters) in active.iter().enumerate() {
            for &i in starters {
                let counts = result.players.entry(teams[side][i].id).or_default();
                counts.starts += 1;
                counts.appearances += 1;
                entered[side].insert(i);
                on_since[side].insert(i, 0.0_f64);
            }
        }
        let mut conversion = [
            conversion_probabilities(
                home,
                &active[0],
                goal_rates[0],
                shot_rates[0],
                p.fm_finishing_log_effect,
            )?,
            conversion_probabilities(
                away,
                &active[1],
                goal_rates[1],
                shot_rates[1],
                p.fm_finishing_log_effect,
            )?,
        ];
        let mut score = [0_u32; 2];
        let mut shot_count = [0_u32; 2];
        let mut path_shooters = BTreeSet::new();
        let mut path_scorers = BTreeSet::new();
        let mut reds = 0_u32;
        let mut subs = [0_u32; 2];
        let mut t = 0.0_f64;
        let red_hazard = -(-p.red_card_probability_per90).ln_1p() / SECONDS;
        while t < SECONDS {
            let mut intensities = [0.0_f64; 2];
            for side in 0..2 {
                let opponent = 1 - side;
                let trailing = if score[side] < score[opponent] {
                    p.trailing_shot_log_effect
                } else {
                    0.0
                };
                let numerical = p.numerical_advantage_log_effect
                    * (active[side].len() as f64 - active[opponent].len() as f64);
                intensities[side] = shot_rates[side] / SECONDS * (trailing + numerical).exp();
            }
            let red_intensities = [
                active[0].len() as f64 * red_hazard,
                active[1].len() as f64 * red_hazard,
            ];
            let mut sub_intensities = [Vec::new(), Vec::new()];
            let mut sub_totals = [0.0_f64; 2];
            if t >= 2700.0 {
                for side in 0..2 {
                    if subs[side] >= u32::from(p.maximum_substitutions_per_team) {
                        continue;
                    }
                    // No bench player may enter twice, and a dismissed player
                    // is ineligible. Keep goalkeepers as a distinct role.
                    let has_bench = teams[side].iter().enumerate().any(|(i, c)| {
                        !c.is_goalkeeper
                            && !entered[side].contains(&i)
                            && c.bench_appearance_probability.unwrap_or(0.0) > 0.0
                    });
                    if !has_bench {
                        continue;
                    }
                    sub_intensities[side] = active[side]
                        .iter()
                        .map(|&index| {
                            if teams[side][index].is_goalkeeper || on_since[side][&index] > 0.0 {
                                0.0
                            } else {
                                substitution_rate_per_second(
                                    teams[side][index].expected_minutes_if_start,
                                )
                            }
                        })
                        .collect();
                    sub_totals[side] = sub_intensities[side].iter().sum();
                }
            }
            let total = intensities
                .iter()
                .chain(red_intensities.iter())
                .sum::<f64>();
            let total = total + sub_totals.iter().sum::<f64>();
            if total <= 0.0 {
                break;
            }
            let next = t - rng.uniform().max(f64::MIN_POSITIVE).ln() / total;
            if t < 2700.0 && next >= 2700.0 && p.maximum_substitutions_per_team > 0 {
                // Memorylessness of exponential waiting times permits exact
                // restart at the piecewise-constant hazard boundary.
                t = 2700.0;
                continue;
            }
            t = next;
            if t >= SECONDS {
                break;
            }
            let mut event = rng.uniform() * total;
            let mut happened = false;
            for side in 0..2 {
                if event < intensities[side] {
                    let lineup = &active[side];
                    let weights: Vec<f64> = lineup
                        .iter()
                        .map(|&i| teams[side][i].shots_per90.max(0.005))
                        .collect();
                    let chosen = sample_weighted(&weights, &mut rng);
                    let id = teams[side][lineup[chosen]].id;
                    result.players.entry(id).or_default().shots += 1;
                    path_shooters.insert(id);
                    shot_count[side] += 1;
                    let window = ((t / 900.0) as usize).min(5);
                    if side == 0 {
                        result.home_window_shots[window] += 1;
                    } else {
                        result.away_window_shots[window] += 1;
                    }
                    if rng.bernoulli(conversion[side][chosen]) {
                        score[side] += 1;
                        result.players.entry(id).or_default().goals += 1;
                        path_scorers.insert(id);
                        if side == 0 {
                            result.home_window_goals[window] += 1;
                        } else {
                            result.away_window_goals[window] += 1;
                        }
                    }
                    happened = true;
                    break;
                }
                event -= intensities[side];
            }
            if happened {
                continue;
            }
            for side in 0..2 {
                if event < red_intensities[side] {
                    if !active[side].is_empty() {
                        let i = (rng.uniform() * active[side].len() as f64).floor() as usize;
                        let out = active[side].remove(i);
                        if let Some(entry) = on_since[side].remove(&out) {
                            result
                                .players
                                .entry(teams[side][out].id)
                                .or_default()
                                .seconds_played += t - entry;
                        }
                        reds += 1;
                        if !active[side].is_empty() {
                            conversion[side] = conversion_probabilities(
                                teams[side],
                                &active[side],
                                goal_rates[side],
                                shot_rates[side],
                                p.fm_finishing_log_effect,
                            )?;
                        }
                    }
                    happened = true;
                    break;
                }
                event -= red_intensities[side];
            }
            if happened {
                continue;
            }
            for side in 0..2 {
                if event < sub_totals[side] {
                    let leaving = sample_weighted(&sub_intensities[side], &mut rng);
                    let incoming: Vec<usize> = teams[side]
                        .iter()
                        .enumerate()
                        .filter_map(|(i, c)| {
                            (!c.is_goalkeeper
                                && !entered[side].contains(&i)
                                && c.bench_appearance_probability.unwrap_or(0.0) > 0.0)
                                .then_some(i)
                        })
                        .collect();
                    let incoming_weights: Vec<f64> = incoming
                        .iter()
                        .map(|&i| teams[side][i].bench_appearance_probability.unwrap_or(0.0))
                        .collect();
                    let incoming_index = incoming[sample_weighted(&incoming_weights, &mut rng)];
                    let outgoing_index = active[side][leaving];
                    if let Some(entry) = on_since[side].remove(&outgoing_index) {
                        result
                            .players
                            .entry(teams[side][outgoing_index].id)
                            .or_default()
                            .seconds_played += t - entry;
                    }
                    active[side][leaving] = incoming_index;
                    entered[side].insert(incoming_index);
                    on_since[side].insert(incoming_index, t);
                    let counts = result
                        .players
                        .entry(teams[side][incoming_index].id)
                        .or_default();
                    counts.appearances += 1;
                    counts.substitutions_on += 1;
                    subs[side] += 1;
                    conversion[side] = conversion_probabilities(
                        teams[side],
                        &active[side],
                        goal_rates[side],
                        shot_rates[side],
                        p.fm_finishing_log_effect,
                    )?;
                    break;
                }
                event -= sub_totals[side];
            }
        }
        for side in 0..2 {
            for (&index, &entry) in &on_since[side] {
                result
                    .players
                    .entry(teams[side][index].id)
                    .or_default()
                    .seconds_played += SECONDS - entry;
            }
        }
        result.mean_home_substitutions += subs[0] as f64;
        result.mean_away_substitutions += subs[1] as f64;
        for id in path_shooters {
            result.players.entry(id).or_default().shot_occurrences += 1;
        }
        for id in path_scorers {
            result.players.entry(id).or_default().goal_occurrences += 1;
        }
        match score[0].cmp(&score[1]) {
            std::cmp::Ordering::Greater => result.home_win += 1.0,
            std::cmp::Ordering::Equal => result.draw += 1.0,
            std::cmp::Ordering::Less => result.away_win += 1.0,
        }
        *result.scorelines.entry((score[0], score[1])).or_default() += 1;
        result.expected_home_goals += score[0] as f64;
        result.expected_away_goals += score[1] as f64;
        result.expected_home_shots += shot_count[0] as f64;
        result.expected_away_shots += shot_count[1] as f64;
        result.mean_red_cards += reds as f64;
    }
    let n = p.paths as f64;
    result.home_win /= n;
    result.draw /= n;
    result.away_win /= n;
    result.expected_home_goals /= n;
    result.expected_away_goals /= n;
    result.expected_home_shots /= n;
    result.expected_away_shots /= n;
    result.mean_red_cards /= n;
    result.mean_home_substitutions /= n;
    result.mean_away_substitutions /= n;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pool(club: &str, base: u64) -> Vec<Candidate> {
        (0..17)
            .map(|i| Candidate {
                id: base + i,
                club: club.into(),
                minutes_prior: if i < 11 { 1800.0 } else { 250.0 },
                shots_per90: if i == 0 { 0.01 } else { 1.0 },
                xg_per90: if i == 0 { 0.001 } else { 0.11 },
                is_goalkeeper: i == 0,
                fm_source: "db".into(),
                fm_finishing: Some(12.0),
                start_probability: Some(if i < 11 { 0.80 } else { 0.25 }),
                expected_minutes_if_start: Some(if i == 0 { 90.0 } else { 76.0 }),
                bench_appearance_probability: Some(if i == 0 { 0.0 } else { 0.65 }),
            })
            .collect()
    }
    #[test]
    fn exact_partition_cardinality_and_seed() {
        let team = pool("A", 100);
        let mut r = Rng64::new(42);
        let xi = starting_eleven(&team, &mut r, None).unwrap();
        assert_eq!(xi.len(), 11);
        assert_eq!(xi.iter().filter(|&&i| team[i].is_goalkeeper).count(), 1);
        let mut r2 = Rng64::new(42);
        assert_eq!(xi, starting_eleven(&team, &mut r2, None).unwrap());
    }
    #[test]
    fn thinning_preserves_goals_shots_and_team_means() {
        let h = pool("A", 100);
        let a = pool("B", 200);
        let p = MatchParameters {
            paths: 20000,
            ..Default::default()
        };
        let r = simulate(&h, &a, [1.5, 1.1], [13.0, 10.0], p).unwrap();
        assert_eq!(
            r.players.values().map(|x| x.goals).sum::<u64>(),
            r.scorelines
                .iter()
                .map(|(&(hg, ag), &n)| (hg + ag) as u64 * n as u64)
                .sum::<u64>()
        );
        assert!(r.players.values().all(|x| x.goals <= x.shots));
        assert!(r
            .players
            .values()
            .all(|x| x.goal_occurrences <= x.shot_occurrences
                && x.goal_occurrences as u64 <= x.goals
                && x.shot_occurrences as u64 <= x.shots));
        assert!((r.expected_home_goals - 1.5).abs() < 0.09);
        assert!((r.expected_away_goals - 1.1).abs() < 0.09);
        assert!((r.expected_home_shots - 13.0).abs() < 0.2);
        assert_eq!(r, simulate(&h, &a, [1.5, 1.1], [13.0, 10.0], p).unwrap());
    }
    #[test]
    fn synthetic_fm_never_changes_conversion() {
        let mut h = pool("A", 100);
        h[1].fm_finishing = Some(20.0);
        let selected: Vec<usize> = (0..11).collect();
        let a = conversion_probabilities(&h, &selected, 1.2, 12.0, 0.6).unwrap();
        for player in &mut h {
            player.fm_source = "derived".into();
            player.fm_finishing = Some(20.0);
        }
        let b = conversion_probabilities(&h, &selected, 1.2, 12.0, 0.6).unwrap();
        assert_ne!(a, b);
        let zeros = conversion_probabilities(&h, &selected, 1.2, 12.0, 0.0).unwrap();
        assert_eq!(b, zeros);
    }

    #[test]
    fn substitution_hazard_recovers_observed_conditional_minutes() {
        assert_eq!(substitution_rate_per_second(None), 0.0);
        assert_eq!(substitution_rate_per_second(Some(90.0)), 0.0);
        for prior in [53.0, 67.0, 76.0, 84.0] {
            let rate = substitution_rate_per_second(Some(prior)) * 60.0;
            let recovered = 45.0 - (-rate * 45.0).exp_m1() / rate;
            assert!((recovered - prior).abs() < 1e-8, "{prior}: {recovered}");
        }
    }

    #[test]
    fn opt_in_substitutions_conserve_player_minutes_and_counts() {
        let h = pool("A", 100);
        let a = pool("B", 200);
        let params = MatchParameters {
            paths: 700,
            maximum_substitutions_per_team: 3,
            ..Default::default()
        };
        let outcome = simulate(&h, &a, [1.5, 1.1], [13.0, 10.0], params).unwrap();
        assert!(outcome.mean_home_substitutions > 0.0);
        assert!(outcome.mean_home_substitutions <= 3.0);
        assert!(outcome.mean_away_substitutions <= 3.0);
        let duration: f64 = outcome.players.values().map(|p| p.seconds_played).sum();
        assert!((duration - 22.0 * 5400.0 * f64::from(params.paths)).abs() < 1e-6);
        let appear: u32 = outcome.players.values().map(|p| p.appearances).sum();
        assert_eq!(
            appear,
            22 * params.paths
                + ((outcome.mean_home_substitutions + outcome.mean_away_substitutions)
                    * f64::from(params.paths))
                .round() as u32
        );
        assert!(outcome.players.values().all(|p| p.goals <= p.shots
            && p.appearances >= p.starts
            && p.appearances >= p.goal_occurrences));
    }

    #[test]
    fn selection_history_required_for_opt_in_models() {
        let mut h = pool("A", 100);
        let a = pool("B", 200);
        h[0].start_probability = None;
        let parameters = MatchParameters {
            use_selection_marginals: true,
            paths: 1,
            ..Default::default()
        };
        assert!(simulate(&h, &a, [1.4, 1.1], [12.0, 10.0], parameters).is_err());
        h[0].start_probability = Some(0.8);
        h[0].expected_minutes_if_start = None;
        let params = MatchParameters {
            maximum_substitutions_per_team: 3,
            paths: 1,
            ..Default::default()
        };
        assert!(simulate(&h, &a, [1.4, 1.1], [12.0, 10.0], params).is_err());
    }
}
