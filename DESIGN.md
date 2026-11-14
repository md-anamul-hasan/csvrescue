---
name: CSVRescue
description: Precision Windows CSV diagnostic and data-recovery workbench built on Fluent 2 hairline structure and calibrated status tokens
colors:
  windows-cobalt: "#1d6fe8"
  windows-cobalt-hover: "#3180f5"
  obsidian-canvas: "#0e1116"
  slate-surface: "#151a22"
  elevated-slate: "#1c222d"
  hairline-border: "#262f3d"
  strong-border: "#364357"
  primary-ink: "#f0f4f8"
  secondary-ink: "#9ba7b8"
  status-clean: "#23a55a"
  status-warning: "#d98b14"
  status-danger: "#e04f5f"
typography:
  headline:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.45
    letterSpacing: "-0.01em"
  title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.45
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: 1.45
  mono:
    fontFamily: "Cascadia Mono, Cascadia Code, Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "tnum"
rounded:
  sm: "4px"
  md: "6px"
  lg: "8px"
spacing:
  xs: "6px"
  sm: "10px"
  md: "14px"
  lg: "20px"
components:
  button-primary:
    backgroundColor: "{colors.windows-cobalt}"
    textColor: "#ffffff"
    rounded: "{rounded.md}"
    padding: "6px 12px"
  button-primary-hover:
    backgroundColor: "{colors.windows-cobalt-hover}"
  button-secondary:
    backgroundColor: "{colors.elevated-slate}"
    textColor: "{colors.primary-ink}"
    rounded: "{rounded.md}"
    padding: "6px 12px"
  input-field:
    backgroundColor: "{colors.obsidian-canvas}"
    textColor: "{colors.primary-ink}"
    rounded: "{rounded.md}"
    height: "30px"
  surface-panel:
    backgroundColor: "{colors.slate-surface}"
    textColor: "{colors.primary-ink}"
    rounded: "{rounded.lg}"
    padding: "14px 16px"
---

# Design System: CSVRescue

## Overview

**Creative North Star: "The Precision Diagnostic Instrument"**

CSVRescue follows native Windows 11 desktop tooling idioms—high-density Fluent 2 hairlines, restrained obsidian slate and porcelain light surfaces, monospace tabular numerics, and unambiguous semantic status signals. Rather than marketing fluff or decorative chrome, every control exists to help analysts inspect raw encoding signals, trace mojibake or column splits back to source bytes, manually verify ambiguous rows, and export verified workbooks safely.

The aesthetic avoids floating card clutter, heavy drop shadows, neon gradients, side-tab edge stripes, and translucent blur effects. Instead, functional hierarchy comes from crisp `1px` borders, restrained tonal stepping between canvas and panels, and strict conservation of saturated accents.

**Key Characteristics:**
- Monospaced tabular typography (`Cascadia Mono` with `tabular-nums`) for all spreadsheet cell values, byte counts, and row indices.
- Dual-theme system (`dark`, `light`, `system`) meeting WCAG 2.2 AA contrast on every interactive label, placeholder (`>= 4.5:1`), and status pill.
- Direct manipulation: clickable `TEXT`/`NUMBER` column pills, column width drag resizing, live dry-run preview before saving, and side-by-side row diffing.

## Colors

The palette pairs cool technical slates (`dark`) and paper whites (`light`) with Windows Cobalt as the sole navigation and primary action accent, reserving green, amber, and crimson strictly for data-integrity telemetry.

### Primary
- **Windows Cobalt** (`#1d6fe8` dark / `#0f62d6` light): Reserved for primary export/apply actions, checked states, active filter ring highlights, and focus rings (`:focus-visible`).

### Neutral
- **Obsidian Canvas** (`#0e1116` dark / `#f5f7fa` light): Root backdrop under scrolling panes and form input wells.
- **Slate Surface** (`#151a22` dark / `#ffffff` light): Sidebar rail, workspace headers, panels, dialogs, and grid base surface.
- **Elevated Slate** (`#1c222d` dark / `#eef2f7` light): Table headers (`<thead>`), secondary buttons, footer action bars, and active document badges.
- **Hairline Border** (`#262f3d` dark / `#d6dee8` light): Universal `1px` structural divider across navigation, grid cells, and panels.
- **Primary Ink** (`#f0f4f8` dark / `#11161f` light) & **Secondary Ink** (`#9ba7b8` dark / `#4b5668` light): Body copy, metadata labels, and high-contrast input placeholders.

