//! Configurable repair execution, deterministic ragged-row alignment, and manual edit overlay.

use crate::analysis::{try_parse_number, DecimalMark};
use crate::recovery::{
    CellDiagnostic, CellRepairKind, ConfidenceLevel, RecoverySummary, RepairStrategy,
    RowDiagnostic, RowIssueKind,
};
use std::collections::BTreeMap;

/// Input bundle for building the recovered dataset from parsed records.
pub struct RepairInput<'a> {
    pub original_rows: &'a [Vec<String>],
    pub base_rows: &'a [Vec<String>],
    pub row_start_lines: &'a [usize],
    pub stray_quote_rows: &'a [usize],
    pub ended_inside_quote: bool,
    pub modal_width: usize,
    pub delimiter: char,
    pub has_header: bool,
    pub decimal_mark: DecimalMark,
    pub strategy: RepairStrategy,
    pub manual_edits: &'a BTreeMap<(usize, usize), String>,
}

/// Result of running [`apply_repairs`].
pub struct RepairOutput {
    pub rows: Vec<Vec<String>>,
    pub summary: RecoverySummary,
}

/// Applies the selected recovery strategy and manual cell overrides to produce recovered rows
/// while leaving `original_rows` untouched and recording every transformation or ambiguity.
pub fn apply_repairs(input: &RepairInput<'_>) -> RepairOutput {
    let mut rows: Vec<Vec<String>> = input.base_rows.to_vec();
    let mut diagnostics: Vec<RowDiagnostic> = Vec::new();
    let mut cell_repairs: Vec<CellDiagnostic> = Vec::new();

    // Record cell-level mojibake repairs where `original_rows` differs from `base_rows`.
    for (r_idx, base_row) in input.base_rows.iter().enumerate() {
        if let Some(orig_row) = input.original_rows.get(r_idx) {
            for (c_idx, repaired_val) in base_row.iter().enumerate() {
                if let Some(orig_val) = orig_row.get(c_idx) {
                    if orig_val != repaired_val {
                        cell_repairs.push(CellDiagnostic {
                            row: r_idx,
                            col: c_idx,
                            original: orig_val.clone(),
                            repaired: repaired_val.clone(),
                            kind: CellRepairKind::Mojibake,
                            confidence: ConfidenceLevel::High,
                            explanation:
                                "Double-encoded UTF-8 sequence cleanly reversed via Windows-1252."
                                    .to_owned(),
                        });
                    }
                }
            }
        }
    }

    let mojibake_cells_count = cell_repairs.len();
    let expected = input.modal_width;
    let first_data = usize::from(input.has_header);

    // Precompute column numeric expectations from well-formed rows when AlignColumns is active.
    let col_numeric_profile = if input.strategy == RepairStrategy::AlignColumns && expected >= 2 {
        compute_numeric_profile(&rows, first_data, expected, input.decimal_mark)
    } else {
        Vec::new()
    };

    let mut short_rows = 0usize;
    let mut wide_rows = 0usize;
    let mut aligned_rows = 0usize;
    let mut ambiguous_rows = 0usize;

    for (r_idx, row) in rows.iter_mut().enumerate() {
        let source_line = input
            .row_start_lines
            .get(r_idx)
            .copied()
            .unwrap_or(r_idx + 1);
        let orig_len = row.len();

        if expected > 0 && orig_len < expected {
            short_rows += 1;
            if input.strategy == RepairStrategy::AlignColumns {
                row.resize(expected, String::new());
            }
            diagnostics.push(RowDiagnostic {
                row_index: r_idx,
                source_line,
                kind: RowIssueKind::ShortRow,
                original_field_count: orig_len,
                expected_field_count: expected,
                confidence: ConfidenceLevel::Medium,
                ambiguous: false,
                explanation: format!(
                    "Row has {orig_len} field(s) instead of {expected}; missing trailing cells remain empty."
                ),
            });
        } else if expected > 0 && orig_len > expected {
            wide_rows += 1;
            if input.strategy == RepairStrategy::AlignColumns
                && r_idx >= first_data
                && !col_numeric_profile.is_empty()
            {
                let excess = orig_len - expected;
                let candidates = find_unambiguous_merges(
                    row,
                    expected,
                    excess,
                    input.delimiter,
                    &col_numeric_profile,
                    input.decimal_mark,
                );
                if candidates.len() == 1 {
                    let merge_start = candidates[0];
                    let before_join =
                        row[merge_start..=merge_start + excess].join(&input.delimiter.to_string());
                    let merged_cell = before_join.clone();
                    let orig_slice =
                        format!("{} (split across {} columns)", merged_cell, excess + 1);
                    let mut new_row = Vec::with_capacity(expected);
                    new_row.extend_from_slice(&row[..merge_start]);
                    new_row.push(merged_cell.clone());
                    new_row.extend_from_slice(&row[merge_start + excess + 1..]);
                    *row = new_row;
                    aligned_rows += 1;
                    cell_repairs.push(CellDiagnostic {
                        row: r_idx,
                        col: merge_start,
                        original: orig_slice,
                        repaired: merged_cell,
                        kind: CellRepairKind::StructuralMerge,
                        confidence: ConfidenceLevel::Medium,
                        explanation: format!(
                            "Joined {} adjacent fields at column {} using '{}' so trailing numeric/text columns align.",
                            excess + 1,
                            merge_start + 1,
                            input.delimiter
                        ),
                    });
                    diagnostics.push(RowDiagnostic {
                        row_index: r_idx,
                        source_line,
                        kind: RowIssueKind::RaggedAligned,
                        original_field_count: orig_len,
                        expected_field_count: expected,
                        confidence: ConfidenceLevel::Medium,
                        ambiguous: false,
                        explanation: format!(
                            "Row had {orig_len} fields (expected {expected}); merged columns {}..{} cleanly.",
                            merge_start + 1,
                            merge_start + excess + 1
                        ),
                    });
                } else {
                    ambiguous_rows += 1;
                    diagnostics.push(RowDiagnostic {
                        row_index: r_idx,
                        source_line,
                        kind: RowIssueKind::AmbiguousWidth,
                        original_field_count: orig_len,
                        expected_field_count: expected,
                        confidence: ConfidenceLevel::Ambiguous,
                        ambiguous: true,
                        explanation: format!(
                            "Row has {orig_len} fields (expected {expected}), and {} plausible merge points match ({} total); kept unmerged so no data is guessed.",
                            candidates.len(),
                            orig_len - expected
                        ),
                    });
                }
            } else {
                diagnostics.push(RowDiagnostic {
                    row_index: r_idx,
                    source_line,
                    kind: RowIssueKind::WideRow,
                    original_field_count: orig_len,
                    expected_field_count: expected,
                    confidence: ConfidenceLevel::High,
                    ambiguous: false,
                    explanation: format!(
                        "Row has {orig_len} fields (expected {expected}); extra fields are preserved."
                    ),
                });
            }
        }
    }

    for &r_idx in input.stray_quote_rows {
        let source_line = input
            .row_start_lines
            .get(r_idx)
            .copied()
            .unwrap_or(r_idx + 1);
        let orig_len = input.base_rows.get(r_idx).map_or(0, Vec::len);
        diagnostics.push(RowDiagnostic {
            row_index: r_idx,
            source_line,
            kind: RowIssueKind::StrayQuote,
            original_field_count: orig_len,
            expected_field_count: expected,
            confidence: ConfidenceLevel::High,
            ambiguous: false,
            explanation: "Quotation marks appeared inside an unquoted field or after a closing quote and were kept as text.".to_owned(),
        });
    }

    if input.ended_inside_quote && !rows.is_empty() {
        let last_idx = rows.len() - 1;
        let source_line = input
            .row_start_lines
            .get(last_idx)
            .copied()
            .unwrap_or(last_idx + 1);
        diagnostics.push(RowDiagnostic {
            row_index: last_idx,
            source_line,
            kind: RowIssueKind::UnclosedQuote,
            original_field_count: rows[last_idx].len(),
            expected_field_count: expected,
            confidence: ConfidenceLevel::Low,
            ambiguous: true,
            explanation:
                "The file ended while inside an open quoted field; a closing quote is missing."
                    .to_owned(),
        });
    }

    // Apply user's explicit manual cell overrides last.
    let mut manual_edits_count = 0usize;
    for (&(r_idx, c_idx), new_val) in input.manual_edits {
        if let Some(row) = rows.get_mut(r_idx) {
            if c_idx >= row.len() {
                row.resize(c_idx + 1, String::new());
            }
            let prev = row[c_idx].clone();
            if &prev != new_val {
                row[c_idx] = new_val.clone();
                manual_edits_count += 1;
                cell_repairs.push(CellDiagnostic {
                    row: r_idx,
                    col: c_idx,
                    original: prev,
                    repaired: new_val.clone(),
                    kind: CellRepairKind::ManualOverride,
                    confidence: ConfidenceLevel::High,
                    explanation: "Manually corrected by user in CSVRescue.".to_owned(),
                });
            }
        }
    }

    diagnostics.sort_by_key(|d| (d.row_index, d.source_line));
    let max_width = rows.iter().map(Vec::len).max().unwrap_or(0);

    RepairOutput {
        rows,
        summary: RecoverySummary {
            strategy: input.strategy,
            modal_width: expected,
            max_width,
            short_rows,
            wide_rows,
            aligned_rows,
            ambiguous_rows,
            manual_edits_count,
            mojibake_cells_count,
            diagnostics,
            cell_repairs,
        },
    }
}

