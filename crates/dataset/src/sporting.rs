use crate::canonical::{stable_hash64, SourceClubRecord, SourceCompetitionRecord};
use crate::{CanonicalDataset, DatasetError, MetricDefinition};
use footy_core::{ClubId, CompetitionId, DatasetVersion, MatchId, MetricId, ObservationState, PlayerId, ProvenanceId, Source};
use footy_performance::{MetricValue, PlayerMatch, TeamMatch};
use footy_source_stats::{MatchResultSourceRecord, MatchStatus, PlayerStatSourceRecord, StatsProvider, TeamStatSourceRecord};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalPlayerStatObservation {
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
    pub source: Source,
    pub external_id: String,
    pub message: String,
}

/// Canonical sporting observations bound to an immutable base canonical dataset version.
/// Additional clubs and competitions remain source-scoped and never imply cross-provider identity.
#[derive(Debug, Clone, PartialEq)]
pub struct SportingDataset {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub clubs: Vec<SourceClubRecord>,
    pub competitions: Vec<SourceCompetitionRecord>,
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
        let provenance: HashSet<ProvenanceId> = base.provenances.iter().map(|value| value.id).collect();
        let clubs: HashSet<ClubId> = base.clubs.iter().map(|value| value.id).chain(self.clubs.iter().map(|value| value.id)).collect();
        let competitions: HashSet<CompetitionId> = base.competitions.iter().map(|value| value.id).chain(self.competitions.iter().map(|value| value.id)).collect();
        let metrics: HashSet<MetricId> = self.metric_registry.iter().map(|value| value.metric).collect();
        let mut match_ids = HashSet::new();

        for value in &self.matches {
            if !match_ids.insert(value.match_id)
                || !clubs.contains(&value.home)
                || !clubs.contains(&value.away)
                || !competitions.contains(&value.competition)
                || !provenance.contains(&value.provenance_id)
            {
                return Err(DatasetError::new("invalid canonical sporting match"));
            }
        }
        for value in &self.player_stats {
            if !players.contains(&value.player)
                || !clubs.contains(&value.club)
                || !competitions.contains(&value.competition)
                || !provenance.contains(&value.provenance_id)
                || value.metrics.iter().any(|metric| !metrics.contains(&metric.metric) || !metric.value.is_finite())
            {
                return Err(DatasetError::new("invalid canonical player statistic"));
            }
        }
        for value in &self.team_stats {
            if !clubs.contains(&value.club)
                || !clubs.contains(&value.opponent)
                || !competitions.contains(&value.competition)
                || !provenance.contains(&value.provenance_id)
                || value.metrics.iter().any(|metric| !metrics.contains(&metric.metric) || !metric.value.is_finite())
            {
                return Err(DatasetError::new("invalid canonical team statistic"));
            }
        }
        Ok(())
    }

    /// Merge existing FBref match-level observations with new provider records for one causal
    /// player-performance surface. Observations without minutes remain available in the sidecar
    /// but are excluded here rather than pretending missing exposure is zero.
    pub fn player_matches(&self, base: &CanonicalDataset) -> Vec<PlayerMatch> {
        let mut out = Vec::new();
        for value in &base.performance {
            if !value.is_match_level() { continue; }
            let Ok(minutes) = u16::try_from(value.minutes) else { continue };
            out.push(PlayerMatch {
                player: value.player,
                club: value.club,
                competition: value.competition,
                date: value.end_date,
                minutes,
                metrics: value.metrics.clone(),
                observation_state: value.observation_state,
            });
        }
        for value in &self.player_stats {
            let Some(minutes) = value.minutes else { continue };
            out.push(PlayerMatch {
                player: value.player,
                club: value.club,
                competition: value.competition,
                date: value.date,
                minutes,
                metrics: value.metrics.clone(),
                observation_state: ObservationState::Measured,
            });
        }
        out.sort_by_key(|value| (value.player, value.date, value.club, value.competition));
        out
    }

    /// Join team statistics to canonical results so rolling team windows can use metrics and form
    /// from the same match identity.
    pub fn team_matches(&self) -> Vec<TeamMatch> {
        let results: HashMap<MatchId, CanonicalMatchResult> = self.matches.iter().map(|value| (value.match_id, *value)).collect();
        let mut out = Vec::with_capacity(self.team_stats.len());
        for value in &self.team_stats {
            let result = results.get(&value.match_id);
            let goals = result.and_then(|result| {
                if value.club == result.home {
                    Some((result.home_goals, result.away_goals))
                } else if value.club == result.away {
                    Some((result.away_goals, result.home_goals))
                } else {
                    None
                }
            });
            out.push(TeamMatch {
                match_id: value.match_id,
                club: value.club,
                opponent: value.opponent,
                competition: value.competition,
                date: value.date,
                is_home: value.is_home,
                goals_for: goals.and_then(|value| value.0),
                goals_against: goals.and_then(|value| value.1),
                metrics: value.metrics.clone(),
                observation_state: ObservationState::Measured,
            });
        }
        out.sort_by_key(|value| (value.club, value.date, value.match_id));
        out
    }
}

