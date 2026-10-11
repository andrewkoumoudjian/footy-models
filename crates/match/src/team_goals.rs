//! Team-only score distribution for leagues without current player/lineup coverage.
//!
//! Separate from the player-driven match engine: a missing player roster must never
//! be represented as a fictional lineup. This is a fitted, regularized independent-
//! Poisson goal baseline; `OutcomeCalibrator` handles empirical H/D/A calibration.
//! Each training observation must precede the forecast kickoff. No bookmaker inputs.
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct TeamGoalsObservation {
    pub day: i32,
    pub home: String,
    pub away: String,
    pub home_goals: u8,
    pub away_goals: u8,
}

#[derive(Clone, Debug)]
pub struct TeamGoalModel {
    pub away_baseline: f64,
    pub home_log_advantage: f64,
    attack: BTreeMap<String, f64>,
    defense: BTreeMap<String, f64>,
}

#[derive(Clone, Debug)]
pub struct GoalScoreForecast {
    pub home_goals: f64,
    pub away_goals: f64,
    pub outcomes: [f64; 3],
    pub modal_home: usize,
    pub modal_away: usize,
    pub modal_probability: f64,
}

fn weight(cutoff_day: i32, observation_day: i32) -> f64 {
    if observation_day >= cutoff_day { return 0.0; }
    // 270-day half-life, matching current season substantially more than the older seasons.
    (-(f64::from(cutoff_day - observation_day)) * std::f64::consts::LN_2 / 270.0).exp()
}

impl TeamGoalModel {
    pub fn fit(premier: &[TeamGoalsObservation], championship: &[TeamGoalsObservation], cutoff_day: i32) -> Option<Self> {
        let active: Vec<(&TeamGoalsObservation, f64)> = premier.iter().filter_map(|match_| {
            let w = weight(cutoff_day, match_.day);
            (w > 0.0).then_some((match_, w))
        }).collect();
        if active.len() < 30 { return None; }

        let mut names = BTreeMap::<String, usize>::new();
        for (game, _) in &active {
            for name in [&game.home, &game.away] {
                if !names.contains_key(name) {
                    let id = names.len();
                    names.insert(name.clone(), id);
                }
            }
        }
        let n = names.len();
        let mut scored_home = 0.0;
        let mut scored_away = 0.0;
        let mut total_weight = 0.0;
        for (game, w) in &active {
            scored_home += w * f64::from(game.home_goals);
            scored_away += w * f64::from(game.away_goals);
            total_weight += w;
        }
        let baseline = (scored_away / total_weight).clamp(0.75, 2.5);
        let home_log_advantage = ((scored_home + 5.0) / (scored_away + 5.0)).ln().clamp(-0.2, 0.5);

        // Championship priors for recent promoted clubs. Downshift their attack and
        // penalize defence in PL units. A prior, not an imaginary PL observation.
        let mut champ_stats = BTreeMap::<String, (f64, f64, f64)>::new();
        let mut champ_gf: f64 = 0.0;
        let mut champ_games: f64 = 0.0;
        for game in championship.iter().filter(|g| g.day < cutoff_day) {
            let h = champ_stats.entry(game.home.clone()).or_default();
            h.0 += f64::from(game.home_goals); h.1 += f64::from(game.away_goals); h.2 += 1.0;
            let a = champ_stats.entry(game.away.clone()).or_default();
            a.0 += f64::from(game.away_goals); a.1 += f64::from(game.home_goals); a.2 += 1.0;
            champ_gf += f64::from(game.home_goals + game.away_goals);
            champ_games += 2.0;
        }
        let champ_average = (champ_gf / champ_games.max(1.0)).max(0.5);
        let mut prior_attack = vec![0.0_f64; n];
        let mut prior_defense = vec![0.0_f64; n];
        for (team, &id) in &names {
            if let Some(&(gf, ga, played)) = champ_stats.get(team) {
                if played >= 15.0 {
                    prior_attack[id] = (0.6 * ((gf / played) / champ_average).ln() - 0.3).clamp(-0.9, 0.3);
                    prior_defense[id] = (0.6 * ((ga / played) / champ_average).ln() + 0.3).clamp(-0.3, 0.95);
                }
            }
        }
        let mut attack = prior_attack.clone();
        let mut defense = prior_defense.clone();
        let prior_precision = 10.0;
        let examples: Vec<(usize, usize, f64, f64, f64)> = active.iter().map(|(g,w)| {
            (names[&g.home], names[&g.away], *w, f64::from(g.home_goals), f64::from(g.away_goals))
        }).collect();
        // Damped diagonal Newton ascent on penalized Poisson log likelihood. Fixed
        // iteration budget for reproducibility, with bounded exponentials.
        for _ in 0..140 {
            let mut grad_a = vec![0.0; n];
            let mut grad_d = vec![0.0; n];
            let mut hess_a = vec![prior_precision; n];
            let mut hess_d = vec![prior_precision; n];
            for &(home, away, w, hg, ag) in &examples {
                let mh = (baseline.ln() + home_log_advantage + attack[home] - defense[away]).clamp(-2.5, 2.0).exp();
                let ma = (baseline.ln() + attack[away] - defense[home]).clamp(-2.5, 2.0).exp();
                grad_a[home] += w * (hg - mh);
                grad_d[away] += w * (mh - hg);
                grad_a[away] += w * (ag - ma);
                grad_d[home] += w * (ma - ag);
                hess_a[home] += w * mh;
                hess_d[away] += w * mh;
                hess_a[away] += w * ma;
                hess_d[home] += w * ma;
            }
            for i in 0..n {
                grad_a[i] -= prior_precision * (attack[i] - prior_attack[i]);
                grad_d[i] -= prior_precision * (defense[i] - prior_defense[i]);
                attack[i] = (attack[i] + 0.6 * grad_a[i] / hess_a[i]).clamp(-1.3, 1.3);
                defense[i] = (defense[i] + 0.6 * grad_d[i] / hess_d[i]).clamp(-1.3, 1.3);
            }
        }
        let attack = names.iter().map(|(name, &id)| (name.clone(), attack[id])).collect();
        let defense = names.iter().map(|(name, &id)| (name.clone(), defense[id])).collect();
        Some(Self { away_baseline: baseline, home_log_advantage, attack, defense })
    }

