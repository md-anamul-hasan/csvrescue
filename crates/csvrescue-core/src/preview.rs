//! Bounded paginated data preview, row filtering, and cell-level diff comparison.

use crate::analysis::{risk_of, ColumnKind};
use crate::core::CsvDocument;
use crate::recovery::{CellRepairKind, ConfidenceLevel, RowDiagnostic};
use crate::security;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Filter categories surfaced by the UI pill bar above the data grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RowFilter {
    #[default]
    All,
    ProblemsOnly,
    RepairedOnly,
    ExcelRiskOnly,
    FormulaRiskOnly,
    ManualEditsOnly,
    DuplicateRowsOnly,
}

/// Paginated request sent from the frontend grid.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewQuery {
    pub offset: usize,
    pub limit: usize,
    pub search: Option<String>,
    pub filter: RowFilter,
}

/// Visual status classification of a single cell inside the preview grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CellStatus {
    Clean,
    RepairedMojibake,
    RepairedStructure,
    ManualEdit,
    ExcelRisk,
    FormulaRisk,
}

/// Single rendered cell inside a paginated preview response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCellDto {
    pub value: String,
    pub original_value: Option<String>,
    pub status: CellStatus,
    pub badge_text: Option<String>,
    pub confidence: Option<ConfidenceLevel>,
    pub tooltip: Option<String>,
}

/// Single preview row with metadata, diagnostics, and cell array aligned to document width.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRowDto {
    pub row_index: usize,
    pub source_line: usize,
    pub is_header: bool,
    pub is_ragged: bool,
    pub is_duplicate: bool,
    pub is_ambiguous: bool,
    pub has_stray_quote: bool,
    pub field_count: usize,
    pub diagnostics: Vec<RowDiagnostic>,
    pub cells: Vec<PreviewCellDto>,
}

/// Header descriptor for grid columns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHeaderDto {
    pub index: usize,
    pub name: String,
    pub kind: ColumnKind,
    pub at_risk: usize,
    pub formula_risk_count: usize,
}

/// Bounded page response sent to the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPageDto {
    pub total_matching: usize,
    pub total_rows: usize,
    pub offset: usize,
    pub limit: usize,
    pub headers: Vec<PreviewHeaderDto>,
    pub rows: Vec<PreviewRowDto>,
}

/// Detailed side-by-side comparison for one selected row (used in `InspectorDrawer`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowComparisonDto {
    pub row_index: usize,
    pub source_line: usize,
    pub is_header: bool,
    pub original_field_count: usize,
    pub recovered_field_count: usize,
    pub diagnostics: Vec<RowDiagnostic>,
    pub cells: Vec<CellComparisonDto>,
}

/// Per-cell side-by-side comparison between original decoded text, recovered value, and manual edit state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellComparisonDto {
    pub col_index: usize,
    pub column_name: String,
    pub column_kind: ColumnKind,
    pub original_value: String,
    pub recovered_value: String,
    pub status: CellStatus,
    pub is_manually_edited: bool,
    pub explanation: Option<String>,
    pub confidence: Option<ConfidenceLevel>,
    pub excel_risk: Option<String>,
    pub formula_risk: Option<String>,
}

