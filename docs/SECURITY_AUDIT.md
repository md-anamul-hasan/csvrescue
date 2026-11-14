# CSVRescue Security & Privacy Audit (v2.0.0)

This report documents the threat model, architectural controls, dependency audit, and verification results for CSVRescue v2.0.0.

## 1. Threat Model & Trust Boundaries

| Surface | Risk | Mitigation |
|---|---|---|
| Untrusted CSV input | Billion-row DoS, huge fields, malformed quoting, invalid UTF-8/UTF-16 | Bounded `MAX_BYTES` (`300 MiB` pre-read check), linear single-pass state-machine parser (`O(n)`), cancellation via `AtomicBool` across parsing and `.xlsx` generation (`adversarial_and_security_tests::adversarial_large_field_malformed_utf16_and_cancellation` verified 5 MiB quoted field + odd UTF-16 + invalid UTF-8) |
| Source data destruction | Overwriting the original CSV or corrupting exports on failure/cancellation | Case-insensitive canonical same-file guard (`security::is_same_file`), sibling temp file (`TempFileGuard`) cleaned up on error/cancel, atomic commit (`MoveFileExW` / replace semantics) |
| CSV Formula Injection | Payloads starting with `=`, `+`, `-`, `@`, `\t`, or `\r` executing inside Excel/Calc | `security::detect_formula_risk` excludes valid negative numbers (`-12.5`) and catches whitespace/tab/CR-prefixed formulas; idempotent `sanitize_formula_cell` prepends `'` without double-prefixing |
| WebView / IPC surface | Arbitrary filesystem reads, XSS script injection, or network exfiltration | CSP in `tauri.conf.json` (`default-src 'self'; script-src 'self'; connect-src 'self' ipc:;`), scoped `capabilities/default.json` (`core:default`, `dialog`, `opener`), no raw SQL or arbitrary shell execution |
| Windows binary loading | DLL preloading from Downloads/working directory, UAC escalation | `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` before Tauri initializes (`main.rs`), `asInvoker` manifest (`app.manifest`) |
| Malformed XML inside `.xlsx` | Control characters corrupting Excel OOXML readers | XML escaping (`&`, `<`, `>`, `"`, `'`) + stripping invalid XML 1.0 control chars (`< 0x20` other than `\t`, `\n`, `\r`, plus `\u{FFFE}`, `\u{FFFF}`) in `export::append_xml_escaped` |

## 2. Unsafe Rust Inventory

- **Business & Parsing Engine (`crates/csvrescue-core`):** Minimal scoped Win32 FFI (`MoveFileExW` in `security.rs` with drop/rollback safety, `GetLocalTime` in `logging.rs`, `WorkingSetSize` query in `examples/bench.rs`). Zero raw pointer arithmetic or unsafe code anywhere in parsing, decoding, recovery, analysis, or `.xlsx`/`.csv` serialization.
- **Desktop Host (`crates/csvrescue-app`):** Single Win32 FFI call (`SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` in `main.rs`) hardening DLL search paths at startup.

## 3. Supply-Chain & Advisory Audit

During the migration audit, `cargo audit` and `cargo deny check` detected 4 high/critical extraction-related advisories in `zip 2.6.1` (`RUSTSEC-2026-0367`, `0368`, `0369`, `0429`). Even though CSVRescue only constructs `.xlsx` ZIP archives during export, `zip` was upgraded to **`9.5.1`** (`>= 9.0.0`).

Latest verification results on `Cargo.lock` + `deny.toml` (`x86_64-pc-windows-msvc`):
- **`cargo deny check`:** `advisories ok, bans ok, licenses ok, sources ok` (exit code `0`).
- **`cargo audit`:** `0` vulnerabilities found (exit code `0`; 5 upstream warnings only in unlinked Linux GTK/D-Bus crates and build-time `proc-macro-error`).
- **Node.js / npm:** Used strictly at build time (`src-ui/`) to bundle static TypeScript/React assets into `CSVRescue.exe`; no Node.js runtime or `node_modules` ships with the application.
