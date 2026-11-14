use csvrescue_core::analysis::ColumnKind;
use csvrescue_core::core::{CsvDocument, CsvSettings, SamplePreset};
use csvrescue_core::encoding::decode;
use csvrescue_core::errors::Error;
use csvrescue_core::export::{preview_export, save_xlsx, ExportOptions};
use csvrescue_core::logging::Logger;
use csvrescue_core::parser::{parse_reader_streaming, parse_with_options, ParseOptions};
use csvrescue_core::recovery::{RepairStrategy, RowIssueKind};
use csvrescue_core::security::{
    atomic_write_bytes, detect_formula_risk, is_same_file, sanitize_formula_cell,
    validate_source_file, MAX_BYTES,
};
use csvrescue_core::storage::{
    AppConfig, ConfigStore, HistoryEntry, HistoryStore, ThemePreference,
};
use csvrescue_core::AppPaths;
use std::fs;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static NEXT_ADV_ID: AtomicUsize = AtomicUsize::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let id = NEXT_ADV_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "csvrescue-adv-{prefix}-{}-{id}",
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

#[test]
fn formula_injection_detection_and_idempotent_sanitization() {
    for dangerous in [
        "=CMD|'/C calc'!A0",
        "+SUM(A1:A5)",
        "-2+3",
        "@SUM(1,2)",
        "\t=IMPORTXML(\"https://example.com\",\"//a\")",
        "\r=1+1",
        "   =HYPERLINK(\"http://evil.invalid\",\"click\")",
    ] {
        assert!(
            detect_formula_risk(dangerous).is_some(),
            "expected formula risk for {dangerous:?}"
        );
        let (sanitized, changed) = sanitize_formula_cell(dangerous);
        assert!(changed);
        assert!(
            sanitized.starts_with('\''),
            "expected leading apostrophe for {dangerous:?}, got {sanitized:?}"
        );
        // Sanitizing twice must be strictly idempotent and must not stack apostrophes.
        let (twice, changed_again) = sanitize_formula_cell(&sanitized);
        assert_eq!(twice, sanitized);
        assert!(!changed_again);
    }

    // Valid negative numbers and clean strings must NOT be flagged as formulas.
    for safe in ["-12.5", "-0,75", "-100", "-0.001", "hello", "1+2=3", ""] {
        assert!(
            detect_formula_risk(safe).is_none(),
            "did not expect formula risk for safe value {safe:?}"
        );
        let (sanitized, changed) = sanitize_formula_cell(safe);
        assert_eq!(sanitized, safe);
        assert!(!changed);
    }
}

#[test]
fn path_validation_same_file_case_insensitivity_and_size_limit() {
    let dir = TempDir::new("paths");
    let target = dir.child("SalesReport.CSV");
    fs::write(&target, b"a,b\r\n1,2\r\n").expect("write target");

    assert!(validate_source_file(&target).is_ok());

    // Windows NTFS paths are case-insensitive; canonical comparison must detect same file.
    let upper_variant = dir.child("salesreport.csv");
    assert!(is_same_file(&target, &upper_variant));

    // Missing file and directories are rejected cleanly.
    let missing = dir.child("missing.csv");
    assert!(validate_source_file(&missing).is_err());
    assert!(validate_source_file(&dir.path).is_err());

    // Source files exceeding MAX_BYTES are rejected before reading into RAM.
    let huge = dir.child("huge-sparse.csv");
    {
        let f = fs::File::create(&huge).expect("create huge file");
        f.set_len(MAX_BYTES + 1).expect("set file length");
    }
    let err = validate_source_file(&huge).expect_err("must reject >300MB file");
    assert!(err.to_string().contains("300"));
}

