---
version: 1
slug: "src-ui-src-app-tsx"
primary_target: "src-ui/src/App.tsx"
related_targets: []
---

## Direction contract

- **THESIS:** Precision Windows CSV diagnostic and data-recovery workbench built around an explicit encoding/delimiter/decimal detector, calibrated `RescueGauge` integrity breakdown, spreadsheet-grade diffing preview grid, and non-destructive Excel/CSV export (`seed:8f20485422226912`).
- **OWN-WORLD:** Native Windows 11 desktop `operate` workspace aligned with Fluent 2, Windows Terminal, and Sysinternals tooling: restrained `224px` left navigation rail, crisp `1px` hairline structural borders, obsidian/slate dark & porcelain light surfaces (no neon glass or decorative blur), Windows Cobalt `#1d6fe8` / `#0f62d6` primary accent, `Cascadia Mono` with `tabular-nums` alignment for cell data and counts, and semantic status tokens restricted strictly to diagnostic states.
- **STORY:** Built for finance, operations, data analysts, and engineers whose ERP exports or UTF-8 CSVs break in Excel (garbled `Ã©` mojibake, collapsed semicolon columns, stripped leading zeros `00123`, truncated 16-digit IDs, unescaped quotes, ragged rows, or dangerous `=cmd|` formula cells); CSVRescue explains every detection, recovers safely without mutating the source file, and lets users inspect or edit cells before exporting clean `.xlsx` or `.csv`.
- **FIRST VIEWPORT:** `Rescue & Preview` opens immediately into an interactive workbench: top file input & sample/recent selector (`Open File…`, drag-and-drop), compact decoding/delimiter/decimal/repair controls, the `RescueGauge` integrity bar with interactive row filter pills (`All Rows`, `Repaired`, `Ragged / Quotes`, `Excel Risk`, `Formula Risk`, `Manual Edits`), and the filterable spreadsheet data grid with clickable `TEXT` / `NUMBER` column badges, side `InspectorDrawer` for cell diffing & manual overrides, and `ExportModal` dry-run preview.
- **FORM:** Code-led `operate` architecture (`seed:8f20485422226912`), token-driven CSS (`tokens.css` + `app.css`) supporting system/dark/light themes, WCAG 2.2 AA contrast, full keyboard navigation, `prefers-reduced-motion`, `1.6px` stroke inline SVGs, styled scrollbars/`::selection`/`:focus-visible`, zero eyebrows/kickers above headings, zero decorative blur, zero placeholder UI.
- **FINISH:** unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.
