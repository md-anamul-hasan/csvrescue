//! CSV, OpenXML `.xlsx`, and JSON/Markdown recovery report exporters.

use crate::analysis::{risk_of, try_parse_number, ColumnInfo, ColumnKind};
use crate::core::CsvDocument;
use crate::detector::LineEnding;
use crate::encoding::{encode_output, OutputEncoding};
use crate::errors::{Error, Result};
use crate::security;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Excel OOXML specification hard limits.
pub const EXCEL_MAX_ROWS: usize = 1_048_576;
pub const EXCEL_MAX_COLUMNS: usize = 16_384;
pub const EXCEL_MAX_CELL_CHARS: usize = 32_767;

/// Options controlling CSV and `.xlsx` export serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub delimiter: char,
    pub encoding: OutputEncoding,
    pub line_ending: LineEnding,
    pub utf8_bom: bool,
    pub decimal_as_dot: bool,
    pub excel_text_protect: bool,
    pub sanitize_formulas: bool,
    pub has_header: bool,
    pub selected_columns: Option<Vec<usize>>,
    pub selected_rows: Option<Vec<usize>>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            encoding: OutputEncoding::Utf8,
            line_ending: LineEnding::Crlf,
            utf8_bom: true,
            decimal_as_dot: false,
            excel_text_protect: false,
            sanitize_formulas: false,
            has_header: true,
            selected_columns: None,
            selected_rows: None,
        }
    }
}

/// Statistics returned after exporting or running an export dry-run preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub rows_written: usize,
    pub columns_written: usize,
    pub bytes_written: usize,
    pub decimals_converted: usize,
    pub excel_text_wrapped: usize,
    pub formulas_sanitized: usize,
    pub lossy_chars: usize,
}

/// Dry-run preview of the first serialized CSV lines before touching disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDryRun {
    pub preview_text: String,
    pub summary: ExportSummary,
    pub warnings: Vec<String>,
}

