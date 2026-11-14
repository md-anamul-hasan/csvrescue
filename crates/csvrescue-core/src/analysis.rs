//! Column type classification, strict numeric parsing, and Excel corruption risk detection.

use crate::detector::detect_decimal_mark;
use crate::security;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Column export representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColumnKind {
    #[default]
    Text,
    Number,
}

impl ColumnKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Number => "Number",
        }
    }
}

/// Decimal mark convention for numeric columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DecimalMark {
    #[default]
    Auto,
    Dot,
    Comma,
}

/// Bitmask describing why a cell value would be silently altered if Excel opens it from plain CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Risk(pub u32);

impl Risk {
    pub const NONE: Self = Self(0);
    pub const LEADING_ZEROS: Self = Self(1 << 0);
    pub const LONG_NUMBER: Self = Self(1 << 1);
    pub const SCIENTIFIC_LOOK: Self = Self(1 << 2);
    pub const DATE_LOOK: Self = Self(1 << 3);
    pub const FRACTION_LOOK: Self = Self(1 << 4);

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Per-column schema, statistics, and corruption risk metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub index: usize,
    pub name: String,
    pub kind: ColumnKind,
    pub detected_kind: ColumnKind,
    pub mark: DecimalMark,
    pub at_risk: usize,
    pub risks: Risk,
    pub non_empty: usize,
    pub empty_count: usize,
    pub unique_count: usize,
    pub numeric_count: usize,
    pub formula_risk_count: usize,
    pub sample_values: Vec<String>,
}

impl ColumnInfo {
    pub fn risk_summary(&self) -> String {
        risk_text(self.risks)
    }
}

/// Evaluates whether Excel would silently coerce `value` when opening a CSV directly.
pub fn risk_of(value: &str) -> Risk {
    let s = value.trim();
    if s.is_empty() {
        return Risk::NONE;
    }
    let mut r = Risk::NONE;
    let unsigned = s.strip_prefix('-').unwrap_or(s);
    if unsigned.len() > 1
        && unsigned.starts_with('0')
        && unsigned.bytes().all(|b| b.is_ascii_digit())
    {
        r.insert(Risk::LEADING_ZEROS);
    }
    if unsigned.len() > 15 && unsigned.bytes().all(|b| b.is_ascii_digit()) {
        r.insert(Risk::LONG_NUMBER);
    }
    if is_scientific_look(s) {
        r.insert(Risk::SCIENTIFIC_LOOK);
    }
    if is_date_like(s) && !looks_like_plain_float(s) {
        r.insert(Risk::DATE_LOOK);
    }
    if is_fraction_look(s) {
        r.insert(Risk::FRACTION_LOOK);
    }
    r
}

fn is_scientific_look(s: &str) -> bool {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    let Some(e_pos) = body.find(['e', 'E']) else {
        return false;
    };
    let (mantissa, exp_part) = (&body[..e_pos], &body[e_pos + 1..]);
    if mantissa.is_empty() || exp_part.is_empty() {
        return false;
    }
    let mantissa_ok = match mantissa.split_once(['.', ',']) {
        Some((l, r)) => !l.is_empty() && !r.is_empty() && is_digits(l) && is_digits(r),
        None => is_digits(mantissa),
    };
    let exp_digits = exp_part.strip_prefix(['+', '-']).unwrap_or(exp_part);
    mantissa_ok && is_digits(exp_digits)
}

fn is_date_like(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut parts = Vec::with_capacity(3);
    let mut start = 0usize;
    for (idx, &b) in bytes.iter().enumerate() {
        if matches!(b, b'-' | b'/' | b'.') {
            parts.push(&s[start..idx]);
            start = idx + 1;
        }
    }
    parts.push(&s[start..]);
    if !(2..=3).contains(&parts.len()) {
        return false;
    }
    if !(1..=4).contains(&parts[0].len()) || !is_digits(parts[0]) {
        return false;
    }
    if !(1..=2).contains(&parts[1].len()) || !is_digits(parts[1]) {
        return false;
    }
    if parts.len() == 3 && (!(1..=4).contains(&parts[2].len()) || !is_digits(parts[2])) {
        return false;
    }
    true
}

fn looks_like_plain_float(s: &str) -> bool {
    if s.contains('-') || s.contains('/') || s.bytes().filter(|&b| b == b'.').count() > 1 {
        return false;
    }
    s.parse::<f64>().is_ok()
}

