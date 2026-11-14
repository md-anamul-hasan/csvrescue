use csvrescue_core::analysis::{risk_of, try_parse_number, ColumnKind, DecimalMark, Risk};
use csvrescue_core::core::{CsvDocument, CsvSettings, SamplePreset};
use csvrescue_core::detector::{detect_delimiter, LineEnding};
use csvrescue_core::encoding::{
    char_to_win1252, decode, encode_output, repair_double_encoding, OutputEncoding,
};
use csvrescue_core::export::{
    cell_name, save_csv, save_recovery_report, save_xlsx, to_csv, ExportOptions, EXCEL_MAX_ROWS,
};
use csvrescue_core::parser::{parse, parse_with_options, EscapeMode, ParseOptions};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use zip::ZipArchive;

static NEXT_TMP_ID: AtomicUsize = AtomicUsize::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let id = NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "csvrescue-test-{prefix}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp test dir");
        Self { path: dir }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn encode_win1252(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| char_to_win1252(ch).unwrap_or(b'?'))
        .collect()
}

fn encode_utf16_le(text: &str, with_bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    if with_bom {
        out.extend_from_slice(&[0xFF, 0xFE]);
    }
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

fn encode_utf16_be(text: &str, with_bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    if with_bom {
        out.extend_from_slice(&[0xFE, 0xFF]);
    }
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_be_bytes());
    }
    out
}

fn read_zip_entry(zip_path: &Path, entry_name: &str) -> String {
    let file = fs::File::open(zip_path).expect("open xlsx archive");
    let mut archive = ZipArchive::new(file).expect("parse xlsx zip");
    let mut entry = archive.by_name(entry_name).expect("find zip entry");
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .expect("read zip entry as UTF-8");
    text
}

fn zip_entry_names(zip_path: &Path) -> Vec<String> {
    let file = fs::File::open(zip_path).expect("open xlsx archive");
    let archive = ZipArchive::new(file).expect("parse xlsx zip");
    archive
        .file_names()
        .filter_map(|r| r.ok().map(std::borrow::Cow::into_owned))
        .collect()
}

#[test]
fn detects_unicode_encodings_with_and_without_bom() {
    let sample = "Name,Price\r\nNaïve café,€12.50\r\n";

    let mut utf8_bom = vec![0xEF, 0xBB, 0xBF];
    utf8_bom.extend_from_slice(sample.as_bytes());
    let dec8 = decode(&utf8_bom, None).expect("utf8 bom");
    assert_eq!(dec8.encoding_name, "UTF-8");
    assert!(dec8.had_bom);
    assert_eq!(dec8.text, sample);

    let utf16_le_bom = encode_utf16_le(sample, true);
    let dec16le_bom = decode(&utf16_le_bom, None).expect("utf16 le bom");
    assert_eq!(dec16le_bom.encoding_name, "UTF-16 LE");
    assert!(dec16le_bom.had_bom);
    assert_eq!(dec16le_bom.text, sample);

    let utf16_be_bom = encode_utf16_be(sample, true);
    let dec16be_bom = decode(&utf16_be_bom, None).expect("utf16 be bom");
    assert_eq!(dec16be_bom.encoding_name, "UTF-16 BE");
    assert!(dec16be_bom.had_bom);
    assert_eq!(dec16be_bom.text, sample);

    let ascii_table = "Id,Name,City\r\n1,Alice,Paris\r\n2,Bob,Berlin\r\n3,Cara,Rome\r\n";
    let utf16_le_nobom = encode_utf16_le(ascii_table, false);
    let dec16le = decode(&utf16_le_nobom, None).expect("utf16 le no bom");
    assert_eq!(dec16le.encoding_name, "UTF-16 LE");
    assert!(!dec16le.had_bom);
    assert_eq!(dec16le.text, ascii_table);

    let utf16_be_nobom = encode_utf16_be(ascii_table, false);
    let dec16be = decode(&utf16_be_nobom, None).expect("utf16 be no bom");
    assert_eq!(dec16be.encoding_name, "UTF-16 BE");
    assert!(!dec16be.had_bom);
    assert_eq!(dec16be.text, ascii_table);
}

