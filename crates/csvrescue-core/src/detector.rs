//! Automatic CSV format detection: delimiter, quote/escape style, line endings, header row, and decimal mark.

use crate::analysis::DecimalMark;
use crate::parser::{parse_with_options, ParseOptions};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Primary delimiter candidates scored during auto-detection.
pub const CANDIDATE_DELIMITERS: &[char] = &[',', ';', '\t', '|'];

/// Extended delimiter candidates inspected when primary candidates produce only 1 column.
pub const EXTENDED_DELIMITERS: &[char] = &[',', ';', '\t', '|', ':'];

/// Line-ending style detected in input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LineEnding {
    #[default]
    Crlf,
    Lf,
    Cr,
    Mixed,
    None,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crlf => "\r\n",
            Self::Lf => "\n",
            Self::Cr => "\r",
            Self::Mixed | Self::None => "\r\n",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Crlf => "CRLF (Windows)",
            Self::Lf => "LF (Unix / macOS)",
            Self::Cr => "CR (Classic Mac)",
            Self::Mixed => "Mixed line endings",
            Self::None => "Single line",
        }
    }
}

/// Summary of line endings discovered across the decoded file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineEndingInfo {
    pub dominant: LineEnding,
    pub crlf_count: usize,
    pub lf_count: usize,
    pub cr_count: usize,
    pub is_mixed: bool,
    pub ends_with_newline: bool,
}

/// Detected quoting and escaping conventions in the source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteFormatInfo {
    pub quote_char: Option<char>,
    pub doubled_quote_escapes: usize,
    pub backslash_escapes: usize,
}

/// Full structural format summary produced by the detector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatDetection {
    pub delimiter: char,
    pub delimiter_name: String,
    pub delimiter_detected: bool,
    pub line_endings: LineEndingInfo,
    pub quote_format: QuoteFormatInfo,
    pub suggested_has_header: bool,
}

/// Human-friendly name for a delimiter character.
pub fn delimiter_name(d: char) -> String {
    match d {
        ',' => "Comma".to_owned(),
        ';' => "Semicolon".to_owned(),
        '\t' => "Tab".to_owned(),
        '|' => "Pipe".to_owned(),
        ':' => "Colon".to_owned(),
        other => other.to_string(),
    }
}

/// Picks the delimiter that splits the sampled leading records into the most consistent
/// multi-column structure (2+ columns). Returns `','` for single-column or empty input.
pub fn detect_delimiter(text: &str) -> char {
    let sample = truncate_char_boundary(text, 200_000);
    if let Some((best, _)) = score_candidates(sample, CANDIDATE_DELIMITERS) {
        return best;
    }
    if let Some((best, _)) = score_candidates(sample, &[',', ';', '\t', '|', ':']) {
        return best;
    }
    ','
}

fn score_candidates(sample: &str, candidates: &[char]) -> Option<(char, f64)> {
    let mut best: Option<(char, f64)> = None;
    for &delim in candidates {
        let opts = ParseOptions {
            delimiter: delim,
            max_rows: 200,
            skip_empty_lines: true,
            ..ParseOptions::default()
        };
        let parsed = parse_with_options(sample, &opts, None);
        if parsed.rows.is_empty() {
            continue;
        }
        let Some((mode_width, mode_count)) = modal_row_width(&parsed.rows) else {
            continue;
        };
        if mode_width < 2 {
            continue;
        }
        let consistency = mode_count as f64 / parsed.rows.len() as f64;
        let bonus = if consistency >= 0.8 { 2.0 } else { 1.0 };
        let score = consistency * ((mode_width as f64) + 1.0).ln() * bonus;
        if best.map_or(true, |(_, best_score)| score > best_score + 1e-9) {
            best = Some((delim, score));
        }
    }
    best
}

pub(crate) fn modal_row_width(rows: &[Vec<String>]) -> Option<(usize, usize)> {
    if rows.is_empty() {
        return None;
    }
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for row in rows {
        *counts.entry(row.len()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by(|(w1, c1), (w2, c2)| c1.cmp(c2).then_with(|| w1.cmp(w2)))
}

fn truncate_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}

/// Counts CRLF, LF, and bare CR line endings outside or across the text stream.
pub fn detect_line_endings(text: &str) -> LineEndingInfo {
    let bytes = text.as_bytes();
    let mut crlf = 0usize;
    let mut lf = 0usize;
    let mut cr = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    crlf += 1;
                    i += 2;
                } else {
                    cr += 1;
                    i += 1;
                }
            }
            b'\n' => {
                lf += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }

    let kinds = (crlf > 0) as u8 + (lf > 0) as u8 + (cr > 0) as u8;
    let is_mixed = kinds > 1;
    let dominant = if kinds == 0 {
        LineEnding::None
    } else if is_mixed {
        if crlf >= lf && crlf >= cr {
            LineEnding::Crlf
        } else if lf >= cr {
            LineEnding::Lf
        } else {
            LineEnding::Cr
        }
    } else if crlf > 0 {
        LineEnding::Crlf
    } else if lf > 0 {
        LineEnding::Lf
    } else {
        LineEnding::Cr
    };

    let ends_with_newline = text.ends_with('\n') || text.ends_with('\r');
    LineEndingInfo {
        dominant,
        crlf_count: crlf,
        lf_count: lf,
        cr_count: cr,
        is_mixed,
        ends_with_newline,
    }
}

