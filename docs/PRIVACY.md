# CSVRescue Privacy Policy

_Last updated: 2026-11-14_

CSVRescue v2.0.0 runs 100% locally on your Windows PC. It opens no network sockets, sends no telemetry, performs no crash uploads, and executes no background update checks.

- **Read-Only Inputs:** CSVRescue reads only the file you open and refuses overwriting the source file when exporting.
- **Local State Only:** Preferences (`settings.json`), export history (`history.json`), and local error logs (`logs/csvrescue.log`, `error.log`) are stored strictly in `%APPDATA%\CSVRescue` and can be cleared at any time from the History panel or by deleting that folder.
- **Restrictive Runtime CSP:** The bundled UI enforces `connect-src 'self' ipc:` with zero external scripts, fonts, or API origins.

Questions: `contact@mdanamulhasan.com`
