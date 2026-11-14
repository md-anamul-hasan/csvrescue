//! RFC-4180-compatible and recovery-tolerant CSV parser with streaming and cancellation support.

use crate::detector::modal_row_width;
use crate::errors::{Error, Result};
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};

/// Quote escaping strategy used during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum EscapeMode {
    #[default]
    DoubledQuote,
    Backslash,
}

/// Configuration for [`parse_with_options`] and streaming parsers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOptions {
    pub delimiter: char,
    pub quote_char: Option<char>,
    pub escape_mode: EscapeMode,
    pub max_rows: usize,
    pub skip_empty_lines: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            quote_char: Some('"'),
            escape_mode: EscapeMode::DoubledQuote,
            max_rows: usize::MAX,
            skip_empty_lines: true,
        }
    }
}

/// Complete parse output including structural diagnostics without altering cell content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub rows: Vec<Vec<String>>,
    /// 1-based physical line number where each parsed record started.
    pub row_start_lines: Vec<usize>,
    pub ragged_rows: usize,
    pub empty_lines: usize,
    pub had_stray_quotes: bool,
    pub ended_inside_quote: bool,
    pub stray_quote_rows: Vec<usize>,
    pub modal_width: usize,
}

/// Parses CSV text using the default CSVRescue tolerances (`"` quotes, doubled-quote escapes,
/// skipping empty lines, unlimited rows).
pub fn parse(text: &str, delimiter: char) -> ParseResult {
    parse_with_options(
        text,
        &ParseOptions {
            delimiter,
            ..ParseOptions::default()
        },
        None,
    )
}

/// Parses CSV text with configurable options and optional cancellation flag.
pub fn parse_with_options(
    text: &str,
    opts: &ParseOptions,
    cancel: Option<&AtomicBool>,
) -> ParseResult {
    let mut state = ParserState::new(opts);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0usize;

    while i < len && state.rows.len() < opts.max_rows {
        if i & 0x3FFF == 0 {
            if let Some(flag) = cancel {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
            }
        }

        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let consumed = state.step(c, next);
        i += consumed;
    }

    state.finish()
}

/// Streams delimited records from a buffered reader in bounded memory, calling `on_row` per record.
///
/// Preserves exact quoting, multi-line fields, stray quotes, and CRLF/LF/CR behavior across chunk boundaries.
pub fn parse_reader_streaming<R: BufRead, F>(
    reader: &mut R,
    opts: &ParseOptions,
    cancel: Option<&AtomicBool>,
    mut on_row: F,
) -> Result<StreamingParseSummary>
where
    F: FnMut(usize, usize, Vec<String>) -> Result<()>,
{
    let mut state = StreamingState::new(opts);
    let mut line_buf = String::new();
    let mut row_count = 0usize;

    loop {
        if let Some(flag) = cancel {
            if flag.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
        }
        line_buf.clear();
        let bytes = reader
            .read_line(&mut line_buf)
            .map_err(|e| Error::io("Failed reading input stream", e))?;
        if bytes == 0 {
            break;
        }
        let chars: Vec<char> = line_buf.chars().collect();
        let mut i = 0usize;
        while i < chars.len() && row_count < opts.max_rows {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            let (consumed, emitted) = state.step(c, next);
            if let Some((start_line, row)) = emitted {
                on_row(row_count, start_line, row)?;
                row_count += 1;
            }
            i += consumed;
        }
    }

    let ended_inside_quote = state.in_quotes;
    if row_count < opts.max_rows {
        if let Some((start_line, row)) = state.finish_eof() {
            on_row(row_count, start_line, row)?;
            row_count += 1;
        }
    }

    Ok(StreamingParseSummary {
        total_rows: row_count,
        empty_lines: state.empty_lines,
        had_stray_quotes: state.had_stray_quotes,
        ended_inside_quote,
    })
}

/// Summary produced by [`parse_reader_streaming`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingParseSummary {
    pub total_rows: usize,
    pub empty_lines: usize,
    pub had_stray_quotes: bool,
    pub ended_inside_quote: bool,
}

struct ParserState {
    delimiter: char,
    quote_char: Option<char>,
    escape_mode: EscapeMode,
    max_rows: usize,
    skip_empty_lines: bool,

    rows: Vec<Vec<String>>,
    row_start_lines: Vec<usize>,
    stray_quote_rows: Vec<usize>,

