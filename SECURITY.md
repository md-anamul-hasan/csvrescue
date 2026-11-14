# Security Policy

CSVRescue processes untrusted CSV and delimited text files offline on Windows. Security boundaries are documented in [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md).

## Supported Versions

| Version | Runtime | Security Support |
|---|---|---|
| 2.0.x | Native Rust + Tauri 2 (x64) | Supported |
| < 2.0.0 | Legacy .NET / WPF | End of Life |

## Security Guarantees & Design Boundaries

1. **Zero Network / Telemetry:** CSVRescue opens no outbound sockets, collects no usage metrics, and performs no background update checks.
2. **Read-Only Source Guarantee:** Source files are validated before loading (`300 MiB` bound in [security.rs](crates/csvrescue-core/src/security.rs)) and never overwritten in place (`is_same_file` canonical/case-insensitive check blocks saving onto the active source path).
3. **Formula Injection Defense:** Cells starting with `=`, `+`, `-`, `@`, `\t`, or `\r` (excluding clean negative numbers like `-12.5`) are flagged cleanly; opt-in formula neutralization (`sanitizeFormulas`) prepends `'` deterministically and idempotently before CSV/XLSX serialization.
4. **Privilege & Binary Hardening:**
   - Standard-user execution (`asInvoker` manifest with `PerMonitorV2` DPI and `longPathAware`).
   - Early startup call to `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` to block current-working-directory DLL preloading.
   - Zero `unsafe` application business logic across the parser, detector, recovery, analysis, or serializer layers.
   - Strict Tauri Content Security Policy (`default-src 'self'`, `connect-src 'none'`, `object-src 'none'`, `frame-src 'none'`) and narrow capability allowlist (`dialog`, scoped `opener`).
   - Dependency audit enforced via `cargo audit` and `cargo deny check`.

## Reporting a Vulnerability

Please report security issues privately by emailing `contact@mdanamulhasan.com` or via GitHub Private Vulnerability Reporting rather than opening a public issue with exploit payloads. Include:
- Affected version (`CSVRescue.exe` version / commit SHA)
- Minimal reproduction input or malformed CSV sample
- Expected vs. actual behavior
