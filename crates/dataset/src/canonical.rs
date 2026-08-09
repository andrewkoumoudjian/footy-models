use crate::{DatasetError, ExternalPlayerRecord};
use footy_core::{
    ClubId, CompetitionId, Confidence, DatasetVersion, Date, MetricId, ObservationState, PlayerId,
    ProvenanceId, Source, StringInterner, TransferId,
};
use footy_entities::{
    normalize_name, normalized_surname, resolve_candidates, score_candidate, IdentityCandidate,
    IdentityMetadata, LinkMethod, MatchWeights, Resolution,
};
use footy_performance::MetricValue;
use footy_source_fbref::PerformanceSourceRecord;
use footy_source_fm::{FmAttribute, FmSnapshotRecord, PositionRating};
use footy_source_salary::{
    CompensationKind, CurrencyRange, Periodicity, SalaryObservation, WageBasis,
};
use footy_source_transfermarkt::{
    ContractSourceRecord, MarketValueSourceRecord, TransferKind as SourceTransferKind,
    TransferSourceRecord,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalProvenance {
    pub id: ProvenanceId,
    pub source: Source,
    pub input_path: String,
    pub source_checksum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPlayer {
    pub player: PlayerId,
    pub fm_external_id: String,
    pub name: String,
    pub birth_date: Option<Date>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalIdentity {
    pub player: PlayerId,
    pub source: Source,
    pub external_id: String,
    pub score: Confidence,
    pub method: LinkMethod,
    pub verified: bool,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedReason {
    NoCandidate,
    BelowThreshold,
    Ambiguous,
    MissingMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalUnresolvedCandidate {
    pub player: PlayerId,
    pub score: Confidence,
    pub method: LinkMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalUnresolvedIdentity {
    pub source: Source,
    pub external_id: String,
    pub name: String,
    pub birth_date: Option<Date>,
    pub reason: UnresolvedReason,
    pub candidates: Vec<CanonicalUnresolvedCandidate>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalIssue {
    pub source: Source,
    pub external_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFmSnapshot {
    pub player: PlayerId,
    pub snapshot_date: Date,
    pub current_ability: Option<u16>,
    pub potential_ability: Option<u16>,
    pub reputation: Option<u16>,
    pub attributes: Vec<FmAttribute>,
    pub positions: Vec<PositionRating>,
    pub personality_attributes: Vec<FmAttribute>,
    pub hidden_attributes: Vec<FmAttribute>,
    pub provenance_id: ProvenanceId,
    pub schema_version: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalMarketValue {
    pub player: PlayerId,
    pub date: Date,
    pub value: footy_core::EuroCents,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceClubRef {
    pub id: ClubId,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceClubRecord {
    pub id: ClubId,
    pub source: Source,
    pub external_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceCompetitionRef {
    pub id: CompetitionId,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCompetitionRecord {
    pub id: CompetitionId,
    pub source: Source,
    pub external_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalTransfer {
    pub id: TransferId,
    pub player: PlayerId,
    pub date: Date,
    pub from: ClubId,
    pub to: ClubId,
    pub kind: footy_market::TransferKind,
    pub fee: Option<footy_core::EuroCents>,
    pub loan_fee: Option<footy_core::EuroCents>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalContract {
    pub player: PlayerId,
    pub club: ClubId,
    pub observed_on: Date,
    pub expiry: Option<Date>,
    pub option_years: Option<u8>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalPerformanceObservation {
    pub player: PlayerId,
    pub club: ClubId,
    pub competition: CompetitionId,
    pub season_external_id: String,
    pub match_external_id: Option<String>,
    pub start_date: Date,
    pub end_date: Date,
    pub minutes: u32,
    pub metrics: Vec<MetricValue>,
    pub observation_state: ObservationState,
    pub provenance_id: ProvenanceId,
}

impl CanonicalPerformanceObservation {
    pub fn is_match_level(&self) -> bool {
        self.start_date == self.end_date
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricDefinition {
    pub metric: MetricId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalSalaryObservation {
    pub player: PlayerId,
    pub club: ClubId,
    pub effective_date: Date,
    pub amount: CurrencyRange,
    pub basis: WageBasis,
    pub periodicity: Periodicity,
    pub kind: CompensationKind,
    pub provenance_id: ProvenanceId,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalDataset {
    pub dataset_version: DatasetVersion,
    pub source_manifest_checksum: u64,
    pub provenances: Vec<CanonicalProvenance>,
    pub players: Vec<CanonicalPlayer>,
    pub identities: Vec<CanonicalIdentity>,
    pub unresolved_identities: Vec<CanonicalUnresolvedIdentity>,
    pub issues: Vec<CanonicalIssue>,
    pub fm_snapshots: Vec<CanonicalFmSnapshot>,
    pub clubs: Vec<SourceClubRecord>,
    pub competitions: Vec<SourceCompetitionRecord>,
    pub metric_registry: Vec<MetricDefinition>,
    pub market_values: Vec<CanonicalMarketValue>,
    pub transfers: Vec<CanonicalTransfer>,
    pub contracts: Vec<CanonicalContract>,
    pub performance: Vec<CanonicalPerformanceObservation>,
    pub salaries: Vec<CanonicalSalaryObservation>,
}

impl CanonicalDataset {
    pub fn validate(&self) -> Result<(), DatasetError> {
        let mut player_ids = HashSet::new();
        let mut fm_ids = HashSet::new();
        for player in &self.players {
            if !player_ids.insert(player.player) {
                return Err(DatasetError::new("duplicate canonical player id"));
            }
            if !fm_ids.insert(player.fm_external_id.as_str()) {
                return Err(DatasetError::new("duplicate FM external id"));
            }
        }
        let mut identity_keys = HashSet::new();
        for identity in &self.identities {
            if !player_ids.contains(&identity.player) {
                return Err(DatasetError::new("identity references unknown player"));
            }
            let key = (source_tag(identity.source), identity.external_id.as_str());
            if !identity_keys.insert(key) {
                return Err(DatasetError::new("source identity is mapped more than once"));
            }
        }
        let mut metric_ids = HashSet::new();
        let mut metric_names = HashSet::new();
        for metric in &self.metric_registry {
            if metric.metric.0 == 0 || !metric_ids.insert(metric.metric) || !metric_names.insert(metric.name.as_str()) {
                return Err(DatasetError::new("invalid or duplicate metric registry entry"));
            }
        }
        for snapshot in &self.fm_snapshots {
            if !player_ids.contains(&snapshot.player) { return Err(DatasetError::new("FM snapshot references unknown player")); }
        }
        for value in &self.market_values {
            if !player_ids.contains(&value.player) || value.value.cents() <= 0 { return Err(DatasetError::new("invalid canonical market value")); }
        }
        for transfer in &self.transfers {
            if !player_ids.contains(&transfer.player) { return Err(DatasetError::new("transfer references unknown player")); }
        }
        for observation in &self.performance {
            if !player_ids.contains(&observation.player) || observation.end_date < observation.start_date {
                return Err(DatasetError::new("invalid canonical performance observation"));
            }
            if observation.metrics.iter().any(|metric| !metric_ids.contains(&metric.metric) || !metric.value.is_finite()) {
                return Err(DatasetError::new("performance observation references invalid metric"));
            }
        }
        for salary in &self.salaries {
            if !player_ids.contains(&salary.player) { return Err(DatasetError::new("salary references unknown player")); }
        }
        Ok(())
    }

    pub fn identity(&self, source: Source, external_id: &str) -> Option<PlayerId> {
        self.identities.iter().find(|identity| identity.source == source && identity.external_id == external_id).map(|identity| identity.player)
    }

    pub fn player(&self, player: PlayerId) -> Option<&CanonicalPlayer> {
        self.players.iter().find(|candidate| candidate.player == player)
    }

    pub fn metric_name(&self, metric: MetricId) -> Option<&str> {
        self.metric_registry.iter().find(|definition| definition.metric == metric).map(|definition| definition.name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalPlayerRecord {
    pub source: Source,
    pub external_id: String,
    pub name: String,
    pub birth_date: Option<Date>,
    pub height_cm: Option<u16>,
    pub nationality: Option<String>,
    pub provenance_id: ProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityOverride {
    pub source: Source,
    pub external_id: String,
    pub fm_external_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalBuildConfig {
    pub minimum_identity_score: f32,
    pub ambiguity_margin: f32,
    pub max_unresolved_candidates: usize,
}

impl Default for CanonicalBuildConfig {
    fn default() -> Self {
        Self { minimum_identity_score: 0.88, ambiguity_margin: 0.05, max_unresolved_candidates: 5 }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_canonical_dataset(
    dataset_version: DatasetVersion,
    source_manifest_checksum: u64,
    provenances: Vec<CanonicalProvenance>,
    mut fm_records: Vec<FmSnapshotRecord>,
    external_players: Vec<ExternalPlayerRecord>,
    tm_market_values: Vec<MarketValueSourceRecord>,
    tm_transfers: Vec<TransferSourceRecord>,
    tm_contracts: Vec<ContractSourceRecord>,
    fbref_performance: Vec<PerformanceSourceRecord>,
    salaries: Vec<SalaryObservation>,
    overrides: Vec<IdentityOverride>,
    config: CanonicalBuildConfig,
) -> Result<CanonicalDataset, DatasetError> {
    if !config.minimum_identity_score.is_finite() || !config.ambiguity_margin.is_finite() {
        return Err(DatasetError::new("identity thresholds must be finite"));
    }
    let provenance_ids: HashSet<ProvenanceId> = provenances.iter().map(|value| value.id).collect();
    if provenance_ids.len() != provenances.len() {
        return Err(DatasetError::new("duplicate provenance id"));
    }

    fm_records.sort_by(|left, right| left.external_player_id.cmp(&right.external_player_id).then(left.snapshot_date.cmp(&right.snapshot_date)));
    let mut groups: BTreeMap<String, Vec<FmSnapshotRecord>> = BTreeMap::new();
    for record in fm_records {
        if !provenance_ids.contains(&record.provenance_id) { return Err(DatasetError::new("FM record references unknown provenance")); }
        groups.entry(record.external_player_id.clone()).or_default().push(record);
    }
    if groups.is_empty() { return Err(DatasetError::new("canonical dataset requires at least one FM player")); }

    let mut players = Vec::with_capacity(groups.len());
    let mut fm_snapshots = Vec::new();
    let mut fm_to_player = HashMap::new();
    let mut used_player_ids = HashMap::<PlayerId, String>::new();
    for (external_id, records) in &groups {
        let player = stable_player_id(external_id);
        if let Some(existing) = used_player_ids.insert(player, external_id.clone()) {
            if existing != *external_id { return Err(DatasetError::new("canonical player hash collision")); }
        }
        fm_to_player.insert(external_id.clone(), player);
        let latest = records.last().ok_or_else(|| DatasetError::new("empty FM player group"))?;
        let birth_date = records.iter().rev().find_map(|record| record.birth_date);
        players.push(CanonicalPlayer { player, fm_external_id: external_id.clone(), name: latest.name.clone(), birth_date });
        for record in records {
            fm_snapshots.push(CanonicalFmSnapshot {
                player,
                snapshot_date: record.snapshot_date,
                current_ability: record.current_ability,
                potential_ability: record.potential_ability,
                reputation: record.reputation,
                attributes: record.attributes.clone(),
                positions: record.positions.clone(),
                personality_attributes: record.personality_attributes.clone(),
                hidden_attributes: record.hidden_attributes.clone(),
                provenance_id: record.provenance_id,
                schema_version: record.schema_version,
            });
        }
    }
    players.sort_by_key(|player| player.player);
    fm_snapshots.sort_by_key(|snapshot| (snapshot.player, snapshot.snapshot_date));

    let mut identities = Vec::new();
    for (external_id, player) in &fm_to_player {
        let provenance_id = groups.get(external_id).and_then(|records| records.last()).map(|record| record.provenance_id).ok_or_else(|| DatasetError::new("missing FM provenance"))?;
        identities.push(CanonicalIdentity { player: *player, source: Source::FootballManager, external_id: external_id.clone(), score: Confidence::ONE, method: LinkMethod::Explicit, verified: true, provenance_id });
    }

    let mut override_map = HashMap::new();
    for value in overrides {
        if value.source == Source::FootballManager || matches!(value.source, Source::Other(_)) {
            return Err(DatasetError::new("identity override source must be an external known source"));
        }
        let Some(player) = fm_to_player.get(&value.fm_external_id).copied() else {
            return Err(DatasetError::new(format!("identity override references unknown FM id {}", value.fm_external_id)));
        };
        let key = (source_tag(value.source), value.external_id.clone());
        if override_map.insert(key, player).is_some() { return Err(DatasetError::new("duplicate identity override")); }
    }

    let index = PlayerIdentityIndex::new(&players)?;
    let mut resolved = HashMap::<(u8, String), PlayerId>::new();
    for identity in &identities { resolved.insert((source_tag(identity.source), identity.external_id.clone()), identity.player); }
    let mut unresolved_identities = Vec::new();
    let mut external_seen = HashSet::new();
    for external in external_players {
        if external.source == Source::FootballManager || matches!(external.source, Source::Other(_)) {
            return Err(DatasetError::new("external player metadata uses unsupported source"));
        }
        if !provenance_ids.contains(&external.provenance_id) { return Err(DatasetError::new("external player references unknown provenance")); }
        let key = (source_tag(external.source), external.external_id.clone());
        if !external_seen.insert(key.clone()) { return Err(DatasetError::new("duplicate external player metadata")); }

        if let Some(player) = override_map.get(&key).copied() {
            identities.push(CanonicalIdentity { player, source: external.source, external_id: external.external_id.clone(), score: Confidence::ONE, method: LinkMethod::Manual, verified: true, provenance_id: external.provenance_id });
            resolved.insert(key, player);
            continue;
        }

        let candidates = index.candidates(&external);
        let scored = candidates.into_iter().filter_map(|player| {
            let canonical = players.iter().find(|candidate| candidate.player == player)?;
            Some(score_external_candidate(player, &external, canonical))
        }).collect::<Vec<_>>();
        let resolution = resolve_candidates(scored.clone(), config.minimum_identity_score, config.ambiguity_margin);
        match resolution {
            Resolution::Unique(candidate) => {
                identities.push(CanonicalIdentity { player: candidate.player, source: external.source, external_id: external.external_id.clone(), score: candidate.score, method: candidate.method, verified: false, provenance_id: external.provenance_id });
                resolved.insert(key, candidate.player);
            }
            Resolution::Ambiguous(candidates) => unresolved_identities.push(unresolved_from(&external, UnresolvedReason::Ambiguous, candidates, config.max_unresolved_candidates)),
            Resolution::NoMatch => {
                let reason = if scored.is_empty() { UnresolvedReason::NoCandidate } else if external.birth_date.is_none() && normalize_name(&external.name).is_empty() { UnresolvedReason::MissingMetadata } else { UnresolvedReason::BelowThreshold };
                unresolved_identities.push(unresolved_from(&external, reason, scored, config.max_unresolved_candidates));
            }
        }
    }
    identities.sort_by(|left, right| source_tag(left.source).cmp(&source_tag(right.source)).then(left.external_id.cmp(&right.external_id)));
    unresolved_identities.sort_by(|left, right| source_tag(left.source).cmp(&source_tag(right.source)).then(left.external_id.cmp(&right.external_id)));

    let (clubs, club_map) = build_club_registry(&tm_transfers, &tm_contracts, &fbref_performance, &salaries)?;
    let (competitions, competition_map) = build_competition_registry(&fbref_performance)?;
    let (metric_registry, metric_map) = build_metric_registry(&fbref_performance)?;

    let mut issues = Vec::new();
    let mut market_values = Vec::new();
    for value in tm_market_values {
        if let Some(player) = lookup_resolved(&resolved, Source::Transfermarkt, &value.player_external_id) {
            market_values.push(CanonicalMarketValue { player, date: value.date, value: value.value, provenance_id: value.provenance_id });
        } else {
            issues.push(orphan_issue(Source::Transfermarkt, &value.player_external_id, "market value skipped because player identity is unresolved"));
        }
    }
    market_values.sort_by_key(|value| (value.player, value.date));

    let mut transfers = Vec::new();
    let mut transfer_ids = HashSet::new();
    for transfer in tm_transfers {
        let Some(player) = lookup_resolved(&resolved, Source::Transfermarkt, &transfer.player_external_id) else {
            issues.push(orphan_issue(Source::Transfermarkt, &transfer.player_external_id, "transfer skipped because player identity is unresolved"));
            continue;
        };
        let from = *club_map.get(&(source_tag(Source::Transfermarkt), transfer.from_club_external_id.clone())).ok_or_else(|| DatasetError::new("missing Transfermarkt club registry entry"))?;
        let to = *club_map.get(&(source_tag(Source::Transfermarkt), transfer.to_club_external_id.clone())).ok_or_else(|| DatasetError::new("missing Transfermarkt club registry entry"))?;
        let identity = transfer.external_transfer_id.clone().unwrap_or_else(|| format!("{}:{}:{}:{}:{}", transfer.player_external_id, transfer.date.0, transfer.from_club_external_id, transfer.to_club_external_id, source_transfer_tag(transfer.kind)));
        let id = TransferId(stable_hash64(b"tm-transfer:", identity.as_bytes()));
        if !transfer_ids.insert(id) { return Err(DatasetError::new("canonical transfer id collision or duplicate")); }
        transfers.push(CanonicalTransfer { id, player, date: transfer.date, from, to, kind: market_transfer_kind(transfer.kind), fee: transfer.fee, loan_fee: transfer.loan_fee, provenance_id: transfer.provenance_id });
    }
    transfers.sort_by_key(|transfer| (transfer.date, transfer.id));

    let mut contracts = Vec::new();
    for contract in tm_contracts {
        let Some(player) = lookup_resolved(&resolved, Source::Transfermarkt, &contract.player_external_id) else {
            issues.push(orphan_issue(Source::Transfermarkt, &contract.player_external_id, "contract skipped because player identity is unresolved"));
            continue;
        };
        let club = *club_map.get(&(source_tag(Source::Transfermarkt), contract.club_external_id.clone())).ok_or_else(|| DatasetError::new("missing Transfermarkt contract club"))?;
        contracts.push(CanonicalContract { player, club, observed_on: contract.observed_on, expiry: contract.expiry, option_years: contract.option_years, provenance_id: contract.provenance_id });
    }
    contracts.sort_by_key(|contract| (contract.player, contract.observed_on));

    let mut performance = Vec::new();
    for observation in fbref_performance {
        let Some(player) = lookup_resolved(&resolved, Source::Fbref, &observation.player_external_id) else {
            issues.push(orphan_issue(Source::Fbref, &observation.player_external_id, "performance skipped because player identity is unresolved"));
            continue;
        };
        let club = *club_map.get(&(source_tag(Source::Fbref), observation.club_external_id.clone())).ok_or_else(|| DatasetError::new("missing FBref club registry entry"))?;
        let competition = *competition_map.get(&(source_tag(Source::Fbref), observation.competition_external_id.clone())).ok_or_else(|| DatasetError::new("missing FBref competition registry entry"))?;
        let mut metrics = Vec::with_capacity(observation.metrics.len());
        for metric in observation.metrics {
            let id = *metric_map.get(&metric.name).ok_or_else(|| DatasetError::new("missing metric registry entry"))?;
            metrics.push(MetricValue { metric: id, value: metric.value });
        }
        metrics.sort_by_key(|metric| metric.metric);
        performance.push(CanonicalPerformanceObservation { player, club, competition, season_external_id: observation.season_external_id, match_external_id: observation.match_external_id, start_date: observation.start_date, end_date: observation.end_date, minutes: observation.minutes, metrics, observation_state: ObservationState::Measured, provenance_id: observation.provenance_id });
    }
    performance.sort_by_key(|observation| (observation.player, observation.end_date, observation.club, observation.competition));

    let mut canonical_salaries = Vec::new();
    for salary in salaries {
        let Some(player) = lookup_resolved(&resolved, Source::Salary, &salary.player_external_id) else {
            issues.push(orphan_issue(Source::Salary, &salary.player_external_id, "salary skipped because player identity is unresolved"));
            continue;
        };
        let club = *club_map.get(&(source_tag(Source::Salary), salary.club_external_id.clone())).ok_or_else(|| DatasetError::new("missing salary club registry entry"))?;
        canonical_salaries.push(CanonicalSalaryObservation { player, club, effective_date: salary.effective_date, amount: salary.amount, basis: salary.basis, periodicity: salary.periodicity, kind: salary.kind, provenance_id: salary.provenance_id, confidence: salary.confidence });
    }
    canonical_salaries.sort_by_key(|salary| (salary.player, salary.effective_date));

    issues.sort_by(|left, right| source_tag(left.source).cmp(&source_tag(right.source)).then(left.external_id.cmp(&right.external_id)).then(left.message.cmp(&right.message)));
    let dataset = CanonicalDataset { dataset_version, source_manifest_checksum, provenances, players, identities, unresolved_identities, issues, fm_snapshots, clubs, competitions, metric_registry, market_values, transfers, contracts, performance, salaries: canonical_salaries };
    dataset.validate()?;
    Ok(dataset)
}

struct PlayerIdentityIndex {
    by_name: HashMap<String, Vec<PlayerId>>,
    by_birth_date: HashMap<Date, Vec<PlayerId>>,
    by_surname: HashMap<String, Vec<PlayerId>>,
}

impl PlayerIdentityIndex {
    fn new(players: &[CanonicalPlayer]) -> Result<Self, DatasetError> {
        let mut by_name: HashMap<String, Vec<PlayerId>> = HashMap::new();
        let mut by_birth_date: HashMap<Date, Vec<PlayerId>> = HashMap::new();
        let mut by_surname: HashMap<String, Vec<PlayerId>> = HashMap::new();
        for player in players {
            let normalized = normalize_name(&player.name);
            if !normalized.is_empty() { by_name.entry(normalized).or_default().push(player.player); }
            if let Some(date) = player.birth_date { by_birth_date.entry(date).or_default().push(player.player); }
            let surname = normalized_surname(&player.name);
            if !surname.is_empty() { by_surname.entry(surname).or_default().push(player.player); }
        }
        if by_name.values().any(|players| players.len() > u16::MAX as usize) { return Err(DatasetError::new("identity name bucket is unexpectedly large")); }
        Ok(Self { by_name, by_birth_date, by_surname })
    }

    fn candidates(&self, external: &ExternalPlayerRecord) -> Vec<PlayerId> {
        let mut set = BTreeSet::new();
        let normalized = normalize_name(&external.name);
        if let Some(players) = self.by_name.get(&normalized) { set.extend(players.iter().copied()); }
        if let Some(date) = external.birth_date {
            if let Some(players) = self.by_birth_date.get(&date) { set.extend(players.iter().copied()); }
        }
        let surname = normalized_surname(&external.name);
        if let Some(players) = self.by_surname.get(&surname) {
            if players.len() <= 32 || external.birth_date.is_some() { set.extend(players.iter().copied()); }
        }
        set.into_iter().collect()
    }
}

fn score_external_candidate(player: PlayerId, external: &ExternalPlayerRecord, canonical: &CanonicalPlayer) -> IdentityCandidate {
    let mut interner = StringInterner::new();
    let external_nationality = external.nationality.as_deref().and_then(|value| interner.intern(&normalize_name(value)).ok());
    let left = IdentityMetadata { name: &external.name, birth_date: external.birth_date, club: None, height_cm: external.height_cm, nationality: external_nationality };
    let right = IdentityMetadata { name: &canonical.name, birth_date: canonical.birth_date, club: None, height_cm: None, nationality: None };
    score_candidate(player, &left, &right, MatchWeights::default())
}

fn unresolved_from(external: &ExternalPlayerRecord, reason: UnresolvedReason, mut candidates: Vec<IdentityCandidate>, limit: usize) -> CanonicalUnresolvedIdentity {
    candidates.sort_by(|left, right| right.score.get().total_cmp(&left.score.get()).then(left.player.cmp(&right.player)));
    candidates.truncate(limit);
    CanonicalUnresolvedIdentity { source: external.source, external_id: external.external_id.clone(), name: external.name.clone(), birth_date: external.birth_date, reason, candidates: candidates.into_iter().map(|candidate| CanonicalUnresolvedCandidate { player: candidate.player, score: candidate.score, method: candidate.method }).collect(), provenance_id: external.provenance_id }
}

fn lookup_resolved(map: &HashMap<(u8, String), PlayerId>, source: Source, external_id: &str) -> Option<PlayerId> {
    map.get(&(source_tag(source), external_id.to_owned())).copied()
}

fn build_club_registry(
    transfers: &[TransferSourceRecord],
    contracts: &[ContractSourceRecord],
    performance: &[PerformanceSourceRecord],
    salaries: &[SalaryObservation],
) -> Result<(Vec<SourceClubRecord>, HashMap<(u8, String), ClubId>), DatasetError> {
    let mut keys = BTreeSet::new();
    for transfer in transfers {
        keys.insert((source_tag(Source::Transfermarkt), transfer.from_club_external_id.clone()));
        keys.insert((source_tag(Source::Transfermarkt), transfer.to_club_external_id.clone()));
    }
    for contract in contracts { keys.insert((source_tag(Source::Transfermarkt), contract.club_external_id.clone())); }
    for observation in performance { keys.insert((source_tag(Source::Fbref), observation.club_external_id.clone())); }
    for salary in salaries { keys.insert((source_tag(Source::Salary), salary.club_external_id.clone())); }
    if keys.len() > u32::MAX as usize { return Err(DatasetError::new("too many source-scoped clubs")); }
    let mut records = Vec::with_capacity(keys.len());
    let mut map = HashMap::with_capacity(keys.len());
    for (index, (tag, external_id)) in keys.into_iter().enumerate() {
        let id = ClubId(u32::try_from(index + 1).map_err(|_| DatasetError::new("club id overflow"))?);
        let source = source_from_tag(tag)?;
        map.insert((tag, external_id.clone()), id);
        records.push(SourceClubRecord { id, source, external_id });
    }
    Ok((records, map))
}

fn build_competition_registry(performance: &[PerformanceSourceRecord]) -> Result<(Vec<SourceCompetitionRecord>, HashMap<(u8, String), CompetitionId>), DatasetError> {
    let keys: BTreeSet<(u8, String)> = performance.iter().map(|observation| (source_tag(Source::Fbref), observation.competition_external_id.clone())).collect();
    if keys.len() > u32::MAX as usize { return Err(DatasetError::new("too many source-scoped competitions")); }
    let mut records = Vec::with_capacity(keys.len());
    let mut map = HashMap::with_capacity(keys.len());
    for (index, (tag, external_id)) in keys.into_iter().enumerate() {
        let id = CompetitionId(u32::try_from(index + 1).map_err(|_| DatasetError::new("competition id overflow"))?);
        let source = source_from_tag(tag)?;
        map.insert((tag, external_id.clone()), id);
        records.push(SourceCompetitionRecord { id, source, external_id });
    }
    Ok((records, map))
}

fn build_metric_registry(performance: &[PerformanceSourceRecord]) -> Result<(Vec<MetricDefinition>, HashMap<String, MetricId>), DatasetError> {
    let names: BTreeSet<String> = performance.iter().flat_map(|observation| observation.metrics.iter().map(|metric| metric.name.clone())).collect();
    if names.len() >= u16::MAX as usize { return Err(DatasetError::new("too many performance metrics")); }
    let mut records = Vec::with_capacity(names.len());
    let mut map = HashMap::with_capacity(names.len());
    for (index, name) in names.into_iter().enumerate() {
        let id = MetricId(u16::try_from(index + 1).map_err(|_| DatasetError::new("metric id overflow"))?);
        map.insert(name.clone(), id);
        records.push(MetricDefinition { metric: id, name });
    }
    Ok((records, map))
}

fn stable_player_id(external_id: &str) -> PlayerId {
    PlayerId(stable_hash64(b"fm-player:", external_id.as_bytes()))
}

pub(crate) fn stable_hash64(prefix: &[u8], value: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in prefix.iter().chain(value) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub(crate) const fn source_tag(source: Source) -> u8 {
    match source {
        Source::FootballManager => 1,
        Source::Transfermarkt => 2,
        Source::Fbref => 3,
        Source::Salary => 4,
        Source::Manual => 5,
        Source::Other(_) => 255,
    }
}

pub(crate) fn source_from_tag(tag: u8) -> Result<Source, DatasetError> {
    match tag {
        1 => Ok(Source::FootballManager),
        2 => Ok(Source::Transfermarkt),
        3 => Ok(Source::Fbref),
        4 => Ok(Source::Salary),
        5 => Ok(Source::Manual),
        _ => Err(DatasetError::new("unsupported persisted source tag")),
    }
}

fn market_transfer_kind(kind: SourceTransferKind) -> footy_market::TransferKind {
    match kind {
        SourceTransferKind::Permanent => footy_market::TransferKind::Permanent,
        SourceTransferKind::Free => footy_market::TransferKind::Free,
        SourceTransferKind::Loan => footy_market::TransferKind::Loan,
        SourceTransferKind::LoanWithOption => footy_market::TransferKind::LoanWithOption,
        SourceTransferKind::LoanWithObligation => footy_market::TransferKind::LoanWithObligation,
        SourceTransferKind::Youth => footy_market::TransferKind::Youth,
        SourceTransferKind::ReturnFromLoan => footy_market::TransferKind::ReturnFromLoan,
        SourceTransferKind::Internal => footy_market::TransferKind::Internal,
        SourceTransferKind::Unknown => footy_market::TransferKind::Unknown,
    }
}

fn source_transfer_tag(kind: SourceTransferKind) -> u8 {
    match kind {
        SourceTransferKind::Permanent => 1,
        SourceTransferKind::Free => 2,
        SourceTransferKind::Loan => 3,
        SourceTransferKind::LoanWithOption => 4,
        SourceTransferKind::LoanWithObligation => 5,
        SourceTransferKind::Youth => 6,
        SourceTransferKind::ReturnFromLoan => 7,
        SourceTransferKind::Internal => 8,
        SourceTransferKind::Unknown => 9,
    }
}

fn orphan_issue(source: Source, external_id: &str, message: &str) -> CanonicalIssue {
    CanonicalIssue { source, external_id: external_id.to_owned(), message: message.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(id: &str, name: &str, dob: Date) -> FmSnapshotRecord {
        FmSnapshotRecord { external_player_id: id.into(), snapshot_date: Date(10), name: name.into(), birth_date: Some(dob), current_ability: Some(100), potential_ability: Some(150), reputation: None, attributes: Vec::new(), positions: Vec::new(), personality_attributes: Vec::new(), hidden_attributes: Vec::new(), provenance_id: ProvenanceId(1), schema_version: 1 }
    }

    #[test]
    fn exact_name_and_dob_resolve_to_fm_anchor() {
        let dob = Date::from_ymd(2000, 1, 1).unwrap();
        let dataset = build_canonical_dataset(
            DatasetVersion(1), 7,
            vec![CanonicalProvenance { id: ProvenanceId(1), source: Source::FootballManager, input_path: "fm.tsv".into(), source_checksum: 1 }, CanonicalProvenance { id: ProvenanceId(2), source: Source::Transfermarkt, input_path: "tm.tsv".into(), source_checksum: 2 }],
            vec![fm("fm1", "João Félix", dob)],
            vec![ExternalPlayerRecord { source: Source::Transfermarkt, external_id: "tm1".into(), name: "Joao Felix".into(), birth_date: Some(dob), height_cm: None, nationality: None, provenance_id: ProvenanceId(2) }],
            Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), CanonicalBuildConfig::default(),
        ).unwrap();
        assert_eq!(dataset.players.len(), 1);
        assert_eq!(dataset.identity(Source::Transfermarkt, "tm1"), Some(dataset.players[0].player));
        assert!(dataset.unresolved_identities.is_empty());
    }

    #[test]
    fn ambiguous_external_identity_is_preserved_for_review() {
        let dob = Date::from_ymd(2000, 1, 1).unwrap();
        let dataset = build_canonical_dataset(
            DatasetVersion(1), 7,
            vec![CanonicalProvenance { id: ProvenanceId(1), source: Source::FootballManager, input_path: "fm.tsv".into(), source_checksum: 1 }, CanonicalProvenance { id: ProvenanceId(2), source: Source::Fbref, input_path: "fb.tsv".into(), source_checksum: 2 }],
            vec![fm("fm1", "Alex Smith", dob), fm("fm2", "Alex Smith", dob)],
            vec![ExternalPlayerRecord { source: Source::Fbref, external_id: "fb1".into(), name: "Alex Smith".into(), birth_date: Some(dob), height_cm: None, nationality: None, provenance_id: ProvenanceId(2) }],
            Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), CanonicalBuildConfig::default(),
        ).unwrap();
        assert!(dataset.identity(Source::Fbref, "fb1").is_none());
        assert_eq!(dataset.unresolved_identities.len(), 1);
        assert_eq!(dataset.unresolved_identities[0].reason, UnresolvedReason::Ambiguous);
    }
}