/// Converts rows and column metadata into serialized CSV text plus conversion counters.
pub fn to_csv(
    rows: &[Vec<String>],
    columns: &[ColumnInfo],
    opts: &ExportOptions,
) -> (String, ExportSummary) {
    let active_cols = resolve_column_indices(columns.len(), opts.selected_columns.as_deref());
    let active_rows =
        resolve_row_indices(rows.len(), opts.selected_rows.as_deref(), opts.has_header);

    let est_bytes = active_rows
        .len()
        .saturating_mul(active_cols.len().max(1) * 16);
    let mut out = String::with_capacity(est_bytes);
    let mut decimals_converted = 0usize;
    let mut excel_text_wrapped = 0usize;
    let mut formulas_sanitized = 0usize;
    let line_ending = opts.line_ending.as_str();

    for &r_idx in &active_rows {
        let row = &rows[r_idx];
        let is_header = opts.has_header && r_idx == 0;

        for (out_c_idx, &c_idx) in active_cols.iter().enumerate() {
            if out_c_idx > 0 {
                out.push(opts.delimiter);
            }
            let raw = row.get(c_idx).map(String::as_str).unwrap_or("");
            let col = columns.get(c_idx);

            let converted: Option<String> = if !is_header {
                if let Some(col_info) = col {
                    if col_info.kind == ColumnKind::Number && opts.decimal_as_dot {
                        try_parse_number(raw, col_info.mark).map(|norm| {
                            if norm != raw {
                                decimals_converted += 1;
                            }
                            norm
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let mut cell_val = converted.as_deref().unwrap_or(raw).to_owned();

            let should_excel_wrap = !is_header
                && opts.excel_text_protect
                && col.is_some_and(|c| c.kind == ColumnKind::Text)
                && !risk_of(&cell_val).is_empty();

            if should_excel_wrap {
                excel_text_wrapped += 1;
                out.push_str("=\"");
                append_doubled_quotes(&mut out, &cell_val);
                out.push('"');
                continue;
            }

            if !is_header && opts.sanitize_formulas {
                let (sanitized, did_sanitize) = security::sanitize_formula_cell(&cell_val);
                if did_sanitize {
                    formulas_sanitized += 1;
                    cell_val = sanitized;
                }
            }

            append_quoted_csv_cell(&mut out, &cell_val, opts.delimiter);
        }
        out.push_str(line_ending);
    }

    let summary = ExportSummary {
        rows_written: active_rows.len(),
        columns_written: active_cols.len(),
        bytes_written: out.len(),
        decimals_converted,
        excel_text_wrapped,
        formulas_sanitized,
        lossy_chars: 0,
    };
    (out, summary)
}

fn append_doubled_quotes(out: &mut String, s: &str) {
    if !s.contains('"') {
        out.push_str(s);
        return;
    }
    for ch in s.chars() {
        if ch == '"' {
            out.push_str("\"\"");
        } else {
            out.push(ch);
        }
    }
}

fn append_quoted_csv_cell(out: &mut String, v: &str, delimiter: char) {
    let needs_quote = !v.is_empty()
        && (v.contains(delimiter)
            || v.contains('"')
            || v.contains('\n')
            || v.contains('\r')
            || v.starts_with(' ')
            || v.ends_with(' '));
    if needs_quote {
        out.push('"');
        append_doubled_quotes(out, v);
        out.push('"');
    } else {
        out.push_str(v);
    }
}

/// Computes a dry-run preview of CSV output without writing to disk.
pub fn preview_export(
    doc: &CsvDocument,
    opts: &ExportOptions,
    preview_rows: usize,
) -> ExportDryRun {
    let (_full, mut summary) = to_csv(&doc.recovered_rows, &doc.columns, opts);
    let sample_count = doc.recovered_rows.len().min(preview_rows.max(1));
    let mut sample_opts = opts.clone();
    if sample_opts.selected_rows.is_none() {
        sample_opts.selected_rows = Some((0..sample_count).collect());
    } else if let Some(ref mut sr) = sample_opts.selected_rows {
        sr.truncate(preview_rows.max(1));
    }
    let (preview_text, _) = to_csv(&doc.recovered_rows, &doc.columns, &sample_opts);

    let (_, lossy) = encode_output(&preview_text, opts.encoding, opts.utf8_bom);
    summary.lossy_chars = lossy;

    let mut warnings = Vec::new();
    if opts.excel_text_protect && summary.excel_text_wrapped > 0 {
        warnings.push(format!(
            "Excel text protection wraps {} risky cell(s) as =\"...\" for Excel; other software will see the literal equals sign and quotes.",
            summary.excel_text_wrapped
        ));
    }
    if opts.sanitize_formulas && summary.formulas_sanitized > 0 {
        warnings.push(format!(
            "Formula neutralization prefixes {} risky cell(s) with a leading apostrophe ('), which changes their exact string value to protect spreadsheet users.",
            summary.formulas_sanitized
        ));
    }
    if lossy > 0 {
        warnings.push(format!(
            "Target encoding {} cannot represent {} character(s); choose UTF-8 to avoid data loss.",
            opts.encoding.label(),
            lossy
        ));
    }

    ExportDryRun {
        preview_text,
        summary,
        warnings,
    }
}

/// Writes a clean CSV file atomically to `dest`, refusing to overwrite `source_path`.
pub fn save_csv(
    dest: &Path,
    source_path: Option<&Path>,
    rows: &[Vec<String>],
    columns: &[ColumnInfo],
    opts: &ExportOptions,
) -> Result<ExportSummary> {
    if let Some(src) = source_path {
        if security::is_same_file(src, dest) {
            return Err(Error::SameFileOverwrite(dest.display().to_string()));
        }
    }

    let (text, mut summary) = to_csv(rows, columns, opts);
    let (bytes, lossy) = encode_output(&text, opts.encoding, opts.utf8_bom);
    summary.bytes_written = bytes.len();
    summary.lossy_chars = lossy;
    security::atomic_write_bytes(dest, &bytes)?;
    Ok(summary)
}

/// Exports rows and typed columns directly to an OpenXML `.xlsx` workbook without external tools.
#[allow(clippy::too_many_arguments)]
pub fn save_xlsx(
    dest: &Path,
    source_path: Option<&Path>,
    rows: &[Vec<String>],
    columns: &[ColumnInfo],
    has_header: bool,
    sanitize_formulas: bool,
    selected_columns: Option<&[usize]>,
    cancel: Option<&AtomicBool>,
) -> Result<ExportSummary> {
    if let Some(src) = source_path {
        if security::is_same_file(src, dest) {
            return Err(Error::SameFileOverwrite(dest.display().to_string()));
        }
    }

    let active_cols = resolve_column_indices(columns.len(), selected_columns);
    if rows.len() > EXCEL_MAX_ROWS {
        return Err(Error::ExcelLimitExceeded(format!(
            "Excel holds at most 1,048,576 rows; this file has {}. Save it as CSV instead.",
            rows.len()
        )));
    }
    if active_cols.len() > EXCEL_MAX_COLUMNS {
        return Err(Error::ExcelLimitExceeded(format!(
            "Excel holds at most 16,384 columns; selected {}.",
            active_cols.len()
        )));
    }

    let (file, guard) = security::create_temp_file(dest)?;
    let write_res: Result<(usize, usize)> = (|| {
        let buf = BufWriter::with_capacity(64 * 1024, file);
        let mut zip = ZipWriter::new(buf);
        let zip_opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        add_zip_part(&mut zip, "[Content_Types].xml", CONTENT_TYPES_XML, zip_opts)?;
        add_zip_part(&mut zip, "_rels/.rels", RELS_XML, zip_opts)?;
        add_zip_part(&mut zip, "xl/workbook.xml", WORKBOOK_XML, zip_opts)?;
        add_zip_part(
            &mut zip,
            "xl/_rels/workbook.xml.rels",
            WORKBOOK_RELS_XML,
            zip_opts,
        )?;
        add_zip_part(&mut zip, "xl/styles.xml", STYLES_XML, zip_opts)?;

        zip.start_file("xl/worksheets/sheet1.xml", zip_opts)
            .map_err(|e| Error::InvalidInput(format!("ZIP creation error: {e}")))?;

        let formulas_sanitized = {
            let mut sheet_writer = BufWriter::with_capacity(128 * 1024, &mut zip);
            let sanitized = write_worksheet_xml(
                &mut sheet_writer,
                rows,
                columns,
                &active_cols,
                has_header,
                sanitize_formulas,
                cancel,
            )?;
            sheet_writer
                .flush()
                .map_err(|e| Error::io("Failed flushing worksheet XML", e))?;
            sanitized
        };

        let buf_back = zip
            .finish()
            .map_err(|e| Error::InvalidInput(format!("Failed finalizing .xlsx archive: {e}")))?;
        let inner_file = buf_back
            .into_inner()
            .map_err(|e| Error::io("Failed flushing .xlsx ZIP buffer", e.into_error()))?;
        inner_file
            .sync_all()
            .map_err(|e| Error::io("Failed syncing .xlsx file to disk", e))?;
        Ok((rows.len(), formulas_sanitized))
    })();

    let (rows_written, formulas_sanitized) = write_res?;
    guard.persist(dest)?;
    let bytes_written = fs::metadata(dest).map(|m| m.len() as usize).unwrap_or(0);

    Ok(ExportSummary {
        rows_written,
        columns_written: active_cols.len(),
        bytes_written,
        decimals_converted: 0,
        excel_text_wrapped: 0,
        formulas_sanitized,
        lossy_chars: 0,
    })
}

fn add_zip_part<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    content: &str,
    opts: SimpleFileOptions,
) -> Result<()> {
    zip.start_file(name, opts)
        .map_err(|e| Error::InvalidInput(format!("Could not create ZIP part {name}: {e}")))?;
    zip.write_all(content.as_bytes())
        .map_err(|e| Error::io("Could not write XML part to .xlsx", e))?;
    Ok(())
}

fn write_worksheet_xml<W: Write>(
    w: &mut W,
    rows: &[Vec<String>],
    columns: &[ColumnInfo],
    active_cols: &[usize],
    has_header: bool,
    sanitize_formulas: bool,
    cancel: Option<&AtomicBool>,
) -> Result<usize> {
    w.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    )
    .map_err(|e| Error::io("Worksheet write failed", e))?;

    if has_header {
        w.write_all(
            br#"<sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>"#,
        )
        .map_err(|e| Error::io("Worksheet write failed", e))?;
    }

    w.write_all(b"<cols>")
        .map_err(|e| Error::io("Worksheet write failed", e))?;
    let mut line = String::with_capacity(256);
    for (out_col, &col_idx) in active_cols.iter().enumerate() {
        let mut width_chars = columns[col_idx].name.chars().count();
        for row in rows.iter().take(200) {
            if let Some(cell) = row.get(col_idx) {
                width_chars = width_chars.max(cell.chars().take(60).count());
            }
        }
        let clamped = (width_chars + 2).clamp(8, 60);
        line.clear();
        let _ = write!(
            line,
            "<col min=\"{}\" max=\"{}\" width=\"{}\" customWidth=\"1\"/>",
            out_col + 1,
            out_col + 1,
            clamped
        );
        w.write_all(line.as_bytes())
            .map_err(|e| Error::io("Worksheet write failed", e))?;
    }
    w.write_all(b"</cols><sheetData>")
        .map_err(|e| Error::io("Worksheet write failed", e))?;

    let mut formulas_sanitized = 0usize;
    let mut row_buf = String::with_capacity(512);
    let mut cell_ref = String::with_capacity(16);

    for (r_idx, row) in rows.iter().enumerate() {
        if r_idx & 0xFFF == 0 {
            if let Some(flag) = cancel {
                if flag.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
            }
        }

        row_buf.clear();
        let row_num = r_idx + 1;
        let _ = write!(row_buf, "<row r=\"{row_num}\">");
        let is_header = has_header && r_idx == 0;

        for (out_col, &c_idx) in active_cols.iter().enumerate() {
            let v = row.get(c_idx).map(String::as_str).unwrap_or("");
            if v.is_empty() {
                continue;
            }
            write_cell_name_into(&mut cell_ref, out_col, row_num);
            let col_info = &columns[c_idx];

            if !is_header && col_info.kind == ColumnKind::Number {
                if let Some(inv) = try_parse_number(v, col_info.mark) {
                    let _ = write!(row_buf, "<c r=\"{cell_ref}\"><v>{inv}</v></c>");
                    continue;
                }
            }

            let text_val = if !is_header && sanitize_formulas {
                let (san, did) = security::sanitize_formula_cell(v);
                if did {
                    formulas_sanitized += 1;
                }
                san
            } else {
                v.to_owned()
            };

            let style = if is_header { 2 } else { 1 };
            let _ = write!(
                row_buf,
                "<c r=\"{cell_ref}\" t=\"inlineStr\" s=\"{style}\"><is><t xml:space=\"preserve\">"
            );
            append_xml_escaped(
                &mut row_buf,
                truncate_chars(&text_val, EXCEL_MAX_CELL_CHARS),
            );
            row_buf.push_str("</t></is></c>");
        }
        row_buf.push_str("</row>");
        w.write_all(row_buf.as_bytes())
            .map_err(|e| Error::io("Worksheet write failed", e))?;
    }

    w.write_all(b"</sheetData></worksheet>")
        .map_err(|e| Error::io("Worksheet write failed", e))?;
    Ok(formulas_sanitized)
}

fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s,
    }
}

/// Converts 0-based column index and 1-based row number to an Excel A1 reference (`A1`, `XFD1`).
pub fn cell_name(col: usize, row: usize) -> String {
    let mut out = String::with_capacity(10);
    write_cell_name_into(&mut out, col, row);
    out
}

fn write_cell_name_into(out: &mut String, col: usize, row: usize) {
    out.clear();
    let mut chars = [0u8; 8];
    let mut len = 0usize;
    let mut c = col + 1;
    while c > 0 {
        chars[len] = b'A' + ((c - 1) % 26) as u8;
        len += 1;
        c = (c - 1) / 26;
    }
    for i in (0..len).rev() {
        out.push(chars[i] as char);
    }
    let _ = write!(out, "{row}");
}

fn append_xml_escaped(out: &mut String, v: &str) {
    for ch in v.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\t' | '\n' | '\r' => out.push(ch),
            ch if (ch as u32) < 0x20 => {} // invalid XML 1.0 control char
            '\u{FFFE}' | '\u{FFFF}' => {}  // invalid XML 1.0 non-characters
            other => out.push(other),
        }
    }
}

