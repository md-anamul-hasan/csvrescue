//! High-level `CsvDocument` orchestration, settings, built-in sample fixtures, and manual edits.

use crate::analysis::{self, ColumnInfo, ColumnKind, DecimalMark};
use crate::detector::{
    self, detect_delimiter, detect_header, detect_line_endings, detect_quote_format,
    FormatDetection,
};
use crate::encoding::{self, mojibake_count, repair_double_encoding, Decoded};
use crate::errors::Result;
use crate::parser::{self, EscapeMode, ParseOptions, ParseResult};
use crate::recovery::{RecoverySummary, RepairStrategy};
use crate::repair::{self, RepairInput};
use crate::security;
use crate::validation::{self, ValidationReport};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Built-in sample datasets so users can try CSVRescue's diagnostic & recovery flows immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SamplePreset {
    EuropeanErp,
    MojibakeAndQuotes,
    FormulaAndTypes,
}

impl SamplePreset {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::EuropeanErp => "sample-kunden-export.csv",
            Self::MojibakeAndQuotes => "sample-mojibake-ragged.csv",
            Self::FormulaAndTypes => "sample-security-audit.csv",
        }
    }

    pub fn bytes(self) -> Vec<u8> {
        match self {
            Self::EuropeanErp => {
                let text = concat!(
                    "Kundennummer;Name;Stadt;Umsatz;Datum;Artikelnummer\r\n",
                    "00123;Müller GmbH;Köln;1.234,50;03.10.2026;00450-A\r\n",
                    "00124;Schäfer & Söhne;München;987,00;04.10.2026;00451-B\r\n",
                    "00125;Café Zürich;Zürich;15.320,75;05.10.2026;00452-C\r\n",
                    "00126;Östlund AB;Göteborg;2.499,99;06.10.2026;00453-D\r\n",
                    "00127;Nuñez y Hijos;Sevilla;310,40;07.10.2026;00454-E\r\n",
                    "00128;Dvořák s.r.o.;Praha;7.805,10;08.10.2026;00455-F\r\n",
                    "00129;Lukasz Sp. z o.o.;Kraków;1.020,00;09.10.2026;00456-G\r\n",
                    "00130;Renée & Fils;Lyon;4.512,30;10.10.2026;00457-H\r\n"
                );
                let mut out = Vec::with_capacity(text.len());
                for ch in text.chars() {
                    out.push(encoding::char_to_win1252(ch).unwrap_or(b'?'));
                }
                out
            }
            Self::MojibakeAndQuotes => concat!(
                "sku,description,qty,unit_price,notes\n",
                "00810,CafÃ© espresso machine,2,429.90,\"Delivered in MÃ¼nchen\"\n",
                "00811,5\" stainless flange,14,18.50,\"Unquoted inch mark\"\n",
                "00812,Precision kit, 1E5 rating,3,99.00,Comma inside unquoted field\n",
                "00813,NaÃ¯ve rÃ©sumÃ© pack,5,12.00,\"Multi-line\nnote inside quotes\"\n",
                "00814,Truncated trailing row\n"
            )
            .as_bytes()
            .to_vec(),
            Self::FormulaAndTypes => concat!(
                "account_id,customer,balance,formula_check,fraction_code\r\n",
                "12345678901234567,Acme Holdings,-124.50,=CMD|' /C calc'!A0,1/2\r\n",
                "00094812,Nordic Supply,842.00,+SUM(1,2),3-4\r\n",
                "00094813,Delta Logistics,150.25,@SUM(1+1)*cmd,1E5\r\n",
                "00094814,Beacon Retail,0.00,\t=IMPORTXML(),0042\r\n"
            )
            .as_bytes()
            .to_vec(),
        }
    }
}

/// Settings governing how a CSV file is decoded, parsed, and recovered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvSettings {
    pub encoding: String,
    pub delimiter: Option<char>,
    pub mark: DecimalMark,
    pub has_header: bool,
    pub repair_double_encoding: bool,
    pub skip_empty_lines: bool,
    pub escape_mode: EscapeMode,
    pub repair_strategy: RepairStrategy,
}