#[test]
fn valid_utf8_without_bom_beats_windows_1252() {
    let utf8_text = "Customer,City,Note\r\nJosé Álvarez,São Paulo,Emoji 🧪 ✓\r\n";
    let dec = decode(utf8_text.as_bytes(), None).expect("valid utf-8");
    assert_eq!(dec.encoding_name, "UTF-8");
    assert!(!dec.had_bom);
    assert!(!dec.was_lossy);
    assert_eq!(dec.invalid_sequences, 0);
    assert_eq!(dec.text, utf8_text);

    let win1252_text = "Name;City;Price\r\nMüller;Straße;12,50 €\r\n";
    let win_bytes = encode_win1252(win1252_text);
    let dec_win = decode(&win_bytes, None).expect("win1252 fallback");
    assert_eq!(dec_win.encoding_name, "Windows-1252 (Western Europe)");
    assert_eq!(dec_win.text, win1252_text);

    // Explicitly forcing UTF-8 on Windows-1252 bytes exposes lossy replacements transparently.
    let forced_lossy = decode(&win_bytes, Some("UTF-8")).expect("forced utf8");
    assert!(forced_lossy.was_lossy);
    assert!(forced_lossy.invalid_sequences > 0);
}

#[test]
fn repairs_text_saved_as_utf8_twice() {
    assert_eq!(repair_double_encoding("cafÃ©"), "café");
    assert_eq!(repair_double_encoding("naÃ¯ve"), "naïve");
    assert_eq!(repair_double_encoding("MÃ¼ller"), "Müller");
    assert_eq!(repair_double_encoding("â‚¬100"), "€100");
    assert_eq!(repair_double_encoding("M8 Ã— 20"), "M8 × 20");
    // Clean UTF-8 must remain untouched.
    assert_eq!(
        repair_double_encoding("José, Zürich, €50"),
        "José, Zürich, €50"
    );
}

#[test]
fn parses_quotes_doubled_quotes_and_line_breaks_inside_quotes() {
    let csv = "id,note,amount\r\n\
               1,\"Hello, \"\"World\"\"\",42\r\n\
               \r\n\
               2,\"Line A\r\nLine B\",99\r\n\
               3,Unescaped \"mid quote,15\r\n";
    let parsed = parse(csv, ',');
    assert_eq!(parsed.rows.len(), 4);
    assert_eq!(parsed.empty_lines, 1);
    assert_eq!(parsed.rows[1], vec!["1", "Hello, \"World\"", "42"]);
    assert_eq!(parsed.rows[2], vec!["2", "Line A\r\nLine B", "99"]);
    assert!(parsed.had_stray_quotes);
    assert_eq!(parsed.rows[3][0], "3");
    assert!(parsed.rows[3][1].contains('"'));

    // Backslash escape mode and unclosed trailing quote recovery.
    let opts = ParseOptions {
        delimiter: ',',
        quote_char: Some('"'),
        escape_mode: EscapeMode::Backslash,
        skip_empty_lines: true,
        max_rows: usize::MAX,
    };
    let res = parse_with_options("a,b\n\"hello \\\"world\\\"\",\"unclosed", &opts, None);
    assert_eq!(res.rows[1][0], "hello \"world\"");
    assert_eq!(res.rows[1][1], "unclosed");
    assert!(res.ended_inside_quote);
}

#[test]
fn detects_the_delimiter() {
    assert_eq!(detect_delimiter("a,b,c\n1,2,3\n4,5,6\n"), ',');
    assert_eq!(detect_delimiter("a;b;c\r\n1;2,5;3\r\n4;5,2;6\r\n"), ';');
    assert_eq!(detect_delimiter("a\tb\tc\n1\t2\t3\n4\t5\t6\n"), '\t');
    assert_eq!(detect_delimiter("a|b|c\n1|2|3\n4|5|6\n"), '|');
    // Delimiters inside quotes must not mislead detection.
    assert_eq!(
        detect_delimiter("\"a;b;c\",val\n\"x;y;z\",10\n\"p;q;r\",20\n"),
        ','
    );
}

