#![forbid(unsafe_code)]
//! Value-surge (future-gem) proposal scoring: candidate screening, model scoring,
//! proposals CSV output, and deliverable validation (MV range, completeness,
//! determinism). All screening/scoring/ranking is deterministic: the same model,
//! candidate matrix, and metadata produce byte-identical proposals.

use footy_artifacts::{
    read_forecast_feature_matrix, read_forecast_model, read_surge_feature_matrix,
    read_surge_model, read_tail_forecast_model, ForecastModelArtifact, SurgeModelArtifact,
    TailForecastModelArtifact,
};
use footy_recruitment::surge::{
    fm_pa_band_midpoint, key_strengths, surge_screen, PositionClass, QualityAttributes,
    ScreenReason,
};
use std::collections::HashMap;
use std::error::Error;
use std::fs;

pub const DEFAULT_MAX_AGE: f64 = 28.0;
pub const DEFAULT_TOP_N: usize = 100;
pub const PROPOSAL_HEADER: [&str; 17] = [
    "player_id",
    "name",
    "club",
    "competition",
    "country",
    "position",
    "pos_cls",
    "age",
    "current_mv",
    "fm_ca",
    "fm_pa",
    "attr_source",
    "predicted_surge_probability",
    "interval_10",
    "interval_90",
    "key_strengths",
    "rationale",
];

/// One ranked transfer proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub player_id: u64,
    pub name: String,
    pub club: String,
    pub competition: String,
    pub country: String,
    pub position: String,
    pub pos_cls: String,
    pub age: f64,
    pub current_mv: f64,
    pub fm_ca: f64,
    pub fm_pa: f64,
    pub attr_source: String,
    pub predicted_surge_probability: f64,
    pub interval_10: f64,
    pub interval_90: f64,
    pub key_strengths: String,
    pub rationale: String,
}

impl Proposal {
    /// Required fields must be non-empty and numeric fields finite.
    pub fn is_complete(&self) -> bool {
        !self.name.is_empty()
            && !self.club.is_empty()
            && !self.pos_cls.is_empty()
            && !self.attr_source.is_empty()
            && self.age.is_finite()
            && self.current_mv.is_finite()
            && self.fm_ca.is_finite()
            && self.fm_pa.is_finite()
            && self.predicted_surge_probability.is_finite()
            && self.interval_10.is_finite()
            && self.interval_90.is_finite()
    }
}

/// Candidate metadata parsed from `surge/candidates_meta.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaRow {
    pub player_id: u64,
    pub name: String,
    pub club: String,
    pub competition: String,
    pub country: String,
    pub position: String,
    pub pos_cls: String,
    pub age: f64,
    pub current_mv: f64,
    pub fm_ca: f64,
    pub fm_pa: f64,
    pub attr_source: String,
    pub pa_gap: f64,
    pub ca_age_pct: f64,
    /// Attribute column -> value (missing columns absent).
    pub attributes: HashMap<String, f64>,
}

fn parse_float_opt(value: &str) -> f64 {
    match value.trim() {
        "" | "NA" | "nan" | "NaN" | "None" => f64::NAN,
        field => field.parse::<f64>().unwrap_or(f64::NAN),
    }
}

/// Parse the candidate metadata TSV into a player_id -> row map. The parser is
/// column-name driven so the attribute set can grow without schema changes.
pub fn parse_meta_tsv(path: &str) -> Result<HashMap<u64, MetaRow>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header = lines.next().ok_or("candidate meta TSV is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();
    let index = |name: &str| columns.iter().position(|column| *column == name);
    let required = ["player_id", "name", "pos_cls", "age", "current_mv", "ca", "pa", "attr_source"];
    for name in required {
        if index(name).is_none() {
            return Err(format!("candidate meta TSV missing required column {name}").into());
        }
    }
    let mut rows = HashMap::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(format!("meta row {} has {} columns; expected {}", offset + 2, fields.len(), columns.len()).into());
        }
        let get = |name: &str| -> &str { fields[index(name).expect("checked above")].trim() };
        let player_id = get("player_id").parse::<u64>()?;
        if rows.insert(
            player_id,
            MetaRow {
                player_id,
                name: get("name").to_string(),
                club: get("current_club_name").to_string(),
                competition: get("competition_name").to_string(),
                country: get("country_name").to_string(),
                position: get("position").to_string(),
                pos_cls: get("pos_cls").to_string(),
                age: parse_float_opt(get("age")),
                current_mv: parse_float_opt(get("current_mv")),
                fm_ca: parse_float_opt(get("ca")),
                fm_pa: parse_float_opt(get("pa")),
                attr_source: get("attr_source").to_string(),
                pa_gap: parse_float_opt(get("pa_gap")),
                ca_age_pct: parse_float_opt(get("ca_age_pct")),
                attributes: columns
                    .iter()
                    .enumerate()
                    .filter(|(_, column)| column.starts_with("attr_"))
                    .map(|(column_index, column)| {
                        (column.to_string(), parse_float_opt(fields[column_index]))
                    })
                    .filter(|(_, value)| value.is_finite())
                    .collect(),
            },
        )
        .is_some()
        {
            return Err(format!("duplicate player_id {player_id} in candidate meta TSV").into());
        }
    }
    if rows.is_empty() {
        return Err("candidate meta TSV contains no rows".into());
    }
    Ok(rows)
}

fn quality_attributes(row: &MetaRow) -> QualityAttributes {
    let value = |column: &str| row.attributes.get(column).copied();
    QualityAttributes {
        finishing: value("attr_Finishing"),
        dribbling: value("attr_Dribbling"),
        passing: value("attr_Passing"),
        vision: value("attr_Vision"),
        tackling: value("attr_Tackling"),
        marking: value("attr_Marking"),
        handling: value("attr_Handling"),
        reflexes: value("attr_Reflexes"),
    }
}

fn screen_text(reason: ScreenReason) -> &'static str {
    match reason {
        ScreenReason::QualityGate => "passed FM position quality gate",
        ScreenReason::DevelopmentSignal => "passed development-signal screen (young, high PA-CA gap, high age-relative CA)",
        ScreenReason::Both => "passed FM position quality gate and development-signal screen",
        ScreenReason::None => "failed FM screen",
    }
}

/// Convert FM26 negative PA codes (randomized potential bands) to the band
/// midpoint for reporting. The model itself treats negative PA as missing
/// (consistent with training); this conversion only makes the CSV readable.
pub fn fm_pa_display(pa: f64) -> Option<f64> {
    let midpoint = match pa as i64 {
        -95 => 185.0,
        -85 => 175.0,
        -75 => 165.0,
        -65 => 155.0,
        -55 => 145.0,
        -45 => 135.0,
        -35 => 125.0,
        -25 => 115.0,
        -15 => 105.0,
        -10 => 185.0,
        -9 => 165.0,
        -8 => 145.0,
        -7 => 125.0,
        -6 => 105.0,
        -5 => 85.0,
        -4 => 65.0,
        -3 => 45.0,
        -2 => 30.0,
        -1 => 20.0,
        _ => return None,
    };
    Some(midpoint)
}