impl Default for CsvSettings {
    fn default() -> Self {
        Self {
            encoding: "Auto".to_owned(),
            delimiter: None,
            mark: DecimalMark::Auto,
            has_header: true,
            repair_double_encoding: true,
            skip_empty_lines: true,
            escape_mode: EscapeMode::DoubledQuote,
            repair_strategy: RepairStrategy::Preserve,
        }
    }
}

/// In-memory opened CSV document preserving raw source bytes alongside recovered structure.
#[derive(Debug, Clone)]
pub struct CsvDocument {
    pub path: String,
    pub is_sample: bool,
    pub source_bytes_len: usize,
    pub raw_bytes: Vec<u8>,
    pub decoded: Decoded,
    pub text: String,
    pub delimiter: char,
    pub delimiter_detected: bool,
    pub format: FormatDetection,
    pub parsed: ParseResult,
    pub original_rows: Vec<Vec<String>>,
    pub recovered_rows: Vec<Vec<String>>,
    pub columns: Vec<ColumnInfo>,
    pub settings: CsvSettings,
    pub repaired_markers: usize,
    pub recovery: RecoverySummary,
    pub validation: ValidationReport,
    pub manual_edits: BTreeMap<(usize, usize), String>,
}

/// Serializable summary snapshot used by recovery reports and UI telemetry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummaryDto {
    pub path: String,
    pub file_name: String,
    pub is_sample: bool,
    pub source_bytes_len: usize,
    pub data_rows: usize,
    pub total_rows: usize,
    pub column_count: usize,
    pub encoding_name: String,
    pub had_bom: bool,
    pub encoding_note: String,
    pub was_lossy: bool,
    pub invalid_sequences: usize,
    pub delimiter: char,
    pub delimiter_name: String,
    pub delimiter_detected: bool,
    pub decimal_mark: DecimalMark,
    pub line_ending_label: String,
    pub mixed_line_endings: bool,
    pub suggested_has_header: bool,
    pub repaired_markers: usize,
    pub cells_at_risk: usize,
    pub risky_columns: usize,
    pub formula_risks: usize,
    pub duplicate_rows: usize,
    pub ragged_rows: usize,
    pub empty_lines: usize,
    pub had_stray_quotes: bool,
    pub ended_inside_quote: bool,
    pub ambiguous_rows: usize,
    pub manual_edits_count: usize,
    pub can_export_xlsx: bool,
    pub problems: Vec<String>,
    pub recovery: RecoverySummary,
    pub validation: ValidationReport,
    pub columns: Vec<ColumnInfo>,
}

impl CsvDocument {
    /// Opens a CSV or delimited text file on disk without ever modifying the source file.
    pub fn load(path: &Path, settings: CsvSettings) -> Result<Self> {
        security::validate_source_file(path)?;
        let bytes =
            fs::read(path).map_err(|e| crate::errors::Error::io("Could not read the file", e))?;
        Self::from_bytes(&path.to_string_lossy(), &bytes, settings, false, None)
    }

    /// Loads one of the built-in diagnostic sample presets.
    pub fn from_sample_preset(preset: SamplePreset, settings: CsvSettings) -> Result<Self> {
        Self::from_bytes(preset.display_name(), &preset.bytes(), settings, true, None)
    }

    /// Re-parses the original raw bytes under updated `settings`, preserving manual edits and matching column overrides.
    pub fn update_settings(&mut self, settings: CsvSettings) -> Result<()> {
        let prev_overrides: Vec<Option<ColumnKind>> = self
            .columns
            .iter()
            .map(|c| (c.kind != c.detected_kind).then_some(c.kind))
            .collect();
        let mut updated = Self::from_bytes_with_edits(
            &self.path,
            &self.raw_bytes,
            settings,
            self.is_sample,
            self.manual_edits.clone(),
            None,
        )?;
        if prev_overrides.len() == updated.columns.len() {
            for (idx, col) in updated.columns.iter_mut().enumerate() {
                if let Some(kind) = prev_overrides[idx] {
                    col.kind = kind;
                }
            }
        }
        *self = updated;
        Ok(())
    }

