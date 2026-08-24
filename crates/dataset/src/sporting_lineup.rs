use crate::canonical::stable_hash64;
use crate::{DatasetError, SportingDataset, SportingIssue};
use footy_core::{ClubId, CompetitionId, DatasetVersion, Date, MatchId, PlayerId, ProvenanceId};
use footy_source_stats::{LineupSourceRecord, SourcePositionInterval, StatsProvider};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPositionInterval {
    pub provider_position_external_id: String,
    pub provider_position_name: Option<String>,
    pub from_minute: u16,
    pub to_minute: Option<u16>,
}

impl From<SourcePositionInterval> for CanonicalPositionInterval {
    fn from(value: SourcePositionInterval) -> Self {
        Self {
            provider_position_external_id: value.position_external_id,
            provider_position_name: value.position_name,
            from_minute: value.from_minute,
            to_minute: value.to_minute,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalLineupPlayer {
    pub player: PlayerId,
    pub started: bool,
    pub minutes: Option<u16>,
    pub positions: Vec<CanonicalPositionInterval>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalLineupObservation {
    pub provider: StatsProvider,
    pub club: ClubId,
    pub competition: CompetitionId,
    pub match_id: MatchId,
    pub date: Date,
    pub formation_external_id: Option<String>,
    pub confirmed: Option<bool>,
    pub players: Vec<CanonicalLineupPlayer>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SportingLineupDataset {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub lineups: Vec<CanonicalLineupObservation>,
    pub issues: Vec<SportingIssue>,
}

impl SportingLineupDataset {
    pub fn validate(&self, sporting: &SportingDataset) -> Result<(), DatasetError> {
        if self.dataset_version != sporting.dataset_version || self.source_manifest_checksum != sporting.source_manifest_checksum {
            return Err(DatasetError::new("lineup sidecar is not bound to the supplied sporting dataset"));
        }
        let players: HashSet<PlayerId> = sporting.player_links.iter().map(|value| value.player).collect();
        let clubs: HashSet<ClubId> = sporting.clubs.iter().map(|value| value.id).collect();
        let competitions: HashSet<CompetitionId> = sporting.competitions.iter().map(|value| value.id).collect();
        let provenances: HashSet<ProvenanceId> = sporting.provenances.iter().map(|value| value.id).collect();
        let mut keys = HashSet::new();
        for lineup in &self.lineups {
            if !clubs.contains(&lineup.club)
                || !competitions.contains(&lineup.competition)
                || !provenances.contains(&lineup.provenance_id)
                || !keys.insert((provider_tag(lineup.provider), lineup.match_id, lineup.club))
            {
                return Err(DatasetError::new("invalid canonical lineup observation"));
            }
            let mut lineup_players = HashSet::new();
            for player in &lineup.players {
                if !players.contains(&player.player) || !lineup_players.insert(player.player) {
                    return Err(DatasetError::new("invalid or duplicate canonical lineup player"));
                }
                if player.positions.iter().any(|position| position.to_minute.is_some_and(|to| to < position.from_minute)) {
                    return Err(DatasetError::new("invalid canonical lineup position interval"));
                }
            }
        }
        Ok(())
    }

    pub fn fixture_dates(&self, club: ClubId) -> Vec<Date> {
        let mut dates: Vec<Date> = self.lineups.iter().filter(|value| value.club == club).map(|value| value.date).collect();
        dates.sort();
        dates.dedup();
        dates
    }

    pub fn player_appearances(&self, player: PlayerId) -> Vec<CanonicalLineupPlayerObservation> {
        let mut values = Vec::new();
        for lineup in &self.lineups {
            if let Some(row) = lineup.players.iter().find(|value| value.player == player) {
                values.push(CanonicalLineupPlayerObservation {
                    player,
                    club: lineup.club,
                    competition: lineup.competition,
                    match_id: lineup.match_id,
                    date: lineup.date,
                    started: row.started,
                    minutes: row.minutes,
                    positions: row.positions.clone(),
                    formation_external_id: lineup.formation_external_id.clone(),
                    confirmed: lineup.confirmed,
                });
            }
        }
        values.sort_by_key(|value| (value.date, value.match_id));
        values
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalLineupPlayerObservation {
    pub player: PlayerId,
    pub club: ClubId,
    pub competition: CompetitionId,
    pub match_id: MatchId,
    pub date: Date,
    pub started: bool,
    pub minutes: Option<u16>,
    pub positions: Vec<CanonicalPositionInterval>,
    pub formation_external_id: Option<String>,
    pub confirmed: Option<bool>,
}

/// Canonicalize provider lineup histories without making provider position tokens into universal
/// roles. Player/club/competition identity is resolved here; tactical role mapping remains a
/// separately versioned model primitive because provider taxonomies do not share semantics.
pub fn canonicalize_sporting_lineups(
    sporting: &SportingDataset,
    source: Vec<LineupSourceRecord>,
) -> Result<SportingLineupDataset, DatasetError> {
    let provenance: HashMap<ProvenanceId, StatsProvider> = sporting.provenances.iter().map(|value| (value.id, value.provider)).collect();
    let links: HashMap<(u8, String), PlayerId> = sporting.player_links.iter().map(|value| ((provider_tag(value.provider), value.external_id.clone()), value.player)).collect();
    let clubs: HashMap<(u8, String), ClubId> = sporting.clubs.iter().map(|value| ((provider_tag(value.provider), value.external_id.clone()), value.id)).collect();
    let competitions: HashMap<(u8, String), CompetitionId> = sporting.competitions.iter().map(|value| ((provider_tag(value.provider), value.external_id.clone()), value.id)).collect();
    let mut issues = Vec::new();
    let mut lineups = Vec::new();

    for value in source {
        match provenance.get(&value.provenance_id) {
            Some(provider) if *provider == value.provider => {}
            Some(_) => return Err(DatasetError::new("lineup provenance provider mismatch")),
            None => return Err(DatasetError::new("lineup references unknown sporting provenance")),
        }
        let tag = provider_tag(value.provider);
        let club = clubs.get(&(tag, value.club_external_id.clone())).copied().ok_or_else(|| DatasetError::new("lineup references unknown sporting club"))?;
        let competition = competitions.get(&(tag, value.competition_external_id.clone())).copied().ok_or_else(|| DatasetError::new("lineup references unknown sporting competition"))?;
        let mut players = Vec::new();
        let mut seen = HashSet::new();
        for row in value.players {
            let Some(player) = links.get(&(tag, row.player_external_id.clone())).copied() else {
                issues.push(SportingIssue {
                    provider: value.provider,
                    external_id: row.player_external_id,
                    message: "lineup player skipped: unresolved canonical player".into(),
                });
                continue;
            };
            if !seen.insert(player) {
                return Err(DatasetError::new("duplicate player in provider lineup"));
            }
            players.push(CanonicalLineupPlayer {
                player,
                started: row.started,
                minutes: row.minutes,
                positions: row.positions.into_iter().map(Into::into).collect(),
            });
        }
        players.sort_by_key(|value| value.player);
        lineups.push(CanonicalLineupObservation {
            provider: value.provider,
            club,
            competition,
            match_id: canonical_match_id(value.provider, &value.match_external_id),
            date: value.date,
            formation_external_id: value.formation_external_id,
            confirmed: value.confirmed,
            players,
            provenance_id: value.provenance_id,
        });
    }
    lineups.sort_by_key(|value| (value.date, value.match_id, value.club));
    issues.sort_by(|a, b| provider_tag(a.provider).cmp(&provider_tag(b.provider)).then(a.external_id.cmp(&b.external_id)));
    let dataset = SportingLineupDataset {
        dataset_version: sporting.dataset_version,
        source_manifest_checksum: sporting.source_manifest_checksum,
        lineups,
        issues,
    };
    dataset.validate(sporting)?;
    Ok(dataset)
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

const fn provider_tag(provider: StatsProvider) -> u8 {
    match provider {
        StatsProvider::Sofascore => 1,
        StatsProvider::StatsBomb => 2,
        StatsProvider::OpenFootball => 3,
        StatsProvider::FootballDataOrg => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SportingClubRecord, SportingCompetitionRecord, SportingPlayerLink, SportingProvenance};
    use footy_core::Confidence;
    use footy_entities::LinkMethod;
    use footy_source_stats::{LineupPlayerSourceRecord, SourcePositionInterval};

    fn sporting() -> SportingDataset {
        SportingDataset {
            dataset_version: DatasetVersion(1), source_manifest_checksum: 7,
            provenances: vec![SportingProvenance { id: ProvenanceId(1), provider: StatsProvider::StatsBomb, input_path: "lineup.json".into(), source_checksum: 1 }],
            player_links: vec![SportingPlayerLink { provider: StatsProvider::StatsBomb, external_id: "99".into(), player: PlayerId(1), score: Confidence::ONE, method: LinkMethod::Manual, verified: true }],
            clubs: vec![SportingClubRecord { id: ClubId(1), provider: StatsProvider::StatsBomb, external_id: "11".into() }],
            competitions: vec![SportingCompetitionRecord { id: CompetitionId(1), provider: StatsProvider::StatsBomb, external_id: "9".into() }],
            metric_registry: Vec::new(), player_stats: Vec::new(), team_stats: Vec::new(), matches: Vec::new(), issues: Vec::new(),
        }
    }

    #[test]
    fn canonicalizes_lineup_identity_but_preserves_provider_role_token() {
        let source = LineupSourceRecord {
            provider: StatsProvider::StatsBomb, club_external_id: "11".into(), competition_external_id: "9".into(), season_external_id: "10".into(),
            match_external_id: "1".into(), date: Date(10), formation_external_id: None, confirmed: Some(true), provenance_id: ProvenanceId(1),
            players: vec![LineupPlayerSourceRecord { player_external_id: "99".into(), started: true, minutes: Some(90), positions: vec![SourcePositionInterval { position_external_id: "17".into(), position_name: Some("Right Wing".into()), from_minute: 0, to_minute: Some(90) }] }],
        };
        let result = canonicalize_sporting_lineups(&sporting(), vec![source]).unwrap();
        assert_eq!(result.lineups[0].players[0].player, PlayerId(1));
        assert_eq!(result.lineups[0].players[0].positions[0].provider_position_external_id, "17");
        assert_eq!(result.fixture_dates(ClubId(1)), vec![Date(10)]);
    }
}