    /// Adds bounded FM26 roster attack/defence priors to the trained results model.
    /// The priors are source-neutral pre-match indices, with coverage shrinkage
    /// applied upstream; coefficient 0 gives the strictly result-driven baseline.
    pub fn forecast_with_fm(&self, home: &str, away: &str, home_attack: f64, home_defense: f64,
                            away_attack: f64, away_defense: f64, coefficient: f64) -> GoalScoreForecast {
        let baseline = self.forecast(home, away);
        if !coefficient.is_finite() || coefficient < 0.0 || coefficient > 0.5 {
            return baseline;
        }
        score_distribution(
            (baseline.home_goals * (coefficient * (home_attack - away_defense)).exp()).clamp(0.15, 5.0),
            (baseline.away_goals * (coefficient * (away_attack - home_defense)).exp()).clamp(0.15, 5.0),
        )
    }

    pub fn forecast(&self, home: &str, away: &str) -> GoalScoreForecast {
        let home_attack = *self.attack.get(home).unwrap_or(&0.0);
        let away_attack = *self.attack.get(away).unwrap_or(&0.0);
        let home_defense = *self.defense.get(home).unwrap_or(&0.0);
        let away_defense = *self.defense.get(away).unwrap_or(&0.0);
        let home_mu = (self.away_baseline * (self.home_log_advantage + home_attack - away_defense).exp()).clamp(0.15, 5.0);
        let away_mu = (self.away_baseline * (away_attack - home_defense).exp()).clamp(0.15, 5.0);
        score_distribution(home_mu, away_mu)
    }
}

fn poisson(mu: f64) -> [f64; 13] {
    let mut p = [0.0; 13];
    p[0] = (-mu).exp();
    for i in 1..p.len() { p[i] = p[i-1] * mu / i as f64; }
    p
}

pub fn score_distribution(home_mu: f64, away_mu: f64) -> GoalScoreForecast {
    let h=poisson(home_mu); let a=poisson(away_mu);
    let mut outcomes=[0.0;3];
    let mut modal_home=0; let mut modal_away=0; let mut modal_probability=0.0;
    let mut mass=0.0;
    for (i, &ph) in h.iter().enumerate() {
        for (j, &pa) in a.iter().enumerate() {
            let p=ph*pa;
            mass+=p;
            outcomes[if i>j {0} else if i==j {1} else {2}]+=p;
            if p > modal_probability {modal_probability=p; modal_home=i;modal_away=j;}
        }
    }
    for v in &mut outcomes {*v/=mass;}
    GoalScoreForecast {home_goals:home_mu,away_goals:away_mu,outcomes,modal_home,modal_away,modal_probability:modal_probability/mass}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn score_distribution_normalizes() {
        let x=score_distribution(1.8,1.1);
        assert!((x.outcomes.iter().sum::<f64>()-1.0).abs()<1e-12);
        assert!(x.outcomes[0]>x.outcomes[2]);
    }
    #[test]
    fn fit_ignores_future_and_same_day_games() {
        let games: Vec<_>=(0..70).map(|i| TeamGoalsObservation {day:i,home:"A".into(),away:"B".into(),home_goals:2,away_goals:1}).collect();
        let a=TeamGoalModel::fit(&games,&[],60).unwrap();
        let b=TeamGoalModel::fit(&games[..60],&[],60).unwrap();
        assert_eq!(a.forecast("A","B").outcomes,b.forecast("A","B").outcomes);
    }
    #[test]
    fn symmetric_ratings_favor_home() {
        let mut games=Vec::new();
        for i in 0..60 {games.push(TeamGoalsObservation{day:i,home:"A".into(),away:"B".into(),home_goals:2,away_goals:1});games.push(TeamGoalsObservation{day:i,home:"B".into(),away:"A".into(),home_goals:2,away_goals:1});}
        let x=TeamGoalModel::fit(&games,&[],60).unwrap().forecast("A","B");
        assert!(x.outcomes[0]>x.outcomes[2]);
    }
}