    /// Parses and analyzes raw bytes with the supplied settings and optional manual edits.
    pub fn from_bytes(
        path: &str,
        data: &[u8],
        settings: CsvSettings,
        is_sample: bool,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        Self::from_bytes_with_edits(path, data, settings, is_sample, BTreeMap::new(), cancel)
    }

    pub fn from_bytes_with_edits(
        path: &str,
        data: &[u8],
        settings: CsvSettings,
        is_sample: bool,
        manual_edits: BTreeMap<(usize, usize), String>,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        let forced = if settings.encoding == "Auto" {
            None
        } else {
            Some(settings.encoding.as_str())
        };
        let decoded = encoding::decode(data, forced)?;

        let mut text = decoded.text.clone();
        let mut repaired_markers = 0usize;
        if settings.repair_double_encoding {
            repaired_markers = mojibake_count(&text);
            if repaired_markers > 0 {
                text = repair_double_encoding(&text);
            }
        }

        let delimiter_detected = settings.delimiter.is_none();
        let delimiter = settings
            .delimiter
            .unwrap_or_else(|| detect_delimiter(&text));
        let line_endings = detect_line_endings(&decoded.text);
        let quote_format = detect_quote_format(&text);

        let parse_opts = ParseOptions {
            delimiter,
            quote_char: Some('"'),
            escape_mode: settings.escape_mode,
            max_rows: usize::MAX,
            skip_empty_lines: settings.skip_empty_lines,
        };

        let parsed = parser::parse_with_options(&text, &parse_opts, cancel);
        if cancel.is_some_and(|f| f.load(std::sync::atomic::Ordering::Relaxed)) {
            return Err(crate::errors::Error::Cancelled);
        }

        let original_rows = if repaired_markers > 0 {
            parser::parse_with_options(&decoded.text, &parse_opts, cancel).rows
        } else {
            parsed.rows.clone()
        };

        let suggested_has_header = detect_header(&parsed.rows);
        let format = FormatDetection {
            delimiter,
            delimiter_name: detector::delimiter_name(delimiter),
            delimiter_detected,
            line_endings,
            quote_format,
            suggested_has_header,
        };

        let mut doc = Self {
            path: path.to_owned(),
            is_sample,
            source_bytes_len: data.len(),
            raw_bytes: data.to_vec(),
            decoded,
            text,
            delimiter,
            delimiter_detected,
            format,
            parsed,
            original_rows,
            recovered_rows: Vec::new(),
            columns: Vec::new(),
            settings,
            repaired_markers,
            recovery: RecoverySummary {
                strategy: RepairStrategy::Preserve,
                modal_width: 0,
                max_width: 0,
                short_rows: 0,
                wide_rows: 0,
                aligned_rows: 0,
                ambiguous_rows: 0,
                manual_edits_count: 0,
                mojibake_cells_count: 0,
                diagnostics: Vec::new(),
                cell_repairs: Vec::new(),
            },
            validation: ValidationReport {
                duplicate_row_count: 0,
                duplicate_row_indices: Vec::new(),
                duplicate_headers: Vec::new(),
                empty_cell_count: 0,
                whitespace_issue_count: 0,
                control_char_count: 0,
                formula_injection_count: 0,
                formula_row_indices: Vec::new(),
                type_outlier_count: 0,
                mixed_line_endings: false,
                lossy_decode: false,
                findings: Vec::new(),
            },
            manual_edits,
        };

        doc.recompute_analysis();
        Ok(doc)
    }