    fields: Vec<String>,
    field_buf: String,
    in_quotes: bool,
    field_quoted: bool,
    any_content: bool,
    row_had_stray: bool,
    had_stray_quotes: bool,
    empty_lines: usize,
    pending_cr: bool,

    current_line: usize,
    record_start_line: usize,
}

impl ParserState {
    fn new(opts: &ParseOptions) -> Self {
        Self {
            delimiter: opts.delimiter,
            quote_char: opts.quote_char,
            escape_mode: opts.escape_mode,
            max_rows: opts.max_rows,
            skip_empty_lines: opts.skip_empty_lines,
            rows: Vec::new(),
            row_start_lines: Vec::new(),
            stray_quote_rows: Vec::new(),
            fields: Vec::new(),
            field_buf: String::new(),
            in_quotes: false,
            field_quoted: false,
            any_content: false,
            row_had_stray: false,
            had_stray_quotes: false,
            empty_lines: 0,
            pending_cr: false,
            current_line: 1,
            record_start_line: 1,
        }
    }

    fn end_field(&mut self) {
        self.fields.push(std::mem::take(&mut self.field_buf));
        self.field_quoted = false;
    }

    fn end_row(&mut self) {
        self.end_field();
        let blank = self.fields.len() == 1 && self.fields[0].is_empty() && !self.any_content;
        if blank {
            self.empty_lines += 1;
            if !self.skip_empty_lines {
                let row_idx = self.rows.len();
                self.rows.push(vec![String::new()]);
                self.row_start_lines.push(self.record_start_line);
                if self.row_had_stray {
                    self.stray_quote_rows.push(row_idx);
                }
            }
        } else {
            let row_idx = self.rows.len();
            self.rows.push(std::mem::take(&mut self.fields));
            self.row_start_lines.push(self.record_start_line);
            if self.row_had_stray {
                self.stray_quote_rows.push(row_idx);
            }
        }
        self.fields.clear();
        self.any_content = false;
        self.row_had_stray = false;
        self.record_start_line = self.current_line;
    }

    fn step(&mut self, c: char, next: Option<char>) -> usize {
        if self.pending_cr {
            self.pending_cr = false;
            if c == '\n' {
                return 1;
            }
        }

        if self.in_quotes {
            if self.escape_mode == EscapeMode::Backslash && c == '\\' {
                if let Some(escaped) = next {
                    if self.quote_char == Some(escaped) || escaped == '\\' {
                        self.field_buf.push(escaped);
                        if escaped == '\n' || escaped == '\r' {
                            self.current_line += 1;
                        }
                        return 2;
                    }
                }
            }
            if Some(c) == self.quote_char {
                if next == self.quote_char {
                    self.field_buf.push(c);
                    return 2;
                }
                self.in_quotes = false;
                return 1;
            }
            if c == '\r' {
                self.current_line += 1;
                if next == Some('\n') {
                    self.field_buf.push_str("\r\n");
                    return 2;
                }
            } else if c == '\n' {
                self.current_line += 1;
            }
            self.field_buf.push(c);
            return 1;
        }

        if Some(c) == self.quote_char {
            if self.field_buf.is_empty() && !self.field_quoted {
                self.in_quotes = true;
                self.field_quoted = true;
                self.any_content = true;
            } else {
                self.field_buf.push(c);
                self.had_stray_quotes = true;
                self.row_had_stray = true;
            }
            return 1;
        }

        if c == self.delimiter {
            self.any_content = true;
            self.end_field();
            return 1;
        }

        if c == '\r' {
            self.current_line += 1;
            if next == Some('\n') {
                self.end_row();
                return 2;
            }
            self.end_row();
            return 1;
        }

        if c == '\n' {
            self.current_line += 1;
            self.end_row();
            return 1;
        }

        if self.field_quoted {
            self.had_stray_quotes = true;
            self.row_had_stray = true;
        }
        self.field_buf.push(c);
        self.any_content = true;
        1
    }