fn display_field(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "NA" {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_proposal(row: &MetaRow, probability: f64, lower: f64, upper: f64, reason: ScreenReason) -> Proposal {
    let pos_cls = PositionClass::parse(&row.pos_cls).unwrap_or(PositionClass::Midfielder);
    let strengths: Vec<String> = key_strengths(pos_cls, &row.attributes.iter().map(|(k, v)| (k.clone(), *v)).collect::<Vec<_>>(), 4);
    let strengths_text = if strengths.is_empty() {
        "no standout FM attributes".to_string()
    } else {
        strengths.join("; ")
    };
    let mut rationale = format!("{}; key strengths: {}", screen_text(reason), strengths_text);
    if row.pa_gap.is_finite() && row.pa_gap >= 0.0 {
        rationale.push_str(&format!("; FM upside: PA-CA gap {:.0}", row.pa_gap));
    }
    if row.ca_age_pct.is_finite() {
        rationale.push_str(&format!("; age-relative CA percentile {:.2}", row.ca_age_pct));
    }
    let (fm_pa, pa_note) = if row.fm_pa.is_finite() && row.fm_pa < 0.0 {
        match fm_pa_display(row.fm_pa) {
            Some(midpoint) => (midpoint, format!("; FM PA is a randomized-band midpoint (raw code {:.0})", row.fm_pa)),
            None => (row.fm_pa, String::new()),
        }
    } else {
        (row.fm_pa, String::new())
    };
    if !pa_note.is_empty() {
        rationale.push_str(&pa_note);
    }
    Proposal {
        player_id: row.player_id,
        name: row.name.clone(),
        club: display_field(&row.club, "Free Agent"),
        competition: display_field(&row.competition, "Unknown"),
        country: display_field(&row.country, "Unknown"),
        position: row.position.clone(),
        pos_cls: row.pos_cls.clone(),
        age: row.age,
        current_mv: row.current_mv,
        fm_ca: row.fm_ca,
        fm_pa,
        attr_source: row.attr_source.clone(),
        predicted_surge_probability: probability,
        interval_10: lower,
        interval_90: upper,
        key_strengths: strengths_text,
        rationale,
    }
}

/// Screen + score all candidates and return the ranked proposal list (top_n).
///
/// Eligibility (STEERING): `current_mv` in [400k, 1M] (enforced by the candidate
/// build), FM CA present with `attr_source` in {db, derived}, age <= `max_age`,
/// and the FM quality gate or development-signal screen passed. Missing metadata
/// (no meta row / non-finite age) disqualifies the candidate; missing is never
/// silently imputed as zero or as eligible.
pub fn screen_and_score(
    model: &SurgeModelArtifact,
    candidates_matrix_path: &str,
    meta: &HashMap<u64, MetaRow>,
    max_age: f64,
    top_n: usize,
) -> Result<Vec<Proposal>, Box<dyn Error>> {
    if !(14.0..=50.0).contains(&max_age) {
        return Err("max age must be within [14, 50]".into());
    }
    if top_n == 0 {
        return Err("top_n must be positive".into());
    }
    let matrix = read_surge_feature_matrix(candidates_matrix_path)?;
    if matrix.feature_set_version != model.metadata.feature_set_version {
        return Err("candidate matrix feature-set version does not match model".into());
    }
    if matrix.feature_names != model.feature_names {
        return Err("candidate matrix feature names do not match model schema".into());
    }

    let mut proposals = Vec::new();
    let mut skipped = 0_usize;
    for row in &matrix.rows {
        let Some(candidate) = meta.get(&row.player.0) else {
            skipped += 1;
            continue;
        };
        if !candidate.age.is_finite() || candidate.age > max_age {
            skipped += 1;
            continue;
        }
        if candidate.attr_source != "db" && candidate.attr_source != "derived" {
            skipped += 1;
            continue;
        }
        let Some(pos_cls) = PositionClass::parse(&candidate.pos_cls) else {
            skipped += 1;
            continue;
        };
        let reason = surge_screen(
            pos_cls,
            &quality_attributes(candidate),
            Some(candidate.age),
            Some(candidate.pa_gap),
            Some(candidate.ca_age_pct),
            &candidate.attr_source,
        );
        if !reason.passed() {
            skipped += 1;
            continue;
        }
        let raw: Vec<f64> = row.features.iter().map(|value| f64::from(*value)).collect();
        let (probability, lower, upper) = model.predict_surge(&raw)?;
        proposals.push(build_proposal(candidate, probability, lower, upper, reason));
    }
    if proposals.is_empty() {
        return Err("no eligible candidates after screening".into());
    }
    // Deterministic ranking: probability desc, then player_id asc.
    proposals.sort_by(|left, right| {
        right
            .predicted_surge_probability
            .total_cmp(&left.predicted_surge_probability)
            .then_with(|| left.player_id.cmp(&right.player_id))
    });
    proposals.truncate(top_n);
    proposals.shrink_to_fit();
    println!(
        "screened candidates: total={} eligible={} skipped={} ranked_top={}",
        matrix.rows.len(),
        matrix.rows.len() - skipped,
        skipped,
        proposals.len()
    );
    Ok(proposals)
}

/// Minimal RFC-4180-style CSV writer with quoting for commas/quotes/newlines.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

pub fn write_proposals_csv(path: &str, proposals: &[Proposal]) -> Result<(), Box<dyn Error>> {
    let mut out = String::with_capacity(proposals.len() * 256);
    out.push_str(&PROPOSAL_HEADER.join(","));
    out.push('\n');
    for proposal in proposals {
        let fields = [
            proposal.player_id.to_string(),
            proposal.name.clone(),
            proposal.club.clone(),
            proposal.competition.clone(),
            proposal.country.clone(),
            proposal.position.clone(),
            proposal.pos_cls.clone(),
            format!("{:.3}", proposal.age),
            format!("{:.0}", proposal.current_mv),
            format!("{:.1}", proposal.fm_ca),
            format!("{:.1}", proposal.fm_pa),
            proposal.attr_source.clone(),
            format!("{:.6}", proposal.predicted_surge_probability),
            format!("{:.6}", proposal.interval_10),
            format!("{:.6}", proposal.interval_90),
            proposal.key_strengths.clone(),
            proposal.rationale.clone(),
        ];
        out.push_str(&fields.iter().map(|field| csv_escape(field)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

/// Parse one proposals CSV line into a `Proposal` (RFC-4180 minimal reader).
fn parse_proposals_csv(path: &str) -> Result<Vec<Proposal>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut records = Vec::new();
    let mut field = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut field));
                if !(row.len() == 1 && row[0].is_empty()) {
                    records.push(std::mem::take(&mut row));
                }
            }
            _ => field.push(ch),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        records.push(row);
    }
    if records.is_empty() {
        return Err("proposals CSV is empty".into());
    }
    let header: Vec<&str> = records[0].iter().map(String::as_str).collect();
    let index = |name: &str| header.iter().position(|column| *column == name);
    let required = ["player_id", "name", "current_mv", "fm_ca", "fm_pa", "attr_source", "predicted_surge_probability", "interval_10", "interval_90"];
    for name in required {
        if index(name).is_none() {
            return Err(format!("proposals CSV missing required column {name}").into());
        }
    }
    let mut proposals = Vec::new();
    for (offset, record) in records.iter().skip(1).enumerate() {
        if record.len() != header.len() {
            return Err(format!("proposals row {} has {} fields; expected {}", offset + 2, record.len(), header.len()).into());
        }
        let get = |name: &str| -> &str { &record[index(name).expect("checked above")] };
        proposals.push(Proposal {
            player_id: get("player_id").trim().parse::<u64>()?,
            name: get("name").to_string(),
            club: get("club").to_string(),
            competition: get("competition").to_string(),
            country: get("country").to_string(),
            position: get("position").to_string(),
            pos_cls: get("pos_cls").to_string(),
            age: parse_float_opt(get("age")),
            current_mv: parse_float_opt(get("current_mv")),
            fm_ca: parse_float_opt(get("fm_ca")),
            fm_pa: parse_float_opt(get("fm_pa")),
            attr_source: get("attr_source").to_string(),
            predicted_surge_probability: parse_float_opt(get("predicted_surge_probability")),
            interval_10: parse_float_opt(get("interval_10")),
            interval_90: parse_float_opt(get("interval_90")),
            key_strengths: get("key_strengths").to_string(),
            rationale: get("rationale").to_string(),
        });
    }
    Ok(proposals)
}

/// Deliverable validation: every row within [min_mv, max_mv], complete (no NaN /
/// empty required fields), probability within [0, 1]. Returns a list of issues.
pub fn validate_proposals(rows: &[Proposal], min_mv: f64, max_mv: f64) -> Vec<String> {
    let mut issues = Vec::new();
    if rows.is_empty() {
        issues.push("proposals list is empty".to_string());
    }
    for (offset, row) in rows.iter().enumerate() {
        let prefix = format!("row {offset}");
        if !row.current_mv.is_finite() || row.current_mv < min_mv || row.current_mv > max_mv {
            issues.push(format!("{prefix}: current_mv {} outside [{min_mv}, {max_mv}]", row.current_mv));
        }
        if !row.is_complete() {
            issues.push(format!("{prefix}: incomplete row (NaN/empty required field)"));
        }
        if !row.predicted_surge_probability.is_finite()
            || !(0.0..=1.0).contains(&row.predicted_surge_probability)
        {
            issues.push(format!("{prefix}: predicted_surge_probability {} outside [0, 1]", row.predicted_surge_probability));
        }
        if row.interval_10.is_finite() && row.interval_90.is_finite() && row.interval_10 > row.interval_90 {
            issues.push(format!("{prefix}: interval_10 > interval_90"));
        }
    }
    issues
}

/// Validate a proposals CSV file on disk (used by tests and the report step).
pub fn validate_proposals_file(path: &str, min_mv: f64, max_mv: f64) -> Result<Vec<String>, Box<dyn Error>> {
    let rows = parse_proposals_csv(path)?;
    Ok(validate_proposals(&rows, min_mv, max_mv))
}

/// Convenience wrapper used by the CLI: full pipeline from artifacts to CSV.
pub fn run_surge_proposals(
    model_path: &str,
    candidates_matrix_path: &str,
    meta_path: &str,
    output_path: &str,
    max_age: f64,
    top_n: usize,
) -> Result<Vec<Proposal>, Box<dyn Error>> {
    let model = read_surge_model(model_path)?;
    let meta = parse_meta_tsv(meta_path)?;
    let proposals = screen_and_score(&model, candidates_matrix_path, &meta, max_age, top_n)?;
    write_proposals_csv(output_path, &proposals)?;
    let issues = validate_proposals(&proposals, 400_000.0, 1_000_000.0);
    if !issues.is_empty() {
        return Err(format!("generated proposals failed validation: {}", issues.join("; ")).into());
    }
    Ok(proposals)
}

pub fn read_proposals_csv(path: &str) -> Result<Vec<Proposal>, Box<dyn Error>> {
    parse_proposals_csv(path)
}

// ---------------------------------------------------------------------------
// 12-month value-forecast proposals
// ---------------------------------------------------------------------------

pub const FORECAST_HEADER: [&str; 19] = [
    "player_id",
    "name",
    "club",
    "competition",
    "country",
    "position",
    "pos_cls",
    "age",
    "current_mv",
    "fm_ca",
    "fm_pa",
    "attr_source",
    "predicted_12m_mv",
    "gap_eur",
    "gap_pct",
    "interval_10",
    "interval_90",
    "key_strengths",
    "rationale",
];

/// One ranked 12-month value-forecast row.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastProposal {
    pub player_id: u64,
    pub name: String,
    pub club: String,
    pub competition: String,
    pub country: String,
    pub position: String,
    pub pos_cls: String,
    pub age: f64,
    pub current_mv: f64,
    pub fm_ca: f64,
    pub fm_pa: f64,
    pub attr_source: String,
    pub predicted_12m_mv: f64,
    pub gap_eur: f64,
    pub gap_pct: f64,
    pub interval_10: f64,
    pub interval_90: f64,
    pub key_strengths: String,
    pub rationale: String,
}