fn resolve_column_indices(total: usize, selected: Option<&[usize]>) -> Vec<usize> {
    match selected {
        Some(list) if !list.is_empty() => list.iter().copied().filter(|&i| i < total).collect(),
        _ => (0..total).collect(),
    }
}

fn resolve_row_indices(total: usize, selected: Option<&[usize]>, has_header: bool) -> Vec<usize> {
    match selected {
        Some(list) if !list.is_empty() => {
            let mut out = Vec::with_capacity(list.len() + 1);
            if has_header && total > 0 && !list.contains(&0) {
                out.push(0);
            }
            for &r in list {
                if r < total && !out.contains(&r) {
                    out.push(r);
                }
            }
            out
        }
        _ => (0..total).collect(),
    }
}

/// Writes a structured JSON or Markdown recovery report for auditability.
pub fn save_recovery_report(dest: &Path, doc: &CsvDocument, markdown: bool) -> Result<usize> {
    if security::is_same_file(Path::new(&doc.path), dest) {
        return Err(Error::SameFileOverwrite(dest.display().to_string()));
    }
    let content = if markdown {
        render_markdown_report(doc)
    } else {
        serde_json::to_string_pretty(&doc.report_snapshot())
            .map_err(|e| Error::InvalidInput(format!("Report serialization failed: {e}")))?
    };
    security::atomic_write_bytes(dest, content.as_bytes())?;
    Ok(content.len())
}

