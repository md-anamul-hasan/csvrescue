# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(Rendered inside a native Windows desktop shell: Tauri 2 with WebView2. The design language follows Windows 11, not a website.)

## Users

People whose spreadsheets, imports, ERP exports, or databases break on malformed or misencoded CSV files: analysts, operations/finance staff, accountants, engineers, and anyone moving delimited text between ERP systems, UTF-8 tools, European locales, and Microsoft Excel. They arrive with a broken or risky file ("accents turned into Ã©", "everything landed in column A", "leading zeros and 16-digit IDs were destroyed by Excel", "stray quotes or ragged rows broke my import") and need to inspect, repair, validate, and safely re-export it without altering the original file.

## Product Purpose

CSVRescue diagnoses encoding, delimiter, decimal mark, line endings, malformed quoting, double-encoded UTF-8 mojibake, ragged rows, spreadsheet formula injection hazards, and Excel data-loss risks (`00123`, 16+ digit numbers, `1E5`, date-like `1-2`, fractions `1/2`), lets users inspect before/after changes and override column types or individual cells, and exports clean CSV or native OpenXML `.xlsx` workbooks with strict preservation of original data.

## Positioning

A diagnostic recovery workbench, not a blind bulk converter: CSVRescue decodes and explains how every decision was made, separates original bytes from recovered values and exported output, preserves identifiers and leading zeros as text by default, detects spreadsheet formula injection (`=`, `+`, `-`, `@`), warns on ambiguous structural repairs rather than silently inventing data, and never overwrites the source file. Free and open source (MIT), 100% offline, zero telemetry or network requests.

## Operating Context

- Runs on Windows 10 (build 19041+) and Windows 11, 64-bit (`x86_64-pc-windows-msvc`), unprivileged (`asInvoker`).
- Supports file browser, drag-and-drop onto the window, CLI argument (`CSVRescue.exe <file>`), and Windows Explorer *Open with*.
- Typical files range from small ERP/accounting exports to large multi-hundred-MB datasets (up to 300 MB).
- Workflows are task-shaped: open file -> inspect detected format/encoding & problems -> adjust overrides or manual cell repairs -> review column types & validation -> dry-run preview -> save clean `.xlsx`, clean `.csv`, or recovery report.

## Capabilities and Constraints

- Encodings: Auto-detects BOM (`UTF-8`, `UTF-16 LE/BE`, `UTF-32 LE/BE`), BOM-less UTF-16 null-byte patterns, strict UTF-8 / ASCII, with Windows-1252 fallback; supports 20 explicit encodings including Windows-1250/1251/1252/1253/1254/1256/1257, ISO-8859-1/2/15, Shift-JIS, GBK, Big5, EUC-KR, and IBM437; repairs run-by-run double-encoded UTF-8 mojibake when clean.
- Detection & Parsing: Auto-detects comma, semicolon, tab, pipe, and colon delimiters, decimal dot vs comma, CRLF/LF/CR line endings, header row presence; handles quoted fields, doubled quotes, embedded newlines, stray mid-field quotes, unclosed quotes, empty lines, and ragged rows.
- Recovery & Validation: Configurable repair strategies (standard preservation vs ragged column alignment with confidence levels and ambiguity warnings), manual cell edit & revert, duplicate row & duplicate header checks, whitespace & control-character diagnostics, and spreadsheet formula injection detection with optional `'` prefix sanitization on export.
- Export: Atomic write to a new path (never overwriting source); `.xlsx` (up to 1,048,576 rows × 16,384 columns, text cells stored as `inlineStr` `@` format, numbers as numeric `<v>`, bold frozen header, auto-fitted column widths) and configurable `.csv` (delimiter, encoding, BOM, line ending, `DecimalAsDot`, `ExcelTextProtect`, `SanitizeFormulas`, column selection), plus JSON/Markdown recovery reports.
- Terminology: "encoding", "delimiter", "decimal mark", "mojibake", "ragged row", "Text / Number", "Excel risk", "formula risk", "dry-run", "recovery report".

## Brand Commitments

- Name: CSVRescue. Publisher: Md Anamul Hasan. License: MIT.
- Existing icon: `Assets/icon.svg` / `Assets/icon.ico` / `Assets/logo.png` (carried over into `crates/csvrescue-app/icons/`).
- Voice: plain, calm, factual English; explains *why* a file broke and *what* changed without jargon overload or hype.
- Design and architecture sibling reference: `D:\00_DEV\Projects\wdm` (gold standard) and `D:\00_DEV\Projects\longpathfix` (2.0.0 Rust + Tauri pattern).

## Evidence on Hand

- Verified 1.1.0 implementation & behavior: `README.md`, `docs/USER_GUIDE.md`, `docs/PRIVACY.md`, `Services/*.cs`, `CSVRescue.Tests/CsvTests.cs` (77 passing tests), and measured 1.1.0 benchmark on 200k rows.
- No invented testimonials, user counts, or unmeasured performance claims.

## Product Principles

1. Never overwrite or mutate the original source file.
2. Never silently invent, discard, reorder, or corrupt data; distinguish original bytes, recovered values, and exported output.
3. Keep identifiers, part numbers, ZIP codes, and long numeric strings intact as Text unless every value is a safe clean number.
4. Expose uncertainty and security risks explicitly: report ambiguous repairs, lossy decodes, and spreadsheet formula injection hazards.
5. Private by construction: offline only, zero network connections or telemetry.

## Accessibility & Inclusion

WCAG 2.2 AA: full keyboard navigation, visible `:focus-visible` focus rings, screen-reader landmarks/labels/live status updates, `prefers-reduced-motion` support, Windows high-DPI scaling (`PerMonitorV2`), and high-contrast legible light/dark/system themes.