    /// Recomputes repairs, column types, and validation after strategy changes or manual cell edits.
    pub fn recompute_analysis(&mut self) {
        let initial_mark = if self.settings.mark == DecimalMark::Auto {
            let first = usize::from(self.settings.has_header);
            let sample = self
                .parsed
                .rows
                .iter()
                .skip(first)
                .flat_map(|r| r.iter().map(String::as_str))
                .take(50_000);
            detector::detect_decimal_mark(sample, self.delimiter)
        } else {
            self.settings.mark
        };

        let repaired = repair::apply_repairs(&RepairInput {
            original_rows: &self.original_rows,
            base_rows: &self.parsed.rows,
            row_start_lines: &self.parsed.row_start_lines,
            stray_quote_rows: &self.parsed.stray_quote_rows,
            ended_inside_quote: self.parsed.ended_inside_quote,
            modal_width: self.parsed.modal_width,
            delimiter: self.delimiter,
            has_header: self.settings.has_header,
            decimal_mark: initial_mark,
            strategy: self.settings.repair_strategy,
            manual_edits: &self.manual_edits,
        });

        self.recovered_rows = repaired.rows;
        self.recovery = repaired.summary;

        // Preserve any explicit column kind overrides when width matches
        let prev_overrides: Vec<Option<ColumnKind>> = self
            .columns
            .iter()
            .map(|c| (c.kind != c.detected_kind).then_some(c.kind))
            .collect();
        self.columns = analysis::analyze(
            &self.recovered_rows,
            self.settings.has_header,
            self.settings.mark,
            self.delimiter,
        );
        if prev_overrides.len() == self.columns.len() {
            for (idx, col) in self.columns.iter_mut().enumerate() {
                if let Some(kind) = prev_overrides[idx] {
                    col.kind = kind;
                }
            }
        }

        self.revalidate();
    }

    fn revalidate(&mut self) {
        self.validation = validation::validate(
            &self.decoded,
            &self.format.line_endings,
            &self.parsed,
            &self.recovered_rows,
            &self.columns,
            &self.recovery,
            self.settings.has_header,
        );
    }

    pub fn set_column_kind(&mut self, col_idx: usize, kind: ColumnKind) -> bool {
        if let Some(col) = self.columns.get_mut(col_idx) {
            col.kind = kind;
            self.revalidate();
            true
        } else {
            false
        }
    }

    pub fn set_all_columns_kind(&mut self, kind: ColumnKind) {
        for col in &mut self.columns {
            col.kind = kind;
        }
        self.revalidate();
    }

    pub fn reset_columns_auto(&mut self) {
        for col in &mut self.columns {
            col.kind = col.detected_kind;
        }
        self.revalidate();
    }

    pub fn set_manual_cell_edit(&mut self, row: usize, col: usize, value: String) {
        if let Some(orig_row) = self.parsed.rows.get(row) {
            let base_val = orig_row.get(col).map(String::as_str).unwrap_or("");
            if base_val == value && self.settings.repair_strategy == RepairStrategy::Preserve {
                self.manual_edits.remove(&(row, col));
            } else {
                self.manual_edits.insert((row, col), value);
            }
        } else {
            self.manual_edits.insert((row, col), value);
        }
        self.recompute_analysis();
    }

    pub fn revert_manual_cell_edit(&mut self, row: usize, col: usize) -> bool {
        if self.manual_edits.remove(&(row, col)).is_some() {
            self.recompute_analysis();
            true
        } else {
            false
        }
    }

    pub fn rows(&self) -> &[Vec<String>] {
        &self.recovered_rows
    }

    pub fn data_rows(&self) -> usize {
        self.recovered_rows
            .len()
            .saturating_sub(usize::from(self.settings.has_header))
    }

    pub fn cells_at_risk(&self) -> usize {
        self.columns
            .iter()
            .filter(|c| c.kind == ColumnKind::Text)
            .map(|c| c.at_risk)
            .sum()
    }