/// Returns a bounded, filtered page of rows from `doc` without copying the whole file to the frontend.
pub fn query_preview(doc: &CsvDocument, query: &PreviewQuery) -> PreviewPageDto {
    let limit = query.limit.clamp(1, 500);
    let search_q = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);

    let diag_by_row = group_diagnostics_by_row(doc);
    let repair_by_cell = map_repairs_by_cell(doc);
    let dup_rows: HashSet<usize> = doc
        .validation
        .duplicate_row_indices
        .iter()
        .copied()
        .collect();
    let formula_rows: HashSet<usize> = doc.validation.formula_row_indices.iter().copied().collect();
    let stray_rows: HashSet<usize> = doc.parsed.stray_quote_rows.iter().copied().collect();

    let mut matching_indices: Vec<usize> = Vec::new();
    for (r_idx, row) in doc.recovered_rows.iter().enumerate() {
        let is_header = doc.settings.has_header && r_idx == 0;
        let has_diag = diag_by_row.contains_key(&r_idx);
        let has_cell_repair = doc.recovery.cell_repairs.iter().any(|c| c.row == r_idx);
        let has_manual = doc.manual_edits.keys().any(|&(r, _)| r == r_idx);
        let has_formula = !is_header && formula_rows.contains(&r_idx);
        let has_excel_risk = !is_header
            && row.iter().enumerate().any(|(c_idx, v)| {
                doc.columns
                    .get(c_idx)
                    .is_some_and(|c| c.kind == ColumnKind::Text && !risk_of(v).is_empty())
            });
        let is_dup = dup_rows.contains(&r_idx);

        let passes_filter = match query.filter {
            RowFilter::All => true,
            RowFilter::ProblemsOnly => {
                has_diag || has_cell_repair || has_excel_risk || has_formula || is_dup
            }
            RowFilter::RepairedOnly => has_cell_repair || has_diag,
            RowFilter::ExcelRiskOnly => has_excel_risk,
            RowFilter::FormulaRiskOnly => has_formula,
            RowFilter::ManualEditsOnly => has_manual,
            RowFilter::DuplicateRowsOnly => is_dup,
        };
        if !passes_filter {
            continue;
        }

        if let Some(ref q) = search_q {
            let matches_text = row
                .iter()
                .any(|cell| cell.to_lowercase().contains(q.as_str()));
            if !matches_text {
                continue;
            }
        }

        matching_indices.push(r_idx);
    }

    let total_matching = matching_indices.len();
    let width = doc.columns.len();
    let page_rows = matching_indices
        .into_iter()
        .skip(query.offset)
        .take(limit)
        .map(|r_idx| {
            let row = &doc.recovered_rows[r_idx];
            let orig_row = doc.original_rows.get(r_idx);
            let is_header = doc.settings.has_header && r_idx == 0;
            let row_diags = diag_by_row.get(&r_idx).cloned().unwrap_or_default();
            let is_ragged = doc.recovery.modal_width > 0 && row.len() != doc.recovery.modal_width;
            let is_ambiguous = row_diags.iter().any(|d| d.ambiguous);

            let mut cells = Vec::with_capacity(width);
            for c_idx in 0..width {
                let val = row.get(c_idx).cloned().unwrap_or_default();
                let orig_val = orig_row
                    .and_then(|r| r.get(c_idx))
                    .filter(|o| *o != &val)
                    .cloned();
                let cell_repair = repair_by_cell.get(&(r_idx, c_idx));
                let col_is_text = doc
                    .columns
                    .get(c_idx)
                    .is_some_and(|c| c.kind == ColumnKind::Text);
                let excel_rk = if !is_header && col_is_text {
                    risk_of(&val)
                } else {
                    crate::analysis::Risk::NONE
                };
                let formula_haz = if !is_header {
                    security::detect_formula_risk(&val)
                } else {
                    None
                };

                let (status, badge_text, confidence, tooltip) = if let Some(rep) = cell_repair {
                    let status = match rep.kind {
                        CellRepairKind::Mojibake => CellStatus::RepairedMojibake,
                        CellRepairKind::StructuralMerge => CellStatus::RepairedStructure,
                        CellRepairKind::ManualOverride => CellStatus::ManualEdit,
                    };
                    let badge = match rep.kind {
                        CellRepairKind::Mojibake => "UTF-8",
                        CellRepairKind::StructuralMerge => "MERGED",
                        CellRepairKind::ManualOverride => "EDITED",
                    };
                    (
                        status,
                        Some(badge.to_owned()),
                        Some(rep.confidence),
                        Some(rep.explanation.clone()),
                    )
                } else if let Some(haz) = formula_haz {
                    (
                        CellStatus::FormulaRisk,
                        Some(format!("FORMULA {}", haz.trigger_char)),
                        None,
                        Some("Starts with a spreadsheet formula trigger character.".to_owned()),
                    )
                } else if !excel_rk.is_empty() {
                    (
                        CellStatus::ExcelRisk,
                        Some("TEXT SAFE".to_owned()),
                        None,
                        Some(format!(
                            "Protected as Text (Excel would coerce: {}).",
                            crate::analysis::risk_text(excel_rk)
                        )),
                    )
                } else {
                    (CellStatus::Clean, None, None, None)
                };

                cells.push(PreviewCellDto {
                    value: val,
                    original_value: orig_val,
                    status,
                    badge_text,
                    confidence,
                    tooltip,
                });
            }

            PreviewRowDto {
                row_index: r_idx,
                source_line: doc
                    .parsed
                    .row_start_lines
                    .get(r_idx)
                    .copied()
                    .unwrap_or(r_idx + 1),
                is_header,
                is_ragged,
                is_duplicate: dup_rows.contains(&r_idx),
                is_ambiguous,
                has_stray_quote: stray_rows.contains(&r_idx),
                field_count: row.len(),
                diagnostics: row_diags,
                cells,
            }
        })
        .collect();

    let headers = doc
        .columns
        .iter()
        .map(|c| PreviewHeaderDto {
            index: c.index,
            name: c.name.clone(),
            kind: c.kind,
            at_risk: c.at_risk,
            formula_risk_count: c.formula_risk_count,
        })
        .collect();

    PreviewPageDto {
        total_matching,
        total_rows: doc.recovered_rows.len(),
        offset: query.offset,
        limit,
        headers,
        rows: page_rows,
    }
}

