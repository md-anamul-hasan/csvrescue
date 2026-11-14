use csvrescue_core::core::{CsvDocument, CsvSettings};
use csvrescue_core::encoding::char_to_win1252;
use csvrescue_core::export::{save_csv, save_xlsx, ExportOptions};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn get_process_memory() -> usize {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let handle = GetCurrentProcess();
        if GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
        {
            return counters.WorkingSetSize;
        }
    }
    0
}

fn ensure_benchmark_fixture() -> PathBuf {
    let repo_fixture = PathBuf::from("CSVRescue.Tests/fixtures/european-200k.csv");
    if repo_fixture.is_file() {
        return repo_fixture;
    }

    let tmp_fixture = std::env::temp_dir().join("european-200k.csv");
    let mut text = String::with_capacity(7_500_000);
    text.push_str("Teil-Nr;Bezeichnung;Menge;Preis;Datum\r\n");
    for i in 0..200_000usize {
        let idx = i + 1;
        let qty = (i % 500) + 1;
        let euros = (i % 999) + 1;
        let cents = i % 100;
        let month = (i % 12) + 1;
        let day = (i % 28) + 1;
        let _ = write!(
            text,
            "{:05};Schraube M8 × {};{};{},{:02};{}-{}\r\n",
            idx % 99999,
            (i % 20) + 10,
            qty,
            euros,
            cents,
            day,
            month
        );
    }
    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        bytes.push(char_to_win1252(ch).unwrap_or(b'?'));
    }
    fs::write(&tmp_fixture, &bytes).expect("failed to write temporary benchmark fixture");
    tmp_fixture
}

fn main() {
    println!("=== CSVRescue Rust Benchmark (200k rows) ===");
    let fixture_path = ensure_benchmark_fixture();
    let fixture_size = fs::metadata(&fixture_path).map(|m| m.len()).unwrap_or(0);
    println!("Fixture: {:?} ({} bytes)", fixture_path, fixture_size);

    for run in 1..=3 {
        let initial_mem = get_process_memory();

        let t0 = Instant::now();
        let doc = CsvDocument::load(&fixture_path, CsvSettings::default())
            .expect("failed to load benchmark document");
        let load_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let tmp_csv = std::env::temp_dir().join("csvrescue-bench-out.csv");
        let t1 = Instant::now();
        let csv_summary = save_csv(
            &tmp_csv,
            Some(&fixture_path),
            doc.rows(),
            &doc.columns,
            &ExportOptions::default(),
        )
        .expect("failed to export CSV");
        let csv_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let _ = fs::remove_file(&tmp_csv);

        let tmp_xlsx = std::env::temp_dir().join("csvrescue-bench-out.xlsx");
        let t2 = Instant::now();
        let xlsx_summary = save_xlsx(
            &tmp_xlsx,
            Some(&fixture_path),
            doc.rows(),
            &doc.columns,
            doc.settings.has_header,
            false,
            None,
            None,
        )
        .expect("failed to export XLSX");
        let xlsx_ms = t2.elapsed().as_secs_f64() * 1000.0;
        let _ = fs::remove_file(&tmp_xlsx);

        let peak_mem_mb = get_process_memory().max(initial_mem) as f64 / (1024.0 * 1024.0);
        let total_ms = load_ms + csv_ms + xlsx_ms;

        println!(
            "Run {}: rows={}, cols={}, Load+Analyze={:.1}ms, ExportCsv={:.1}ms ({}B), ExportXlsx={:.1}ms ({}B), Total={:.1}ms, WorkingSet={:.1}MB",
            run,
            doc.data_rows(),
            doc.columns.len(),
            load_ms,
            csv_ms,
            csv_summary.bytes_written,
            xlsx_ms,
            xlsx_summary.bytes_written,
            total_ms,
            peak_mem_mb
        );
    }
}