impl ForecastProposal {
    /// Required fields non-empty and all numeric fields finite.
    pub fn is_complete(&self) -> bool {
        !self.name.is_empty()
            && !self.club.is_empty()
            && !self.pos_cls.is_empty()
            && !self.attr_source.is_empty()
            && !self.key_strengths.is_empty()
            && !self.rationale.is_empty()
            && self.age.is_finite()
            && self.current_mv.is_finite()
            && self.fm_ca.is_finite()
            && self.fm_pa.is_finite()
            && self.predicted_12m_mv.is_finite()
            && self.gap_eur.is_finite()
            && self.gap_pct.is_finite()
            && self.interval_10.is_finite()
            && self.interval_90.is_finite()
    }
}

/// Effective FM potential ability for filtering/reporting: negative band codes
/// are converted to documented midpoints; positive values pass through.
fn effective_pa(pa: f64) -> f64 {
    if pa.is_finite() && pa < 0.0 {
        fm_pa_band_midpoint(pa).unwrap_or(f64::NAN)
    } else {
        pa
    }
}

/// Top-`limit` FM attributes by value across all `attr_*` columns, formatted
/// "Finishing 15; Dribbling 14; ...". Deterministic: ties break alphabetically
/// by attribute label.
fn forecast_key_strengths(attributes: &HashMap<String, f64>, limit: usize) -> String {
    let mut items: Vec<(String, f64)> = attributes
        .iter()
        .filter(|(column, value)| column.starts_with("attr_") && value.is_finite())
        .map(|(column, value)| (column.trim_start_matches("attr_").to_string(), *value))
        .collect();
    items.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    items.truncate(limit);
    items
        .iter()
        .map(|(label, value)| format!("{label} {}", value.round() as i64))
        .collect::<Vec<_>>()
        .join("; ")
}

fn age_band(age: f64) -> u32 {
    (age.floor() as u32) + 1
}

fn build_forecast_proposal(row: &MetaRow, prediction: f64, lower: f64, upper: f64) -> ForecastProposal {
    let pa_eff = effective_pa(row.fm_pa);
    let gap = if row.fm_ca.is_finite() && pa_eff.is_finite() { pa_eff - row.fm_ca } else { f64::NAN };
    let predicted_12m_mv = row.current_mv * prediction.exp();
    let gap_eur = predicted_12m_mv - row.current_mv;
    let gap_pct = gap_eur / row.current_mv;
    let multiplier = predicted_12m_mv / row.current_mv;
    let mut rationale = format!(
        "U{} {}, PA {:.0} (gap {:.0}), CA percentile {:.2} in {}; model sees {:.1}x 12m appreciation from EUR {:.0}",
        age_band(row.age),
        row.pos_cls,
        pa_eff,
        gap,
        row.ca_age_pct,
        display_field(&row.competition, "unknown"),
        multiplier,
        row.current_mv,
    );
    if !row.fm_pa.is_finite() || row.fm_pa < 0.0 {
        rationale.push_str(&format!(" (FM PA band midpoint from raw code {:.0})", row.fm_pa));
    }
    ForecastProposal {
        player_id: row.player_id,
        name: row.name.clone(),
        club: display_field(&row.club, "Free Agent"),
        competition: display_field(&row.competition, "Unknown"),
        country: display_field(&row.country, "Unknown"),
        position: row.position.clone(),
        pos_cls: row.pos_cls.clone(),
        age: row.age,
        current_mv: row.current_mv,
        fm_ca: row.fm_ca,
        fm_pa: pa_eff,
        attr_source: row.attr_source.clone(),
        predicted_12m_mv,
        gap_eur,
        gap_pct,
        interval_10: row.current_mv * lower.exp(),
        interval_90: row.current_mv * upper.exp(),
        key_strengths: forecast_key_strengths(&row.attributes, 4),
        rationale,
    }
}

