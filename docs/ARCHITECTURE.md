# CSVRescue Architecture (v2.0.0)

CSVRescue v2.0.0 is a **100% native Rust + Tauri 2.x desktop application** for Windows 10/11 (x64). All file parsing, encoding detection, mojibake reversal, structural recovery, validation, formula inspection, and `.csv`/`.xlsx` serialization live in pure Rust; the WebView2 UI renders a thin TypeScript/React presentation layer over Tauri IPC.

## Workspace Layout

```
crates/
├── csvrescue-core/          Pure-Rust engine (no UI framework dependency)
│   ├── src/
│   │   ├── lib.rs           Public API surface, VERSION constant, AppPaths (%APPDATA%\CSVRescue)
│   │   ├── core.rs          CsvDocument lifecycle, sample presets, manual overrides, DTO projection
│   │   ├── encoding.rs      BOM + UTF-16 heuristic + UTF-8/CP1252 + 20 WHATWG encodings & mojibake repair
│   │   ├── detector.rs      Delimiter (`,`, `;`, `\t`, `|`), quote format, CRLF/LF/CR, header & decimal mark
│   │   ├── parser.rs        Tolerant RFC-4180 + backslash parser & bounded-memory streaming reader
│   │   ├── recovery.rs      Recovery types (`Preserve` vs `AlignColumns`), confidence levels, diagnostics
│   │   ├── repair.rs        Deterministic ragged-row column alignment & manual cell override application
│   │   ├── analysis.rs      Column inference (`Text` / `Number`) & Excel corruption heuristics
│   │   ├── validation.rs    Duplicate rows, duplicate headers, whitespace, control chars, formula audit
│   │   ├── export.rs        CSV writer, pure-Rust OpenXML `.xlsx` ZIP builder, dry-run preview & MD/JSON reports
│   │   ├── preview.rs       Paginated filter/search projection & original-vs-recovered row comparison
│   │   ├── security.rs      Path canonicalization, 300 MiB size bound, atomic MoveFileExW writes, formula sanitization
│   │   ├── storage.rs       Atomic JSON persistence (`settings.json`, `history.json`)
│   │   ├── logging.rs       Rotating structured logs (`logs/csvrescue.log` 2 MiB + `error.log`)
│   │   └── errors.rs        Centralized `Error` enum and `Result<T>` alias
│   ├── examples/bench.rs    200,000-row European CSV benchmark & WorkingSet measurement
│   └── tests/               Legacy parity suite (12 tests) + adversarial/security suite (6 tests)
└── csvrescue-app/           Tauri 2.x desktop host (`CSVRescue.exe`)
    ├── src/main.rs          Win32 DLL search-path hardening, CLI file preload, 22 Tauri commands
    ├── src/state.rs         Thread-safe `AppState` (`Mutex<Option<CsvDocument>>`, `AtomicBool` cancel)
    ├── src/commands.rs      Typed JSON/IPC command bridge
    ├── capabilities/        Minimal Tauri v2 capability allowlist (`dialog`, scoped `opener`)
    ├── app.manifest         Standard `asInvoker`, `PerMonitorV2`, `longPathAware`, active `UTF-8`
    └── tauri.conf.json      Strict CSP + embedded `src-ui/dist` static bundle
src-ui/                      TypeScript + React 19 UI (build-time Vite bundle only; no Node.js in release)
installer/csvrescue.iss      Inno Setup 6 script preserving legacy AppId for seamless upgrades
scripts/                     PowerShell release automation (`build-installer.ps1`, `package-release.ps1`)
```

## Processing & Recovery Pipeline

1. **Preflight Validation (`security::validate_source_file`):** Verifies the target exists, is a regular file, and does not exceed `MAX_BYTES` (300 MiB). Source bytes are kept immutable in memory (`raw_bytes`) while original files on disk are opened read-only and never overwritten.
2. **Decoding & Mojibake Repair (`encoding.rs`):**
   - Inspects UTF-8 / UTF-16 LE/BE / UTF-32 LE/BE BOMs, then UTF-16 null-byte distribution heuristics, then strict UTF-8 validation, falling back cleanly to Windows-1252 (with explicit lossy tracking) or any of 20 user-selected encodings.
   - `repair_double_encoding_detailed` detects UTF-8 bytes misread as Windows-1252 and re-saved (`Ã©`, `â€™`, `â‚¬`, emoji runs) and reverses valid runs only when every byte reconstructs clean UTF-8.
3. **Format Detection & Tolerant Parsing (`detector.rs` + `parser.rs`):**
   - Scores candidate delimiters inside unquoted spans, identifies LF/CRLF/CR mix, detects header row shape and decimal mark (`Dot` vs `Comma`), and parses quotes, embedded newlines, stray mid-field quotes, and unclosed EOF quotes without crashing or silently dropping records.
4. **Recovery & Manual Override Layer (`repair.rs`):**
   - `RepairStrategy::Preserve` reports short and wide rows faithfully without guessing.
   - `RepairStrategy::AlignColumns` pads trailing cells on short rows and merges split unquoted fields ONLY when column numeric profiles yield a unique alignment; multi-candidate merges are tagged `AmbiguousWidth` with `ConfidenceLevel::Ambiguous` for human review.
   - Users can inspect original vs. recovered values side-by-side (`get_row_comparison`) and override or revert individual cells cleanly.
5. **Analysis, Validation & Export (`analysis.rs`, `validation.rs`, `export.rs`):**
   - Flags Excel-coercible patterns (`LEADING_ZEROS` including signed `-0042`, 16+ digit `LONG_NUMBER`, `SCIENTIFIC_LOOK`, `DATE_LOOK`, `FRACTION_LOOK`) and keeps risky columns `Text`.
   - Flags spreadsheet formula triggers (`=`, `+`, `-`, `@`, `\t`, `\r`) and offers idempotent `'` sanitization across both `.csv` and `.xlsx` exports.
   - Writes `.xlsx` directly via `zip` (`deflate`) + custom streaming XML (`xl/styles.xml`, `@` text format `numFmtId="49"`, frozen bold headers, stripped XML 1.0 invalid controls) through sibling `.tmp` files committed via Win32 `MoveFileExW`.
