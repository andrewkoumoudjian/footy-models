use crate::second_resolved_mc;
use crate::{DynamicMentalTransitionParameters, MentalMatchModel, PrimitiveMatchSimulationInput};

pub use crate::second_resolved_mc::{
    SecondResolvedDynamicMentalForecast, SecondResolvedEventSummary,
    SecondResolvedPlayerMentalForecast, SecondResolvedSimulationParameters,
    SecondResolvedStateEventKind, DYNAMIC_MENTAL_REFERENCE_SECONDS,
};

/// Preferred public entry point for the one-second/sub-second ordered engine.
///
/// The internal engine keeps historical coefficient signs compatible with its first implementation,
/// where the negative-performance branch multiplies the magnitude by a branch-local signed weight.
/// This boundary normalizes caller-facing `DynamicMentalTransitionParameters` so the repository-wide
/// convention remains intuitive: success coefficients are normally positive and error coefficients
/// are normally negative.
pub fn simulate_second_resolved_dynamic_mental_match(
    input: &PrimitiveMatchSimulationInput,
    mental_model: &MentalMatchModel,
    transition: DynamicMentalTransitionParameters,
    parameters: SecondResolvedSimulationParameters,
    paths: u32,
    seed: u64,
) -> Option<SecondResolvedDynamicMentalForecast> {
    // A continuous exponential hazard cannot represent probability=1 with a finite intensity.
    // Exact certainty should be supplied as a scheduled/deterministic event primitive rather than
    // an incident probability. Reject it instead of producing infinity/NaN inside the event clock.
    if input.in_match.red_card_probability_per90 >= 1.0
        || input.in_match.injury_probability_per90 >= 1.0
    {
        return None;
    }

    let mut normalized = transition;
    normalized.personal_error_self_belief = transition.personal_error_self_belief.abs();
    normalized.personal_error_morale = transition.personal_error_morale.abs();
    second_resolved_mc::simulate_second_resolved_dynamic_mental_match(
        input,
        mental_model,
        normalized,
        parameters,
        paths,
        seed,
    )
}