fn render_markdown_report(doc: &CsvDocument) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# CSVRescue Recovery Report\n");
    let _ = writeln!(s, "- **Source file:** `{}`", doc.path);
    let _ = writeln!(s, "- **Source size:** {} bytes", doc.source_bytes_len);
    let _ = writeln!(
        s,
        "- **Detected encoding:** {} (BOM: {}, lossy: {})",
        doc.decoded.encoding_name, doc.decoded.had_bom, doc.decoded.was_lossy
    );
    let _ = writeln!(s, "- **Encoding note:** {}", doc.decoded.note);
    let _ = writeln!(
        s,
        "- **Delimiter:** `{}` ({})  |  **Line endings:** {}",
        doc.delimiter,
        doc.format.delimiter_name,
        doc.format.line_endings.dominant.label()
    );
    let _ = writeln!(
        s,
        "- **Dimensions:** {} data rows × {} columns",
        doc.data_rows(),
        doc.columns.len()
    );
    let _ = writeln!(
        s,
        "- **Repairs:** {} mojibake cell(s), {} aligned row(s), {} ambiguous row(s), {} manual edit(s)\n",
        doc.recovery.mojibake_cells_count,
        doc.recovery.aligned_rows,
        doc.recovery.ambiguous_rows,
        doc.recovery.manual_edits_count
    );

    let _ = writeln!(s, "## Columns\n");
    let _ = writeln!(
        s,
        "| # | Name | Kind | Decimal Mark | Excel Risks | Formula Risks |"
    );
    let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- |");
    for col in &doc.columns {
        let risk = if col.at_risk > 0 {
            format!("{} ({})", col.at_risk, col.risk_summary())
        } else {
            "None".to_owned()
        };
        let _ = writeln!(
            s,
            "| {} | {} | {} | {:?} | {} | {} |",
            col.index + 1,
            col.name.replace('|', "\\|"),
            col.kind.label(),
            col.mark,
            risk,
            col.formula_risk_count
        );
    }

    if !doc.validation.findings.is_empty() {
        let _ = writeln!(s, "\n## Validation & Security Findings\n");
        for f in &doc.validation.findings {
            let _ = writeln!(
                s,
                "- **[{:?}] {}** ({}): {}",
                f.severity, f.category, f.count, f.summary
            );
        }
    }

    s
}

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
const RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
const WORKBOOK_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
const WORKBOOK_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
const STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="49" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1" applyNumberFormat="1"/></cellXfs></styleSheet>"#;
