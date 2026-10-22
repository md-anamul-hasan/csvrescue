# Changelog

All notable changes to CSVRescue. Versioning follows [Semantic Versioning](https://semver.org/); the version lives in `Directory.Build.props`.

## [1.1.0] - 2026-10-22

First open-source release (MIT). Earlier builds were a paid product; this release removes licensing entirely, so every feature is free.

### Added
- Detects encoding, delimiter and decimal mark, with overrides; repairs text saved as UTF-8 twice.
- Per-column Text/Number choice that keeps IDs, part numbers and leading zeros intact.
- Saves a clean Excel (.xlsx) or CSV file; the original is never changed.
- Reports malformed rows, empty lines and stray quotes.
- Light and dark themes; self-contained installer.

### Removed
- Licence-key activation, the licence dialog, evaluation limits and the licence-server connection. Everything is unlocked.
