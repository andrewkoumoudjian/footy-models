#![forbid(unsafe_code)]
//! Recruitment decision primitives preserving financial, sporting, fit, and risk components.

use footy_core::{Confidence, Date, EuroCents, PlayerId, Probability, RoleId};
use footy_market::{
    AcquisitionPrice, FairWage, FundamentalValue, FutureValue, MarketPerceivedValue, PriceDistribution,
    TransferHazard,
};
use footy_state::{RoleScore, RoleVector};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecruitmentAlpha {
    pub current_gap: EuroCents,
    pub development_gap: EuroCents,
    pub expected_net_value: EuroCents,
    pub current_gap_ratio: Option<f64>,
}

pub fn recruitment_alpha(
    fundamental: FundamentalValue,
    acquisition: AcquisitionPrice,
    future: FutureValue,
    total_cost: EuroCents,
) -> RecruitmentAlpha {
    let current_gap = EuroCents(fundamental.0.p50.cents().saturating_sub(acquisition.0.p50.cents()));
    let development_gap = EuroCents(future.0.p50.cents().saturating_sub(fundamental.0.p50.cents()));
    let expected_net_value = EuroCents(future.0.p50.cents().saturating_sub(total_cost.cents()));
    let current_gap_ratio = (acquisition.0.p50.cents() > 0).then(|| current_gap.cents() as f64 / acquisition.0.p50.cents() as f64);
    RecruitmentAlpha { current_gap, development_gap, expected_net_value, current_gap_ratio }
}

/// Acquisition costs retain component-level missingness. `known_total` never silently treats an
/// unknown fee/bonus as a known zero; callers can inspect `is_complete` before capital decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TotalAcquisitionCost {
    pub transfer_fee: EuroCents,
    pub wages_npv: EuroCents,
    pub agent_fee: Option<EuroCents>,
    pub signing_fee: Option<EuroCents>,
    pub expected_bonuses: Option<EuroCents>,
    pub other_costs: Option<EuroCents>,
}

impl TotalAcquisitionCost {
    pub fn known_total(self) -> Option<EuroCents> {
        let mut total = self.transfer_fee.checked_add(self.wages_npv).ok()?;
        for value in [self.agent_fee, self.signing_fee, self.expected_bonuses, self.other_costs].into_iter().flatten() {
            total = total.checked_add(value).ok()?;
        }
        Some(total)
    }

