use crate::simulation::{simulate_specific_match, SpecificMatchSimulationInput};
use footy_core::{ClubId, MatchId, PlayerId};
use footy_forecast::MatchOutcomeForecast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamSide {
    Home,
    Away,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAbsenceCounterfactual {
    pub match_id: MatchId,
    pub player: PlayerId,
    pub club: ClubId,
    pub side: TeamSide,
    pub baseline: MatchOutcomeForecast,
    pub without_player: MatchOutcomeForecast,
    /// Positive means the player's presence increases the home team's win probability.
    pub home_win_probability_delta: f64,
    pub draw_probability_delta: f64,
    pub away_win_probability_delta: f64,
    pub home_expected_points_delta: f64,
    pub away_expected_points_delta: f64,
    pub home_expected_goals_delta: f64,
    pub away_expected_goals_delta: f64,
}

/// Common-random-number counterfactual: use the same seed for baseline and intervention, forcing one
/// player unavailable and removing any confirmed-starter lock. The whole XI is then re-projected,
/// so the effect includes replacement quality, role reshuffling, teammate interactions, opponent
/// matchups, player event distributions, and finally the result distribution.
pub fn simulate_player_absence(
    input: &SpecificMatchSimulationInput,
    player: PlayerId,
    paths: u32,
    seed: u64,
) -> Option<PlayerAbsenceCounterfactual> {
    let home_has = input.home_lineup.candidates.iter().any(|value| value.player == player);
    let away_has = input.away_lineup.candidates.iter().any(|value| value.player == player);
    if home_has == away_has { return None; }

    let baseline = simulate_specific_match(input, paths, seed)?.outcome;
    let mut intervention = input.clone();
    let (club, side, lineup) = if home_has {
        (input.home, TeamSide::Home, &mut intervention.home_lineup)
    } else {
        (input.away, TeamSide::Away, &mut intervention.away_lineup)
    };
    lineup.locked_starters.retain(|value| value.player != player);
    let candidate = lineup.candidates.iter_mut().find(|value| value.player == player)?;
    candidate.availability_probability = 0.0;
    candidate.start_probability = 0.0;
    candidate.substitute_probability = 0.0;

    let without_player = simulate_specific_match(&intervention, paths, seed)?.outcome;
    Some(PlayerAbsenceCounterfactual {
        match_id: input.match_id,
        player,
        club,
        side,
        home_win_probability_delta: f64::from(baseline.home_win.get() - without_player.home_win.get()),
        draw_probability_delta: f64::from(baseline.draw.get() - without_player.draw.get()),
        away_win_probability_delta: f64::from(baseline.away_win.get() - without_player.away_win.get()),
        home_expected_points_delta: baseline.home_expected_points - without_player.home_expected_points,
        away_expected_points_delta: baseline.away_expected_points - without_player.away_expected_points,
        home_expected_goals_delta: baseline.home_goals.mean - without_player.home_goals.mean,
        away_expected_goals_delta: baseline.away_goals.mean - without_player.away_goals.mean,
        baseline,
        without_player,
    })
}

#[cfg(test)]
mod tests {
    // End-to-end counterfactual behavior is covered by the same simulation primitive; fixture-heavy
    // construction lives in `simulation` tests to keep this module focused on the intervention.
    use super::*;

    #[test]
    fn team_side_is_stable() {
        assert_ne!(TeamSide::Home, TeamSide::Away);
    }
}