    fn finish(mut self) -> ParseResult {
        let ended_inside_quote = self.in_quotes;
        if self.rows.len() < self.max_rows
            && (!self.field_buf.is_empty() || !self.fields.is_empty() || self.field_quoted)
        {
            self.end_row();
        }
        let modal_width = modal_row_width(&self.rows).map_or(0, |(w, _)| w);
        let ragged_rows = self
            .rows
            .iter()
            .filter(|row| row.len() != modal_width)
            .count();

        ParseResult {
            rows: self.rows,
            row_start_lines: self.row_start_lines,
            ragged_rows,
            empty_lines: self.empty_lines,
            had_stray_quotes: self.had_stray_quotes,
            ended_inside_quote,
            stray_quote_rows: self.stray_quote_rows,
            modal_width,
        }
    }
}

/// Lightweight streaming state that yields rows without accumulating the full dataset in RAM.
struct StreamingState {
    delimiter: char,
    quote_char: Option<char>,
    escape_mode: EscapeMode,
    skip_empty_lines: bool,

    fields: Vec<String>,
    field_buf: String,
    in_quotes: bool,
    field_quoted: bool,
    any_content: bool,
    had_stray_quotes: bool,
    empty_lines: usize,
    pending_cr: bool,
    current_line: usize,
    record_start_line: usize,
}

impl StreamingState {
    fn new(opts: &ParseOptions) -> Self {
        Self {
            delimiter: opts.delimiter,
            quote_char: opts.quote_char,
            escape_mode: opts.escape_mode,
            skip_empty_lines: opts.skip_empty_lines,
            fields: Vec::new(),
            field_buf: String::new(),
            in_quotes: false,
            field_quoted: false,
            any_content: false,
            had_stray_quotes: false,
            empty_lines: 0,
            pending_cr: false,
            current_line: 1,
            record_start_line: 1,
        }
    }

    fn emit_row(&mut self) -> Option<(usize, Vec<String>)> {
        self.fields.push(std::mem::take(&mut self.field_buf));
        self.field_quoted = false;
        let blank = self.fields.len() == 1 && self.fields[0].is_empty() && !self.any_content;
        let start_line = self.record_start_line;
        self.record_start_line = self.current_line;
        self.any_content = false;

        if blank {
            self.empty_lines += 1;
            self.fields.clear();
            if self.skip_empty_lines {
                None
            } else {
                Some((start_line, vec![String::new()]))
            }
        } else {
            Some((start_line, std::mem::take(&mut self.fields)))
        }
    }

    fn step(&mut self, c: char, next: Option<char>) -> (usize, Option<(usize, Vec<String>)>) {
        if self.pending_cr {
            self.pending_cr = false;
            if c == '\n' {
                return (1, None);
            }
        }

        if self.in_quotes {
            if self.escape_mode == EscapeMode::Backslash && c == '\\' {
                if let Some(escaped) = next {
                    if self.quote_char == Some(escaped) || escaped == '\\' {
                        self.field_buf.push(escaped);
                        return (2, None);
                    }
                }
            }
            if Some(c) == self.quote_char {
                if next == self.quote_char {
                    self.field_buf.push(c);
                    return (2, None);
                }
                self.in_quotes = false;
                return (1, None);
            }
            if c == '\r' {
                self.current_line += 1;
                if next == Some('\n') {
                    self.field_buf.push_str("\r\n");
                    return (2, None);
                }
            } else if c == '\n' {
                self.current_line += 1;
            }
            self.field_buf.push(c);
            return (1, None);
        }

        if Some(c) == self.quote_char {
            if self.field_buf.is_empty() && !self.field_quoted {
                self.in_quotes = true;
                self.field_quoted = true;
                self.any_content = true;
            } else {
                self.field_buf.push(c);
                self.had_stray_quotes = true;
            }
            return (1, None);
        }

        if c == self.delimiter {
            self.any_content = true;
            self.fields.push(std::mem::take(&mut self.field_buf));
            self.field_quoted = false;
            return (1, None);
        }

        if c == '\r' {
            self.current_line += 1;
            if next == Some('\n') {
                return (2, self.emit_row());
            }
            self.pending_cr = next.is_none();
            return (1, self.emit_row());
        }

        if c == '\n' {
            self.current_line += 1;
            return (1, self.emit_row());
        }

        if self.field_quoted {
            self.had_stray_quotes = true;
        }
        self.field_buf.push(c);
        self.any_content = true;
        (1, None)
    }

    fn finish_eof(&mut self) -> Option<(usize, Vec<String>)> {
        if !self.field_buf.is_empty() || !self.fields.is_empty() || self.field_quoted {
            self.emit_row()
        } else {
            None
        }
    }
}
