//! Workout log types matching `exports/grouped.json`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    UpperLower,
    FullBody,
    AnteriorPosterior,
    Other,
}

impl Split {
    pub fn as_str(self) -> &'static str {
        match self {
            Split::UpperLower => "upper_lower",
            Split::FullBody => "full_body",
            Split::AnteriorPosterior => "anterior_posterior",
            Split::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "upper_lower" => Some(Split::UpperLower),
            "full_body" => Some(Split::FullBody),
            "anterior_posterior" => Some(Split::AnteriorPosterior),
            "other" => Some(Split::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePrecision {
    Exact,
    Week,
    Unknown,
}

impl DatePrecision {
    pub fn as_str(self) -> &'static str {
        match self {
            DatePrecision::Exact => "exact",
            DatePrecision::Week => "week",
            DatePrecision::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "exact" => Some(DatePrecision::Exact),
            "week" => Some(DatePrecision::Week),
            "unknown" => Some(DatePrecision::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    App,
    NotesImport,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::App => "app",
            Source::NotesImport => "notes_import",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "app" => Some(Source::App),
            "notes_import" => Some(Source::NotesImport),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AliasStatus {
    Confirmed,
    Suggested,
}

impl AliasStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AliasStatus::Confirmed => "confirmed",
            AliasStatus::Suggested => "suggested",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "confirmed" => Some(AliasStatus::Confirmed),
            "suggested" => Some(AliasStatus::Suggested),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub split: Split,
    pub label: Option<String>,
    pub started_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBlock {
    pub slug: String,
    pub name: String,
    pub split: Split,
    pub label: Option<String>,
    pub started_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cycle {
    pub id: i64,
    pub block_id: i64,
    pub week: i32,
    pub title: Option<String>,
    pub note_id: Option<String>,
    pub started_on: Option<String>,
    pub date_precision: DatePrecision,
    pub override_flag: bool,
    pub source: Source,
    pub source_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCycle {
    pub block_id: i64,
    pub week: i32,
    pub title: Option<String>,
    pub note_id: Option<String>,
    pub started_on: Option<String>,
    pub date_precision: DatePrecision,
    pub override_flag: bool,
    pub source: Source,
    pub source_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub cycle_id: i64,
    pub day_number: Option<i32>,
    pub day_label: Option<String>,
    pub comment: Option<String>,
    pub header_raw: Option<String>,
    pub implicit: bool,
    pub index: i32,
    pub notes: Vec<String>,
    pub performed_on: Option<String>,
    pub date_precision: DatePrecision,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSession {
    pub cycle_id: i64,
    pub day_number: Option<i32>,
    pub day_label: Option<String>,
    pub comment: Option<String>,
    pub header_raw: Option<String>,
    pub implicit: bool,
    pub index: i32,
    pub notes: Vec<String>,
    pub performed_on: Option<String>,
    pub date_precision: DatePrecision,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub id: i64,
    pub slug: String,
    pub display: String,
    pub family: Option<String>,
    pub gym: Option<String>,
    pub status: AliasStatus,
    pub joint_action: Option<String>,
    pub equipment: Option<String>,
    pub load_unit: String,
    pub default_settings: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewVariant {
    pub slug: String,
    pub display: String,
    pub family: Option<String>,
    pub gym: Option<String>,
    pub status: AliasStatus,
    pub joint_action: Option<String>,
    pub equipment: Option<String>,
    pub load_unit: String,
    pub default_settings: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    pub normalized: String,
    pub variant_id: i64,
    pub status: AliasStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub session_id: i64,
    pub variant_id: Option<i64>,
    pub position: i32,
    pub tracked: bool,
    pub name: Option<String>,
    pub name_raw: String,
    pub qualifiers: Vec<String>,
    pub untracked_sets: Option<i32>,
    pub settings: Option<String>,
    pub comment: Option<String>,
    pub raw: Option<String>,
    pub excluded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEntry {
    pub session_id: i64,
    pub variant_id: Option<i64>,
    pub position: i32,
    pub tracked: bool,
    pub name: Option<String>,
    pub name_raw: String,
    pub qualifiers: Vec<String>,
    pub untracked_sets: Option<i32>,
    pub settings: Option<String>,
    pub comment: Option<String>,
    pub raw: Option<String>,
    pub excluded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLog {
    pub id: i64,
    pub entry_id: i64,
    pub set_number: i32,
    pub weight: Option<f64>,
    pub weight_inferred: bool,
    pub weight_note: Option<String>,
    pub reps: Option<f64>,
    pub rir_low: Option<f64>,
    pub rir_high: Option<f64>,
    pub logged: bool,
    pub comment: Option<String>,
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSet {
    pub set_number: i32,
    pub weight: Option<f64>,
    pub weight_inferred: bool,
    pub weight_note: Option<String>,
    pub reps: Option<f64>,
    pub rir_low: Option<f64>,
    pub rir_high: Option<f64>,
    pub logged: bool,
    pub comment: Option<String>,
    pub raw: Option<String>,
}

/// Last logged performance of a variant (for Log tab prefill).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPerformance {
    pub session_id: i64,
    pub entry_id: i64,
    pub day_label: Option<String>,
    pub performed_on: Option<String>,
    pub settings: Option<String>,
    pub sets: Vec<SetLog>,
}

/// Session summary for History listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session: Session,
    pub block_slug: String,
    pub block_name: String,
    pub week: i32,
    pub cycle_title: Option<String>,
    pub entry_count: i32,
}

/// Full session payload for `ha log show`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub session: Session,
    pub block_slug: String,
    pub block_name: String,
    pub week: i32,
    pub cycle_title: Option<String>,
    pub entries: Vec<EntryDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryDetail {
    pub entry: Entry,
    pub variant_slug: Option<String>,
    pub variant_display: Option<String>,
    pub sets: Vec<SetLog>,
}

/// One set with its block, cycle, session and variant, for metrics and CSV export.
#[derive(Debug, Clone)]
pub struct SetRow {
    pub block_slug: String,
    pub block_label: Option<String>,
    pub split: Split,
    pub cycle_id: i64,
    pub week: i32,
    pub session_id: i64,
    pub performed_on: Option<String>,
    pub day_label: Option<String>,
    pub position: i32,
    pub variant_id: Option<i64>,
    pub variant_slug: Option<String>,
    pub variant_display: Option<String>,
    pub family: Option<String>,
    pub gym: Option<String>,
    pub joint_action: Option<String>,
    pub set: SetLog,
}

/// Minimal seed shape from `exports/grouped.json` (variants only).
#[derive(Debug, Clone, Deserialize)]
pub struct GroupedSeed {
    pub variants: Vec<GroupedVariant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupedVariant {
    pub slug: String,
    pub display: String,
    pub family: Option<String>,
    pub gym: Option<String>,
    pub status: String,
    #[serde(default)]
    pub names: Vec<String>,
}

/// Full `exports/grouped.json` document for `ha import`.
#[derive(Debug, Clone, Deserialize)]
pub struct GroupedFile {
    #[serde(default)]
    pub parser_version: i32,
    #[serde(default)]
    pub exported_at: Option<String>,
    pub blocks: Vec<GroupedBlock>,
    pub cycles: Vec<GroupedCycle>,
    #[serde(default)]
    pub review: Vec<GroupedReviewItem>,
    #[serde(default)]
    pub variants: Vec<GroupedVariant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupedBlock {
    pub slug: String,
    pub name: String,
    pub split: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub started_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedCycle {
    pub block: String,
    pub week: i32,
    #[serde(default)]
    pub title: Option<String>,
    pub note_id: String,
    #[serde(default)]
    pub started_on: Option<String>,
    #[serde(default)]
    pub date_precision: Option<String>,
    #[serde(default, rename = "override")]
    pub override_reason: Option<String>,
    pub sessions: Vec<GroupedSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedSession {
    #[serde(default)]
    pub day_number: Option<i32>,
    #[serde(default)]
    pub day_label: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub header_raw: Option<String>,
    #[serde(default)]
    pub implicit: bool,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub index: i32,
    pub entries: Vec<GroupedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedEntry {
    #[serde(default)]
    pub name: Option<String>,
    pub name_raw: String,
    #[serde(default)]
    pub qualifiers: Vec<String>,
    #[serde(default)]
    pub settings: Option<String>,
    #[serde(default = "default_true")]
    pub tracked: bool,
    #[serde(default)]
    pub untracked_sets: Option<i32>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub sets: Vec<GroupedSet>,
    pub position: i32,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub excluded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedSet {
    pub set_number: i32,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub weight_inferred: bool,
    #[serde(default)]
    pub weight_note: Option<String>,
    #[serde(default)]
    pub reps: Option<f64>,
    #[serde(default)]
    pub rir_low: Option<f64>,
    #[serde(default)]
    pub rir_high: Option<f64>,
    #[serde(default = "default_true")]
    pub logged: bool,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupedReviewItem {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub note_id: Option<String>,
    pub line: i32,
    #[serde(default)]
    pub raw: Option<String>,
    #[serde(default)]
    pub issue: Option<String>,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

fn default_true() -> bool {
    true
}
