//! Future-gem (value-surge) screening: FM quality gate, development-signal fallback,
//! position strength summaries, and the canonical surge feature schema.

/// Canonical order of the 47 raw surge features shared by the training matrix,
/// candidate matrix, and scoring artifacts. Column `i` of a surge TSV/matrix maps
/// to `SURGE_FEATURE_NAMES[i]`.
pub const SURGE_FEATURE_NAMES: [&str; 47] = [
    "log_mv",
    "log_mv_180d_ratio",
    "log_mv_365d_ratio",
    "log_peak_prior_ratio",
    "log_min_prior_ratio",
    "log_n_vals_prior",
    "log_days_since_first_val",
    "val_freq",
    "log_days_since_last_val",
    "last_change_dir",
    "mv_vol_log",
    "perf_present_730",
    "log_mins_365",
    "log_apps_365",
    "g_per90_365",
    "a_per90_365",
    "log_mins_180",
    "mins_trend",
    "g_per90_180",
    "ever_transferred",
    "log_days_since_transfer",
    "log_transfer_fee",
    "free_transfer",
    "log_mv_transfer_ratio",
    "age",
    "age_sq",
    "pos_gk",
    "pos_df",
    "pos_mf",
    "height_cm",
    "comp_mapped",
    "first_tier",
    "confed_uefa",
    "confed_sam",
    "fm_present",
    "fm_ca",
    "fm_pa",
    "fm_pa_gap",
    "fm_ca_age_pct",
    "fm_sig_technical",
    "fm_sig_physical",
    "fm_sig_mental",
    "fm_sig_defensive",
    "fm_sig_attacking",
    "fm_sig_creative",
    "fm_attr_db",
    "fm_attr_derived",
];

/// The 12-month value-forecast pipeline reuses the identical canonical 47-feature
/// schema as the surge model (market state, performance, transfers, player state,
/// competition context, FM state). Kept as an explicit alias so the forecast
/// artifacts, apps, and report name the schema they consume.
pub const FORECAST_FEATURE_NAMES: [&str; 47] = SURGE_FEATURE_NAMES;

/// Convert a negative FM PA band code (randomized potential) to its documented
/// band midpoint. Returns `None` for non-finite values, positive PA, or codes
/// outside the table. Callers keep positive PA as-is and use this only for the
/// negative codes.
pub fn fm_pa_band_midpoint(pa: f64) -> Option<f64> {
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

/// Position class inferred from FM attributes (GK/DF/MF/FW).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionClass {
    Goalkeeper,
    Defender,
    Midfielder,
    Forward,
}

impl PositionClass {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "GK" => Some(Self::Goalkeeper),
            "DF" => Some(Self::Defender),
            "MF" => Some(Self::Midfielder),
            "FW" => Some(Self::Forward),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Goalkeeper => "GK",
            Self::Defender => "DF",
            Self::Midfielder => "MF",
            Self::Forward => "FW",
        }
    }
}

/// Position-appropriate FM attribute minimums for the "good on the field" gate.
/// Threshold 12 is the documented bar: a 12 is a solid professional-level attribute
/// (FM scale 1-20); requiring two position-critical attributes avoids one-dimensional
/// players. Forwards need finishing+dribbling, midfielders passing+vision, defenders
/// tackling+marking, goalkeepers handling+reflexes.
pub const QUALITY_ATTRIBUTE_MIN: f64 = 12.0;
pub const DEV_MAX_AGE: f64 = 22.0;
pub const DEV_MIN_PA_GAP: f64 = 20.0;
pub const DEV_MIN_CA_AGE_PCT: f64 = 0.5;

/// FM attributes used by the quality gate, in fixed order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityAttributes {
    pub finishing: Option<f64>,
    pub dribbling: Option<f64>,
    pub passing: Option<f64>,
    pub vision: Option<f64>,
    pub tackling: Option<f64>,
    pub marking: Option<f64>,
    pub handling: Option<f64>,
    pub reflexes: Option<f64>,
}