pub fn build_sporting_dataset(
    base: &CanonicalDataset,
    player_source: Vec<PlayerStatSourceRecord>,
    team_source: Vec<TeamStatSourceRecord>,
    match_source: Vec<MatchResultSourceRecord>,
) -> Result<SportingDataset, DatasetError> {
    let provenance_source: HashMap<ProvenanceId, Source> = base.provenances.iter().map(|value| (value.id, value.source)).collect();
    for value in &player_source { validate_source_provenance(&provenance_source, value.provenance_id, provider_source(value.provider))?; }
    for value in &team_source { validate_source_provenance(&provenance_source, value.provenance_id, provider_source(value.provider))?; }
    for value in &match_source { validate_source_provenance(&provenance_source, value.provenance_id, provider_source(value.provider))?; }

    let (clubs, club_map) = extend_clubs(base, &player_source, &team_source, &match_source)?;
    let (competitions, competition_map) = extend_competitions(base, &player_source, &team_source, &match_source)?;
    let (metric_registry, metric_map) = extend_metrics(base, &player_source, &team_source)?;
    let mut issues = Vec::new();

    let mut matches = Vec::with_capacity(match_source.len());
    let mut seen_match_ids = HashSet::new();
    for value in match_source {
        let source = provider_source(value.provider);
        let match_id = canonical_match_id(value.provider, &value.match_external_id);
        if !seen_match_ids.insert(match_id) { return Err(DatasetError::new("duplicate/colliding canonical sporting match id")); }
        matches.push(CanonicalMatchResult {
            match_id,
            competition: lookup_competition(&competition_map, source, &value.competition_external_id)?,
            date: value.date,
            home: lookup_club(&club_map, source, &value.home_club_external_id)?,
            away: lookup_club(&club_map, source, &value.away_club_external_id)?,
            home_goals: value.home_goals,
            away_goals: value.away_goals,
            status: value.status,
            provenance_id: value.provenance_id,
        });
    }
    matches.sort_by_key(|value| (value.date, value.match_id));

    let mut player_stats = Vec::new();
    for value in player_source {
        let source = provider_source(value.provider);
        let Some(player) = base.identity(source, &value.player_external_id) else {
            issues.push(SportingIssue { source, external_id: value.player_external_id, message: "player statistic skipped: unresolved canonical player".into() });
            continue;
        };
        player_stats.push(CanonicalPlayerStatObservation {
            player,
            club: lookup_club(&club_map, source, &value.club_external_id)?,
            competition: lookup_competition(&competition_map, source, &value.competition_external_id)?,
            match_id: canonical_match_id(value.provider, &value.match_external_id),
            date: value.date,
            minutes: value.minutes,
            metrics: map_metrics(value.metrics, &metric_map)?,
            provenance_id: value.provenance_id,
        });
    }
    player_stats.sort_by_key(|value| (value.player, value.date, value.match_id));

    let mut team_stats = Vec::with_capacity(team_source.len());
    for value in team_source {
        let source = provider_source(value.provider);
        team_stats.push(CanonicalTeamStatObservation {
            club: lookup_club(&club_map, source, &value.club_external_id)?,
            opponent: lookup_club(&club_map, source, &value.opponent_external_id)?,
            competition: lookup_competition(&competition_map, source, &value.competition_external_id)?,
            match_id: canonical_match_id(value.provider, &value.match_external_id),
            date: value.date,
            is_home: value.is_home,
            metrics: map_metrics(value.metrics, &metric_map)?,
            provenance_id: value.provenance_id,
        });
    }
    team_stats.sort_by_key(|value| (value.club, value.date, value.match_id));
    issues.sort_by(|a, b| source_sort_key(a.source).cmp(&source_sort_key(b.source)).then(a.external_id.cmp(&b.external_id)));

    let dataset = SportingDataset {
        dataset_version: base.dataset_version,
        source_manifest_checksum: base.source_manifest_checksum,
        clubs,
        competitions,
        metric_registry,
        player_stats,
        team_stats,
        matches,
        issues,
    };
    dataset.validate(base)?;
    Ok(dataset)
}

