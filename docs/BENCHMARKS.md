# CSVRescue Benchmarks: Legacy .NET 10 vs. Native Rust v2.0.0

All measurements below were executed locally on the same Windows 11 x64 workstation under `--release` / Release AOT-singlefile configuration against a 200,000-row European semicolon-delimited Windows-1252 fixture containing mixed leading-zero part numbers, decimal commas, date-like cells, and `×` Win-1252 characters.

Note: In addition to matching every detection, parsing, and export invariant from the `.NET 10` baseline, the Rust v2.0.0 `CsvDocument::load` path also runs full structural recovery diagnostics, duplicate-row hashing, whitespace/control-character validation, and spreadsheet formula hazard scanning across all 200,000 rows (`1,000,000` cells).

## 1. Binary & Distribution Footprint

| Artifact | .NET 10 (v1.1.0 Baseline) | Rust + Tauri 2 (v2.0.0) | Delta |
|---|---|---|---|
| Desktop Executable (`CSVRescue.exe`) | `61,955,096 B` (`59.09 MiB`) | `5,646,848 B` (`5.39 MiB`) | **-90.9% (11.0x smaller)** |
| Inno Setup Installer (`CSVRescue-Setup-*.exe`) | `56,765,785 B` (`54.14 MiB`) | `3,935,793 B` (`3.75 MiB`) | **-93.1% (14.4x smaller)** |
| Portable Win64 Archive (`.zip`) | *(not shipped)* | `2,365,037 B` (`2.26 MiB`) | New |
| Bundled Runtime Dependencies | Self-contained .NET 10 CLR | Zero (OS WebView2 only) | Removed .NET & C# |

## 2. 200,000-Row Throughput & Working Set

- **Legacy `.NET 10` Fixture:** `7,117,199` bytes (`200,000` data rows × `5` columns)
- **Rust v2.0.0 Fixture (`bench.rs`):** `7,920,842` bytes (`200,000` data rows × `5` columns, generating `8,520,845 B` UTF-8 CSV and `6,116,220 B` compressed `.xlsx`)

### Observed Results

| Run | .NET 10 Baseline (v1.1.0, Run 2) | Rust v2.0.0 (Run 1) | Rust v2.0.0 (Run 2) | Rust v2.0.0 (Run 3) |
|---|---|---|---|---|
| **Load + Analyze (+ Validation)** | `305.0 ms` | `352.0 ms` | `359.2 ms` | `339.7 ms` |
| **Export Clean CSV** | `118.0 ms` | `53.1 ms` | `40.4 ms` | `51.2 ms` |
| **Export Safe `.xlsx`** | `428.0 ms` | `418.9 ms` | `417.6 ms` | `406.2 ms` |
| **Total End-to-End** | `851.0 ms` | `824.0 ms` | `817.2 ms` | `797.1 ms` |
| **Process Working Set (RAM)** | `452.7 MB` | `191.9 MB` | `192.2 MB` | `192.2 MB` |

### Summary Takeaways

- **Peak Memory Usage:** Reduced from **`452.7 MB`** to **`192.2 MB`** (**57.5% reduction** in working set RAM) while retaining full immutable original + repaired rows for side-by-side diffing and cell rollback.
- **CSV Serialization Throughput:** Reduced from **`118.0 ms`** to **`40.4 ms`–`53.1 ms`** (**2.2x–2.9x faster**).
- **XLSX Serialization Throughput:** Reduced from **`428.0 ms`** to **`406.2 ms`–`418.9 ms`** using zero-C-dependency Rust `zip 9.5.1` (`deflate` via `flate2`/`zlib-rs`).

Reproduce locally:

```powershell
cargo run --release -p csvrescue-core --example bench
```
