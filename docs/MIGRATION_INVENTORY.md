# Migration Inventory: .NET 10 / WPF (v1.1.0) -> Rust + Tauri 2 (v2.0.0)

This inventory maps every component, algorithm, and workflow from the legacy `.NET 10 / WPF` codebase to its 100% native Rust + Tauri v2.0.0 replacement. Legacy C#/.NET/WPF sources (`App.xaml*`, `Services/`, `ViewModels/`, `Controls/`, `Converters/`, `Themes/`, `CSVRescue.Tests/`, `.csproj`, `.slnx`, `Directory.Build.props`) have been removed cleanly.

## 1. Engine & Service Migration

| Legacy Component (.NET 10) | Rust Module (`csvrescue-core`) | Improvements in v2.0.0 |
|---|---|---|
| `Services/EncodingDetector.cs` & `MojibakeRepair.cs` | `crates/csvrescue-core/src/encoding.rs` | Preserves BOM / UTF-16 heuristic / UTF-8 / CP1252 precedence, expands to 20 WHATWG encodings (`encoding_rs`), repairs 3-byte UTF-8 mojibake (`â‚¬` -> `€`, `â€™` -> `’`) in addition to Latin (`Ã©`), and reports exact lossy byte counts |
| `Services/DelimiterDetector.cs` & `DecimalDetector.cs` | `crates/csvrescue-core/src/detector.rs` | Detects `,`, `;`, `\t`, `|`, quote styles, CRLF/LF/CR mix, header shape, and `Dot`/`Comma` decimal marks without mutating input |
| `Services/CsvReader.cs` | `crates/csvrescue-core/src/parser.rs` | State-machine parser supporting RFC-4180 doubled quotes, backslash escapes, embedded newlines, stray mid-field quotes, unclosed EOF quotes, plus bounded streaming reader with `AtomicBool` cancellation |
| *(Partial diagnostics only)* | `crates/csvrescue-core/src/recovery.rs`, `repair.rs`, `preview.rs` | Adds configurable `Preserve` vs `AlignColumns` recovery strategies, confidence scoring, ambiguous wide-row protection, side-by-side Original vs Recovered row diffs, and manual cell overrides/reverts |
| `Services/ColumnAnalyzer.cs` | `crates/csvrescue-core/src/analysis.rs` | Adds signed leading-zero detection (`-0042`), 16+ digit IEEE-754 precision guard, `1E5`, date/fraction heuristics, and formula hazard counts per column |
| *(None)* | `crates/csvrescue-core/src/validation.rs` | Full audit for duplicate records, duplicate headers, empty cells, whitespace hazards, control characters, and spreadsheet formula injection |
| `Services/CsvWriter.cs` & `XlsxWriter.cs` | `crates/csvrescue-core/src/export.rs` | Retains zero-dependency OpenXML `.xlsx` (`numFmtId="49"` `@` text format, frozen bold header, XML 1.0 invalid-char stripping), adds dry-run preview, column selection, opt-in formula neutralization, and Markdown/JSON recovery reports |
| `Services/FileSafety.cs` & `AppPaths.cs` | `crates/csvrescue-core/src/security.rs`, `storage.rs`, `logging.rs`, `lib.rs` | Enforces 300 MiB file bound, canonical case-insensitive same-file check, atomic `.tmp` + `MoveFileExW` commit, rotating 2 MiB logs, and local JSON `settings.json` / `history.json` |

## 2. UI, Shell & Packaging Migration

| Legacy Component (.NET 10 / WPF) | v2.0.0 Replacement | Notes |
|---|---|---|
| `MainWindow.xaml` + `ViewModels/*` + `Controls/*` + `Themes/*` | `crates/csvrescue-app/` + `src-ui/` | Fluent 2 inspired workbench (`Obsidian Dark` & `Porcelain Light` + `System`), `RescueGauge`, virtualized grid, `InspectorDrawer` cell editor/diff, `ExportModal` dry-run preview, `Columns & Validation`, `History`, `Settings & Guide` |
| `CSVRescue.Tests/CsvTests.cs` (77 `.NET` assertions) | `crates/csvrescue-core/tests/legacy_compat_tests.rs` & `adversarial_and_security_tests.rs` | 100% parity ported to 12 compatibility tests + 6 adversarial/security/recovery tests |
| `dotnet publish` single-file (~59.1 MiB) + Inno Setup 6 | `cargo build --release` + `installer/csvrescue.iss` | Preserves `AppId={{7B9F7875-EC40-5640-B663-3D4A2CFD66AE}}` for seamless upgrades, eliminates bundled .NET runtime, adds portable Win64 `.zip` alongside setup `.exe` and `SHA256SUMS.txt` |

## 3. Intentionally Excluded / Preserved Boundaries

- No `.NET` runtime, COM/Excel automation, telemetry, auto-updater, or network calls.
- Removed WPF-only UIAutomation screenshot capture script (`scripts/capture-media.ps1`), superseded by headless Edge CDP verification and design checks.