#[test]
fn atomic_write_leaves_no_temp_files() {
    let dir = TempDir::new("atomic");
    let out = dir.child("atomic.csv");
    atomic_write_bytes(&out, b"col1,col2\r\n1,2\r\n").expect("first write");
    atomic_write_bytes(&out, b"col1,col2\r\n3,4\r\n").expect("overwrite dest");
    assert_eq!(fs::read(&out).unwrap(), b"col1,col2\r\n3,4\r\n");

    let tmps: Vec<_> = fs::read_dir(&dir.path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(tmps.is_empty());
}

#[test]
fn adversarial_large_field_malformed_utf16_and_cancellation() {
    // 5 MiB single quoted field must parse cleanly without stack overflow or quadratic stall.
    let big_field = "A".repeat(5 * 1024 * 1024);
    let big_csv = format!("id,blob\r\n1,\"{big_field}\"\r\n");
    let parsed = parse_with_options(&big_csv, &ParseOptions::default(), None);
    assert_eq!(parsed.rows.len(), 2);
    assert_eq!(parsed.rows[1][1].len(), 5 * 1024 * 1024);

    // Mid-parse cancellation via AtomicBool aborts early.
    let mut big_many_rows = String::from("a,b\n");
    for i in 0..30_000 {
        big_many_rows.push_str(&format!("{i},val\n"));
    }
    let cancel_now = AtomicBool::new(true);
    let partial = parse_with_options(&big_many_rows, &ParseOptions::default(), Some(&cancel_now));
    assert!(partial.rows.len() < 30_001);

    // Streaming reader also honors cancellation and returns Error::Cancelled.
    let mut cursor = BufReader::new(big_many_rows.as_bytes());
    let stream_res = parse_reader_streaming(
        &mut cursor,
        &ParseOptions::default(),
        Some(&cancel_now),
        |_, _, _| Ok(()),
    );
    assert!(matches!(stream_res, Err(Error::Cancelled)));

    // Mid-XLSX write cancellation removes any partial temp file cleanly.
    let dir = TempDir::new("xlsx-cancel");
    let dest = dir.child("cancelled.xlsx");
    let doc =
        CsvDocument::from_sample_preset(SamplePreset::FormulaAndTypes, CsvSettings::default())
            .expect("sample");
    let xlsx_err = save_xlsx(
        &dest,
        None,
        doc.rows(),
        &doc.columns,
        true,
        false,
        None,
        Some(&cancel_now),
    )
    .expect_err("cancelled xlsx");
    assert!(matches!(xlsx_err, Error::Cancelled));
    assert!(!dest.exists());

    // Odd-byte UTF-16 & forced UTF-8 invalid bytes must return clean Decoded errors/losses, never panic.
    let odd_utf16 = [0xFFu8, 0xFE, 0x41, 0x00, 0x42];
    let dec_odd = decode(&odd_utf16, None).expect("odd utf16 must not panic");
    assert!(dec_odd.text.starts_with('A'));

    let invalid_utf8 = [0xC3u8, 0x28, b',', b'x'];
    let dec_invalid = decode(&invalid_utf8, Some("UTF-8")).expect("forced utf8 invalid bytes");
    assert!(dec_invalid.was_lossy);
    assert!(dec_invalid.invalid_sequences > 0);
}

#[test]
fn recovery_strategies_ambiguity_manual_edits_and_column_override_persistence() {
    // 1. AlignColumns merges split unquoted delimiter when trailing numeric column gives unique alignment.
    let split_csv = b"Name,Qty,Price\r\n\
                      Widget,4,12.50\r\n\
                      Bolt,10,3.25\r\n\
                      Acme, Corp,5,9.99\r\n\
                      ShortRow,2\r\n";

    let preserve_doc = CsvDocument::from_bytes(
        "split.csv",
        split_csv,
        CsvSettings {
            repair_strategy: RepairStrategy::Preserve,
            ..CsvSettings::default()
        },
        false,
        None,
    )
    .expect("preserve doc");
    assert_eq!(preserve_doc.recovery.short_rows, 1);
    assert_eq!(preserve_doc.recovery.wide_rows, 1);
    assert_eq!(preserve_doc.rows()[3].len(), 4);
    assert_eq!(preserve_doc.rows()[4].len(), 2);

    let align_doc = CsvDocument::from_bytes(
        "split.csv",
        split_csv,
        CsvSettings {
            repair_strategy: RepairStrategy::AlignColumns,
            ..CsvSettings::default()
        },
        false,
        None,
    )
    .expect("align doc");
    assert_eq!(align_doc.rows()[3], vec!["Acme, Corp", "5", "9.99"]);
    assert_eq!(align_doc.rows()[4], vec!["ShortRow", "2", ""]);

    // 2. Ambiguous all-text wide row is never silently guessed; flagged as AmbiguousWidth.
    let ambiguous_csv = b"A,B,C\r\nfoo,bar,baz\r\nalpha,beta,gamma,delta\r\n";
    let mut amb_doc = CsvDocument::from_bytes(
        "ambiguous.csv",
        ambiguous_csv,
        CsvSettings {
            repair_strategy: RepairStrategy::AlignColumns,
            ..CsvSettings::default()
        },
        false,
        None,
    )
    .expect("ambiguous doc");
    assert_eq!(amb_doc.rows()[2].len(), 4);
    assert!(amb_doc
        .recovery
        .diagnostics
        .iter()
        .any(|d| d.kind == RowIssueKind::AmbiguousWidth && d.ambiguous));

    // 3. Manual edit + revert preserves original row comparison and survives validation updates.
    amb_doc.set_manual_cell_edit(2, 0, "alpha_fixed".to_owned());
    assert_eq!(amb_doc.rows()[2][0], "alpha_fixed");
    assert_eq!(amb_doc.recovery.manual_edits_count, 1);
    amb_doc.revert_manual_cell_edit(2, 0);
    assert_eq!(amb_doc.rows()[2][0], "alpha");
    assert_eq!(amb_doc.recovery.manual_edits_count, 0);

    // 4. Manual column kind override survives update_settings (e.g. toggling repair_double_encoding).
    assert!(amb_doc.set_column_kind(0, ColumnKind::Text));
    amb_doc
        .update_settings(CsvSettings {
            repair_double_encoding: false,
            ..amb_doc.settings.clone()
        })
        .expect("update settings");
    assert_eq!(amb_doc.columns[0].kind, ColumnKind::Text);

    // 5. Dry-run export preview reflects selected columns & formula sanitization accurately.
    let sample =
        CsvDocument::from_sample_preset(SamplePreset::FormulaAndTypes, CsvSettings::default())
            .expect("sample");
    let formula_col = sample
        .columns
        .iter()
        .find(|c| c.formula_risk_count > 0)
        .expect("sample preset has formula risk column")
        .index;
    let other_col = usize::from(formula_col == 0);
    let dry = preview_export(
        &sample,
        &ExportOptions {
            sanitize_formulas: true,
            selected_columns: Some(vec![formula_col, other_col]),
            ..ExportOptions::default()
        },
        25,
    );
    assert_eq!(dry.summary.columns_written, 2);
    assert!(dry.summary.formulas_sanitized > 0);
}

#[test]
fn config_history_and_logger_rotation() {
    let dir = TempDir::new("storage");
    let paths = AppPaths::with_root(dir.child("appdata"));

    let cfg_store = ConfigStore::new(&paths);
    let cfg = AppConfig {
        theme: ThemePreference::Dark,
        default_out_delimiter: ';',
        default_utf8_bom: false,
        default_decimal_as_dot: true,
        default_excel_protect: true,
        default_sanitize_formulas: true,
        default_repair_double_encoding: true,
        recent_files: vec!["C:\\sample.csv".to_owned()],
    };
    cfg_store.save(&cfg).expect("save config");
    let loaded_cfg = cfg_store.load();
    assert_eq!(loaded_cfg, cfg);

    let hist_store = HistoryStore::from_app_paths(&paths);
    hist_store
        .append(HistoryEntry {
            id: "1".to_owned(),
            timestamp: "2026-11-14 15:50:00".to_owned(),
            source_path: "C:\\in.csv".to_owned(),
            output_path: "C:\\out.xlsx".to_owned(),
            format: "XLSX".to_owned(),
            encoding_name: "UTF-8 (OpenXML)".to_owned(),
            delimiter: ',',
            rows_written: 100,
            columns_written: 4,
            mojibake_repaired: 3,
            formulas_sanitized: 1,
        })
        .expect("record history");
    let hist = hist_store.list();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].format, "XLSX");
    hist_store.clear().expect("clear history");
    assert!(hist_store.list().is_empty());

    // Logger creates rotated log file once csvrescue.log exceeds 2 MiB.
    let logger = Logger::from_app_paths(&paths);
    let log_dir = paths.logs();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        log_dir.join("csvrescue.log"),
        vec![b'x'; 2 * 1024 * 1024 + 16],
    )
    .unwrap();
    logger.info("test", "Triggered rotation check");
    assert!(log_dir.join("csvrescue.log.1").is_file());
    assert!(log_dir.join("csvrescue.log").is_file());
    logger.error("test", "Sample error line");
    assert!(paths.error_log().is_file());
}