impl QualityAttributes {
    fn at_least(value: Option<f64>, minimum: f64) -> bool {
        value.is_some_and(|value| value >= minimum)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenReason {
    QualityGate,
    DevelopmentSignal,
    Both,
    None,
}

impl ScreenReason {
    pub fn passed(self) -> bool {
        !matches!(self, Self::None)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QualityGate => "quality-gate",
            Self::DevelopmentSignal => "development-signal",
            Self::Both => "quality-gate+development-signal",
            Self::None => "failed",
        }
    }
}

/// Quality gate: position-appropriate FM attributes at or above 12.
pub fn quality_gate(pos_cls: PositionClass, attrs: &QualityAttributes) -> bool {
    match pos_cls {
        PositionClass::Goalkeeper => {
            QualityAttributes::at_least(attrs.handling, QUALITY_ATTRIBUTE_MIN)
                && QualityAttributes::at_least(attrs.reflexes, QUALITY_ATTRIBUTE_MIN)
        }
        PositionClass::Defender => {
            QualityAttributes::at_least(attrs.tackling, QUALITY_ATTRIBUTE_MIN)
                && QualityAttributes::at_least(attrs.marking, QUALITY_ATTRIBUTE_MIN)
        }
        PositionClass::Midfielder => {
            QualityAttributes::at_least(attrs.passing, QUALITY_ATTRIBUTE_MIN)
                && QualityAttributes::at_least(attrs.vision, QUALITY_ATTRIBUTE_MIN)
        }
        PositionClass::Forward => {
            QualityAttributes::at_least(attrs.finishing, QUALITY_ATTRIBUTE_MIN)
                && QualityAttributes::at_least(attrs.dribbling, QUALITY_ATTRIBUTE_MIN)
        }
    }
}

/// Development-signal fallback: young, large PA-CA upside, above-median age-relative
/// ability (FM `ca_age_pct` percentile within the player's age band).
pub fn development_signal(age: Option<f64>, pa_gap: Option<f64>, ca_age_pct: Option<f64>) -> bool {
    age.is_some_and(|age| age <= DEV_MAX_AGE)
        && pa_gap.is_some_and(|gap| gap >= DEV_MIN_PA_GAP)
        && ca_age_pct.is_some_and(|pct| pct >= DEV_MIN_CA_AGE_PCT)
}

/// Full eligibility screen for the proposal universe. `attr_source` must be a
/// real or statistically-derived FM attribute source (`db`/`derived`).
pub fn surge_screen(
    pos_cls: PositionClass,
    attrs: &QualityAttributes,
    age: Option<f64>,
    pa_gap: Option<f64>,
    ca_age_pct: Option<f64>,
    attr_source: &str,
) -> ScreenReason {
    if attr_source != "db" && attr_source != "derived" {
        return ScreenReason::None;
    }
    let quality = quality_gate(pos_cls, attrs);
    let development = development_signal(age, pa_gap, ca_age_pct);
    match (quality, development) {
        (true, true) => ScreenReason::Both,
        (true, false) => ScreenReason::QualityGate,
        (false, true) => ScreenReason::DevelopmentSignal,
        (false, false) => ScreenReason::None,
    }
}

/// Attribute columns considered when summarizing strengths per position.
const STRENGTH_ATTRIBUTES: [(&str, &str); 18] = [
    ("Finishing", "attr_Finishing"),
    ("Dribbling", "attr_Dribbling"),
    ("Passing", "attr_Passing"),
    ("Vision", "attr_Vision"),
    ("Tackling", "attr_Tackling"),
    ("Marking", "attr_Marking"),
    ("Handling", "attr_Handling"),
    ("Reflexes", "attr_Reflexes"),
    ("FirstTouch", "attr_FirstTouch"),
    ("Acceleration", "attr_Acceleration"),
    ("Pace", "attr_Pace"),
    ("Stamina", "attr_Stamina"),
    ("WorkRate", "attr_WorkRate"),
    ("Anticipation", "attr_Anticipation"),
    ("Positioning", "attr_Positioning"),
    ("Composure", "attr_Composure"),
    ("Technique", "attr_Technique"),
    ("OffTheBall", "attr_OffTheBall"),
];

fn position_relevant(pos_cls: PositionClass, label: &str) -> bool {
    match pos_cls {
        PositionClass::Goalkeeper => matches!(label, "Handling" | "Reflexes" | "Anticipation" | "Composure" | "Positioning" | "FirstTouch"),
        PositionClass::Defender => matches!(label, "Tackling" | "Marking" | "Positioning" | "Anticipation" | "Stamina" | "Pace" | "Acceleration" | "WorkRate" | "Composure" | "Passing" | "FirstTouch"),
        PositionClass::Midfielder => matches!(label, "Passing" | "Vision" | "FirstTouch" | "Technique" | "Dribbling" | "Composure" | "WorkRate" | "Stamina" | "Anticipation" | "OffTheBall" | "Acceleration" | "Pace"),
        PositionClass::Forward => matches!(label, "Finishing" | "Dribbling" | "OffTheBall" | "Composure" | "Technique" | "Pace" | "Acceleration" | "Anticipation" | "FirstTouch" | "Passing"),
    }
}

/// Up to `limit` position-relevant attributes at or above 12, strongest first,
/// formatted as "Finishing 15". Deterministic (fixed attribute order tie-break).
pub fn key_strengths(
    pos_cls: PositionClass,
    values: &[(String, f64)],
    limit: usize,
) -> Vec<String> {
    let mut candidates: Vec<(&str, f64)> = values
        .iter()
        .filter_map(|(label, value)| {
            let label = label.as_str();
            STRENGTH_ATTRIBUTES
                .iter()
                .find(|(_, column)| *column == label)
                .map(|(short, _)| (*short, *value))
        })
        .filter(|(short, value)| position_relevant(pos_cls, short) && value.is_finite() && *value >= QUALITY_ATTRIBUTE_MIN)
        .collect();
    candidates.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(right.0))
    });
    candidates.truncate(limit);
    candidates
        .into_iter()
        .map(|(label, value)| format!("{label} {}", value.round() as i64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(
        finishing: Option<f64>,
        dribbling: Option<f64>,
        passing: Option<f64>,
        vision: Option<f64>,
        tackling: Option<f64>,
        marking: Option<f64>,
        handling: Option<f64>,
        reflexes: Option<f64>,
    ) -> QualityAttributes {
        QualityAttributes { finishing, dribbling, passing, vision, tackling, marking, handling, reflexes }
    }

    #[test]
    fn quality_gate_requires_position_appropriate_minimums() {
        let forward = attrs(Some(14.0), Some(11.0), None, None, None, None, None, None);
        assert!(!quality_gate(PositionClass::Forward, &forward));
        let forward = attrs(Some(14.0), Some(13.0), None, None, None, None, None, None);
        assert!(quality_gate(PositionClass::Forward, &forward));
        let keeper = attrs(None, None, None, None, None, None, Some(15.0), Some(11.0));
        assert!(!quality_gate(PositionClass::Goalkeeper, &keeper));
        let keeper = attrs(None, None, None, None, None, None, Some(15.0), Some(14.0));
        assert!(quality_gate(PositionClass::Goalkeeper, &keeper));
    }

    #[test]
    fn development_signal_requires_young_high_upside() {
        assert!(development_signal(Some(20.0), Some(25.0), Some(0.6)));
        assert!(!development_signal(Some(23.0), Some(25.0), Some(0.6)));
        assert!(!development_signal(Some(20.0), Some(10.0), Some(0.6)));
        assert!(!development_signal(Some(20.0), Some(25.0), None));
    }

    #[test]
    fn screen_rejects_non_fm_attr_sources() {
        let strong = attrs(Some(14.0), Some(13.0), None, None, None, None, None, None);
        assert_eq!(surge_screen(PositionClass::Forward, &strong, Some(20.0), Some(25.0), Some(0.6), "none"), ScreenReason::None);
        assert_eq!(surge_screen(PositionClass::Forward, &strong, Some(20.0), Some(25.0), Some(0.6), "db"), ScreenReason::Both);
    }

    #[test]
    fn strengths_are_ordered_and_filtered_by_position() {
        let values = vec![
            ("attr_Finishing".to_string(), 15.0),
            ("attr_Dribbling".to_string(), 13.0),
            ("attr_Pace".to_string(), 16.0),
            ("attr_Handling".to_string(), 18.0),
        ];
        let strengths = key_strengths(PositionClass::Forward, &values, 2);
        assert_eq!(strengths, vec!["Pace 16".to_string(), "Finishing 15".to_string()]);
        let keeper_strengths = key_strengths(PositionClass::Goalkeeper, &values, 3);
        assert_eq!(keeper_strengths, vec!["Handling 18".to_string()]);
    }

    #[test]
    fn feature_names_are_unique_and_complete() {
        let mut seen = std::collections::BTreeSet::new();
        assert!(SURGE_FEATURE_NAMES.iter().all(|name| seen.insert(*name)));
        assert_eq!(SURGE_FEATURE_NAMES.len(), 47);
    }
}
