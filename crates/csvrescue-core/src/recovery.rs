//! Recovery strategy types, confidence scoring, and per-row/per-cell diagnostic records.

use serde::{Deserialize, Serialize};

/// Strategy for handling structural anomalies such as ragged rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RepairStrategy {
    /// Preserves all parsed fields as-is and expands table/export width to the widest row so no data is ever dropped.
    #[default]
    Preserve,
    /// Attempts deterministic column realignment on ragged rows when column types indicate a single unambiguous split; flags ambiguous rows without guessing.
    AlignColumns,
}

/// Confidence level assigned to an automatic repair or diagnostic inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
    Ambiguous,
}

impl ConfidenceLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Ambiguous => "Ambiguous",
        }
    }
}

/// Category of structural issue recorded for a parsed row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RowIssueKind {
    ShortRow,
    WideRow,
    StrayQuote,
    UnclosedQuote,
    RaggedAligned,
    AmbiguousWidth,
}

/// Category of repair applied to a specific cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CellRepairKind {
    Mojibake,
    StructuralMerge,
    ManualOverride,
}

/// Row-level recovery diagnostic explaining what was detected and how it was handled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowDiagnostic {
    pub row_index: usize,
    pub source_line: usize,
    pub kind: RowIssueKind,
    pub original_field_count: usize,
    pub expected_field_count: usize,
    pub confidence: ConfidenceLevel,
    pub ambiguous: bool,
    pub explanation: String,
}

/// Cell-level before/after repair trace for auditability and UI diff highlighting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellDiagnostic {
    pub row: usize,
    pub col: usize,
    pub original: String,
    pub repaired: String,
    pub kind: CellRepairKind,
    pub confidence: ConfidenceLevel,
    pub explanation: String,
}

/// Aggregate summary of all recovery actions and ambiguity warnings for a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySummary {
    pub strategy: RepairStrategy,
    pub modal_width: usize,
    pub max_width: usize,
    pub short_rows: usize,
    pub wide_rows: usize,
    pub aligned_rows: usize,
    pub ambiguous_rows: usize,
    pub manual_edits_count: usize,
    pub mojibake_cells_count: usize,
    pub diagnostics: Vec<RowDiagnostic>,
    pub cell_repairs: Vec<CellDiagnostic>,
}
