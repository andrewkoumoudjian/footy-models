use footy_core::{ClubId, Date, PlayerId, Probability, RoleId};
use footy_forecast::Rng64;
use footy_state::PlayerState;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TacticalSlot {
    pub slot: u8,
    pub role: RoleId,
    /// Own-goal = 0, opponent-goal = 1.
    pub x: f64,
    /// Team-left = 0, team-right = 1.
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormationTemplate {
    pub id: u16,
    pub slots: Vec<TacticalSlot>,
}

impl FormationTemplate {
    pub fn validate(&self) -> bool {
        if self.slots.len() != 11 { return false; }
        let mut ids = BTreeSet::new();
        self.slots.iter().all(|slot| {
            ids.insert(slot.slot)
                && slot.x.is_finite()
                && slot.y.is_finite()
                && (0.0..=1.0).contains(&slot.x)
                && (0.0..=1.0).contains(&slot.y)
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormationPrior {
    pub formation: FormationTemplate,
    pub probability: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoleFit {
    pub role: RoleId,
    pub fit: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineupCandidate {
    pub player: PlayerId,
    /// Pre-match probability of starting before exact role assignment.
    pub start_probability: f64,
    /// Probability the player is available to be selected on this path.
    pub availability_probability: f64,
    /// Source-neutral latent quality index. It is standardized within the squad before use.
    pub ability_index: f64,
    /// 0 = fully fresh, 1 = maximal modeled fatigue.
    pub fatigue: f64,
    pub expected_minutes_if_start: f64,
    pub minutes_sigma: f64,
    /// Retained for later substitution/game-state simulation and reporting.
    pub substitute_probability: f64,
    pub role_fits: Vec<RoleFit>,
}

impl LineupCandidate {
    pub fn role_fit(&self, role: RoleId) -> f64 {
        self.role_fits
            .iter()
            .find(|value| value.role == role)
            .map(|value| value.fit.clamp(0.0, 1.0))
            .unwrap_or(0.0)
    }

    fn validate(&self) -> bool {
        self.start_probability.is_finite()
            && (0.0..=1.0).contains(&self.start_probability)
            && self.availability_probability.is_finite()
            && (0.0..=1.0).contains(&self.availability_probability)
            && self.ability_index.is_finite()
            && self.fatigue.is_finite()
            && (0.0..=1.0).contains(&self.fatigue)
            && self.expected_minutes_if_start.is_finite()
            && (0.0..=90.0).contains(&self.expected_minutes_if_start)
            && self.minutes_sigma.is_finite()
            && self.minutes_sigma >= 0.0
            && self.substitute_probability.is_finite()
            && (0.0..=1.0).contains(&self.substitute_probability)
            && self.role_fits.iter().all(|fit| fit.fit.is_finite() && (0.0..=1.0).contains(&fit.fit))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockedStarter {
    pub player: PlayerId,
    /// When absent, the player is confirmed as a starter but his slot/role remains stochastic.
    pub slot: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineupSamplingParameters {
    /// Exponent on role suitability. Larger values punish awkward assignments more strongly.
    pub role_fit_power: f64,
    /// Weight on within-squad standardized latent ability.
    pub ability_weight: f64,
    /// Exponential penalty applied to fatigue.
    pub fatigue_weight: f64,
    /// Used only to order scarce positions before flexible positions.
    pub scarce_role_fit_threshold: f64,
    /// Availability draws are rejected until the team can field eleven players.
    pub max_availability_resamples: u8,
}

impl Default for LineupSamplingParameters {
    fn default() -> Self {
        Self {
            role_fit_power: 2.0,
            ability_weight: 0.35,
            fatigue_weight: 0.75,
            scarce_role_fit_threshold: 0.35,
            max_availability_resamples: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineupProjectionInput {
    pub club: ClubId,
    pub formations: Vec<FormationPrior>,
    pub candidates: Vec<LineupCandidate>,
    /// Confirmed starters can be all eleven, none, or a partial lineup.
    pub locked_starters: Vec<LockedStarter>,
    pub parameters: LineupSamplingParameters,
}

impl LineupProjectionInput {
    pub fn validate(&self) -> bool {
        if self.candidates.len() < 11 || self.formations.is_empty() { return false; }
        if !self.parameters.role_fit_power.is_finite()
            || self.parameters.role_fit_power < 0.0
            || !self.parameters.ability_weight.is_finite()
            || !self.parameters.fatigue_weight.is_finite()
            || self.parameters.fatigue_weight < 0.0
            || !self.parameters.scarce_role_fit_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.parameters.scarce_role_fit_threshold)
        {
            return false;
        }
        let mut players = BTreeSet::new();
        if !self.candidates.iter().all(|candidate| players.insert(candidate.player) && candidate.validate()) {
            return false;
        }
        let mut locked = BTreeSet::new();
        if !self.locked_starters.iter().all(|value| players.contains(&value.player) && locked.insert(value.player)) {
            return false;
        }
        let mut formation_ids = BTreeSet::new();
        let mut total = 0.0;
        for prior in &self.formations {
            if !formation_ids.insert(prior.formation.id)
                || !prior.formation.validate()
                || !prior.probability.is_finite()
                || prior.probability < 0.0
            {
                return false;
            }
            total += prior.probability;
        }
        total > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampledStarter {
    pub player: PlayerId,
    pub slot: u8,
    pub role: RoleId,
    pub x: f64,
    pub y: f64,
    pub minutes: f64,
    pub role_fit: f64,
    pub fatigue: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampledLineup {
    pub club: ClubId,
    pub formation_id: u16,
    pub starters: Vec<SampledStarter>,
}

impl SampledLineup {
    pub fn starter(&self, player: PlayerId) -> Option<&SampledStarter> {
        self.starters.iter().find(|starter| starter.player == player)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoleProjection {
    pub role: RoleId,
    pub probability_given_start: Probability,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerLineupProjection {
    pub player: PlayerId,
    pub start_probability: Probability,
    pub expected_minutes_unconditional: f64,
    pub expected_minutes_if_start: f64,
    pub roles: Vec<RoleProjection>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationProjection {
    pub formation_id: u16,
    pub probability: Probability,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineupProjection {
    pub club: ClubId,
    pub formations: Vec<FormationProjection>,
    pub players: Vec<PlayerLineupProjection>,
    pub paths: u32,
}

/// Historical appearance primitive used to calibrate a projected XI without leaking future team
/// selections. Absence is represented by a team fixture with no appearance row, not by a fake zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionObservation {
    pub player: PlayerId,
    pub club: ClubId,
    pub date: Date,
    pub started: bool,
    pub minutes: u16,
    pub role: Option<RoleId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionPrior {
    pub start_probability: f64,
    pub appearance_probability: f64,
    pub strength_fixtures: f64,
    pub minutes_if_start: f64,
    pub minute_strength_starts: f64,
}

impl SelectionPrior {
    pub fn validate(self) -> bool {
        self.start_probability.is_finite()
            && (0.0..=1.0).contains(&self.start_probability)
            && self.appearance_probability.is_finite()
            && (0.0..=1.0).contains(&self.appearance_probability)
            && self.strength_fixtures.is_finite()
            && self.strength_fixtures >= 0.0
            && self.minutes_if_start.is_finite()
            && (0.0..=90.0).contains(&self.minutes_if_start)
            && self.minute_strength_starts.is_finite()
            && self.minute_strength_starts >= 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoleSelectionShare {
    pub role: RoleId,
    pub share_of_starts: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionWindow {
    pub player: PlayerId,
    pub club: ClubId,
    pub as_of: Date,
    pub start_probability: f64,
    pub appearance_probability: f64,
    pub expected_minutes_if_start: f64,
    pub roles: Vec<RoleSelectionShare>,
    pub effective_fixtures: f64,
    pub effective_starts: f64,
}

/// Build exponentially weighted, empirical-Bayes selection state. `team_fixture_dates` must contain
/// the club's fixtures, not merely the player's appearances, so missed selections affect the rate.
pub fn build_selection_window(
    observations: &[SelectionObservation],
    team_fixture_dates: &[Date],
    player: PlayerId,
    club: ClubId,
    as_of: Date,
    horizon_days: u16,
    half_life_days: f64,
    prior: SelectionPrior,
) -> Option<SelectionWindow> {
    if !prior.validate() || horizon_days == 0 { return None; }
    let horizon = i64::from(horizon_days);
    let half_life = if half_life_days.is_finite() && half_life_days > 0.0 {
        half_life_days
    } else {
        f64::from(horizon_days) / 2.0
    };
    let mut dates = team_fixture_dates.to_vec();
    dates.sort();
    dates.dedup();
    let mut fixture_weight = 0.0;
    for date in dates {
        let age = as_of.days_since(date);
        if age >= 0 && age < horizon {
            fixture_weight += decay(age, half_life);
        }
    }

    let mut start_weight = 0.0;
    let mut appearance_weight = 0.0;
    let mut minute_weight = 0.0;
    let mut weighted_minutes = 0.0;
    let mut roles = BTreeMap::<RoleId, f64>::new();
    for observation in observations {
        if observation.player != player || observation.club != club { continue; }
        let age = as_of.days_since(observation.date);
        if age < 0 || age >= horizon { continue; }
        let weight = decay(age, half_life);
        if observation.minutes > 0 { appearance_weight += weight; }
        if observation.started {
            start_weight += weight;
            weighted_minutes += f64::from(observation.minutes.min(90)) * weight;
            minute_weight += weight;
            if let Some(role) = observation.role {
                *roles.entry(role).or_default() += weight;
            }
        }
    }

    let rate_denominator = fixture_weight + prior.strength_fixtures;
    let start_probability = if rate_denominator > 0.0 {
        (start_weight + prior.start_probability * prior.strength_fixtures) / rate_denominator
    } else {
        prior.start_probability
    };
    let appearance_probability = if rate_denominator > 0.0 {
        (appearance_weight + prior.appearance_probability * prior.strength_fixtures) / rate_denominator
    } else {
        prior.appearance_probability
    };
    let minute_denominator = minute_weight + prior.minute_strength_starts;
    let expected_minutes_if_start = if minute_denominator > 0.0 {
        (weighted_minutes + prior.minutes_if_start * prior.minute_strength_starts) / minute_denominator
    } else {
        prior.minutes_if_start
    };
    let role_total = roles.values().sum::<f64>();
    let roles = if role_total > 0.0 {
        roles
            .into_iter()
            .map(|(role, weight)| RoleSelectionShare { role, share_of_starts: weight / role_total })
            .collect()
    } else {
        Vec::new()
    };

    Some(SelectionWindow {
        player,
        club,
        as_of,
        start_probability: start_probability.clamp(0.0, 1.0),
        appearance_probability: appearance_probability.max(start_probability).clamp(0.0, 1.0),
        expected_minutes_if_start: expected_minutes_if_start.clamp(0.0, 90.0),
        roles,
        effective_fixtures: fixture_weight,
        effective_starts: start_weight,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormationObservation {
    pub club: ClubId,
    pub date: Date,
    pub formation_id: u16,
}

/// Convert historical formations into a causal probability distribution with a uniform pseudo-count
/// so an infrequently used but valid template never becomes impossible solely from a short window.
pub fn calibrate_formation_priors(
    templates: &[FormationTemplate],
    observations: &[FormationObservation],
    club: ClubId,
    as_of: Date,
    horizon_days: u16,
    half_life_days: f64,
    uniform_prior_strength: f64,
) -> Option<Vec<FormationPrior>> {
    if templates.is_empty()
        || templates.iter().any(|formation| !formation.validate())
        || horizon_days == 0
        || !uniform_prior_strength.is_finite()
        || uniform_prior_strength < 0.0
    {
        return None;
    }
    let half_life = if half_life_days.is_finite() && half_life_days > 0.0 {
        half_life_days
    } else {
        f64::from(horizon_days) / 2.0
    };
    let ids: BTreeSet<u16> = templates.iter().map(|value| value.id).collect();
    if ids.len() != templates.len() { return None; }
    let mut counts = BTreeMap::<u16, f64>::new();
    let prior_each = uniform_prior_strength / templates.len() as f64;
    for template in templates { counts.insert(template.id, prior_each); }
    for observation in observations {
        if observation.club != club || !ids.contains(&observation.formation_id) { continue; }
        let age = as_of.days_since(observation.date);
        if age < 0 || age >= i64::from(horizon_days) { continue; }
        *counts.entry(observation.formation_id).or_default() += decay(age, half_life);
    }
    let total = counts.values().sum::<f64>();
    if total <= 0.0 { return None; }
    Some(templates.iter().cloned().map(|formation| FormationPrior {
        probability: counts.get(&formation.id).copied().unwrap_or(0.0) / total,
        formation,
    }).collect())
}

/// Use the canonical player state and causal selection window to create the source-neutral lineup
/// candidate consumed by the stochastic XI sampler.
pub fn lineup_candidate_from_state(
    state: &PlayerState,
    selection: &SelectionWindow,
    availability_probability: f64,
    fatigue: f64,
    minutes_sigma: f64,
) -> Option<LineupCandidate> {
    if state.player != selection.player
        || state.as_of != selection.as_of
        || !availability_probability.is_finite()
        || !(0.0..=1.0).contains(&availability_probability)
        || !fatigue.is_finite()
        || !(0.0..=1.0).contains(&fatigue)
        || !minutes_sigma.is_finite()
        || minutes_sigma < 0.0
    {
        return None;
    }
    let mut roles = BTreeMap::<RoleId, (f64, f64)>::new();
    for role in &state.roles.scores {
        roles.entry(role.role).or_default().0 = f64::from(role.score.max(0.0));
    }
    for role in &selection.roles {
        roles.entry(role.role).or_default().1 = role.share_of_starts.max(0.0);
    }
    let role_fits = roles.into_iter().map(|(role, (state_fit, recent_share))| RoleFit {
        role,
        fit: (0.75 * state_fit + 0.25 * recent_share).clamp(0.0, 1.0),
    }).collect();
    Some(LineupCandidate {
        player: state.player,
        start_probability: selection.start_probability,
        availability_probability,
        ability_index: f64::from(state.ability.mean()),
        fatigue,
        expected_minutes_if_start: selection.expected_minutes_if_start,
        minutes_sigma,
        substitute_probability: (selection.appearance_probability - selection.start_probability).max(0.0),
        role_fits,
    })
}

/// Sample one XI. Unknown formation, availability, starters, roles, and starter minutes are all
/// path variables. Confirmed starters simply collapse the relevant uncertainty dimensions.
pub fn sample_lineup(input: &LineupProjectionInput, rng: &mut Rng64) -> Option<SampledLineup> {
    if !input.validate() { return None; }
    let formation_index = weighted_index(input.formations.iter().map(|value| value.probability), rng)?;
    let formation = &input.formations[formation_index].formation;
    let candidate_map: BTreeMap<PlayerId, &LineupCandidate> = input.candidates.iter().map(|value| (value.player, value)).collect();
    let locked_players: BTreeSet<PlayerId> = input.locked_starters.iter().map(|value| value.player).collect();

    let mut sampled_available = BTreeSet::<PlayerId>::new();
    let attempts = input.parameters.max_availability_resamples.max(1);
    for _ in 0..attempts {
        sampled_available.clear();
        sampled_available.extend(locked_players.iter().copied());
        for candidate in &input.candidates {
            if locked_players.contains(&candidate.player) { continue; }
            if rng.bernoulli(candidate.availability_probability) {
                sampled_available.insert(candidate.player);
            }
        }
        if sampled_available.len() >= 11 { break; }
    }
    if sampled_available.len() < 11 { return None; }

    let (ability_mean, ability_sigma) = squad_ability_moments(&input.candidates);
    let mut remaining_slots = formation.slots.clone();
    let mut used_players = BTreeSet::<PlayerId>::new();
    let mut starters = Vec::<SampledStarter>::with_capacity(11);

    // First honor confirmed starter/slot pairs exactly.
    for locked in input.locked_starters.iter().filter(|value| value.slot.is_some()) {
        let candidate = *candidate_map.get(&locked.player)?;
        let slot_id = locked.slot?;
        let position = remaining_slots.iter().position(|value| value.slot == slot_id)?;
        let slot = remaining_slots.remove(position);
        used_players.insert(candidate.player);
        starters.push(make_starter(candidate, slot, rng));
    }

    // Confirmed starter with unknown position: project only the role/slot conditional on starting.
    for locked in input.locked_starters.iter().filter(|value| value.slot.is_none()) {
        if used_players.contains(&locked.player) { continue; }
        let candidate = *candidate_map.get(&locked.player)?;
        let index = weighted_index(
            remaining_slots.iter().map(|slot| (candidate.role_fit(slot.role) + 0.01).powf(input.parameters.role_fit_power)),
            rng,
        )?;
        let slot = remaining_slots.remove(index);
        used_players.insert(candidate.player);
        starters.push(make_starter(candidate, slot, rng));
    }

    // Assign scarce positions first. This avoids consuming the only credible goalkeeper/full-back
    // candidate on a flexible midfield slot merely because that slot happened to appear earlier.
    remaining_slots.sort_by(|left, right| {
        let left_count = eligible_count(left.role, &input.candidates, &sampled_available, &used_players, input.parameters.scarce_role_fit_threshold);
        let right_count = eligible_count(right.role, &input.candidates, &sampled_available, &used_players, input.parameters.scarce_role_fit_threshold);
        left_count.cmp(&right_count).then(left.slot.cmp(&right.slot))
    });

    for slot in remaining_slots {
        let available: Vec<&LineupCandidate> = input.candidates.iter()
            .filter(|candidate| sampled_available.contains(&candidate.player) && !used_players.contains(&candidate.player))
            .collect();
        if available.is_empty() { return None; }
        let weights = available.iter().map(|candidate| selection_weight(
            candidate,
            slot.role,
            ability_mean,
            ability_sigma,
            input.parameters,
        ));
        let selected = available[weighted_index(weights, rng)?];
        used_players.insert(selected.player);
        starters.push(make_starter(selected, slot, rng));
    }

    if starters.len() != 11 { return None; }
    starters.sort_by_key(|value| value.slot);
    Some(SampledLineup { club: input.club, formation_id: formation.id, starters })
}

pub fn project_lineup(input: &LineupProjectionInput, paths: u32, seed: u64) -> Option<LineupProjection> {
    if paths == 0 || !input.validate() { return None; }
    let mut rng = Rng64::new(seed);
    let mut start_counts = BTreeMap::<PlayerId, u32>::new();
    let mut minute_totals = BTreeMap::<PlayerId, f64>::new();
    let mut role_counts = BTreeMap::<(PlayerId, RoleId), u32>::new();
    let mut formation_counts = BTreeMap::<u16, u32>::new();
    for _ in 0..paths {
        let lineup = sample_lineup(input, &mut rng)?;
        *formation_counts.entry(lineup.formation_id).or_default() += 1;
        for starter in lineup.starters {
            *start_counts.entry(starter.player).or_default() += 1;
            *minute_totals.entry(starter.player).or_default() += starter.minutes;
            *role_counts.entry((starter.player, starter.role)).or_default() += 1;
        }
    }
    let denominator = paths as f32;
    let formations = input.formations.iter().map(|value| FormationProjection {
        formation_id: value.formation.id,
        probability: Probability::clamped(formation_counts.get(&value.formation.id).copied().unwrap_or(0) as f32 / denominator),
    }).collect();
    let players = input.candidates.iter().map(|candidate| {
        let starts = start_counts.get(&candidate.player).copied().unwrap_or(0);
        let minutes = minute_totals.get(&candidate.player).copied().unwrap_or(0.0);
        let roles = if starts > 0 {
            candidate.role_fits.iter().filter_map(|fit| {
                let count = role_counts.get(&(candidate.player, fit.role)).copied().unwrap_or(0);
                (count > 0).then_some(RoleProjection {
                    role: fit.role,
                    probability_given_start: Probability::clamped(count as f32 / starts as f32),
                })
            }).collect()
        } else { Vec::new() };
        PlayerLineupProjection {
            player: candidate.player,
            start_probability: Probability::clamped(starts as f32 / denominator),
            expected_minutes_unconditional: minutes / f64::from(paths),
            expected_minutes_if_start: if starts > 0 { minutes / f64::from(starts) } else { 0.0 },
            roles,
        }
    }).collect();
    Some(LineupProjection { club: input.club, formations, players, paths })
}

/// Counterfactual primitive: preserve every other pre-match assumption and force one player out.
pub fn with_player_unavailable(input: &LineupProjectionInput, player: PlayerId) -> Option<LineupProjectionInput> {
    let mut out = input.clone();
    if out.locked_starters.iter().any(|value| value.player == player) { return None; }
    let candidate = out.candidates.iter_mut().find(|value| value.player == player)?;
    candidate.availability_probability = 0.0;
    Some(out)
}

fn make_starter(candidate: &LineupCandidate, slot: TacticalSlot, rng: &mut Rng64) -> SampledStarter {
    let minutes = if candidate.minutes_sigma <= f64::EPSILON {
        candidate.expected_minutes_if_start
    } else {
        candidate.expected_minutes_if_start + candidate.minutes_sigma * rng.normal()
    }.clamp(1.0, 90.0);
    SampledStarter {
        player: candidate.player,
        slot: slot.slot,
        role: slot.role,
        x: slot.x,
        y: slot.y,
        minutes,
        role_fit: candidate.role_fit(slot.role),
        fatigue: candidate.fatigue,
    }
}

fn eligible_count(
    role: RoleId,
    candidates: &[LineupCandidate],
    available: &BTreeSet<PlayerId>,
    used: &BTreeSet<PlayerId>,
    threshold: f64,
) -> usize {
    candidates.iter().filter(|candidate| {
        available.contains(&candidate.player)
            && !used.contains(&candidate.player)
            && candidate.role_fit(role) >= threshold
    }).count()
}

fn selection_weight(
    candidate: &LineupCandidate,
    role: RoleId,
    ability_mean: f64,
    ability_sigma: f64,
    parameters: LineupSamplingParameters,
) -> f64 {
    let start = candidate.start_probability.max(1e-6);
    let role_fit = (candidate.role_fit(role) + 0.01).powf(parameters.role_fit_power);
    let z = if ability_sigma > f64::EPSILON { (candidate.ability_index - ability_mean) / ability_sigma } else { 0.0 };
    let ability = (parameters.ability_weight * z).exp();
    let fatigue = (-parameters.fatigue_weight * candidate.fatigue).exp();
    start * role_fit * ability * fatigue
}

fn squad_ability_moments(candidates: &[LineupCandidate]) -> (f64, f64) {
    let mean = candidates.iter().map(|value| value.ability_index).sum::<f64>() / candidates.len() as f64;
    let variance = if candidates.len() > 1 {
        candidates.iter().map(|value| (value.ability_index - mean).powi(2)).sum::<f64>() / (candidates.len() - 1) as f64
    } else { 0.0 };
    (mean, variance.sqrt())
}

fn weighted_index<I>(weights: I, rng: &mut Rng64) -> Option<usize>
where
    I: IntoIterator<Item = f64>,
{
    let values: Vec<f64> = weights.into_iter().map(|value| if value.is_finite() && value > 0.0 { value } else { 0.0 }).collect();
    let total = values.iter().sum::<f64>();
    if values.is_empty() || total <= 0.0 { return None; }
    let target = rng.uniform() * total;
    let mut cumulative = 0.0;
    for (index, weight) in values.iter().enumerate() {
        cumulative += *weight;
        if target <= cumulative { return Some(index); }
    }
    Some(values.len() - 1)
}

fn decay(age_days: i64, half_life_days: f64) -> f64 {
    (-std::f64::consts::LN_2 * age_days as f64 / half_life_days).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use footy_core::RoleId;

    fn formation() -> FormationTemplate {
        FormationTemplate {
            id: 433,
            slots: (0..11).map(|slot| TacticalSlot {
                slot,
                role: RoleId(slot as u16 + 1),
                x: if slot == 0 { 0.05 } else { 0.2 + 0.07 * f64::from(slot) },
                y: f64::from(slot % 5) / 4.0,
            }).collect(),
        }
    }

    fn candidate(player: u64, preferred: u16, start: f64) -> LineupCandidate {
        let mut fits = Vec::new();
        for role in 1..=11 {
            fits.push(RoleFit { role: RoleId(role), fit: if role == preferred { 1.0 } else { 0.15 } });
        }
        LineupCandidate {
            player: PlayerId(player), start_probability: start, availability_probability: 1.0,
            ability_index: player as f64, fatigue: 0.0, expected_minutes_if_start: 80.0,
            minutes_sigma: 4.0, substitute_probability: 0.1, role_fits: fits,
        }
    }

    fn input() -> LineupProjectionInput {
        LineupProjectionInput {
            club: ClubId(1),
            formations: vec![FormationPrior { formation: formation(), probability: 1.0 }],
            candidates: (1..=16).map(|player| candidate(player, ((player - 1) % 11 + 1) as u16, if player <= 11 { 0.8 } else { 0.2 })).collect(),
            locked_starters: Vec::new(),
            parameters: LineupSamplingParameters::default(),
        }
    }

    #[test]
    fn lineup_projection_is_reproducible_and_fields_eleven() {
        let first = project_lineup(&input(), 500, 42).unwrap();
        let second = project_lineup(&input(), 500, 42).unwrap();
        assert_eq!(first, second);
        let starts = first.players.iter().map(|value| f64::from(value.start_probability.get())).sum::<f64>();
        assert!((starts - 11.0).abs() < 1e-4);
    }

    #[test]
    fn confirmed_starter_collapses_start_uncertainty() {
        let mut value = input();
        value.locked_starters.push(LockedStarter { player: PlayerId(16), slot: Some(0) });
        let projection = project_lineup(&value, 200, 9).unwrap();
        assert_eq!(projection.players.iter().find(|value| value.player == PlayerId(16)).unwrap().start_probability, Probability::ONE);
    }

    #[test]
    fn unavailable_counterfactual_never_starts_player() {
        let value = with_player_unavailable(&input(), PlayerId(1)).unwrap();
        let projection = project_lineup(&value, 200, 7).unwrap();
        assert_eq!(projection.players.iter().find(|value| value.player == PlayerId(1)).unwrap().start_probability, Probability::ZERO);
    }

    #[test]
    fn selection_window_counts_missing_appearance_via_team_fixtures() {
        let observations = [
            SelectionObservation { player: PlayerId(1), club: ClubId(1), date: Date(8), started: true, minutes: 90, role: Some(RoleId(7)) },
        ];
        let prior = SelectionPrior { start_probability: 0.5, appearance_probability: 0.7, strength_fixtures: 1.0, minutes_if_start: 75.0, minute_strength_starts: 1.0 };
        let window = build_selection_window(&observations, &[Date(6), Date(8)], PlayerId(1), ClubId(1), Date(10), 30, 15.0, prior).unwrap();
        assert!(window.start_probability < 0.75);
        assert!(window.start_probability > 0.25);
    }
}
