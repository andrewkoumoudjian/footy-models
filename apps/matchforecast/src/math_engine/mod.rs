//! Research-only, coherent shot -> goal point process.
//!
//! A goal is a mark of a sampled shot, not an independent Poisson clock.
//! This module deliberately does not modify the production second-resolved model.
use footy_forecast::Rng64;
use footy_match::draw_exact_lineup;
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
        }
    }
}

impl MatchParameters {
    fn valid(self) -> bool {
        self.paths > 0
            && self.paths <= 1_000_000
            && self.red_card_probability_per90.is_finite()
            && (0.0..1.0).contains(&self.red_card_probability_per90)
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
    pub scorelines: BTreeMap<(u32, u32), u32>,
    pub players: BTreeMap<u64, PlayerCounts>,
    pub home_window_shots: [u64; 6],
    pub away_window_shots: [u64; 6],
    pub home_window_goals: [u64; 6],
    pub away_window_goals: [u64; 6],
    pub paths: u32,
}

fn starting_eleven(pool: &[Candidate], rng: &mut Rng64) -> Result<Vec<usize>, String> {
    // The two disjoint fixed-cardinality subsets are a partition-matroid special case.
    // The DP samples exactly from P(S) proportional to product_i(weight_i), |S|=k.
    // This is NOT a claim that the resulting start marginals are calibrated.
    let mut selected = Vec::with_capacity(11);
    for (keeper, count) in [(true, 1_usize), (false, 10_usize)] {
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
        let log_weights: Vec<f64> = candidates
            .iter()
            .map(|&i| (pool[i].minutes_prior.max(1.0)).ln())
            .collect();
        let draw = draw_exact_lineup(&log_weights, count, rng)
            .ok_or("exact-cardinality dynamic program could not draw lineup")?;
        selected.extend(draw.into_iter().map(|i| candidates[i]));
    }
    Ok(selected)
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
        })
    {
        return Err("invalid player pool or match-rate parameters".into());
    }
    let mut rng = Rng64::new(p.seed);
    let mut result = MatchResult {
        home_win: 0.0,
        draw: 0.0,
        away_win: 0.0,
        expected_home_goals: 0.0,
        expected_away_goals: 0.0,
        expected_home_shots: 0.0,
        expected_away_shots: 0.0,
        mean_red_cards: 0.0,
        scorelines: BTreeMap::new(),
        players: BTreeMap::new(),
        home_window_shots: [0; 6],
        away_window_shots: [0; 6],
        home_window_goals: [0; 6],
        away_window_goals: [0; 6],
        paths: p.paths,
    };
    let teams = [home, away];
    for _ in 0..p.paths {
        let mut active = [
            starting_eleven(home, &mut rng)?,
            starting_eleven(away, &mut rng)?,
        ];
        for (side, starters) in active.iter().enumerate() {
            for &i in starters {
                result.players.entry(teams[side][i].id).or_default().starts += 1;
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
            let total = intensities
                .iter()
                .chain(red_intensities.iter())
                .sum::<f64>();
            if total <= 0.0 {
                break;
            }
            t += -rng.uniform().max(f64::MIN_POSITIVE).ln() / total;
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
                        active[side].remove(i);
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
                    break;
                }
                event -= red_intensities[side];
            }
        }
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
            })
            .collect()
    }
    #[test]
    fn exact_partition_cardinality_and_seed() {
        let team = pool("A", 100);
        let mut r = Rng64::new(42);
        let xi = starting_eleven(&team, &mut r).unwrap();
        assert_eq!(xi.len(), 11);
        assert_eq!(xi.iter().filter(|&&i| team[i].is_goalkeeper).count(), 1);
        let mut r2 = Rng64::new(42);
        assert_eq!(xi, starting_eleven(&team, &mut r2).unwrap());
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
}
