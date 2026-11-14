//! Structural and cell-level data validation across rows and columns.

use crate::analysis::{try_parse_number, ColumnInfo, ColumnKind};
use crate::detector::LineEndingInfo;
use crate::encoding::Decoded;
use crate::parser::ParseResult;
use crate::recovery::RecoverySummary;
use crate::security;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Severity of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingSeverity {
    Info,
    Warning,
    Security,
}

/// Individual structured validation finding surfaced in Columns & Validation and recovery reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFinding {
    pub severity: FindingSeverity,
    pub category: String,
    pub count: usize,
    pub summary: String,
    pub sample_locations: Vec<String>,
}

/// Full data-quality and security validation report for an opened CSV document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub duplicate_row_count: usize,
    pub duplicate_row_indices: Vec<usize>,
    pub duplicate_headers: Vec<String>,
    pub empty_cell_count: usize,
    pub whitespace_issue_count: usize,
    pub control_char_count: usize,
    pub formula_injection_count: usize,
    pub formula_row_indices: Vec<usize>,
    pub type_outlier_count: usize,
    pub mixed_line_endings: bool,
    pub lossy_decode: bool,
    pub findings: Vec<ValidationFinding>,
}

/// Validates decoded metadata, structural parsing results, recovery diagnostics, and cell data quality.
pub fn validate(
    decoded: &Decoded,
    line_endings: &LineEndingInfo,
    parsed: &ParseResult,
    rows: &[Vec<String>],
    columns: &[ColumnInfo],
    recovery: &RecoverySummary,
    has_header: bool,
) -> ValidationReport {
    let first_data = usize::from(has_header);

    // 1. Duplicate headers (case-insensitive trimmed header check)
    let mut duplicate_headers = Vec::new();
    if has_header {
        if let Some(header_row) = rows.first() {
            let mut seen_headers: HashMap<String, usize> = HashMap::new();
            for h in header_row {
                let norm = h.trim().to_ascii_lowercase();
                if norm.is_empty() {
                    continue;
                }
                let entry = seen_headers.entry(norm).or_insert(0);
                *entry += 1;
                if *entry == 2 {
                    duplicate_headers.push(h.trim().to_owned());
                }
            }
        }
    }

    // 2. Duplicate rows, empty cells, leading/trailing whitespace, control characters, formula triggers
    let mut seen_rows: HashSet<&[String]> = HashSet::with_capacity(rows.len());
    let mut duplicate_row_indices = Vec::new();
    let mut empty_cell_count = 0usize;
    let mut whitespace_issue_count = 0usize;
    let mut control_char_count = 0usize;
    let mut formula_injection_count = 0usize;
    let mut formula_row_indices: Vec<usize> = Vec::new();
    let mut formula_samples: Vec<String> = Vec::new();
    let mut control_samples: Vec<String> = Vec::new();
    let mut ws_samples: Vec<String> = Vec::new();

    for (r_idx, row) in rows.iter().enumerate() {
        if r_idx >= first_data && !seen_rows.insert(row.as_slice()) {
            duplicate_row_indices.push(r_idx);
        }

        let mut row_has_formula = false;
        for (c_idx, cell) in row.iter().enumerate() {
            if cell.is_empty() {
                empty_cell_count += 1;
                continue;
            }
            if cell.len() != cell.trim().len() {
                whitespace_issue_count += 1;
                if ws_samples.len() < 5 {
                    ws_samples.push(format!("Row {}, Col {}", r_idx + 1, c_idx + 1));
                }
            }
            if cell
                .chars()
                .any(|ch| (ch as u32) < 0x20 && !matches!(ch, '\t' | '\n' | '\r'))
            {
                control_char_count += 1;
                if control_samples.len() < 5 {
                    control_samples.push(format!("Row {}, Col {}", r_idx + 1, c_idx + 1));
                }
            }
            if r_idx >= first_data && security::detect_formula_risk(cell).is_some() {
                formula_injection_count += 1;
                row_has_formula = true;
                if formula_samples.len() < 5 {
                    formula_samples.push(format!("Row {}, Col {}", r_idx + 1, c_idx + 1));
                }
            }
        }
        if row_has_formula {
            formula_row_indices.push(r_idx);
        }
    }

    // 3. Type outliers in mostly-numeric columns (>= 85% numeric, but < 100%)
    let mut type_outlier_count = 0usize;
    let mut outlier_samples = Vec::new();
    for col in columns {
        if col.non_empty >= 4
            && col.kind == ColumnKind::Text
            && col.numeric_count > 0
            && col.numeric_count < col.non_empty
            && col.numeric_count * 100 >= col.non_empty * 85
            && col.at_risk == 0
        {
            let bad = col.non_empty - col.numeric_count;
            type_outlier_count += bad;
            if outlier_samples.len() < 5 {
                let first_bad =
                    rows.iter()
                        .enumerate()
                        .skip(first_data)
                        .find_map(|(r_idx, row)| {
                            let v = row.get(col.index).map(String::as_str).unwrap_or("");
                            if !v.trim().is_empty() && try_parse_number(v, col.mark).is_none() {
                                Some(format!("{} (Row {})", col.name, r_idx + 1))
                            } else {
                                None
                            }
                        });
                if let Some(loc) = first_bad {
                    outlier_samples.push(loc);
                }
            }
        }
    }

    let mut findings = Vec::new();

    if decoded.was_lossy || decoded.invalid_sequences > 0 {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            category: "Encoding".to_owned(),
            count: decoded.invalid_sequences.max(1),
            summary: format!(
                "{} invalid byte sequence(s) detected under {}; verify accents/characters or pick another source encoding.",
                decoded.invalid_sequences.max(1),
                decoded.encoding_name
            ),
            sample_locations: Vec::new(),
        });
    }

    if recovery.mojibake_cells_count > 0 {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Info,
            category: "Mojibake Repaired".to_owned(),
            count: recovery.mojibake_cells_count,
            summary: format!(
                "Repaired {} cell(s) containing double-encoded UTF-8 sequences (such as Ã© -> é).",
                recovery.mojibake_cells_count
            ),
            sample_locations: recovery
                .cell_repairs
                .iter()
                .take(5)
                .map(|c| format!("Row {}, Col {}", c.row + 1, c.col + 1))
                .collect(),
        });
    }

    if formula_injection_count > 0 {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Security,
            category: "Spreadsheet Formula Risk".to_owned(),
            count: formula_injection_count,
            summary: format!(
                "{formula_injection_count} cell(s) begin with formula-triggering characters (=, +, -, @ or leading whitespace before them). Enable formula neutralization in Export when sharing untrusted CSVs with Excel/LibreOffice."
            ),
            sample_locations: formula_samples,
        });
    }

    if parsed.ragged_rows > 0 || parsed.ended_inside_quote || parsed.had_stray_quotes {
        let mut details = Vec::new();
        if parsed.ragged_rows > 0 {
            details.push(format!("{} ragged row(s)", parsed.ragged_rows));
        }
        if parsed.had_stray_quotes {
            details.push(format!(
                "{} row(s) with stray quotes",
                parsed.stray_quote_rows.len().max(1)
            ));
        }
        if parsed.ended_inside_quote {
            details.push("unclosed trailing quote at EOF".to_owned());
        }
        if recovery.ambiguous_rows > 0 {
            details.push(format!(
                "{} ambiguous row(s) kept unmerged",
                recovery.ambiguous_rows
            ));
        }
        findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            category: "CSV Structure".to_owned(),
            count: parsed.ragged_rows
                + usize::from(parsed.ended_inside_quote)
                + parsed.stray_quote_rows.len(),
            summary: format!("Structural issues found: {}.", details.join("; ")),
            sample_locations: recovery
                .diagnostics
                .iter()
                .take(5)
                .map(|d| format!("Row {} (line {})", d.row_index + 1, d.source_line))
                .collect(),
        });
    }

    if !duplicate_headers.is_empty() {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            category: "Duplicate Headers".to_owned(),
            count: duplicate_headers.len(),
            summary: format!(
                "Duplicate column header name(s): {}.",
                duplicate_headers.join(", ")
            ),
            sample_locations: duplicate_headers.clone(),
        });
    }

    if !duplicate_row_indices.is_empty() {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Info,
            category: "Duplicate Rows".to_owned(),
            count: duplicate_row_indices.len(),
            summary: format!(
                "{} exact duplicate data row(s) detected (kept intact unless you edit or filter).",
                duplicate_row_indices.len()
            ),
            sample_locations: duplicate_row_indices
                .iter()
                .take(5)
                .map(|&r| format!("Row {}", r + 1))
                .collect(),
        });
    }

    if line_endings.is_mixed {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Info,
            category: "Line Endings".to_owned(),
            count: line_endings.crlf_count + line_endings.lf_count + line_endings.cr_count,
            summary: format!(
                "Mixed line endings detected (CRLF: {}, LF: {}, CR: {}).",
                line_endings.crlf_count, line_endings.lf_count, line_endings.cr_count
            ),
            sample_locations: Vec::new(),
        });
    }

    if control_char_count > 0 {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            category: "Control Characters".to_owned(),
            count: control_char_count,
            summary: format!(
                "{control_char_count} cell(s) contain non-printable ASCII control characters (stripped in .xlsx XML output because XML 1.0 forbids them)."
            ),
            sample_locations: control_samples,
        });
    }

    if type_outlier_count > 0 {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Info,
            category: "Mixed Column Types".to_owned(),
            count: type_outlier_count,
            summary: format!(
                "{type_outlier_count} non-numeric cell(s) inside mostly-numeric column(s) kept the column as Text to avoid dropping values."
            ),
            sample_locations: outlier_samples,
        });
    }

    if whitespace_issue_count > 0 {
        findings.push(ValidationFinding {
            severity: FindingSeverity::Info,
            category: "Leading/Trailing Spaces".to_owned(),
            count: whitespace_issue_count,
            summary: format!(
                "{whitespace_issue_count} cell(s) contain leading or trailing whitespace (automatically quoted on CSV export so spacing is preserved)."
            ),
            sample_locations: ws_samples,
        });
    }

    ValidationReport {
        duplicate_row_count: duplicate_row_indices.len(),
        duplicate_row_indices,
        duplicate_headers,
        empty_cell_count,
        whitespace_issue_count,
        control_char_count,
        formula_injection_count,
        formula_row_indices,
        type_outlier_count,
        mixed_line_endings: line_endings.is_mixed,
        lossy_decode: decoded.was_lossy,
        findings,
    }
}