fn is_fraction_look(s: &str) -> bool {
    let Some((left, right)) = s.split_once('/') else {
        return false;
    };
    let l = left.trim_end();
    let r = right.trim_start();
    is_digits(l) && is_digits(r)
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Parses a clean number (no leading zeros, at most 15 ASCII digits, no `+` sign, no exponent).
/// Returns normalized invariant decimal notation (preserving exact decimal digits like `"1234.50"`).
pub fn try_parse_number(value: &str, mark: DecimalMark) -> Option<String> {
    let cleaned: String = value
        .trim()
        .chars()
        .filter(|&c| !matches!(c, '\u{00A0}' | '\u{202F}' | ' ' | '\''))
        .collect();
    if cleaned.is_empty() {
        return None;
    }

    let valid_pattern = match mark {
        DecimalMark::Comma => matches_comma_number(&cleaned),
        DecimalMark::Dot | DecimalMark::Auto => matches_dot_number(&cleaned),
    };
    if !valid_pattern {
        return None;
    }

    let mut norm = if mark == DecimalMark::Comma {
        cleaned.replace('.', "").replace(',', ".")
    } else {
        cleaned.replace(',', "")
    };
    if norm.starts_with('.') {
        norm.insert(0, '0');
    } else if norm.starts_with("-.") {
        norm.insert(1, '0');
    }

    let digit_count = norm.bytes().filter(u8::is_ascii_digit).count();
    if digit_count == 0 || digit_count > 15 {
        return None;
    }
    if norm.parse::<f64>().is_err() {
        return None;
    }
    Some(norm)
}

fn matches_dot_number(s: &str) -> bool {
    matches_number_with_seps(s, '.', ',')
}

fn matches_comma_number(s: &str) -> bool {
    matches_number_with_seps(s, ',', '.')
}

fn matches_number_with_seps(s: &str, decimal_sep: char, group_sep: char) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    if body.is_empty() {
        return false;
    }

    let (int_part, frac_part) = match body.split_once(decimal_sep) {
        Some((i, f)) => {
            if f.is_empty() || !is_digits(f) {
                return false;
            }
            if i.is_empty() {
                return true; // -.5 or .5
            }
            (i, Some(f))
        }
        None => (body, None),
    };

    if int_part == "0" {
        return true;
    }
    if int_part.contains(group_sep) {
        let mut groups = int_part.split(group_sep);
        let Some(first) = groups.next() else {
            return false;
        };
        if first.is_empty() || first.len() > 3 || first.starts_with('0') || !is_digits(first) {
            return false;
        }
        let mut rest_count = 0usize;
        for g in groups {
            rest_count += 1;
            if g.len() != 3 || !is_digits(g) {
                return false;
            }
        }
        return rest_count >= 1;
    }

    // Plain integer without thousands separator: no leading zero unless single "0"
    let _ = frac_part;
    !int_part.starts_with('0') && is_digits(int_part)
}

/// Analyzes columns across parsed rows, inferring `ColumnKind`, `DecimalMark`, Excel risks, and summaries.
pub fn analyze(
    rows: &[Vec<String>],
    has_header: bool,
    requested_mark: DecimalMark,
    delimiter: char,
) -> Vec<ColumnInfo> {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return Vec::new();
    }
    let first = usize::from(has_header);

    let file_mark = match requested_mark {
        DecimalMark::Auto => {
            let sample_vals = rows
                .iter()
                .skip(first)
                .flat_map(|r| r.iter().map(String::as_str))
                .take(200_000);
            detect_decimal_mark(sample_vals, delimiter)
        }
        explicit => explicit,
    };

    let mut cols = Vec::with_capacity(width);
    for col_idx in 0..width {
        let name = if has_header {
            rows.first()
                .and_then(|r| r.get(col_idx))
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("Column {}", col_idx + 1))
        } else {
            format!("Column {}", col_idx + 1)
        };

        let mut non_empty = 0usize;
        let mut empty_count = 0usize;
        let mut at_risk = 0usize;
        let mut risks = Risk::NONE;
        let mut numeric = true;
        let mut numeric_count = 0usize;
        let mut formula_risk_count = 0usize;
        let mut unique: HashSet<&str> = HashSet::new();
        let mut sample_values: Vec<String> = Vec::with_capacity(5);

        for row in rows.iter().skip(first) {
            let v = row.get(col_idx).map(String::as_str).unwrap_or("");
            if v.trim().is_empty() {
                empty_count += 1;
                continue;
            }
            non_empty += 1;
            if unique.len() < 10_000 {
                unique.insert(v);
            }
            if sample_values.len() < 4 && !sample_values.iter().any(|s| s == v) {
                sample_values.push(v.to_owned());
            }
            let rk = risk_of(v);
            if !rk.is_empty() {
                at_risk += 1;
                risks = risks.union(rk);
            }
            if security::detect_formula_risk(v).is_some() {
                formula_risk_count += 1;
            }
            if try_parse_number(v, file_mark).is_some() {
                numeric_count += 1;
            } else {
                numeric = false;
            }
        }

        let detected_kind = if numeric && non_empty > 0 && risks.is_empty() {
            ColumnKind::Number
        } else {
            ColumnKind::Text
        };

        cols.push(ColumnInfo {
            index: col_idx,
            name,
            kind: detected_kind,
            detected_kind,
            mark: file_mark,
            at_risk,
            risks,
            non_empty,
            empty_count,
            unique_count: unique.len(),
            numeric_count,
            formula_risk_count,
            sample_values,
        });
    }
    cols
}

/// Formats human-readable labels for active Excel risks in a column or cell.
pub fn risk_text(r: Risk) -> String {
    let mut parts = Vec::new();
    if r.contains(Risk::LEADING_ZEROS) {
        parts.push("leading zeros");
    }
    if r.contains(Risk::LONG_NUMBER) {
        parts.push("numbers over 15 digits");
    }
    if r.contains(Risk::SCIENTIFIC_LOOK) {
        parts.push("look like 1E5");
    }
    if r.contains(Risk::DATE_LOOK) {
        parts.push("look like dates");
    }
    if r.contains(Risk::FRACTION_LOOK) {
        parts.push("look like fractions");
    }
    parts.join(", ")
}