#[test]
fn risk_of_finds_what_excel_would_damage() {
    assert!(risk_of("00123").contains(Risk::LEADING_ZEROS));
    assert!(risk_of("-0042").contains(Risk::LEADING_ZEROS));
    assert!(risk_of("1234567890123456").contains(Risk::LONG_NUMBER));
    assert!(risk_of("-1234567890123456").contains(Risk::LONG_NUMBER));
    assert!(risk_of("1E5").contains(Risk::SCIENTIFIC_LOOK));
    assert!(risk_of("3e12").contains(Risk::SCIENTIFIC_LOOK));
    let r_date_or_lz = risk_of("01-02");
    assert!(r_date_or_lz.contains(Risk::DATE_LOOK) || r_date_or_lz.contains(Risk::LEADING_ZEROS));
    assert!(risk_of("10-12-2025").contains(Risk::DATE_LOOK));
    assert!(risk_of("2026/04/09").contains(Risk::DATE_LOOK));
    let r_frac = risk_of("1/2");
    assert!(r_frac.contains(Risk::DATE_LOOK) || r_frac.contains(Risk::FRACTION_LOOK));

    assert_eq!(risk_of("0"), Risk::NONE);
    assert_eq!(risk_of("42"), Risk::NONE);
    assert_eq!(risk_of("-12.5"), Risk::NONE);
    assert_eq!(risk_of("Hello"), Risk::NONE);
}

#[test]
fn parses_clean_numbers() {
    assert_eq!(
        try_parse_number("42", DecimalMark::Dot).as_deref(),
        Some("42")
    );
    assert_eq!(
        try_parse_number("-12.50", DecimalMark::Dot).as_deref(),
        Some("-12.50")
    );
    assert_eq!(
        try_parse_number("12,50", DecimalMark::Comma).as_deref(),
        Some("12.50")
    );
    assert_eq!(
        try_parse_number("-0,75", DecimalMark::Comma).as_deref(),
        Some("-0.75")
    );
    assert_eq!(
        try_parse_number("1,234.50", DecimalMark::Dot).as_deref(),
        Some("1234.50")
    );
    assert_eq!(
        try_parse_number("1.234,50", DecimalMark::Comma).as_deref(),
        Some("1234.50")
    );
    assert_eq!(
        try_parse_number("1.234.567", DecimalMark::Comma).as_deref(),
        Some("1234567")
    );
    assert_eq!(
        try_parse_number("0", DecimalMark::Dot).as_deref(),
        Some("0")
    );

    // Leading zeros, 16+ digit IDs exceeding IEEE 754 precision, and scientific-look product codes must not coerce.
    assert_eq!(try_parse_number("00123", DecimalMark::Dot), None);
    assert_eq!(try_parse_number("1234567890123456", DecimalMark::Dot), None);
    assert_eq!(try_parse_number("1E5", DecimalMark::Dot), None);
    assert_eq!(try_parse_number("seven", DecimalMark::Dot), None);
}

#[test]
fn reads_a_european_windows_file_exactly() {
    let dir = TempDir::new("euro");
    let src = dir.child("european.csv");
    let raw = "Teil-Nr;Bezeichnung;Menge;Preis;Datum\r\n\
               00123;Schraube M8 × 20;4;12,50;01-02\r\n\
               00456;Mutter M8;10;3,75;14-03\r\n";
    fs::write(&src, encode_win1252(raw)).expect("write euro fixture");

    let doc = CsvDocument::load(&src, CsvSettings::default()).expect("load european file");
    let dto = doc.summary_dto();
    assert_eq!(dto.encoding_name, "Windows-1252 (Western Europe)");
    assert_eq!(doc.delimiter, ';');
    assert_eq!(dto.decimal_mark, DecimalMark::Comma);
    assert_eq!(doc.data_rows(), 2);

    assert_eq!(doc.columns[0].name, "Teil-Nr");
    assert_eq!(doc.columns[0].kind, ColumnKind::Text);
    assert!(doc.columns[0].at_risk > 0);
    assert_eq!(doc.columns[2].kind, ColumnKind::Number);
    assert_eq!(doc.columns[3].kind, ColumnKind::Number);
    assert_eq!(doc.columns[4].kind, ColumnKind::Text);
}