/// Screen + score all forecast candidates and return the ranked top-`top_n`.
///
/// Eligibility (STEERING): age < 23 (floating years as of 2026-06-12), effective
/// FM PA >= 140 (negative band codes converted first), 0 < current_mv <= 1M, and
/// `attr_source` in {db, derived}. Missing metadata disqualifies a candidate;
/// missing is never silently imputed as eligible.
pub fn screen_and_score_forecast(
    model: &ForecastModelArtifact,
    candidates_matrix_path: &str,
    meta: &HashMap<u64, MetaRow>,
    top_n: usize,
    min_pa: f64,
    max_mv: f64,
) -> Result<Vec<ForecastProposal>, Box<dyn Error>> {
    if top_n == 0 {
        return Err("top_n must be positive".into());
    }
    if !min_pa.is_finite() || min_pa < 0.0 {
        return Err("min_pa must be a finite non-negative value".into());
    }
    if !max_mv.is_finite() || max_mv <= 0.0 {
        return Err("max_mv must be a finite positive value".into());
    }
    let matrix = read_forecast_feature_matrix(candidates_matrix_path)?;
    if matrix.feature_set_version != model.metadata.feature_set_version {
        return Err("candidate matrix feature-set version does not match model".into());
    }
    if matrix.feature_names != model.feature_names {
        return Err("candidate matrix feature names do not match model schema".into());
    }

    let mut proposals = Vec::new();
    let mut skipped = 0_usize;
    for row in &matrix.rows {
        let Some(candidate) = meta.get(&row.player.0) else {
            skipped += 1;
            continue;
        };
        if !candidate.age.is_finite() || candidate.age >= 23.0 {
            skipped += 1;
            continue;
        }
        if candidate.attr_source != "db" && candidate.attr_source != "derived" {
            skipped += 1;
            continue;
        }
        let pa_eff = effective_pa(candidate.fm_pa);
        if !pa_eff.is_finite() || pa_eff < min_pa {
            skipped += 1;
            continue;
        }
        if !candidate.current_mv.is_finite() || candidate.current_mv <= 0.0 || candidate.current_mv > max_mv {
            skipped += 1;
            continue;
        }
        let raw: Vec<f64> = row.features.iter().map(|value| f64::from(*value)).collect();
        let (prediction, lower, upper) = model.predict_log_ratio(&raw)?;
        if !prediction.is_finite() || !lower.is_finite() || !upper.is_finite() {
            skipped += 1;
            continue;
        }
        proposals.push(build_forecast_proposal(candidate, prediction, lower, upper));
    }
    if proposals.is_empty() {
        return Err("no eligible forecast candidates after screening".into());
    }
    // Deterministic ranking: gap_eur desc, then player_id asc.
    proposals.sort_by(|left, right| {
        right
            .gap_eur
            .total_cmp(&left.gap_eur)
            .then_with(|| left.player_id.cmp(&right.player_id))
    });
    proposals.truncate(top_n);
    proposals.shrink_to_fit();
    println!(
        "forecast screened candidates: total={} eligible={} skipped={} ranked_top={}",
        matrix.rows.len(),
        matrix.rows.len() - skipped,
        skipped,
        proposals.len()
    );
    Ok(proposals)
}

