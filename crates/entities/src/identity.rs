use std::cmp::Ordering;

use footy_core::{ClubId, Confidence, Date, PlayerId, Source, StringId};

use crate::{name_similarity, normalize_name, normalized_surname};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkMethod {
    Explicit,
    ExactMetadata,
    WeightedMetadata,
    Manual,
}

/// Auditable canonical-to-source identity link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdentityLink {
    pub player: PlayerId,
    pub source: Source,
    pub source_id: StringId,
    pub score: Confidence,
    pub method: LinkMethod,
    pub verified: bool,
}

/// Metadata used to compare two source/canonical identity records.
#[derive(Debug, Clone, Copy)]
pub struct IdentityMetadata<'a> {
    pub name: &'a str,
    pub birth_date: Option<Date>,
    pub club: Option<ClubId>,
    pub height_cm: Option<u16>,
    pub nationality: Option<StringId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchWeights {
    pub name: f32,
    pub birth_date: f32,
    pub club: f32,
    pub height: f32,
    pub nationality: f32,
}

impl Default for MatchWeights {
    fn default() -> Self {
        Self {
            name: 0.55,
            birth_date: 0.20,
            club: 0.10,
            height: 0.05,
            nationality: 0.10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchEvidence {
    pub name_similarity: f32,
    pub birth_date_match: Option<bool>,
    pub club_match: Option<bool>,
    pub height_similarity: Option<f32>,
    pub nationality_match: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdentityCandidate {
    pub player: PlayerId,
    pub score: Confidence,
    pub method: LinkMethod,
    pub evidence: MatchEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    NoMatch,
    Unique(IdentityCandidate),
    Ambiguous(Vec<IdentityCandidate>),
}

/// Score a candidate while renormalizing over metadata actually present on both sides.
pub fn score_candidate(
    player: PlayerId,
    left: &IdentityMetadata<'_>,
    right: &IdentityMetadata<'_>,
    weights: MatchWeights,
) -> IdentityCandidate {
    let normalized_left = normalize_name(left.name);
    let normalized_right = normalize_name(right.name);
    let exact_name = !normalized_left.is_empty() && normalized_left == normalized_right;
    let name_score = name_similarity(left.name, right.name);
    let birth_date_match = pair_match(left.birth_date, right.birth_date);
    let club_match = pair_match(left.club, right.club);
    let nationality_match = pair_match(left.nationality, right.nationality);
    let height_similarity = height_similarity(left.height_cm, right.height_cm);

    let exact_name_and_dob = exact_name && birth_date_match == Some(true);
    let exact_surname_dob_nationality = birth_date_match == Some(true)
        && nationality_match == Some(true)
        && !normalized_surname(left.name).is_empty()
        && normalized_surname(left.name) == normalized_surname(right.name);

    let (method, score) = if exact_name_and_dob {
        (LinkMethod::ExactMetadata, 1.0)
    } else if exact_surname_dob_nationality {
        (LinkMethod::ExactMetadata, 0.95)
    } else {
        let mut weighted = weights.name * name_score;
        let mut total_weight = weights.name;

        if let Some(value) = birth_date_match {
            weighted += weights.birth_date * bool_score(value);
            total_weight += weights.birth_date;
        }
        if let Some(value) = club_match {
            weighted += weights.club * bool_score(value);
            total_weight += weights.club;
        }
        if let Some(value) = height_similarity {
            weighted += weights.height * value;
            total_weight += weights.height;
        }
        if let Some(value) = nationality_match {
            weighted += weights.nationality * bool_score(value);
            total_weight += weights.nationality;
        }

        let score = if total_weight > 0.0 {
            weighted / total_weight
        } else {
            0.0
        };
        (LinkMethod::WeightedMetadata, score)
    };

    IdentityCandidate {
        player,
        score: Confidence::clamped(score),
        method,
        evidence: MatchEvidence {
            name_similarity: name_score,
            birth_date_match,
            club_match,
            height_similarity,
            nationality_match,
        },
    }
}

/// Resolve only when the best candidate clears both an absolute threshold and an
/// ambiguity margin. Otherwise retain the relevant candidates for manual review.
pub fn resolve_candidates(
    mut candidates: Vec<IdentityCandidate>,
    minimum_score: f32,
    ambiguity_margin: f32,
) -> Resolution {
    candidates.sort_by(|left, right| {
        right
            .score
            .get()
            .partial_cmp(&left.score.get())
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.player.cmp(&right.player))
    });

    let Some(best) = candidates.first().copied() else {
        return Resolution::NoMatch;
    };

    if best.score.get() < minimum_score {
        return Resolution::NoMatch;
    }

    if let Some(second) = candidates.get(1) {
        if best.score.get() - second.score.get() < ambiguity_margin.max(0.0) {
            return Resolution::Ambiguous(candidates);
        }
    }

    Resolution::Unique(best)
}

fn pair_match<T: PartialEq + Copy>(left: Option<T>, right: Option<T>) -> Option<bool> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left == right),
        _ => None,
    }
}

fn height_similarity(left: Option<u16>, right: Option<u16>) -> Option<f32> {
    let (left, right) = (left?, right?);
    let difference = left.abs_diff(right) as f32;
    Some((1.0 - difference / 10.0).clamp(0.0, 1.0))
}

const fn bool_score(value: bool) -> f32 {
    if value { 1.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_ymd(year, month, day).unwrap()
    }

    #[test]
    fn exact_name_and_dob_are_deterministic_evidence() {
        let left = IdentityMetadata {
            name: "João Félix",
            birth_date: Some(date(1999, 11, 10)),
            club: None,
            height_cm: Some(181),
            nationality: None,
        };
        let right = IdentityMetadata {
            name: "Joao Felix",
            birth_date: Some(date(1999, 11, 10)),
            club: None,
            height_cm: Some(181),
            nationality: None,
        };
        let candidate = score_candidate(PlayerId(7), &left, &right, MatchWeights::default());
        assert_eq!(candidate.method, LinkMethod::ExactMetadata);
        assert_eq!(candidate.score, Confidence::ONE);
    }

    #[test]
    fn weak_close_candidates_remain_ambiguous() {
        let a = IdentityCandidate {
            player: PlayerId(1),
            score: Confidence::new(0.90).unwrap(),
            method: LinkMethod::WeightedMetadata,
            evidence: MatchEvidence {
                name_similarity: 0.9,
                birth_date_match: None,
                club_match: None,
                height_similarity: None,
                nationality_match: None,
            },
        };
        let b = IdentityCandidate { player: PlayerId(2), score: Confidence::new(0.88).unwrap(), ..a };
        assert!(matches!(
            resolve_candidates(vec![a, b], 0.8, 0.05),
            Resolution::Ambiguous(_)
        ));
    }

    #[test]
    fn a_clear_candidate_can_resolve() {
        let a = IdentityCandidate {
            player: PlayerId(1),
            score: Confidence::new(0.94).unwrap(),
            method: LinkMethod::WeightedMetadata,
            evidence: MatchEvidence {
                name_similarity: 0.94,
                birth_date_match: Some(true),
                club_match: Some(true),
                height_similarity: Some(1.0),
                nationality_match: Some(true),
            },
        };
        let b = IdentityCandidate { player: PlayerId(2), score: Confidence::new(0.70).unwrap(), ..a };
        assert!(matches!(
            resolve_candidates(vec![a, b], 0.8, 0.05),
            Resolution::Unique(candidate) if candidate.player == PlayerId(1)
        ));
    }
}
