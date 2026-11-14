# CSVRescue — User Guide (v2.0.0)

CSVRescue repairs damaged, garbled, or inconsistently formatted CSV files offline on Windows and exports clean **Excel (.xlsx)** or **CSV** files without changing your original data.

## Opening & Inspecting a File

- Drag and drop any `.csv`, `.tsv`, `.txt`, `.tab`, `.psv`, or `.dat` file onto the window, click **Open File**, or launch `CSVRescue.exe <path>`.
- Three built-in **Sample Presets** let you test European semicolons/commas, UTF-8 mojibake & malformed quotes, and formula/leading-zero hazards without touching real files.
- Files up to **300 MiB** are supported offline with zero network connections.

## Automatic Detection & Overrides

| Control | Automatic Behavior | When to Override |
|---|---|---|
| **Encoding** | Checks UTF-8 / UTF-16 / UTF-32 BOM, UTF-16 zero-byte signature, strict UTF-8, then Windows-1252 | Characters appear wrong: choose Windows-1250/1251/1253/1254/1256/1257, ISO-8859-1/2/15, Shift-JIS, GBK, Big5, EUC-KR, or IBM437 |
| **Mojibake Repair** | Automatically fixes text saved as UTF-8 twice (`Ã©` -> `é`, `â‚¬` -> `€`, `M8 Ã— 20` -> `M8 × 20`) when every byte decodes cleanly | Disable *Repair double UTF-8* if literal source sequences must remain untouched |
| **Delimiter & Quotes** | Scores `,`, `;`, `\t`, and `|` on unquoted spans; supports RFC-4180 doubled quotes and backslash escapes | Switch delimiter, toggle *First row is header*, or toggle backslash escaping in the top bar |
| **Recovery Strategy** | *Preserve As Parsed* keeps ragged rows faithfully; *Align Columns* pads short rows and merges uniquely identified split fields without guessing | Ambiguous splits stay tagged **AMBIGUOUS** so you can inspect original vs recovered rows side-by-side and override cells manually |

## Protecting Data from Excel & Spreadsheet Hazards

- **Text vs. Number Columns:** A column is marked `Number` only when every non-empty value is a clean number (no leading zeros like `00123` or `-0042`, max 15 digits, no `1E5`, no `01-02` / `1/2` date patterns). Click any column badge in the grid or **Columns & Validation** tab to switch `Text` / `Number`.
- **Non-Numeric Values in Number Columns:** Never dropped—e.g., `"seven"` in a forced `Number` column is written safely as text while valid numbers store natively.
- **Formula Injection Protection:** Cells beginning with `=`, `+`, `-`, `@`, `\t`, or `\r` (except plain negative numbers like `-12.5`) are flagged as **FORMULA** hazards. Enable **Sanitize formulas (`'`)** during export to neutralize active spreadsheet formulas.

## Exporting Safely

- **Save Safe .xlsx:** Writes a pure OpenXML workbook with `@` text cells (`numFmtId="49"`), native numeric values, frozen bold headers, and stripped XML-invalid control characters. Supports up to `1,048,576` rows and `16,384` columns.
- **Export Clean CSV:** Supports UTF-8 (BOM / no BOM), UTF-16 LE (with BOM), and Windows-1252, custom delimiter, CRLF/LF/CR line endings, optional Excel `="00123"` formula wrapping, column selection, and live dry-run preview before saving.
- **Recovery Report:** Export a `.md` or `.json` summary detailing encoding, repairs applied, ambiguous rows, manual edits, and validation findings.
- **Original File Safety:** CSVRescue never modifies the opened file and refuses saving to the same path.

## Logs & Data Directory

Application settings (`settings.json`), local export history (`history.json`), and rotating diagnostics (`logs/csvrescue.log`, `error.log`) stay locally inside `%APPDATA%\CSVRescue`. Open an [issue on GitHub](https://github.com/md-anamul-hasan/csvrescue/issues) if you encounter a bug (remove sensitive data before attaching logs).