pub fn write_forecast_proposals_csv(path: &str, proposals: &[ForecastProposal]) -> Result<(), Box<dyn Error>> {
    let mut out = String::with_capacity(proposals.len() * 256);
    out.push_str(&FORECAST_HEADER.join(","));
    out.push('\n');
    for proposal in proposals {
        let fields = [
            proposal.player_id.to_string(),
            proposal.name.clone(),
            proposal.club.clone(),
            proposal.competition.clone(),
            proposal.country.clone(),
            proposal.position.clone(),
            proposal.pos_cls.clone(),
            format!("{:.3}", proposal.age),
            format!("{:.0}", proposal.current_mv),
            format!("{:.1}", proposal.fm_ca),
            format!("{:.1}", proposal.fm_pa),
            proposal.attr_source.clone(),
            format!("{:.0}", proposal.predicted_12m_mv),
            format!("{:.0}", proposal.gap_eur),
            format!("{:.4}", proposal.gap_pct),
            format!("{:.0}", proposal.interval_10),
            format!("{:.0}", proposal.interval_90),
            proposal.key_strengths.clone(),
            proposal.rationale.clone(),
        ];
        out.push_str(&fields.iter().map(|field| csv_escape(field)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

/// Deliverable validation for the forecast CSV: every row within the target
/// population (age < 23, effective PA >= 140, 0 < current_mv <= 1M), complete,
/// positive predicted value, and sane intervals. Returns a list of issues.
pub fn validate_forecast_proposals(
    rows: &[ForecastProposal],
    min_pa: f64,
    max_mv: f64,
    required_rows: Option<usize>,
) -> Vec<String> {
    let mut issues = Vec::new();
    if let Some(expected) = required_rows {
        if rows.len() != expected {
            issues.push(format!("forecast rows must be exactly {expected}, got {}", rows.len()));
        }
    } else if rows.is_empty() {
        issues.push("forecast rows must be non-empty".to_string());
    }
    for (offset, row) in rows.iter().enumerate() {
        let prefix = format!("row {offset}");
        if !row.age.is_finite() || row.age >= 23.0 {
            issues.push(format!("{prefix}: age {} not < 23", row.age));
        }
        if !row.fm_pa.is_finite() || row.fm_pa < min_pa {
            issues.push(format!("{prefix}: fm_pa {} < {min_pa}", row.fm_pa));
        }
        if !row.current_mv.is_finite() || row.current_mv <= 0.0 || row.current_mv > max_mv {
            issues.push(format!("{prefix}: current_mv {} out of (0, {max_mv}]", row.current_mv));
        }
        if row.attr_source != "db" && row.attr_source != "derived" {
            issues.push(format!("{prefix}: attr_source {}", row.attr_source));
        }
        if !row.is_complete() {
            issues.push(format!("{prefix}: incomplete row (NaN/empty required field)"));
        }
        if row.predicted_12m_mv <= 0.0 {
            issues.push(format!("{prefix}: predicted_12m_mv {} <= 0", row.predicted_12m_mv));
        }
        if row.gap_pct < -1.0 {
            issues.push(format!("{prefix}: gap_pct {} < -100%", row.gap_pct));
        }
        if row.interval_10.is_finite() && row.interval_90.is_finite() && row.interval_10 > row.interval_90 {
            issues.push(format!("{prefix}: interval_10 > interval_90"));
        }
    }
    issues
}

/// Convenience wrapper used by the CLI: full pipeline from artifacts to CSV.
pub fn run_forecast_proposals(
    model_path: &str,
    candidates_matrix_path: &str,
    meta_path: &str,
    output_path: &str,
    top_n: usize,
    min_pa: f64,
    max_mv: f64,
    required_rows: Option<usize>,
) -> Result<Vec<ForecastProposal>, Box<dyn Error>> {
    let model = read_forecast_model(model_path)?;
    let meta = parse_meta_tsv(meta_path)?;
    let proposals = screen_and_score_forecast(&model, candidates_matrix_path, &meta, top_n, min_pa, max_mv)?;
    write_forecast_proposals_csv(output_path, &proposals)?;
    let issues = validate_forecast_proposals(&proposals, min_pa, max_mv, required_rows);
    if !issues.is_empty() {
        return Err(format!("generated forecast proposals failed validation: {}", issues.join("; ")).into());
    }
    Ok(proposals)
}


// ---------------------------------------------------------------------------
// Tail-aware forecast scoring (spike-refinement run)
// ---------------------------------------------------------------------------

pub const TAIL_HEADER: [&str; 19] = [
    "player_id",
    "name",
    "club",
    "competition",
    "country",
    "position",
    "pos_cls",
    "age",
    "current_mv",
    "fm_ca",
    "fm_pa",
    "attr_source",
    "tail_score",
    "predicted_12m_mv",
    "gap_eur",
    "gap_pct",
    "key_strengths",
    "rationale",
    "objective",
];

/// One scored row from the tail-aware forecast model.
#[derive(Debug, Clone, PartialEq)]
pub struct TailScoredRow {
    pub player_id: u64,
    pub name: String,
    pub club: String,
    pub competition: String,
    pub country: String,
    pub position: String,
    pub pos_cls: String,
    pub age: f64,
    pub current_mv: f64,
    pub fm_ca: f64,
    pub fm_pa: f64,
    pub attr_source: String,
    pub tail_score: f64,
    pub predicted_12m_mv: f64,
    pub gap_eur: f64,
    pub gap_pct: f64,
    pub key_strengths: String,
    pub rationale: String,
    pub objective: String,
}

/// Score every candidate in the matrix with the tail model (no population
/// filters; ranking by tail score descending, ties by player_id). Returns the
/// scored rows so callers can write any CSV layout.
pub fn score_all_tail(
    model: &TailForecastModelArtifact,
    candidates_matrix_path: &str,
    meta: &HashMap<u64, MetaRow>,
) -> Result<Vec<TailScoredRow>, Box<dyn Error>> {
    let matrix = read_forecast_feature_matrix(candidates_matrix_path)?;
    if matrix.feature_set_version != model.metadata.feature_set_version {
        return Err("candidate matrix feature-set version does not match tail model".into());
    }
    if matrix.feature_names != model.feature_names {
        return Err("candidate matrix feature names do not match tail model schema".into());
    }
    let objective = match model.objective {
        footy_artifacts::TailObjective::Quantile { tau } => format!("quantile-tau{tau}"),
        footy_artifacts::TailObjective::Expectile { omega } => format!("expectile-omega{omega}"),
    };
    let mut rows = Vec::new();
    let mut skipped = 0_usize;
    for row in &matrix.rows {
        let Some(candidate) = meta.get(&row.player.0) else {
            skipped += 1;
            continue;
        };
        let raw: Vec<f64> = row.features.iter().map(|value| f64::from(*value)).collect();
        let tail_score = model.predict_tail(&raw)?;
        let pa_eff = effective_pa(candidate.fm_pa);
        let predicted_12m_mv = candidate.current_mv * tail_score.exp();
        let gap_eur = predicted_12m_mv - candidate.current_mv;
        let gap_pct = gap_eur / candidate.current_mv;
        let multiplier = predicted_12m_mv / candidate.current_mv;
        let mut rationale = format!(
            "tail score {:.4} = predicted {}-quantile 12m log-ratio; {:.1}x implied 12m appreciation from EUR {:.0}",
            tail_score,
            objective_name_short(&objective),
            multiplier,
            candidate.current_mv,
        );
        if candidate.pa_gap.is_finite() && candidate.pa_gap >= 0.0 {
            rationale.push_str(&format!("; FM upside: PA-CA gap {:.0}", candidate.pa_gap));
        }
        rows.push(TailScoredRow {
            player_id: candidate.player_id,
            name: display_field(&candidate.name, "Unknown"),
            club: display_field(&candidate.club, "Free Agent"),
            competition: display_field(&candidate.competition, "Unknown"),
            country: display_field(&candidate.country, "Unknown"),
            position: candidate.position.clone(),
            pos_cls: candidate.pos_cls.clone(),
            age: candidate.age,
            current_mv: candidate.current_mv,
            fm_ca: candidate.fm_ca,
            fm_pa: pa_eff,
            attr_source: candidate.attr_source.clone(),
            tail_score,
            predicted_12m_mv,
            gap_eur,
            gap_pct,
            key_strengths: forecast_key_strengths(&candidate.attributes, 4),
            rationale,
            objective: objective.clone(),
        });
    }
    rows.sort_by(|left, right| {
        right
            .tail_score
            .total_cmp(&left.tail_score)
            .then_with(|| left.player_id.cmp(&right.player_id))
    });
    println!(
        "tail scored candidates: total={} scored={} skipped_no_meta={}",
        matrix.rows.len(),
        rows.len(),
        skipped
    );
    Ok(rows)
}

fn objective_name_short(objective: &str) -> &str {
    if objective.starts_with("quantile") {
        objective.trim_start_matches("quantile-tau")
    } else {
        objective.trim_start_matches("expectile-omega")
    }
}

pub fn write_tail_csv(path: &str, rows: &[TailScoredRow]) -> Result<(), Box<dyn Error>> {
    let mut out = String::with_capacity(rows.len() * 256);
    out.push_str(&TAIL_HEADER.join(","));
    out.push('\n');
    for row in rows {
        let fields = [
            row.player_id.to_string(),
            row.name.clone(),
            row.club.clone(),
            row.competition.clone(),
            row.country.clone(),
            row.position.clone(),
            row.pos_cls.clone(),
            format!("{:.3}", row.age),
            format!("{:.0}", row.current_mv),
            format!("{:.1}", row.fm_ca),
            format!("{:.1}", row.fm_pa),
            row.attr_source.clone(),
            format!("{:.6}", row.tail_score),
            format!("{:.0}", row.predicted_12m_mv),
            format!("{:.0}", row.gap_eur),
            format!("{:.4}", row.gap_pct),
            row.key_strengths.clone(),
            row.rationale.clone(),
            row.objective.clone(),
        ];
        out.push_str(&fields.iter().map(|field| csv_escape(field)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

/// Convenience wrapper: score all candidates with the tail model and write CSV.
pub fn run_tail_scoring(
    model_path: &str,
    candidates_matrix_path: &str,
    meta_path: &str,
    output_path: &str,
) -> Result<Vec<TailScoredRow>, Box<dyn Error>> {
    let model = read_tail_forecast_model(model_path)?;
    let meta = parse_meta_tsv(meta_path)?;
    let rows = score_all_tail(&model, candidates_matrix_path, &meta)?;
    write_tail_csv(output_path, &rows)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Unfiltered surge scoring (score every candidate, no quality gate)
// ---------------------------------------------------------------------------

pub const SURGE_ALL_HEADER: [&str; 15] = [
    "player_id",
    "name",
    "club",
    "competition",
    "country",
    "position",
    "pos_cls",
    "age",
    "current_mv",
    "fm_ca",
    "fm_pa",
    "attr_source",
    "surge_probability",
    "interval_10",
    "interval_90",
];

/// Score every candidate row in the matrix with the surge model. No age cap and
/// no FM quality gate: the caller decides the population. Ranking by surge
/// probability descending, ties by player_id.
pub fn score_all_surge(
    model: &SurgeModelArtifact,
    candidates_matrix_path: &str,
    meta: &HashMap<u64, MetaRow>,
) -> Result<Vec<Proposal>, Box<dyn Error>> {
    let matrix = read_surge_feature_matrix(candidates_matrix_path)?;
    if matrix.feature_set_version != model.metadata.feature_set_version {
        return Err("candidate matrix feature-set version does not match surge model".into());
    }
    if matrix.feature_names != model.feature_names {
        return Err("candidate matrix feature names do not match surge model schema".into());
    }
    let mut proposals = Vec::new();
    let mut skipped = 0_usize;
    for row in &matrix.rows {
        let Some(candidate) = meta.get(&row.player.0) else {
            skipped += 1;
            continue;
        };
        let raw: Vec<f64> = row.features.iter().map(|value| f64::from(*value)).collect();
        let (probability, lower, upper) = model.predict_surge(&raw)?;
        let reason = ScreenReason::None;
        proposals.push(build_proposal(candidate, probability, lower, upper, reason));
    }
    proposals.sort_by(|left, right| {
        right
            .predicted_surge_probability
            .total_cmp(&left.predicted_surge_probability)
            .then_with(|| left.player_id.cmp(&right.player_id))
    });
    println!(
        "surge scored all candidates: total={} scored={} skipped_no_meta={}",
        matrix.rows.len(),
        proposals.len(),
        skipped
    );
    Ok(proposals)
}

/// Write an unfiltered surge scoring CSV (all rows, surge columns).
pub fn write_surge_all_csv(path: &str, proposals: &[Proposal]) -> Result<(), Box<dyn Error>> {
    let mut out = String::with_capacity(proposals.len() * 256);
    out.push_str(&SURGE_ALL_HEADER.join(","));
    out.push('\n');
    for proposal in proposals {
        let fields = [
            proposal.player_id.to_string(),
            proposal.name.clone(),
            proposal.club.clone(),
            proposal.competition.clone(),
            proposal.country.clone(),
            proposal.position.clone(),
            proposal.pos_cls.clone(),
            format!("{:.3}", proposal.age),
            format!("{:.0}", proposal.current_mv),
            format!("{:.1}", proposal.fm_ca),
            format!("{:.1}", proposal.fm_pa),
            proposal.attr_source.clone(),
            format!("{:.6}", proposal.predicted_surge_probability),
            format!("{:.6}", proposal.interval_10),
            format!("{:.6}", proposal.interval_90),
        ];
        out.push_str(&fields.iter().map(|field| csv_escape(field)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

/// Convenience wrapper: score all candidates with the surge model and write CSV.
pub fn run_surge_all_scoring(
    model_path: &str,
    candidates_matrix_path: &str,
    meta_path: &str,
    output_path: &str,
) -> Result<Vec<Proposal>, Box<dyn Error>> {
    let model = read_surge_model(model_path)?;
    let meta = parse_meta_tsv(meta_path)?;
    let proposals = score_all_surge(&model, candidates_matrix_path, &meta)?;
    write_surge_all_csv(output_path, &proposals)?;
    Ok(proposals)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_proposal(player_id: u64, mv: f64, probability: f64) -> Proposal {
        Proposal {
            player_id,
            name: format!("Player {player_id}"),
            club: "Test Club".to_string(),
            competition: "test-league".to_string(),
            country: "Testland".to_string(),
            position: "Midfield".to_string(),
            pos_cls: "MF".to_string(),
            age: 21.5,
            current_mv: mv,
            fm_ca: 120.0,
            fm_pa: 150.0,
            attr_source: "db".to_string(),
            predicted_surge_probability: probability,
            interval_10: (probability - 0.3).max(0.0),
            interval_90: (probability + 0.3).min(1.0),
            key_strengths: "Passing 14; Vision 13".to_string(),
            rationale: "test".to_string(),
        }
    }

    #[test]
    fn validation_flags_out_of_range_market_values() {
        let rows = vec![
            sample_proposal(1, 600_000.0, 0.5),
            sample_proposal(2, 1_200_000.0, 0.5),
            sample_proposal(3, 300_000.0, 0.5),
        ];
        let issues = validate_proposals(&rows, 400_000.0, 1_000_000.0);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|issue| issue.contains("1200000")));
    }

    #[test]
    fn validation_flags_nan_outputs() {
        let mut row = sample_proposal(1, 600_000.0, 0.5);
        row.fm_ca = f64::NAN;
        let issues = validate_proposals(&[row], 400_000.0, 1_000_000.0);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|issue| issue.contains("incomplete")));
    }

    #[test]
    fn proposals_csv_round_trips_with_quoting() {
        let dir = std::env::temp_dir().join(format!("footy-score-csv-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proposals.csv");
        let rows = vec![sample_proposal(7, 550_000.0, 0.42), sample_proposal(3, 750_000.0, 0.61)];
        write_proposals_csv(path.to_str().unwrap(), &rows).unwrap();
        let loaded = parse_proposals_csv(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded.len(), rows.len());
        for (left, right) in loaded.iter().zip(&rows) {
            assert_eq!(left.player_id, right.player_id);
            assert_eq!(left.name, right.name);
            assert_eq!(left.club, right.club);
            assert_eq!(left.competition, right.competition);
            assert_eq!(left.country, right.country);
            assert_eq!(left.position, right.position);
            assert_eq!(left.pos_cls, right.pos_cls);
            assert_eq!(left.attr_source, right.attr_source);
            assert_eq!(left.key_strengths, right.key_strengths);
            assert_eq!(left.rationale, right.rationale);
            for (field, expected) in [
                (left.age, right.age),
                (left.current_mv, right.current_mv),
                (left.fm_ca, right.fm_ca),
                (left.fm_pa, right.fm_pa),
                (left.predicted_surge_probability, right.predicted_surge_probability),
                (left.interval_10, right.interval_10),
                (left.interval_90, right.interval_90),
            ] {
                assert!((field - expected).abs() < 1e-6, "{field} != {expected}");
            }
        }
        assert!(validate_proposals_file(path.to_str().unwrap(), 400_000.0, 1_000_000.0).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    fn synthetic_matrix_rows() -> Vec<footy_artifacts::SurgeFeatureRow> {
        let mut rows = Vec::new();
        for index in 0..12_u64 {
            let mut features = vec![f32::NAN; footy_recruitment::surge::SURGE_FEATURE_NAMES.len()];
            features[0] = 13.5 + (index % 3) as f32 * 0.1; // log_mv
            features[24] = 19.0 + (index % 4) as f32; // age
            features[35] = 90.0 + index as f32; // fm_ca
            features[36] = 120.0 + index as f32; // fm_pa
            rows.push(footy_artifacts::SurgeFeatureRow {
                player: footy_core::PlayerId(index + 1),
                as_of: footy_core::Date(20_000),
                mv_t: 600_000.0,
                target: None,
                realized_return: None,
                weight: 1.0,
                features,
            });
        }
        rows
    }

    fn synthetic_meta() -> HashMap<u64, MetaRow> {
        let mut meta = HashMap::new();
        for index in 1..=12_u64 {
            let mut attributes = HashMap::new();
            attributes.insert("attr_Passing".to_string(), 13.0 + index as f64 * 0.5);
            attributes.insert("attr_Vision".to_string(), 12.0 + index as f64 * 0.5);
            attributes.insert("attr_Finishing".to_string(), 14.0);
            attributes.insert("attr_Dribbling".to_string(), 13.0);
            attributes.insert("attr_Tackling".to_string(), 15.0);
            attributes.insert("attr_Marking".to_string(), 14.0);
            attributes.insert("attr_Handling".to_string(), 15.0);
            attributes.insert("attr_Reflexes".to_string(), 16.0);
            meta.insert(
                index,
                MetaRow {
                    player_id: index,
                    name: format!("Player {index}"),
                    club: "Test Club".to_string(),
                    competition: "test-league".to_string(),
                    country: "Testland".to_string(),
                    position: "Midfield".to_string(),
                    pos_cls: "MF".to_string(),
                    age: 19.0 + index as f64 * 0.4,
                    current_mv: 500_000.0 + (index * 20_000) as f64,
                    fm_ca: 90.0 + index as f64,
                    fm_pa: 120.0 + index as f64,
                    attr_source: "db".to_string(),
                    pa_gap: 20.0,
                    ca_age_pct: 0.6,
                    attributes,
                },
            );
        }
        meta
    }

    fn synthetic_model() -> SurgeModelArtifact {
        // Hand-built artifact: intercept 0, zero coefficients except log_mv.
        let names: Vec<String> = footy_recruitment::surge::SURGE_FEATURE_NAMES.iter().map(|name| name.to_string()).collect();
        let width = names.len();
        let mut coefficients = vec![0.0; width * 2 + 1];
        coefficients[0] = -1.0; // intercept
        coefficients[1] = 2.0; // log_mv standardized slot
        SurgeModelArtifact {
            metadata: footy_models::ModelMetadata {
                trained_through: footy_core::Date(20_000),
                model_version: footy_core::ModelVersion(1),
                dataset_version: footy_core::DatasetVersion(1),
                feature_set_version: footy_core::FeatureSetVersion(1),
                seed: 42,
            },
            source_manifest_checksum: 1,
            calibration_version: footy_core::CalibrationVersion(1),
            ridge: 1.0,
            feature_names: names,
            transform: footy_models::FeatureTransform {
                means: vec![0.0; width],
                stds: vec![1.0; width],
                missing_rates: vec![0.0; width],
            },
            coefficients,
            walkforward: footy_artifacts::SurgeWalkForwardMetrics {
                periods: Vec::new(),
                pooled_auc: 0.75,
                auc_without_fm: 0.70,
                ece: 0.02,
                deciles: Vec::new(),
                conformal_radius: 0.3,
                calibration_coverage: 0.8,
                calibration_size: 1,
            },
        }
    }

    #[test]
    fn scoring_is_deterministic_and_ranks_by_probability() {
        let dir = std::env::temp_dir().join(format!("footy-score-det-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("candidates.bin");
        let artifact = footy_artifacts::SurgeFeatureMatrixArtifact {
            dataset_version: footy_core::DatasetVersion(1),
            source_manifest_checksum: 1,
            feature_set_version: footy_core::FeatureSetVersion(1),
            feature_names: footy_recruitment::surge::SURGE_FEATURE_NAMES.iter().map(|name| name.to_string()).collect(),
            rows: synthetic_matrix_rows(),
        };
        footy_artifacts::write_surge_feature_matrix(&matrix_path, &artifact).unwrap();
        let meta = synthetic_meta();
        let model = synthetic_model();
        let first = screen_and_score(&model, matrix_path.to_str().unwrap(), &meta, 28.0, 100).unwrap();
        let second = screen_and_score(&model, matrix_path.to_str().unwrap(), &meta, 28.0, 100).unwrap();
        assert_eq!(first, second, "same inputs must produce identical proposals");
        // Higher log_mv -> higher probability -> ranked first.
        for pair in first.windows(2) {
            assert!(pair[0].predicted_surge_probability >= pair[1].predicted_surge_probability);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// Deliverable gate: when the real proposals file exists (workspace root),
    /// it must have >= 100 rows, all market values in [400k, 1M], and no NaN.
    #[test]
    fn real_proposals_deliverable_is_valid() {
        use std::path::Path;
        let candidates = ["data/proposals_top100.csv", "../../data/proposals_top100.csv"];
        let Some(path) = candidates.iter().find(|path| Path::new(path).exists()) else {
            eprintln!("skipping: data/proposals_top100.csv not present");
            return;
        };
        let rows = parse_proposals_csv(path).unwrap();
        assert!(rows.len() >= 100, "proposals must have >= 100 rows, got {}", rows.len());
        let issues = validate_proposals(&rows, 400_000.0, 1_000_000.0);
        assert!(issues.is_empty(), "proposals validation failed: {:?}", issues);
    }

    fn forecast_synthetic_meta() -> HashMap<u64, MetaRow> {
        let mut meta = HashMap::new();
        for index in 1..=200_u64 {
            let mut attributes = HashMap::new();
            for (offset, name) in [
                "attr_Finishing", "attr_Dribbling", "attr_Passing", "attr_Vision",
                "attr_Tackling", "attr_Marking", "attr_Handling", "attr_Reflexes",
                "attr_FirstTouch", "attr_Acceleration", "attr_Pace", "attr_Stamina",
                "attr_WorkRate", "attr_Anticipation", "attr_Positioning", "attr_Composure",
                "attr_Technique", "attr_OffTheBall",
            ]
            .iter()
            .enumerate()
            {
                attributes.insert(name.to_string(), 12.0 + (index as f64 + offset as f64) % 6.0);
            }
            meta.insert(
                index,
                MetaRow {
                    player_id: index,
                    name: format!("Player {index}"),
                    club: "Test Club".to_string(),
                    competition: "test-league".to_string(),
                    country: "Testland".to_string(),
                    position: "Midfield".to_string(),
                    pos_cls: "MF".to_string(),
                    age: if index % 10 == 0 { 24.0 } else { 20.0 + (index % 3) as f64 },
                    current_mv: match index % 10 {
                        1 => 50_000.0,
                        2 => 1_100_000.0,
                        _ => 300_000.0 + (index * 5_000) as f64,
                    },
                    fm_ca: 100.0 + index as f64,
                    fm_pa: if index % 9 == 0 { -8.0 } else { 150.0 }, // -8 -> 145 effective
                    attr_source: if index % 11 == 0 { "none".to_string() } else { "db".to_string() },
                    pa_gap: 20.0,
                    ca_age_pct: 0.6,
                    attributes,
                },
            );
        }
        meta
    }

    fn forecast_synthetic_matrix_rows() -> Vec<footy_artifacts::ForecastFeatureRow> {
        let mut rows = Vec::new();
        for index in 1..=200_u64 {
            let mut features = vec![f32::NAN; footy_recruitment::surge::FORECAST_FEATURE_NAMES.len()];
            features[0] = (300_000.0_f64 + (index * 5_000) as f64).ln() as f32; // log_mv
            features[24] = 20.0; // age
            features[35] = 100.0 + index as f32; // fm_ca
            features[36] = 150.0; // fm_pa
            rows.push(footy_artifacts::ForecastFeatureRow {
                player: footy_core::PlayerId(index),
                as_of: footy_core::Date(20_500),
                mv_t: 300_000.0 + (index * 5_000) as f64,
                target: None,
                realized_return: None,
                weight: 1.0,
                features,
            });
        }
        rows
    }

    fn forecast_synthetic_model() -> ForecastModelArtifact {
        let names: Vec<String> = footy_recruitment::surge::FORECAST_FEATURE_NAMES
            .iter()
            .map(|name| name.to_string())
            .collect();
        let width = names.len();
        let mut coefficients = vec![0.0; width * 2 + 1];
        coefficients[0] = 0.2; // intercept
        coefficients[1] = 0.05; // log_mv standardized slot
        ForecastModelArtifact {
            metadata: footy_models::ModelMetadata {
                trained_through: footy_core::Date(20_000),
                model_version: footy_core::ModelVersion(1),
                dataset_version: footy_core::DatasetVersion(1),
                feature_set_version: footy_core::FeatureSetVersion(1),
                seed: 42,
            },
            source_manifest_checksum: 1,
            calibration_version: footy_core::CalibrationVersion(1),
            ridge: 1.0,
            feature_names: names,
            transform: footy_models::FeatureTransform {
                means: vec![0.0; width],
                stds: vec![1.0; width],
                missing_rates: vec![0.0; width],
            },
            coefficients,
            walkforward: footy_artifacts::ForecastWalkForwardMetrics {
                periods: Vec::new(),
                pooled_rmse: 0.5,
                pooled_mae: 0.4,
                pooled_spearman_ic: Some(0.5),
                deciles: Vec::new(),
                conformal_radius: 0.3,
                calibration_coverage: 0.8,
                calibration_size: 1,
            },
        }
    }

    #[test]
    fn forecast_scoring_is_deterministic_and_filters_population() {
        let dir = std::env::temp_dir().join(format!("footy-score-forecast-det-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("candidates.bin");
        let artifact = footy_artifacts::ForecastFeatureMatrixArtifact {
            dataset_version: footy_core::DatasetVersion(1),
            source_manifest_checksum: 1,
            feature_set_version: footy_core::FeatureSetVersion(1),
            feature_names: footy_recruitment::surge::FORECAST_FEATURE_NAMES
                .iter()
                .map(|name| name.to_string())
                .collect(),
            rows: forecast_synthetic_matrix_rows(),
        };
        footy_artifacts::write_forecast_feature_matrix(&matrix_path, &artifact).unwrap();
        let meta = forecast_synthetic_meta();
        let model = forecast_synthetic_model();
        let first = screen_and_score_forecast(&model, matrix_path.to_str().unwrap(), &meta, 100, 140.0, 1_000_000.0).unwrap();
        let second = screen_and_score_forecast(&model, matrix_path.to_str().unwrap(), &meta, 100, 140.0, 1_000_000.0).unwrap();
        assert_eq!(first, second, "same inputs must produce identical proposals");
        assert_eq!(first.len(), 100);
        for proposal in &first {
            assert!(proposal.age < 23.0, "over-age must be excluded");
            assert!(proposal.fm_pa >= 140.0, "low-PA must be excluded");
            assert!(proposal.current_mv > 0.0 && proposal.current_mv <= 1_000_000.0, "MV out of range must be excluded");
            assert!(proposal.attr_source == "db" || proposal.attr_source == "derived");
            assert!(!proposal.key_strengths.is_empty());
            assert!(!proposal.rationale.is_empty());
        }
        // Deterministic ranking by gap_eur desc
        for pair in first.windows(2) {
            assert!(pair[0].gap_eur >= pair[1].gap_eur);
        }
        let issues = validate_forecast_proposals(&first, 140.0, 1_000_000.0, Some(100));
        assert!(issues.is_empty(), "validation failed: {:?}", issues);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Deliverable gate: when the real forecast file exists (workspace root),
    /// it must have exactly 100 rows, all within the target population, and no
    /// NaN.
    #[test]
    fn real_forecast_deliverable_is_valid() {
        use std::path::Path;
        let candidates = ["data/forecast_u23_top100.csv", "../../data/forecast_u23_top100.csv"];
        let Some(path) = candidates.iter().find(|path| Path::new(path).exists()) else {
            eprintln!("skipping: data/forecast_u23_top100.csv not present");
            return;
        };
        let text = fs::read_to_string(path).unwrap();
        let mut records = Vec::new();
        let mut field = String::new();
        let mut row: Vec<String> = Vec::new();
        let mut chars = text.chars().peekable();
        let mut in_quotes = false;
        while let Some(ch) = chars.next() {
            match ch {
                '"' if in_quotes => {
                    if chars.peek() == Some(&'"') {
                        field.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                }
                '"' => in_quotes = true,
                ',' if !in_quotes => row.push(std::mem::take(&mut field)),
                '\n' if !in_quotes => {
                    row.push(std::mem::take(&mut field));
                    if !(row.len() == 1 && row[0].is_empty()) {
                        records.push(std::mem::take(&mut row));
                    }
                }
                _ => field.push(ch),
            }
        }
        if !field.is_empty() || !row.is_empty() {
            row.push(field);
            records.push(row);
        }
        assert!(records.len() >= 2, "forecast CSV must have a header and rows");
        let header: Vec<&str> = records[0].iter().map(String::as_str).collect();
        let index = |name: &str| header.iter().position(|column| *column == name).expect("column");
        let mut proposals = Vec::new();
        for record in records.iter().skip(1) {
            proposals.push(ForecastProposal {
                player_id: record[index("player_id")].trim().parse::<u64>().unwrap(),
                name: record[index("name")].to_string(),
                club: record[index("club")].to_string(),
                competition: record[index("competition")].to_string(),
                country: record[index("country")].to_string(),
                position: record[index("position")].to_string(),
                pos_cls: record[index("pos_cls")].to_string(),
                age: parse_float_opt(record[index("age")].trim()),
                current_mv: parse_float_opt(record[index("current_mv")].trim()),
                fm_ca: parse_float_opt(record[index("fm_ca")].trim()),
                fm_pa: parse_float_opt(record[index("fm_pa")].trim()),
                attr_source: record[index("attr_source")].to_string(),
                predicted_12m_mv: parse_float_opt(record[index("predicted_12m_mv")].trim()),
                gap_eur: parse_float_opt(record[index("gap_eur")].trim()),
                gap_pct: parse_float_opt(record[index("gap_pct")].trim()),
                interval_10: parse_float_opt(record[index("interval_10")].trim()),
                interval_90: parse_float_opt(record[index("interval_90")].trim()),
                key_strengths: record[index("key_strengths")].to_string(),
                rationale: record[index("rationale")].to_string(),
            });
        }
        assert_eq!(proposals.len(), 100, "forecast deliverable must have exactly 100 rows");
        let issues = validate_forecast_proposals(&proposals, 140.0, 1_000_000.0, Some(100));
        assert!(issues.is_empty(), "forecast validation failed: {:?}", issues);
    }

    #[test]
    fn screen_excludes_overage_and_failed_quality_candidates() {
        let mut meta = synthetic_meta();
        let mut overage = meta.remove(&1).unwrap();
        overage.age = 31.0;
        meta.insert(1, overage);
        let mut failed = meta.remove(&2).unwrap();
        failed.attributes.insert("attr_Passing".to_string(), 8.0);
        failed.attributes.insert("attr_Vision".to_string(), 7.0);
        failed.pa_gap = 0.0; // kills the development-signal fallback
        failed.ca_age_pct = 0.1;
        meta.insert(2, failed);

        let dir = std::env::temp_dir().join(format!("footy-score-screen-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let matrix_path = dir.join("candidates.bin");
        let artifact = footy_artifacts::SurgeFeatureMatrixArtifact {
            dataset_version: footy_core::DatasetVersion(1),
            source_manifest_checksum: 1,
            feature_set_version: footy_core::FeatureSetVersion(1),
            feature_names: footy_recruitment::surge::SURGE_FEATURE_NAMES.iter().map(|name| name.to_string()).collect(),
            rows: synthetic_matrix_rows(),
        };
        footy_artifacts::write_surge_feature_matrix(&matrix_path, &artifact).unwrap();
        let model = synthetic_model();
        let proposals = screen_and_score(&model, matrix_path.to_str().unwrap(), &meta, 28.0, 100).unwrap();
        let ids: Vec<u64> = proposals.iter().map(|proposal| proposal.player_id).collect();
        assert!(!ids.contains(&1), "age 31 must be excluded at max_age 28");
        assert!(!ids.contains(&2), "failed quality gate without dev signal must be excluded");
        assert_eq!(ids.len(), 10);
        let _ = fs::remove_dir_all(&dir);
    }
}