fn extend_clubs(
    base: &CanonicalDataset,
    players: &[PlayerStatSourceRecord],
    teams: &[TeamStatSourceRecord],
    matches: &[MatchResultSourceRecord],
) -> Result<(Vec<SourceClubRecord>, HashMap<(u8, String), ClubId>), DatasetError> {
    let mut map: HashMap<(u8, String), ClubId> = base.clubs.iter().map(|value| ((source_sort_key(value.source), value.external_id.clone()), value.id)).collect();
    let mut missing = BTreeSet::new();
    for value in players { missing.insert((provider_tag(value.provider), value.club_external_id.clone())); }
    for value in teams {
        missing.insert((provider_tag(value.provider), value.club_external_id.clone()));
        missing.insert((provider_tag(value.provider), value.opponent_external_id.clone()));
    }
    for value in matches {
        missing.insert((provider_tag(value.provider), value.home_club_external_id.clone()));
        missing.insert((provider_tag(value.provider), value.away_club_external_id.clone()));
    }
    missing.retain(|key| !map.contains_key(key));
    let mut next = base.clubs.iter().map(|value| value.id.0).max().unwrap_or(0).checked_add(1).ok_or_else(|| DatasetError::new("club id overflow"))?;
    let mut added = Vec::new();
    for (tag, external_id) in missing {
        let id = ClubId(next);
        next = next.checked_add(1).ok_or_else(|| DatasetError::new("club id overflow"))?;
        let source = source_from_provider_tag(tag)?;
        map.insert((tag, external_id.clone()), id);
        added.push(SourceClubRecord { id, source, external_id });
    }
    Ok((added, map))
}

fn extend_competitions(
    base: &CanonicalDataset,
    players: &[PlayerStatSourceRecord],
    teams: &[TeamStatSourceRecord],
    matches: &[MatchResultSourceRecord],
) -> Result<(Vec<SourceCompetitionRecord>, HashMap<(u8, String), CompetitionId>), DatasetError> {
    let mut map: HashMap<(u8, String), CompetitionId> = base.competitions.iter().map(|value| ((source_sort_key(value.source), value.external_id.clone()), value.id)).collect();
    let mut missing = BTreeSet::new();
    for value in players { missing.insert((provider_tag(value.provider), value.competition_external_id.clone())); }
    for value in teams { missing.insert((provider_tag(value.provider), value.competition_external_id.clone())); }
    for value in matches { missing.insert((provider_tag(value.provider), value.competition_external_id.clone())); }
    missing.retain(|key| !map.contains_key(key));
    let mut next = base.competitions.iter().map(|value| value.id.0).max().unwrap_or(0).checked_add(1).ok_or_else(|| DatasetError::new("competition id overflow"))?;
    let mut added = Vec::new();
    for (tag, external_id) in missing {
        let id = CompetitionId(next);
        next = next.checked_add(1).ok_or_else(|| DatasetError::new("competition id overflow"))?;
        let source = source_from_provider_tag(tag)?;
        map.insert((tag, external_id.clone()), id);
        added.push(SourceCompetitionRecord { id, source, external_id });
    }
    Ok((added, map))
}