#[test]
fn xlsx_has_the_right_parts_text_cells_and_numbers() {
    let dir = TempDir::new("xlsx");
    let src = dir.child("input.csv");
    let out = dir.child("output.xlsx");

    let csv_text = "Code,Label,Qty,Note\r\n\
                    00123,Widget\x0B A,42,Clean\r\n\
                    1234567890123456,ID Badge,7,Emoji 🚀\r\n";
    fs::write(&src, csv_text.as_bytes()).expect("write src");

    let doc = CsvDocument::load(&src, CsvSettings::default()).expect("load doc");
    let summary = save_xlsx(
        &out,
        Some(&src),
        doc.rows(),
        &doc.columns,
        doc.settings.has_header,
        false,
        None,
        None,
    )
    .expect("save xlsx");

    assert_eq!(summary.rows_written, 3);
    let entries = zip_entry_names(&out);
    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/styles.xml",
        "xl/worksheets/sheet1.xml",
    ] {
        assert!(
            entries.iter().any(|e| e == required),
            "missing zip entry {required}"
        );
    }

    let styles = read_zip_entry(&out, "xl/styles.xml");
    assert!(styles.contains("numFmtId=\"49\""));

    let sheet = read_zip_entry(&out, "xl/worksheets/sheet1.xml");
    assert!(sheet.contains("ySplit=\"1\""));
    assert!(sheet.contains("state=\"frozen\""));
    assert!(sheet.contains("<c r=\"A1\" t=\"inlineStr\" s=\"2\"><is><t xml:space=\"preserve\">"));
    assert!(sheet.contains(
        "<c r=\"A2\" t=\"inlineStr\" s=\"1\"><is><t xml:space=\"preserve\">00123</t></is></c>"
    ));
    assert!(sheet.contains(
        "<c r=\"A3\" t=\"inlineStr\" s=\"1\"><is><t xml:space=\"preserve\">1234567890123456</t></is></c>"
    ));
    assert!(sheet.contains("<c r=\"C2\"><v>42</v></c>"));
    assert!(!sheet.contains('\x0B'));
    assert!(sheet.contains("Widget A"));
    assert!(sheet.contains("Emoji 🚀"));
}