/// Builds a side-by-side comparison of one row for the `InspectorDrawer`.
pub fn get_row_comparison(doc: &CsvDocument, row_index: usize) -> Option<RowComparisonDto> {
    let recovered = doc.recovered_rows.get(row_index)?;
    let original = doc.original_rows.get(row_index);
    let diag_by_row = group_diagnostics_by_row(doc);
    let repair_by_cell = map_repairs_by_cell(doc);
    let is_header = doc.settings.has_header && row_index == 0;
    let width = doc.columns.len().max(recovered.len());

    let mut cells = Vec::with_capacity(width);
    for c_idx in 0..width {
        let rec_val = recovered.get(c_idx).cloned().unwrap_or_default();
        let orig_val = original
            .and_then(|r| r.get(c_idx))
            .cloned()
            .unwrap_or_default();
        let col_info = doc.columns.get(c_idx);
        let col_name = col_info
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Column {}", c_idx + 1));
        let col_kind = col_info.map(|c| c.kind).unwrap_or(ColumnKind::Text);

        let excel_rk = if !is_header && col_kind == ColumnKind::Text {
            let rk = risk_of(&rec_val);
            if rk.is_empty() {
                None
            } else {
                Some(crate::analysis::risk_text(rk))
            }
        } else {
            None
        };

        let formula_risk = if !is_header {
            security::detect_formula_risk(&rec_val).map(|haz| {
                if haz.has_leading_whitespace {
                    format!(
                        "Starts with whitespace/control characters followed by '{}'",
                        haz.trigger_char
                    )
                } else {
                    format!("Starts with formula trigger '{}'", haz.trigger_char)
                }
            })
        } else {
            None
        };

        let rep = repair_by_cell.get(&(row_index, c_idx));
        let status = if let Some(r) = rep {
            match r.kind {
                CellRepairKind::Mojibake => CellStatus::RepairedMojibake,
                CellRepairKind::StructuralMerge => CellStatus::RepairedStructure,
                CellRepairKind::ManualOverride => CellStatus::ManualEdit,
            }
        } else if formula_risk.is_some() {
            CellStatus::FormulaRisk
        } else if excel_rk.is_some() {
            CellStatus::ExcelRisk
        } else {
            CellStatus::Clean
        };

        cells.push(CellComparisonDto {
            col_index: c_idx,
            column_name: col_name,
            column_kind: col_kind,
            original_value: orig_val,
            recovered_value: rec_val,
            status,
            is_manually_edited: doc.manual_edits.contains_key(&(row_index, c_idx)),
            explanation: rep.map(|r| r.explanation.clone()),
            confidence: rep.map(|r| r.confidence),
            excel_risk: excel_rk,
            formula_risk,
        });
    }

    Some(RowComparisonDto {
        row_index,
        source_line: doc
            .parsed
            .row_start_lines
            .get(row_index)
            .copied()
            .unwrap_or(row_index + 1),
        is_header,
        original_field_count: original.map_or(0, Vec::len),
        recovered_field_count: recovered.len(),
        diagnostics: diag_by_row.get(&row_index).cloned().unwrap_or_default(),
        cells,
    })
}

fn group_diagnostics_by_row(doc: &CsvDocument) -> HashMap<usize, Vec<RowDiagnostic>> {
    let mut map: HashMap<usize, Vec<RowDiagnostic>> = HashMap::new();
    for d in &doc.recovery.diagnostics {
        map.entry(d.row_index).or_default().push(d.clone());
    }
    map
}

fn map_repairs_by_cell(
    doc: &CsvDocument,
) -> HashMap<(usize, usize), &crate::recovery::CellDiagnostic> {
    let mut map = HashMap::new();
    for rep in &doc.recovery.cell_repairs {
        // Later entries (e.g. ManualOverride) take visual precedence over Mojibake
        map.insert((rep.row, rep.col), rep);
    }
    map
}
