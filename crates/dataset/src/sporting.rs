use crate::canonical::stable_hash64;
use crate::{CanonicalBuildConfig, CanonicalDataset, DatasetError, MetricDefinition};
use footy_core::{ClubId, CompetitionId, Confidence, DatasetVersion, MatchId, MetricId, ObservationState, PlayerId, ProvenanceId};
use footy_entities::{resolve_candidates, score_candidate, IdentityMetadata, LinkMethod, MatchWeights, Resolution};
use footy_performance::{MetricValue, PlayerMatch, TeamMatch};
use footy_source_stats::{MatchResultSourceRecord, MatchStatus, PlayerStatSourceRecord, StatsProvider, TeamStatSourceRecord};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SportingProvenance {
    pub id: ProvenanceId,
    pub provider: StatsProvider,
    pub input_path: String,
    pub source_checksum: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SportingExternalPlayer {
    pub provider: StatsProvider,
    pub external_id: String,
    pub name: String,
    pub birth_date: Option<footy_core::Date>,
    pub height_cm: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SportingPlayerLink {
    pub provider: StatsProvider,
    pub external_id: String,
    pub player: PlayerId,
    pub score: Confidence,
    pub method: LinkMethod,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SportingClubRecord {
    pub id: ClubId,
    pub provider: StatsProvider,
    pub external_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SportingCompetitionRecord {
    pub id: CompetitionId,
    pub provider: StatsProvider,
    pub external_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalPlayerStatObservation {
    pub provider: StatsProvider,
    pub player: PlayerId,
    pub club: ClubId,
    pub competition: CompetitionId,
    pub match_id: MatchId,
    pub date: footy_core::Date,
    pub minutes: Option<u16>,
    pub metrics: Vec<MetricValue>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalTeamStatObservation {
    pub provider: StatsProvider,
    pub club: ClubId,
    pub opponent: ClubId,
    pub competition: CompetitionId,
    pub match_id: MatchId,
    pub date: footy_core::Date,
    pub is_home: bool,
    pub metrics: Vec<MetricValue>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalMatchResult {
    pub provider: StatsProvider,
    pub match_id: MatchId,
    pub competition: CompetitionId,
    pub date: footy_core::Date,
    pub home: ClubId,
    pub away: ClubId,
    pub home_goals: Option<u8>,
    pub away_goals: Option<u8>,
    pub status: MatchStatus,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SportingIssue {
    pub provider: StatsProvider,
    pub external_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SportingIdentityResolution {
    pub links: Vec<SportingPlayerLink>,
    pub issues: Vec<SportingIssue>,
}

/// Canonical sporting observations bound to an immutable base valuation dataset version.
/// New provider clubs/competitions use IDs above the base maxima and remain source-scoped.
#[derive(Debug, Clone, PartialEq)]
pub struct SportingDataset {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub provenances: Vec<SportingProvenance>,
    pub player_links: Vec<SportingPlayerLink>,
    pub clubs: Vec<SportingClubRecord>,
    pub competitions: Vec<SportingCompetitionRecord>,
    pub metric_registry: Vec<MetricDefinition>,
    pub player_stats: Vec<CanonicalPlayerStatObservation>,
    pub team_stats: Vec<CanonicalTeamStatObservation>,
    pub matches: Vec<CanonicalMatchResult>,
    pub issues: Vec<SportingIssue>,
}

impl SportingDataset {
    pub fn validate(&self, base: &CanonicalDataset) -> Result<(), DatasetError> {
        if self.dataset_version != base.dataset_version || self.source_manifest_checksum != base.source_manifest_checksum {
            return Err(DatasetError::new("sporting dataset is not bound to the supplied canonical dataset"));
        }
        let players: HashSet<PlayerId> = base.players.iter().map(|value| value.player).collect();
        let provenance: HashSet<ProvenanceId> = self.provenances.iter().map(|value| value.id).collect();
        if provenance.len() != self.provenances.len() { return Err(DatasetError::new("duplicate sporting provenance id")); }
        let base_clubs: HashSet<ClubId> = base.clubs.iter().map(|value| value.id).collect();
        let base_competitions: HashSet<CompetitionId> = base.competitions.iter().map(|value| value.id).collect();
        if self.clubs.iter().any(|value| base_clubs.contains(&value.id)) || self.competitions.iter().any(|value| base_competitions.contains(&value.id)) {
            return Err(DatasetError::new("sporting IDs collide with base canonical IDs"));
        }
        let clubs: HashSet<ClubId> = base_clubs.into_iter().chain(self.clubs.iter().map(|value| value.id)).collect();
        let competitions: HashSet<CompetitionId> = base_competitions.into_iter().chain(self.competitions.iter().map(|value| value.id)).collect();
        let metrics: HashSet<MetricId> = self.metric_registry.iter().map(|value| value.metric).collect();
        let mut match_ids = HashSet::new();
        let mut links = HashSet::new();
        for value in &self.player_links {
            if !players.contains(&value.player) || !links.insert((provider_tag(value.provider), value.external_id.as_str())) {
                return Err(DatasetError::new("invalid sporting player link"));
            }
        }
        for value in &self.matches {
            if !match_ids.insert(value.match_id) || !clubs.contains(&value.home) || !clubs.contains(&value.away) || !competitions.contains(&value.competition) || !provenance.contains(&value.provenance_id) {
                return Err(DatasetError::new("invalid canonical sporting match"));
            }
        }
        for value in &self.player_stats {
            if !players.contains(&value.player) || !clubs.contains(&value.club) || !competitions.contains(&value.competition) || !provenance.contains(&value.provenance_id) || value.metrics.iter().any(|metric| !metrics.contains(&metric.metric) || !metric.value.is_finite()) {
                return Err(DatasetError::new("invalid canonical player statistic"));
            }
        }
        for value in &self.team_stats {
            if !clubs.contains(&value.club) || !clubs.contains(&value.opponent) || !competitions.contains(&value.competition) || !provenance.contains(&value.provenance_id) || value.metrics.iter().any(|metric| !metrics.contains(&metric.metric) || !metric.value.is_finite()) {
                return Err(DatasetError::new("invalid canonical team statistic"));
            }
        }
        Ok(())
    }

    /// Merge existing FBref match-level observations with new provider records into one player
    /// performance surface. Records without minutes stay in the sidecar but are excluded from
    /// per-90 construction rather than turning missing exposure into zero.
    pub fn player_matches(&self, base: &CanonicalDataset) -> Vec<PlayerMatch> {
        let mut out = Vec::new();
        for value in &base.performance {
            if !value.is_match_level() { continue; }
            let Ok(minutes) = u16::try_from(value.minutes) else { continue };
            out.push(PlayerMatch { player: value.player, club: value.club, competition: value.competition, date: value.end_date, minutes, metrics: value.metrics.clone(), observation_state: value.observation_state });
        }
        for value in &self.player_stats {
            let Some(minutes) = value.minutes else { continue; };
            out.push(PlayerMatch { player: value.player, club: value.club, competition: value.competition, date: value.date, minutes, metrics: value.metrics.clone(), observation_state: ObservationState::Measured });
        }
        out.sort_by_key(|value| (value.player, value.date, value.club, value.competition));
        out
    }

    pub fn team_matches(&self) -> Vec<TeamMatch> {
        let results: HashMap<MatchId, CanonicalMatchResult> = self.matches.iter().map(|value| (value.match_id, *value)).collect();
        let mut out = Vec::with_capacity(self.team_stats.len());
        for value in &self.team_stats {
            let goals = results.get(&value.match_id).and_then(|result| {
                if value.club == result.home { Some((result.home_goals, result.away_goals)) }
                else if value.club == result.away { Some((result.away_goals, result.home_goals)) }
                else { None }
            });
            out.push(TeamMatch {
                match_id: value.match_id, club: value.club, opponent: value.opponent, competition: value.competition, date: value.date, is_home: value.is_home,
                goals_for: goals.and_then(|score| score.0), goals_against: goals.and_then(|score| score.1), metrics: value.metrics.clone(), observation_state: ObservationState::Measured,
            });
        }
        out.sort_by_key(|value| (value.club, value.date, value.match_id));
        out
    }
}

/// Resolve provider player identities against the FM-anchored canonical player universe using the
/// same weighted metadata resolver as the base dataset. Ambiguous and low-confidence rows remain
/// explicit issues; they are never force-linked.
pub fn resolve_sporting_player_links(
    base: &CanonicalDataset,
    external: Vec<SportingExternalPlayer>,
    config: CanonicalBuildConfig,
) -> SportingIdentityResolution {
    let mut links = Vec::new();
    let mut issues = Vec::new();
    for record in external {
        let mut candidates = Vec::new();
        for canonical in &base.players {
            let left = IdentityMetadata { name: &record.name, birth_date: record.birth_date, club: None, height_cm: record.height_cm, nationality: None };
            let right = IdentityMetadata { name: &canonical.name, birth_date: canonical.birth_date, club: None, height_cm: None, nationality: None };
            let candidate = score_candidate(canonical.player, &left, &right, MatchWeights::default());
            if candidate.score.get() >= (config.minimum_identity_score - 0.20).max(0.0) { candidates.push(candidate); }
        }
        match resolve_candidates(candidates, config.minimum_identity_score, config.ambiguity_margin) {
            Resolution::Unique(candidate) => links.push(SportingPlayerLink { provider: record.provider, external_id: record.external_id, player: candidate.player, score: candidate.score, method: candidate.method, verified: false }),
            Resolution::Ambiguous(_) => issues.push(SportingIssue { provider: record.provider, external_id: record.external_id, message: "sporting identity ambiguous".into() }),
            Resolution::NoMatch => issues.push(SportingIssue { provider: record.provider, external_id: record.external_id, message: "sporting identity unresolved".into() }),
        }
    }
    links.sort_by(|a, b| provider_tag(a.provider).cmp(&provider_tag(b.provider)).then(a.external_id.cmp(&b.external_id)));
    issues.sort_by(|a, b| provider_tag(a.provider).cmp(&provider_tag(b.provider)).then(a.external_id.cmp(&b.external_id)));
    SportingIdentityResolution { links, issues }
}

pub fn build_sporting_dataset(
    base: &CanonicalDataset,
    provenances: Vec<SportingProvenance>,
    player_links: Vec<SportingPlayerLink>,
    player_source: Vec<PlayerStatSourceRecord>,
    team_source: Vec<TeamStatSourceRecord>,
    match_source: Vec<MatchResultSourceRecord>,
) -> Result<SportingDataset, DatasetError> {
    let provenance_map: HashMap<ProvenanceId, StatsProvider> = provenances.iter().map(|value| (value.id, value.provider)).collect();
    if provenance_map.len() != provenances.len() { return Err(DatasetError::new("duplicate sporting provenance id")); }
    for value in &player_source { validate_provenance(&provenance_map, value.provenance_id, value.provider)?; }
    for value in &team_source { validate_provenance(&provenance_map, value.provenance_id, value.provider)?; }
    for value in &match_source { validate_provenance(&provenance_map, value.provenance_id, value.provider)?; }

    let players: HashSet<PlayerId> = base.players.iter().map(|value| value.player).collect();
    let mut link_map = HashMap::new();
    for link in &player_links {
        if !players.contains(&link.player) || link_map.insert((provider_tag(link.provider), link.external_id.clone()), link.player).is_some() {
            return Err(DatasetError::new("duplicate or invalid sporting player link"));
        }
    }

    let (clubs, club_map) = build_clubs(base, &player_source, &team_source, &match_source)?;
    let (competitions, competition_map) = build_competitions(base, &player_source, &team_source, &match_source)?;
    let (metric_registry, metric_map) = extend_metrics(base, &player_source, &team_source)?;
    let mut issues = Vec::new();

    let mut matches = Vec::with_capacity(match_source.len());
    let mut seen_match_ids = HashSet::new();
    for value in match_source {
        let match_id = canonical_match_id(value.provider, &value.match_external_id);
        if !seen_match_ids.insert(match_id) { return Err(DatasetError::new("duplicate/colliding canonical sporting match id")); }
        matches.push(CanonicalMatchResult {
            provider: value.provider,
            match_id,
            competition: lookup_competition(&competition_map, value.provider, &value.competition_external_id)?,
            date: value.date,
            home: lookup_club(&club_map, value.provider, &value.home_club_external_id)?,
            away: lookup_club(&club_map, value.provider, &value.away_club_external_id)?,
            home_goals: value.home_goals,
            away_goals: value.away_goals,
            status: value.status,
            provenance_id: value.provenance_id,
        });
    }
    matches.sort_by_key(|value| (value.date, value.match_id));

    let mut player_stats = Vec::new();
    for value in player_source {
        let key = (provider_tag(value.provider), value.player_external_id.clone());
        let Some(player) = link_map.get(&key).copied() else {
            issues.push(SportingIssue { provider: value.provider, external_id: value.player_external_id, message: "player statistic skipped: unresolved canonical player".into() });
            continue;
        };
        player_stats.push(CanonicalPlayerStatObservation {
            provider: value.provider, player,
            club: lookup_club(&club_map, value.provider, &value.club_external_id)?,
            competition: lookup_competition(&competition_map, value.provider, &value.competition_external_id)?,
            match_id: canonical_match_id(value.provider, &value.match_external_id), date: value.date, minutes: value.minutes,
            metrics: map_metrics(value.metrics, &metric_map)?, provenance_id: value.provenance_id,
        });
    }
    player_stats.sort_by_key(|value| (value.player, value.date, value.match_id));

    let mut team_stats = Vec::with_capacity(team_source.len());
    for value in team_source {
        team_stats.push(CanonicalTeamStatObservation {
            provider: value.provider,
            club: lookup_club(&club_map, value.provider, &value.club_external_id)?, opponent: lookup_club(&club_map, value.provider, &value.opponent_external_id)?,
            competition: lookup_competition(&competition_map, value.provider, &value.competition_external_id)?, match_id: canonical_match_id(value.provider, &value.match_external_id),
            date: value.date, is_home: value.is_home, metrics: map_metrics(value.metrics, &metric_map)?, provenance_id: value.provenance_id,
        });
    }
    team_stats.sort_by_key(|value| (value.club, value.date, value.match_id));
    issues.sort_by(|a, b| provider_tag(a.provider).cmp(&provider_tag(b.provider)).then(a.external_id.cmp(&b.external_id)));

    let dataset = SportingDataset { dataset_version: base.dataset_version, source_manifest_checksum: base.source_manifest_checksum, provenances, player_links, clubs, competitions, metric_registry, player_stats, team_stats, matches, issues };
    dataset.validate(base)?;
    Ok(dataset)
}

fn build_clubs(base: &CanonicalDataset, players: &[PlayerStatSourceRecord], teams: &[TeamStatSourceRecord], matches: &[MatchResultSourceRecord]) -> Result<(Vec<SportingClubRecord>, HashMap<(u8, String), ClubId>), DatasetError> {
    let mut keys = BTreeSet::new();
    for value in players { keys.insert((provider_tag(value.provider), value.club_external_id.clone())); }
    for value in teams { keys.insert((provider_tag(value.provider), value.club_external_id.clone())); keys.insert((provider_tag(value.provider), value.opponent_external_id.clone())); }
    for value in matches { keys.insert((provider_tag(value.provider), value.home_club_external_id.clone())); keys.insert((provider_tag(value.provider), value.away_club_external_id.clone())); }
    let mut next = base.clubs.iter().map(|value| value.id.0).max().unwrap_or(0).checked_add(1).ok_or_else(|| DatasetError::new("club id overflow"))?;
    let mut records = Vec::new();
    let mut map = HashMap::new();
    for (tag, external_id) in keys {
        let id = ClubId(next); next = next.checked_add(1).ok_or_else(|| DatasetError::new("club id overflow"))?;
        let provider = provider_from_tag(tag)?; map.insert((tag, external_id.clone()), id); records.push(SportingClubRecord { id, provider, external_id });
    }
    Ok((records, map))
}

fn build_competitions(base: &CanonicalDataset, players: &[PlayerStatSourceRecord], teams: &[TeamStatSourceRecord], matches: &[MatchResultSourceRecord]) -> Result<(Vec<SportingCompetitionRecord>, HashMap<(u8, String), CompetitionId>), DatasetError> {
    let mut keys = BTreeSet::new();
    for value in players { keys.insert((provider_tag(value.provider), value.competition_external_id.clone())); }
    for value in teams { keys.insert((provider_tag(value.provider), value.competition_external_id.clone())); }
    for value in matches { keys.insert((provider_tag(value.provider), value.competition_external_id.clone())); }
    let mut next = base.competitions.iter().map(|value| value.id.0).max().unwrap_or(0).checked_add(1).ok_or_else(|| DatasetError::new("competition id overflow"))?;
    let mut records = Vec::new();
    let mut map = HashMap::new();
    for (tag, external_id) in keys {
        let id = CompetitionId(next); next = next.checked_add(1).ok_or_else(|| DatasetError::new("competition id overflow"))?;
        let provider = provider_from_tag(tag)?; map.insert((tag, external_id.clone()), id); records.push(SportingCompetitionRecord { id, provider, external_id });
    }
    Ok((records, map))
}

fn extend_metrics(base: &CanonicalDataset, players: &[PlayerStatSourceRecord], teams: &[TeamStatSourceRecord]) -> Result<(Vec<MetricDefinition>, HashMap<String, MetricId>), DatasetError> {
    let mut registry = base.metric_registry.clone();
    let mut map: HashMap<String, MetricId> = registry.iter().map(|value| (value.name.clone(), value.metric)).collect();
    let mut names = BTreeSet::new();
    for value in players { for metric in &value.metrics { names.insert(metric.name.clone()); } }
    for value in teams { for metric in &value.metrics { names.insert(metric.name.clone()); } }
    let mut next = registry.iter().map(|value| value.metric.0).max().unwrap_or(0).checked_add(1).ok_or_else(|| DatasetError::new("metric id overflow"))?;
    for name in names {
        if map.contains_key(&name) { continue; }
        let id = MetricId(next); next = next.checked_add(1).ok_or_else(|| DatasetError::new("metric id overflow"))?;
        map.insert(name.clone(), id); registry.push(MetricDefinition { metric: id, name });
    }
    registry.sort_by_key(|value| value.metric);
    Ok((registry, map))
}

fn map_metrics(values: Vec<footy_source_stats::SourceMetric>, map: &HashMap<String, MetricId>) -> Result<Vec<MetricValue>, DatasetError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let metric = *map.get(&value.name).ok_or_else(|| DatasetError::new("missing sporting metric mapping"))?;
        if !value.value.is_finite() { return Err(DatasetError::new("non-finite sporting metric")); }
        out.push(MetricValue { metric, value: value.value });
    }
    out.sort_by_key(|value| value.metric);
    Ok(out)
}

fn validate_provenance(map: &HashMap<ProvenanceId, StatsProvider>, provenance: ProvenanceId, provider: StatsProvider) -> Result<(), DatasetError> {
    match map.get(&provenance) {
        Some(actual) if *actual == provider => Ok(()),
        Some(_) => Err(DatasetError::new("sporting record provenance provider mismatch")),
        None => Err(DatasetError::new("sporting record references unknown provenance")),
    }
}

fn lookup_club(map: &HashMap<(u8, String), ClubId>, provider: StatsProvider, external_id: &str) -> Result<ClubId, DatasetError> {
    map.get(&(provider_tag(provider), external_id.to_owned())).copied().ok_or_else(|| DatasetError::new("missing sporting club mapping"))
}

fn lookup_competition(map: &HashMap<(u8, String), CompetitionId>, provider: StatsProvider, external_id: &str) -> Result<CompetitionId, DatasetError> {
    map.get(&(provider_tag(provider), external_id.to_owned())).copied().ok_or_else(|| DatasetError::new("missing sporting competition mapping"))
}

fn canonical_match_id(provider: StatsProvider, external_id: &str) -> MatchId {
    let prefix: &[u8] = match provider { StatsProvider::Sofascore => b"sofascore-match:", StatsProvider::StatsBomb => b"statsbomb-match:", StatsProvider::OpenFootball => b"openfootball-match:", StatsProvider::FootballDataOrg => b"football-data-org-match:" };
    MatchId(stable_hash64(prefix, external_id.as_bytes()))
}

const fn provider_tag(provider: StatsProvider) -> u8 {
    match provider { StatsProvider::Sofascore => 1, StatsProvider::StatsBomb => 2, StatsProvider::OpenFootball => 3, StatsProvider::FootballDataOrg => 4 }
}

fn provider_from_tag(tag: u8) -> Result<StatsProvider, DatasetError> {
    match tag { 1 => Ok(StatsProvider::Sofascore), 2 => Ok(StatsProvider::StatsBomb), 3 => Ok(StatsProvider::OpenFootball), 4 => Ok(StatsProvider::FootballDataOrg), _ => Err(DatasetError::new("invalid sporting provider tag")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalPlayer;
    use footy_core::Date;
    use footy_source_stats::SourceMetric;

    fn base_dataset() -> CanonicalDataset {
        CanonicalDataset { dataset_version: DatasetVersion(1), source_manifest_checksum: 7, provenances: Vec::new(), players: vec![CanonicalPlayer { player: PlayerId(1), fm_external_id: "fm1".into(), name: "Player".into(), birth_date: None }], identities: Vec::new(), unresolved_identities: Vec::new(), issues: Vec::new(), fm_snapshots: Vec::new(), clubs: Vec::new(), competitions: Vec::new(), metric_registry: Vec::new(), market_values: Vec::new(), transfers: Vec::new(), contracts: Vec::new(), performance: Vec::new(), salaries: Vec::new() }
    }

    #[test]
    fn identity_resolution_uses_existing_metadata_matcher() {
        let base = base_dataset();
        let external = vec![SportingExternalPlayer { provider: StatsProvider::Sofascore, external_id: "p1".into(), name: "Player".into(), birth_date: None, height_cm: None }];
        let resolution = resolve_sporting_player_links(&base, external, CanonicalBuildConfig { minimum_identity_score: 0.80, ambiguity_margin: 0.05, max_unresolved_candidates: 5 });
        assert_eq!(resolution.links.len(), 1);
        assert_eq!(resolution.links[0].player, PlayerId(1));
    }

    #[test]
    fn canonicalizes_sporting_records_and_joins_results() {
        let base = base_dataset();
        let date = Date(10);
        let provenance = vec![SportingProvenance { id: ProvenanceId(1), provider: StatsProvider::Sofascore, input_path: "sofa.json".into(), source_checksum: 9 }];
        let links = vec![SportingPlayerLink { provider: StatsProvider::Sofascore, external_id: "p1".into(), player: PlayerId(1), score: Confidence::ONE, method: LinkMethod::Manual, verified: true }];
        let players = vec![PlayerStatSourceRecord { provider: StatsProvider::Sofascore, player_external_id: "p1".into(), club_external_id: "c1".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), match_external_id: "m1".into(), date, minutes: Some(90), metrics: vec![SourceMetric { name: "shots".into(), value: 3.0 }], provenance_id: ProvenanceId(1) }];
        let teams = vec![TeamStatSourceRecord { provider: StatsProvider::Sofascore, club_external_id: "c1".into(), opponent_external_id: "c2".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), match_external_id: "m1".into(), date, is_home: true, metrics: vec![SourceMetric { name: "shots".into(), value: 12.0 }], provenance_id: ProvenanceId(1) }];
        let matches = vec![MatchResultSourceRecord { provider: StatsProvider::Sofascore, match_external_id: "m1".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), date, home_club_external_id: "c1".into(), away_club_external_id: "c2".into(), home_goals: Some(2), away_goals: Some(1), status: MatchStatus::Finished, provenance_id: ProvenanceId(1) }];
        let sporting = build_sporting_dataset(&base, provenance, links, players, teams, matches).unwrap();
        assert_eq!(sporting.player_stats.len(), 1);
        assert_eq!(sporting.team_matches()[0].goals_for, Some(2));
        assert_eq!(sporting.player_matches(&base)[0].minutes, 90);
    }
}