    pub const fn is_complete(self) -> bool {
        self.agent_fee.is_some() && self.signing_fee.is_some() && self.expected_bonuses.is_some() && self.other_costs.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnaloguePoint {
    pub player: PlayerId,
    pub as_of: Date,
    pub features: Vec<f32>,
    pub future_return: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogueMatch {
    pub player: PlayerId,
    pub as_of: Date,
    pub distance: f32,
    pub future_return: Option<f32>,
}

/// Find historical analogues using only candidate states at or before the query date.
pub fn historical_analogues(
    query_player: PlayerId,
    query_date: Date,
    query_features: &[f32],
    candidates: &[AnaloguePoint],
    weights: &[f32],
    limit: usize,
) -> Vec<AnalogueMatch> {
    if query_features.is_empty() || query_features.len() != weights.len() || weights.iter().any(|weight| !weight.is_finite() || *weight < 0.0) {
        return Vec::new();
    }
    let weight_sum = weights.iter().sum::<f32>();
    if weight_sum <= 0.0 { return Vec::new(); }
    let mut matches: Vec<AnalogueMatch> = candidates.iter().filter_map(|candidate| {
        if candidate.player == query_player || candidate.as_of > query_date || candidate.features.len() != query_features.len() { return None; }
        if candidate.features.iter().any(|value| !value.is_finite()) || query_features.iter().any(|value| !value.is_finite()) { return None; }
        let squared = query_features.iter().zip(&candidate.features).zip(weights).map(|((left, right), weight)| weight * (left - right).powi(2)).sum::<f32>();
        Some(AnalogueMatch { player: candidate.player, as_of: candidate.as_of, distance: (squared / weight_sum).sqrt(), future_return: candidate.future_return })
    }).collect();
    matches.sort_by(|left, right| left.distance.total_cmp(&right.distance).then_with(|| left.player.cmp(&right.player)));
    matches.truncate(limit);
    matches
}

/// Cosine similarity over role scores, mapped to `[0, 1]` for non-negative role vectors.
pub fn tactical_fit(player: &RoleVector, requirement: &RoleVector) -> f32 {
    let mut dot = 0.0_f32;
    let mut player_norm = 0.0_f32;
    let mut requirement_norm = 0.0_f32;
    for player_score in &player.scores { player_norm += player_score.score.max(0.0).powi(2); }
    for requirement_score in &requirement.scores { requirement_norm += requirement_score.score.max(0.0).powi(2); }
    for player_score in &player.scores {
        if let Some(required) = requirement.scores.iter().find(|value| value.role == player_score.role) {
            dot += player_score.score.max(0.0) * required.score.max(0.0);
        }
    }
    if player_norm <= 0.0 || requirement_norm <= 0.0 { 0.0 } else { (dot / (player_norm.sqrt() * requirement_norm.sqrt())).clamp(0.0, 1.0) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquadRoleNeed {
    pub role: RoleId,
    pub importance: f32,
    pub current_coverage: f32,
}

/// Marginal role coverage supplied where the squad is currently deficient.
pub fn squad_complementarity(player: &RoleVector, needs: &[SquadRoleNeed]) -> f32 {
    let denominator = needs.iter().map(|need| need.importance.max(0.0)).sum::<f32>();
    if denominator <= 0.0 { return 0.0; }
    let gain = needs.iter().map(|need| {
        let score = player.score(need.role).unwrap_or(0.0).max(0.0);
        need.importance.max(0.0) * (score - need.current_coverage.max(0.0)).max(0.0)
    }).sum::<f32>();
    (gain / denominator).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitComponent {
    pub score: f32,
    pub confidence: Confidence,
}

impl FitComponent {
    pub fn new(score: f32, confidence: Confidence) -> Self {
        Self { score: score.clamp(0.0, 1.0), confidence }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentFit {
    pub adaptation: FitComponent,
    pub league_transition: FitComponent,
    pub language_culture: FitComponent,
    pub playing_time_path: FitComponent,
}

impl EnvironmentFit {
    pub fn aggregate(self, weights: [f32; 4]) -> Option<FitComponent> {
        let components = [self.adaptation, self.league_transition, self.language_culture, self.playing_time_path];
        let mut numerator = 0.0_f32;
        let mut effective_weight = 0.0_f32;
        let mut nominal_weight = 0.0_f32;
        for (component, weight) in components.iter().zip(weights) {
            let weight = weight.max(0.0);
            nominal_weight += weight;
            let effective = weight * component.confidence.get();
            effective_weight += effective;
            numerator += component.score * effective;
        }
        if effective_weight <= 0.0 || nominal_weight <= 0.0 { None } else {
            Some(FitComponent::new(numerator / effective_weight, Confidence::clamped(effective_weight / nominal_weight)))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SquadMember {
    pub player: PlayerId,
    pub expected_minutes: u32,
    pub availability: f32,
    pub roles: RoleVector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SquadState {
    pub as_of: Date,
    pub members: Vec<SquadMember>,
}

impl SquadState {
    pub fn role_coverage(&self, role: RoleId) -> f32 {
        let maximum_minutes = self.members.iter().map(|member| member.expected_minutes).max().unwrap_or(0);
        if maximum_minutes == 0 { return 0.0; }
        self.members.iter().map(|member| {
            let minutes_weight = member.expected_minutes as f32 / maximum_minutes as f32;
            member.roles.score(role).unwrap_or(0.0).max(0.0) * minutes_weight * member.availability.clamp(0.0, 1.0)
        }).fold(0.0_f32, f32::max).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateAnalysis {
    pub player: PlayerId,
    pub perceived_value: Option<MarketPerceivedValue>,
    pub fundamental_value: FundamentalValue,
    pub acquisition_price: Option<AcquisitionPrice>,
    pub future_12m: Option<FutureValue>,
    pub future_24m: Option<FutureValue>,
    pub future_36m: Option<FutureValue>,
    pub transfer_hazard: Option<TransferHazard>,
    pub tactical_fit: Option<FitComponent>,
    pub squad_fit: Option<FitComponent>,
    pub environment_fit: Option<FitComponent>,
    pub fair_wage: Option<FairWage>,
    pub downside_probability: Option<Probability>,
    pub breakout_probability: Option<Probability>,
    pub data_confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateOption {
    pub player: PlayerId,
    pub total_cost: EuroCents,
    /// Expected incremental sporting value expressed in euros or another explicitly monetized unit.
    pub sporting_value_eur: f64,
    pub expected_resale_gain: EuroCents,
    pub fit: f32,
    pub risk: Probability,
}

fn valid_option(option: &CandidateOption) -> bool {
    option.total_cost.cents() >= 0 && option.sporting_value_eur.is_finite() && option.fit.is_finite()
}

/// Non-dominated options: lower cost/risk and higher sporting/resale/fit are preferred.
pub fn pareto_frontier(candidates: &[CandidateOption]) -> Vec<CandidateOption> {
    let valid: Vec<CandidateOption> = candidates.iter().copied().filter(valid_option).collect();
    let mut frontier = Vec::new();
    'candidate: for candidate in &valid {
        for other in &valid {
            if other.player == candidate.player { continue; }
            let weakly_better = other.total_cost <= candidate.total_cost
                && other.sporting_value_eur >= candidate.sporting_value_eur
                && other.expected_resale_gain >= candidate.expected_resale_gain
                && other.fit >= candidate.fit
                && other.risk.get() <= candidate.risk.get();
            let strictly_better = other.total_cost < candidate.total_cost
                || other.sporting_value_eur > candidate.sporting_value_eur
                || other.expected_resale_gain > candidate.expected_resale_gain
                || other.fit > candidate.fit
                || other.risk.get() < candidate.risk.get();
            if weakly_better && strictly_better { continue 'candidate; }
        }
        frontier.push(*candidate);
    }
    frontier.sort_by_key(|candidate| (candidate.total_cost, candidate.player));
    frontier
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectiveWeights {
    pub sporting: f64,
    pub resale: f64,
    pub fit: f64,
    pub risk_penalty: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectiveBreakdown {
    pub sporting: f64,
    pub resale: f64,
    pub fit: f64,
    pub risk_penalty: f64,
    pub total: f64,
}

pub fn objective(option: CandidateOption, weights: ObjectiveWeights) -> Option<ObjectiveBreakdown> {
    if !valid_option(&option) || [weights.sporting, weights.resale, weights.fit, weights.risk_penalty].iter().any(|value| !value.is_finite()) { return None; }
    let sporting = weights.sporting * option.sporting_value_eur;
    let resale = weights.resale * option.expected_resale_gain.cents() as f64 / 100.0;
    let fit = weights.fit * option.fit.clamp(0.0, 1.0) as f64;
    let risk_penalty = weights.risk_penalty * option.risk.get() as f64;
    Some(ObjectiveBreakdown { sporting, resale, fit, risk_penalty, total: sporting + resale + fit - risk_penalty })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMethod { GreedyExplicitObjective }

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub players: Vec<PlayerId>,
    pub total_cost: EuroCents,
    pub objective_total: f64,
    pub method: SelectionMethod,
}

/// Deterministic budget selection using an explicitly supplied objective. The objective components
/// remain inspectable; this is intentionally not presented as a universal recruitment score.
pub fn greedy_select(
    candidates: &[CandidateOption],
    budget: EuroCents,
    max_players: usize,
    weights: ObjectiveWeights,
) -> Selection {
    let mut ranked: Vec<(CandidateOption, ObjectiveBreakdown, f64)> = candidates.iter().copied().filter_map(|candidate| {
        let breakdown = objective(candidate, weights)?;
        if breakdown.total <= 0.0 { return None; }
        let denominator = (candidate.total_cost.cents().max(1) as f64) / 100.0;
        Some((candidate, breakdown, breakdown.total / denominator))
    }).collect();
    ranked.sort_by(|left, right| right.2.total_cmp(&left.2).then_with(|| left.0.player.cmp(&right.0.player)));
    let mut players = Vec::new();
    let mut total_cost = EuroCents::ZERO;
    let mut objective_total = 0.0;
    for (candidate, breakdown, _) in ranked {
        if players.len() >= max_players { break; }
        let Some(next_cost) = total_cost.checked_add(candidate.total_cost).ok() else { continue; };
        if next_cost > budget { continue; }
        total_cost = next_cost;
        players.push(candidate.player);
        objective_total += breakdown.total;
    }
    Selection { players, total_cost, objective_total, method: SelectionMethod::GreedyExplicitObjective }
}

pub fn distribution_gap(left: PriceDistribution, right: PriceDistribution) -> EuroCents {
    EuroCents(left.p50.cents().saturating_sub(right.p50.cents()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distribution(value: i64) -> PriceDistribution {
        PriceDistribution::point(EuroCents(value), Confidence::ONE).unwrap()
    }

    #[test]
    fn unknown_costs_are_not_silently_zeroed() {
        let cost = TotalAcquisitionCost { transfer_fee: EuroCents(100), wages_npv: EuroCents(50), agent_fee: None, signing_fee: Some(EuroCents(10)), expected_bonuses: Some(EuroCents(5)), other_costs: Some(EuroCents(0)) };
        assert_eq!(cost.known_total(), Some(EuroCents(165)));
        assert!(!cost.is_complete());
    }

    #[test]
    fn analogues_never_use_future_states() {
        let candidates = [
            AnaloguePoint { player: PlayerId(2), as_of: Date(9), features: vec![1.0], future_return: Some(0.5) },
            AnaloguePoint { player: PlayerId(3), as_of: Date(11), features: vec![1.0], future_return: Some(1.0) },
        ];
        let result = historical_analogues(PlayerId(1), Date(10), &[1.0], &candidates, &[1.0], 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].player, PlayerId(2));
    }

    #[test]
    fn tactical_and_squad_fit_are_separate() {
        let player = RoleVector { definition_version: 1, scores: vec![RoleScore { role: RoleId(1), score: 1.0 }] };
        let requirement = RoleVector { definition_version: 1, scores: vec![RoleScore { role: RoleId(1), score: 1.0 }] };
        assert_eq!(tactical_fit(&player, &requirement), 1.0);
        assert_eq!(squad_complementarity(&player, &[SquadRoleNeed { role: RoleId(1), importance: 1.0, current_coverage: 0.9 }]), 0.1);
    }

    #[test]
    fn dominated_candidate_is_removed_from_frontier() {
        let low_risk = Probability::new(0.1).unwrap();
        let candidates = [
            CandidateOption { player: PlayerId(1), total_cost: EuroCents(100), sporting_value_eur: 10.0, expected_resale_gain: EuroCents(50), fit: 0.8, risk: low_risk },
            CandidateOption { player: PlayerId(2), total_cost: EuroCents(120), sporting_value_eur: 9.0, expected_resale_gain: EuroCents(40), fit: 0.7, risk: Probability::new(0.2).unwrap() },
        ];
        let frontier = pareto_frontier(&candidates);
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier[0].player, PlayerId(1));
    }

    #[test]
    fn alpha_preserves_current_and_development_components() {
        let alpha = recruitment_alpha(FundamentalValue(distribution(200)), AcquisitionPrice(distribution(150)), FutureValue(distribution(300)), EuroCents(180));
        assert_eq!(alpha.current_gap, EuroCents(50));
        assert_eq!(alpha.development_gap, EuroCents(100));
        assert_eq!(alpha.expected_net_value, EuroCents(120));
    }
}