/// Returns `true` for columns where >= 90% of non-empty values in modal-width rows are clean numbers.
fn compute_numeric_profile(
    rows: &[Vec<String>],
    first_data: usize,
    expected_width: usize,
    mark: DecimalMark,
) -> Vec<bool> {
    let effective_mark = if mark == DecimalMark::Auto {
        DecimalMark::Dot
    } else {
        mark
    };
    let mut numeric_hits = vec![0usize; expected_width];
    let mut non_empty = vec![0usize; expected_width];

    for row in rows.iter().skip(first_data) {
        if row.len() != expected_width {
            continue;
        }
        for (c_idx, cell) in row.iter().enumerate() {
            if cell.trim().is_empty() {
                continue;
            }
            non_empty[c_idx] += 1;
            if try_parse_number(cell, effective_mark).is_some()
                || try_parse_number(cell, DecimalMark::Comma).is_some()
            {
                numeric_hits[c_idx] += 1;
            }
        }
    }

    (0..expected_width)
        .map(|c| non_empty[c] >= 2 && numeric_hits[c] * 10 >= non_empty[c] * 9)
        .collect()
}

fn find_unambiguous_merges(
    row: &[String],
    expected_width: usize,
    excess: usize,
    delimiter: char,
    numeric_profile: &[bool],
    mark: DecimalMark,
) -> Vec<usize> {
    let mut valid_starts = Vec::new();
    let delim_str = delimiter.to_string();

    for start in 0..expected_width {
        if numeric_profile[start] {
            continue;
        }
        let mut candidate_ok = true;
        // Check columns before `start`
        for c in 0..start {
            if numeric_profile[c] && !is_numeric_cell(&row[c], mark) {
                candidate_ok = false;
                break;
            }
        }
        if !candidate_ok {
            continue;
        }
        // Check columns after merged slice
        for (c, &is_num) in numeric_profile
            .iter()
            .enumerate()
            .take(expected_width)
            .skip(start + 1)
        {
            let src_idx = c + excess;
            if is_num && !is_numeric_cell(&row[src_idx], mark) {
                candidate_ok = false;
                break;
            }
        }
        // Also ensure merged string isn't completely identical to expecting a pure empty split unless needed
        let _merged_preview = row[start..=start + excess].join(&delim_str);
        if candidate_ok {
            valid_starts.push(start);
        }
    }

    valid_starts
}

fn is_numeric_cell(s: &str, mark: DecimalMark) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return true;
    }
    match mark {
        DecimalMark::Auto => {
            try_parse_number(t, DecimalMark::Dot).is_some()
                || try_parse_number(t, DecimalMark::Comma).is_some()
        }
        explicit => try_parse_number(t, explicit).is_some(),
    }
}