#[test]
fn xlsx_cell_names_and_limits() {
    assert_eq!(cell_name(0, 1), "A1");
    assert_eq!(cell_name(25, 1), "Z1");
    assert_eq!(cell_name(26, 1), "AA1");
    assert_eq!(cell_name(51, 1), "AZ1");
    assert_eq!(cell_name(701, 1), "ZZ1");
    assert_eq!(cell_name(702, 1), "AAA1");
    assert_eq!(cell_name(27, 11), "AB11");

    let dir = TempDir::new("limit");
    let out = dir.child("too-big.xlsx");
    let too_many_rows: Vec<Vec<String>> = vec![vec!["x".to_owned()]; EXCEL_MAX_ROWS + 1];
    let cols = vec![csvrescue_core::analysis::ColumnInfo {
        index: 0,
        name: "Col 1".to_owned(),
        kind: ColumnKind::Text,
        detected_kind: ColumnKind::Text,
        mark: DecimalMark::Dot,
        at_risk: 0,
        risks: Risk::NONE,
        non_empty: EXCEL_MAX_ROWS + 1,
        empty_count: 0,
        unique_count: 1,
        numeric_count: 0,
        formula_risk_count: 0,
        sample_values: vec!["x".to_owned()],
    }];

    let err = save_xlsx(&out, None, &too_many_rows, &cols, false, false, None, None)
        .expect_err("must reject rows exceeding EXCEL_MAX_ROWS");
    assert!(err.to_string().contains("1,048,576") || err.to_string().contains("limit"));
    assert!(!out.exists());

    let leftovers: Vec<_> = fs::read_dir(&dir.path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover tmp files: {leftovers:?}");
}

#[test]
fn forced_number_column_preserves_invalid_value_as_text() {
    let dir = TempDir::new("forced-num");
    let out = dir.child("mixed.xlsx");
    let raw = b"Val\r\n42\r\nseven\r\n10\r\n";

    let mut doc = CsvDocument::from_bytes("mixed.csv", raw, CsvSettings::default(), false, None)
        .expect("load bytes");
    assert!(doc.set_column_kind(0, ColumnKind::Number));
    save_xlsx(
        &out,
        None,
        doc.rows(),
        &doc.columns,
        doc.settings.has_header,
        false,
        None,
        None,
    )
    .expect("save xlsx");

    let sheet = read_zip_entry(&out, "xl/worksheets/sheet1.xml");
    assert!(sheet.contains("<c r=\"A2\"><v>42</v></c>"));
    assert!(sheet.contains(
        "<c r=\"A3\" t=\"inlineStr\" s=\"1\"><is><t xml:space=\"preserve\">seven</t></is></c>"
    ));
    assert!(sheet.contains("<c r=\"A4\"><v>10</v></c>"));
}

#[test]
fn csv_export_encodings_same_file_guard_and_report_export() {
    let dir = TempDir::new("csv-export");
    let src = dir.child("orders.csv");
    let out_csv = dir.child("orders-rescued.csv");
    let report_md = dir.child("orders-report.md");

    let doc =
        CsvDocument::from_sample_preset(SamplePreset::FormulaAndTypes, CsvSettings::default())
            .expect("sample preset");
    fs::write(&src, b"placeholder").expect("write source");
    let loaded = CsvDocument::load(&src, CsvSettings::default()).unwrap();
    assert!(loaded.is_same_file(&src));

    // Refuses silently overwriting source.
    let err = save_csv(
        &src,
        Some(&src),
        doc.rows(),
        &doc.columns,
        &ExportOptions::default(),
    )
    .expect_err("must block overwriting source file");
    assert!(matches!(
        err,
        csvrescue_core::errors::Error::SameFileOverwrite(_)
    ));

    // Custom delimiter + CRLF + formula sanitization + UTF-8 BOM.
    let opts = ExportOptions {
        delimiter: ';',
        encoding: OutputEncoding::Utf8,
        utf8_bom: true,
        line_ending: LineEnding::Crlf,
        decimal_as_dot: false,
        excel_text_protect: false,
        sanitize_formulas: true,
        has_header: true,
        selected_columns: None,
        selected_rows: None,
    };
    let summary = save_csv(&out_csv, Some(&src), doc.rows(), &doc.columns, &opts)
        .expect("save sanitized csv");
    assert!(summary.bytes_written > 0);
    assert!(summary.formulas_sanitized > 0);

    let saved = fs::read(&out_csv).expect("read exported csv");
    assert!(saved.starts_with(&[0xEF, 0xBB, 0xBF]));

    // Encoding output roundtrip for UTF-16 LE and Windows-1252.
    let (utf16_bytes, _) = encode_output("a;b\r\n", OutputEncoding::Utf16LeBom, true);
    assert!(utf16_bytes.starts_with(&[0xFF, 0xFE]));
    let (win_bytes, _) = encode_output("Café;€\r\n", OutputEncoding::Windows1252, false);
    assert_eq!(
        decode(&win_bytes, Some("Windows-1252 (Western Europe)"))
            .unwrap()
            .text,
        "Café;€\r\n"
    );

    // Inline to_csv helper wraps embedded delimiters/quotes properly.
    let (serialized, _) = to_csv(
        &[vec!["a,b".to_owned(), "c\"d".to_owned()]],
        &doc.columns[..2],
        &ExportOptions {
            has_header: false,
            ..ExportOptions::default()
        },
    );
    assert_eq!(serialized, "\"a,b\",\"c\"\"d\"\r\n");

    // Suggested filenames keep parent directory cleanly.
    let mut file_doc = doc;
    file_doc.path = src.to_string_lossy().into_owned();
    file_doc.is_sample = false;
    assert_eq!(
        file_doc.suggested_name(".xlsx"),
        dir.child("orders-clean.xlsx")
    );

    // Markdown recovery report writes atomically without errors.
    save_recovery_report(&report_md, &file_doc, true).expect("save markdown report");
    let report = fs::read_to_string(&report_md).expect("read report");
    assert!(report.contains("# CSVRescue Recovery Report"));
}