    pub fn suggested_name(&self, extension: &str) -> PathBuf {
        let p = Path::new(&self.path);
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "rescued".to_owned());
        let file_name = format!("{stem}-clean{extension}");
        if self.is_sample {
            return PathBuf::from(file_name);
        }
        match p.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir.join(file_name),
            _ => PathBuf::from(file_name),
        }
    }

    pub fn is_same_file(&self, other: &Path) -> bool {
        !self.is_sample && security::is_same_file(Path::new(&self.path), other)
    }

    pub fn problems_list(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.repaired_markers > 0 {
            p.push(format!(
                "Repaired {} garbled letter group(s) (text saved as UTF-8 twice, such as é shown as Ã©).",
                self.repaired_markers
            ));
        }
        if self.parsed.ragged_rows > 0 {
            p.push(format!(
                "{} row(s) have a different number of columns than most rows.",
                self.parsed.ragged_rows
            ));
        }
        if self.recovery.ambiguous_rows > 0 {
            p.push(format!(
                "{} ragged row(s) have ambiguous column splits and were kept unmerged for manual review.",
                self.recovery.ambiguous_rows
            ));
        }
        if self.parsed.empty_lines > 0 {
            p.push(format!(
                "{} empty line(s) skipped.",
                self.parsed.empty_lines
            ));
        }
        if self.parsed.had_stray_quotes {
            p.push("Quotation marks in the middle of fields were kept as text.".to_owned());
        }
        if self.parsed.ended_inside_quote {
            p.push("The file ends inside a quoted field: a closing quote is missing.".to_owned());
        }
        if self.decoded.was_lossy {
            p.push(format!(
                "{} invalid byte sequence(s) were encountered during decoding ({}).",
                self.decoded.invalid_sequences, self.decoded.encoding_name
            ));
        }
        let risky_cols = self
            .columns
            .iter()
            .filter(|c| c.kind == ColumnKind::Text && c.at_risk > 0)
            .count();
        if risky_cols > 0 {
            p.push(format!(
                "Excel would change {} cell(s) in {} column(s) when it opens this file as CSV (leading zeros, long numbers, date-like or 1E5-like values). CSVRescue keeps them as text.",
                self.cells_at_risk(),
                risky_cols
            ));
        }
        if self.validation.formula_injection_count > 0 {
            p.push(format!(
                "{} cell(s) start with spreadsheet formula triggers (=, +, -, @).",
                self.validation.formula_injection_count
            ));
        }
        p
    }

    pub fn summary_dto(&self) -> DocumentSummaryDto {
        let file_name = Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone());
        let risky_columns = self
            .columns
            .iter()
            .filter(|c| c.kind == ColumnKind::Text && c.at_risk > 0)
            .count();
        let decimal_mark = self
            .columns
            .first()
            .map(|c| c.mark)
            .unwrap_or(self.settings.mark);

        DocumentSummaryDto {
            path: self.path.clone(),
            file_name,
            is_sample: self.is_sample,
            source_bytes_len: self.source_bytes_len,
            data_rows: self.data_rows(),
            total_rows: self.recovered_rows.len(),
            column_count: self.columns.len(),
            encoding_name: self.decoded.encoding_name.clone(),
            had_bom: self.decoded.had_bom,
            encoding_note: self.decoded.note.clone(),
            was_lossy: self.decoded.was_lossy,
            invalid_sequences: self.decoded.invalid_sequences,
            delimiter: self.delimiter,
            delimiter_name: self.format.delimiter_name.clone(),
            delimiter_detected: self.delimiter_detected,
            decimal_mark,
            line_ending_label: self.format.line_endings.dominant.label().to_owned(),
            mixed_line_endings: self.format.line_endings.is_mixed,
            suggested_has_header: self.format.suggested_has_header,
            repaired_markers: self.repaired_markers,
            cells_at_risk: self.cells_at_risk(),
            risky_columns,
            formula_risks: self.validation.formula_injection_count,
            duplicate_rows: self.validation.duplicate_row_count,
            ragged_rows: self.parsed.ragged_rows,
            empty_lines: self.parsed.empty_lines,
            had_stray_quotes: self.parsed.had_stray_quotes,
            ended_inside_quote: self.parsed.ended_inside_quote,
            ambiguous_rows: self.recovery.ambiguous_rows,
            manual_edits_count: self.recovery.manual_edits_count,
            can_export_xlsx: self.recovered_rows.len() <= crate::export::EXCEL_MAX_ROWS,
            problems: self.problems_list(),
            recovery: self.recovery.clone(),
            validation: self.validation.clone(),
            columns: self.columns.clone(),
        }
    }

    pub fn report_snapshot(&self) -> DocumentSummaryDto {
        self.summary_dto()
    }
}
