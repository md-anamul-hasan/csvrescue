# Changelog

All notable changes to CSVRescue follow [Semantic Versioning](https://semver.org/). The workspace version is defined in `Cargo.toml` and `crates/csvrescue-app/tauri.conf.json`.

## [2.1.0] - 2026-11-19

### Changed
- New look: Technical Glassmorphism v1.0, the design system of mdanamulhasan.com. Slate canvas, Precision Cobalt accent with PLM teal highlights in dark mode, hairline borders, crisp 6/10/14 px corners, restrained shadows.

## [2.0.0] - 2026-11-14

Complete native **Rust + Tauri 2.x** rewrite and Fluent 2 UI redesign for Windows 10/11 (x64). Eliminates all `.NET`, `C#`, and `WPF` runtime code.

### Added
- Native Rust modular core engine (`csvrescue-core`): decoding (20 encodings), mojibake repair (including 3-byte `â‚¬` / `€` sequences), delimiter/line-ending/header detection, tolerant & streaming parsing, validation, and OpenXML `.xlsx` / `.csv` serialization.
- Configurable recovery strategies (`Preserve` vs `AlignColumns`) with confidence scoring (`High` / `Medium` / `Low` / `Ambiguous`) and protection against silent ambiguous column merges.
- Side-by-side Original vs. Recovered cell inspection drawer with manual cell override and single-cell revert.
- Comprehensive validation report (duplicate rows, duplicate headers, empty cells, whitespace/control chars, Excel coercion risks, and spreadsheet formula injection checks).
- Opt-in idempotent formula neutralization (`'`) and live dry-run export preview (`ExportModal`), plus Markdown and JSON recovery reports.
- Redesigned accessible Fluent 2 desktop workbench (`Dark`, `Light`, `System`), `RescueGauge`, interactive row filters, column resizing, `History` ledger, and portable Win64 `.zip` distribution alongside the Inno Setup installer.

### Changed
- Replaced `.NET 10 / WPF` single-file runtime (~59.1 MiB binary, ~453 MB peak WorkingSet on 200k rows) with native Rust + OS WebView2 host (`csvrescue-app`), cutting 200k-row memory usage by >55% and accelerating CSV serialization by >2x.
- Upgraded OpenXML ZIP dependency cleanly to `zip 9.5.1` (`cargo audit` and `cargo deny check` passing with zero known advisories).
- Signed leading-zero identifiers (`-0042`) now trigger Excel `LEADING_ZEROS` protection alongside unsigned IDs (`00123`).

### Removed
- `.NET 10` SDK / WPF codebase, legacy `Services/`, `ViewModels/`, `Controls/`, `Converters/`, `Themes/`, and WPF UIAutomation screenshot tooling.

## [1.1.0] - 2026-10-22

First open-source release (MIT). Removed licensing entirely and unlocked all features.
