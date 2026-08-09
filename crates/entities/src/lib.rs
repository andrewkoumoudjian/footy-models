#![forbid(unsafe_code)]
//! Canonical football entities and auditable cross-source identity resolution.

mod domain;
mod identity;
mod normalize;

pub use domain::{Club, Competition, Player, PreferredFoot};
pub use identity::{
    resolve_candidates, score_candidate, IdentityCandidate, IdentityLink, IdentityMetadata,
    LinkMethod, MatchEvidence, MatchWeights, Resolution,
};
pub use normalize::{name_similarity, normalize_name, normalized_surname};