/// Inspects quoting and escaping signals inside the text.
pub fn detect_quote_format(text: &str) -> QuoteFormatInfo {
    let sample = truncate_char_boundary(text, 200_000);
    let bytes = sample.as_bytes();
    let mut quotes = 0usize;
    let mut doubled = 0usize;
    let mut backslash_escaped = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                quotes += 1;
                if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    doubled += 1;
                    i += 2;
                    continue;
                }
            }
            b'\\' if i + 1 < bytes.len() && bytes[i + 1] == b'"' => {
                backslash_escaped += 1;
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    QuoteFormatInfo {
        quote_char: if quotes > 0 { Some('"') } else { None },
        doubled_quote_escapes: doubled,
        backslash_escapes: backslash_escaped,
    }
}

/// Heuristically determines whether row 0 looks like a header row rather than data.
pub fn detect_header(rows: &[Vec<String>]) -> bool {
    if rows.len() < 2 {
        return !rows.is_empty();
    }
    let header = &rows[0];
    let non_empty_header: Vec<&str> = header
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if non_empty_header.is_empty() {
        return false;
    }

    // Header cells are usually unique non-numeric labels while subsequent rows contain numbers or varied values.
    let header_numeric = non_empty_header
        .iter()
        .filter(|s| {
            s.chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == '-')
        })
        .count();
    let distinct: HashSet<&str> = non_empty_header.iter().copied().collect();
    if header_numeric == 0 && distinct.len() == non_empty_header.len() {
        return true;
    }

    // Check if body rows contain numeric columns where header cell is non-numeric text
    let mut text_over_number = 0usize;
    for (col_idx, h) in header.iter().enumerate() {
        let ht = h.trim();
        let h_is_alpha = ht.chars().any(|c| c.is_alphabetic());
        if !h_is_alpha {
            continue;
        }
        let body_num = rows[1..]
            .iter()
            .take(25)
            .filter_map(|r| r.get(col_idx))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .any(|s| {
                s.chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == '-')
            });
        if body_num {
            text_over_number += 1;
        }
    }
    text_over_number > 0
}

/// Detects whether numeric values in the dataset use decimal comma or decimal dot.
///
/// Ties fall back to `Comma` for semicolon-delimited files and `Dot` otherwise.
pub fn detect_decimal_mark<'a>(
    values: impl Iterator<Item = &'a str>,
    delimiter: char,
) -> DecimalMark {
    let mut comma = 0usize;
    let mut dot = 0usize;

    for raw in values {
        let s = raw.trim();
        if is_comma_decimal_evidence(s) {
            comma += 1;
        } else if is_dot_decimal_evidence(s) {
            dot += 1;
        }
    }

    if comma == dot {
        if delimiter == ';' {
            DecimalMark::Comma
        } else {
            DecimalMark::Dot
        }
    } else if comma > dot {
        DecimalMark::Comma
    } else {
        DecimalMark::Dot
    }
}

fn is_comma_decimal_evidence(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    if let Some((int_part, frac_part)) = body.split_once(',') {
        if !frac_part.is_empty()
            && frac_part.bytes().all(|b| b.is_ascii_digit())
            && (frac_part.len() != 3 || is_dot_thousands_int(int_part))
            && (is_plain_digits(int_part) || is_dot_thousands_int(int_part))
        {
            return true;
        }
    }
    false
}

fn is_dot_decimal_evidence(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    if let Some((int_part, frac_part)) = body.split_once('.') {
        if !frac_part.is_empty()
            && frac_part.bytes().all(|b| b.is_ascii_digit())
            && (frac_part.len() != 3 || is_comma_thousands_int(int_part))
            && (is_plain_digits(int_part) || is_comma_thousands_int(int_part))
        {
            return true;
        }
    }
    false
}

fn is_plain_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_grouped_thousands(s: &str, sep: u8) -> bool {
    let mut groups = s.as_bytes().split(|&b| b == sep);
    let Some(first) = groups.next() else {
        return false;
    };
    if !(1..=3).contains(&first.len()) || !first.iter().all(u8::is_ascii_digit) {
        return false;
    }
    let mut count = 0usize;
    for g in groups {
        count += 1;
        if g.len() != 3 || !g.iter().all(u8::is_ascii_digit) {
            return false;
        }
    }
    count >= 1
}

fn is_dot_thousands_int(s: &str) -> bool {
    is_grouped_thousands(s, b'.')
}

fn is_comma_thousands_int(s: &str) -> bool {
    is_grouped_thousands(s, b',')
}