### Named Rules
**The Diagnostic Signal Rule.** Clean green (`#23a55a`), repaired blue (`#1d6fe8`), Excel-risk amber (`#d98b14`), and formula-hazard crimson (`#e04f5f`) appear only inside `RescueGauge`, cell status badges, column type pills, and validation findings—never as decorative chrome.

## Typography

**Display & Body Font:** `'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif`
**Data / Code Font:** `'Cascadia Mono', 'Cascadia Code', 'Consolas', monospace`

**Character:** Compact, utilitarian Windows UI sans paired with crisp tabular monospace (`font-variant-numeric: tabular-nums`) so numeric columns, leading-zero identifiers (`00123`), and byte sizes align vertically across thousands of rows.

### Hierarchy
- **Headline** (600, `16px`, `1.45` line-height, `-0.01em`): Top file title in workspace header.
- **Title** (600, `14px`, `1.45`): Section panel titles and modal/drawer headings.
- **Body** (400, `13px`, `1.45`): Controls, banner diagnostics, navigation labels, and prose.
- **Mono Data** (400, `12.5px`, `tabular-nums`): Grid cells, row numbers, dry-run previews, and original/recovered cell diffs.
- **Functional Pill / Label** (600, `11px`): Column type buttons (`TEXT`/`NUMBER`), cell risk tags (`TEXT SAFE`, `UTF-8`, `FORMULA`), and control labels (minimum `11px` floor everywhere).

## Layout

Desktop uses a sticky `224px` left navigation rail paired with a vertically flexing workbench (`100vh`) whose spreadsheet grid scrolls independently on both axes. Below `820px`, the sidebar collapses into a wrapping top strip and `.control-strip` reflows into a balanced two-column grid so the viewport maintains zero horizontal page overflow down to `390px`.

## Elevation & Depth

Surfaces communicate hierarchy through tonal contrast and crisp `1px` hairline borders at rest (`--border-hairline` and `--border-strong`), avoiding floating card elevation or backdrop blur. Subtle elevation shadows (`0 6px 20px rgba(0,0,0,0.45)`) are reserved exclusively for modal overlays (`ExportModal`), slide-over drawers (`InspectorDrawer`), and transient toast notifications.

## Shapes

Compact desktop geometry (`4px` inner toggles and cell badges, `6px` buttons/inputs/pills, `8px` panels/modals) with `1px` uniform strokes and zero asymmetric thick side borders.

## Components

### Buttons
- **Shape:** Subtle rectangular desktop rounding (`6px` standard, `4px` inline micro-controls).
- **Primary:** Windows Cobalt fill (`#1d6fe8`) with crisp inverse white text (`6px 12px` padding).
- **Hover / Focus:** Cobalt Light (`#3180f5`) on hover; high-visibility `2px solid` Cobalt focus outline offset `2px` on `:focus-visible`.
- **Secondary:** Elevated Slate (`#1c222d`) with Strong Border (`#364357`) and Primary Ink text.

### Inputs / Fields
- **Style:** `30px` height with `1px solid` Strong Border on Obsidian Canvas background (`6px` radius) and high-contrast Secondary Ink placeholders (`opacity: 1`).
- **Focus:** External `2px` Cobalt focus ring matching keyboard navigation standards.

### Navigation
- Left `224px` rail featuring `16px` stroke icons (`1.6px` stroke width), subtle selection fill (`#192b49` dark / `#e3edfc` light) with Cobalt border highlight, inline warning/count pills, and bottom active document card + 3-way theme segmented switch (`System` / `Dark` / `Light`).

### RescueGauge
- Composite health indicator showing weighted proportions of Clean, Repaired, Excel Text-Safe, and Formula Trigger cells above interactive single-click row filter pills (`All Rows`, `All Issues`, `Repaired / Ragged`, `Excel Risk`, `Formula Risk`, `Manual Edits`, `Duplicates`).

## Do's and Don'ts

### Do:
- **Do** enforce the `11px` minimum font size across every interactive pill, badge, and functional label (`impeccable detect` verified).
- **Do** render spreadsheet cell data and row counts in monospace `tabular-nums`.
- **Do** validate placeholder contrast against dark canvas (`>= 4.5:1`).

### Don't:
- **Don't** add `3px` colored left stripes (`border-left-width: 3px`) on cards or rows—use uniform `1px` borders or subtle semantic badges.
- **Don't** add marketing eyebrow labels, decorative glass blur, or animations that ignore `@media (prefers-reduced-motion: reduce)`.