fn extend_metrics(
    base: &CanonicalDataset,
    players: &[PlayerStatSourceRecord],
    teams: &[TeamStatSourceRecord],
) -> Result<(Vec<MetricDefinition>, HashMap<String, MetricId>), DatasetError> {
    let mut registry = base.metric_registry.clone();
    let mut map: HashMap<String, MetricId> = registry.iter().map(|value| (value.name.clone(), value.metric)).collect();
    let mut names = BTreeSet::new();
    for value in players { for metric in &value.metrics { names.insert(metric.name.clone()); } }
    for value in teams { for metric in &value.metrics { names.insert(metric.name.clone()); } }
    let mut next = registry.iter().map(|value| value.metric.0).max().unwrap_or(0).checked_add(1).ok_or_else(|| DatasetError::new("metric id overflow"))?;
    for name in names {
        if map.contains_key(&name) { continue; }
        let id = MetricId(next);
        next = next.checked_add(1).ok_or_else(|| DatasetError::new("metric id overflow"))?;
        map.insert(name.clone(), id);
        registry.push(MetricDefinition { metric: id, name });
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

fn validate_source_provenance(map: &HashMap<ProvenanceId, Source>, provenance: ProvenanceId, source: Source) -> Result<(), DatasetError> {
    match map.get(&provenance) {
        Some(actual) if *actual == source => Ok(()),
        Some(_) => Err(DatasetError::new("sporting record provenance source mismatch")),
        None => Err(DatasetError::new("sporting record references unknown provenance")),
    }
}

fn lookup_club(map: &HashMap<(u8, String), ClubId>, source: Source, external_id: &str) -> Result<ClubId, DatasetError> {
    map.get(&(source_sort_key(source), external_id.to_owned())).copied().ok_or_else(|| DatasetError::new("missing sporting club mapping"))
}

fn lookup_competition(map: &HashMap<(u8, String), CompetitionId>, source: Source, external_id: &str) -> Result<CompetitionId, DatasetError> {
    map.get(&(source_sort_key(source), external_id.to_owned())).copied().ok_or_else(|| DatasetError::new("missing sporting competition mapping"))
}

fn canonical_match_id(provider: StatsProvider, external_id: &str) -> MatchId {
    let prefix: &[u8] = match provider {
        StatsProvider::Sofascore => b"sofascore-match:",
        StatsProvider::StatsBomb => b"statsbomb-match:",
        StatsProvider::OpenFootball => b"openfootball-match:",
        StatsProvider::FootballDataOrg => b"football-data-org-match:",
    };
    MatchId(stable_hash64(prefix, external_id.as_bytes()))
}

pub const fn provider_source(provider: StatsProvider) -> Source {
    match provider {
        StatsProvider::Sofascore => Source::Sofascore,
        StatsProvider::StatsBomb => Source::StatsBomb,
        StatsProvider::OpenFootball => Source::OpenFootball,
        StatsProvider::FootballDataOrg => Source::FootballDataOrg,
    }
}

const fn provider_tag(provider: StatsProvider) -> u8 { source_sort_key(provider_source(provider)) }

const fn source_sort_key(source: Source) -> u8 {
    match source {
        Source::FootballManager => 1,
        Source::Transfermarkt => 2,
        Source::Fbref => 3,
        Source::Salary => 4,
        Source::Manual => 5,
        Source::Sofascore => 6,
        Source::StatsBomb => 7,
        Source::OpenFootball => 8,
        Source::FootballDataOrg => 9,
        Source::Other(_) => 255,
    }
}

fn source_from_provider_tag(tag: u8) -> Result<Source, DatasetError> {
    match tag {
        6 => Ok(Source::Sofascore),
        7 => Ok(Source::StatsBomb),
        8 => Ok(Source::OpenFootball),
        9 => Ok(Source::FootballDataOrg),
        _ => Err(DatasetError::new("invalid sporting provider tag")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalIdentity, CanonicalPlayer, CanonicalProvenance};
    use footy_core::{Confidence, Date};
    use footy_entities::LinkMethod;
    use footy_source_stats::SourceMetric;

    fn base_dataset() -> CanonicalDataset {
        CanonicalDataset {
            dataset_version: DatasetVersion(1),
            source_manifest_checksum: 7,
            provenances: vec![CanonicalProvenance { id: ProvenanceId(1), source: Source::Sofascore, input_path: "sofa.json".into(), source_checksum: 9 }],
            players: vec![CanonicalPlayer { player: PlayerId(1), fm_external_id: "fm1".into(), name: "Player".into(), birth_date: None }],
            identities: vec![CanonicalIdentity { player: PlayerId(1), source: Source::Sofascore, external_id: "p1".into(), score: Confidence::ONE, method: LinkMethod::Manual, verified: true, provenance_id: ProvenanceId(1) }],
            unresolved_identities: Vec::new(), issues: Vec::new(), fm_snapshots: Vec::new(), clubs: Vec::new(), competitions: Vec::new(), metric_registry: Vec::new(), market_values: Vec::new(), transfers: Vec::new(), contracts: Vec::new(), performance: Vec::new(), salaries: Vec::new(),
        }
    }

    #[test]
    fn canonicalizes_sporting_records_against_base_identity() {
        let base = base_dataset();
        let date = Date(10);
        let players = vec![PlayerStatSourceRecord { provider: StatsProvider::Sofascore, player_external_id: "p1".into(), club_external_id: "c1".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), match_external_id: "m1".into(), date, minutes: Some(90), metrics: vec![SourceMetric { name: "shots".into(), value: 3.0 }], provenance_id: ProvenanceId(1) }];
        let teams = vec![TeamStatSourceRecord { provider: StatsProvider::Sofascore, club_external_id: "c1".into(), opponent_external_id: "c2".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), match_external_id: "m1".into(), date, is_home: true, metrics: vec![SourceMetric { name: "shots".into(), value: 12.0 }], provenance_id: ProvenanceId(1) }];
        let matches = vec![MatchResultSourceRecord { provider: StatsProvider::Sofascore, match_external_id: "m1".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), date, home_club_external_id: "c1".into(), away_club_external_id: "c2".into(), home_goals: Some(2), away_goals: Some(1), status: MatchStatus::Finished, provenance_id: ProvenanceId(1) }];
        let sporting = build_sporting_dataset(&base, players, teams, matches).unwrap();
        assert_eq!(sporting.player_stats.len(), 1);
        assert_eq!(sporting.team_matches()[0].goals_for, Some(2));
        assert_eq!(sporting.player_matches(&base)[0].minutes, 90);
    }

    #[test]
    fn unresolved_provider_player_is_preserved_as_issue() {
        let base = base_dataset();
        let players = vec![PlayerStatSourceRecord { provider: StatsProvider::Sofascore, player_external_id: "unknown".into(), club_external_id: "c1".into(), competition_external_id: "l1".into(), season_external_id: "s1".into(), match_external_id: "m1".into(), date: Date(10), minutes: Some(90), metrics: Vec::new(), provenance_id: ProvenanceId(1) }];
        let sporting = build_sporting_dataset(&base, players, Vec::new(), Vec::new()).unwrap();
        assert!(sporting.player_stats.is_empty());
        assert_eq!(sporting.issues.len(), 1);
    }
}
