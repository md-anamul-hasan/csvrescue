import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppConfig,
  BootstrapPayload,
  ColumnInfo,
  ColumnKind,
  CsvSettings,
  DocumentSummaryDto,
  ExportDryRun,
  ExportOptions,
  ExportSummary,
  HistoryEntry,
  PreviewCellDto,
  PreviewPageDto,
  PreviewQuery,
  PreviewRowDto,
  RowComparisonDto,
  SamplePreset,
} from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isTauriRuntime = (): boolean =>
  typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

const DEFAULT_ENCODINGS = [
  "UTF-8",
  "UTF-16 LE",
  "UTF-16 BE",
  "UTF-32 LE",
  "UTF-32 BE",
  "Windows-1252 (Western Europe)",
  "ISO-8859-1 (Latin 1)",
  "ISO-8859-15 (Latin 9)",
  "Windows-1250 (Central Europe)",
  "ISO-8859-2 (Central Europe)",
  "Windows-1251 (Cyrillic)",
  "Windows-1253 (Greek)",
  "Windows-1254 (Turkish)",
  "Windows-1256 (Arabic)",
  "Windows-1257 (Baltic)",
  "Shift-JIS (Japanese)",
  "GBK (Simplified Chinese)",
  "Big5 (Traditional Chinese)",
  "EUC-KR (Korean)",
  "IBM437 (old DOS)",
];

// Lightweight browser-only mock store used only when opened outside Tauri (e.g. Vite preview / headless UI inspection).
const mockState: {
  config: AppConfig;
  preset: SamplePreset;
  settings: CsvSettings;
  manualEdits: Map<string, string>;
  kindOverrides: Map<number, ColumnKind>;
  history: HistoryEntry[];
} = {
  config: {
    theme: "dark",
    defaultOutDelimiter: ",",
    defaultUtf8Bom: true,
    defaultDecimalAsDot: false,
    defaultExcelProtect: false,
    defaultSanitizeFormulas: false,
    defaultRepairDoubleEncoding: true,
    recentFiles: [],
  },
  preset: "europeanErp",
  settings: {
    encoding: "Auto",
    delimiter: null,
    mark: "auto",
    hasHeader: true,
    repairDoubleEncoding: true,
    skipEmptyLines: true,
    escapeMode: "doubledQuote",
    repairStrategy: "preserve",
  },
  manualEdits: new Map(),
  kindOverrides: new Map(),
  history: [
    {
      id: "hist-demo-1",
      timestamp: "2026-11-14 09:12:04",
      sourcePath: "C:\\Exports\\kunden-export.csv",
      outputPath: "C:\\Exports\\kunden-export-clean.xlsx",
      format: "XLSX",
      encodingName: "Windows-1252 (Western Europe)",
      delimiter: ";",
      rowsWritten: 9,
      columnsWritten: 6,
      mojibakeRepaired: 0,
      formulasSanitized: 0,
    },
  ],
};

function buildMockDataset(preset: SamplePreset): {
  fileName: string;
  encodingName: string;
  encodingNote: string;
  delimiter: string;
  delimiterName: string;
  decimalMark: "comma" | "dot";
  headers: string[];
  rawRows: string[][];
  repairedRows: string[][];
  kinds: ColumnKind[];
} {
  if (preset === "mojibakeAndQuotes") {
    return {
      fileName: "sample-mojibake-ragged.csv",
      encodingName: "UTF-8",
      encodingNote: "Valid UTF-8 with double-encoded mojibake repaired.",
      delimiter: ",",
      delimiterName: "Comma",
      decimalMark: "dot",
      headers: ["sku", "description", "qty", "unit_price", "notes"],
      rawRows: [
        ["sku", "description", "qty", "unit_price", "notes"],
        ["00810", "CafÃ© espresso machine", "2", "429.90", "Delivered in MÃ¼nchen"],
        ["00811", "5\" stainless flange", "14", "18.50", "Unquoted inch mark"],
        ["00812", "Precision kit", " 1E5 rating", "3", "99.00", "Comma inside unquoted field"],
        ["00813", "NaÃ¯ve rÃ©sumÃ© pack", "5", "12.00", "Multi-line\nnote inside quotes"],
        ["00814", "Truncated trailing row"],
      ],
      repairedRows: [
        ["sku", "description", "qty", "unit_price", "notes", ""],
        ["00810", "Café espresso machine", "2", "429.90", "Delivered in München", ""],
        ["00811", "5\" stainless flange", "14", "18.50", "Unquoted inch mark", ""],
        ["00812", "Precision kit", " 1E5 rating", "3", "99.00", "Comma inside unquoted field"],
        ["00813", "Naïve résumé pack", "5", "12.00", "Multi-line\nnote inside quotes", ""],
        ["00814", "Truncated trailing row", "", "", "", ""],
      ],
      kinds: ["text", "text", "text", "text", "text", "text"],
    };
  }
  if (preset === "formulaAndTypes") {
    return {
      fileName: "sample-security-audit.csv",
      encodingName: "UTF-8",
      encodingNote: "Plain ASCII text.",
      delimiter: ",",
      delimiterName: "Comma",
      decimalMark: "dot",
      headers: ["account_id", "customer", "balance", "formula_check", "fraction_code"],
      rawRows: [
        ["account_id", "customer", "balance", "formula_check", "fraction_code"],
        ["12345678901234567", "Acme Holdings", "-124.50", "=CMD|' /C calc'!A0", "1/2"],
        ["00094812", "Nordic Supply", "842.00", "+SUM(1,2)", "3-4"],
        ["00094813", "Delta Logistics", "150.25", "@SUM(1+1)*cmd", "1E5"],
        ["00094814", "Beacon Retail", "0.00", "\t=IMPORTXML()", "0042"],
      ],
      repairedRows: [
        ["account_id", "customer", "balance", "formula_check", "fraction_code"],
        ["12345678901234567", "Acme Holdings", "-124.50", "=CMD|' /C calc'!A0", "1/2"],
        ["00094812", "Nordic Supply", "842.00", "+SUM(1,2)", "3-4"],
        ["00094813", "Delta Logistics", "150.25", "@SUM(1+1)*cmd", "1E5"],
        ["00094814", "Beacon Retail", "0.00", "\t=IMPORTXML()", "0042"],
      ],
      kinds: ["text", "text", "number", "text", "text"],
    };
  }
  return {
    fileName: "sample-kunden-export.csv",
    encodingName: "Windows-1252 (Western Europe)",
    encodingNote: "Not valid UTF-8, so Windows-1252 is assumed.",
    delimiter: ";",
    delimiterName: "Semicolon",
    decimalMark: "comma",
    headers: ["Kundennummer", "Name", "Stadt", "Umsatz", "Datum", "Artikelnummer"],
    rawRows: [
      ["Kundennummer", "Name", "Stadt", "Umsatz", "Datum", "Artikelnummer"],
      ["00123", "Müller GmbH", "Köln", "1.234,50", "03.10.2026", "00450-A"],
      ["00124", "Schäfer & Söhne", "München", "987,00", "04.10.2026", "00451-B"],
      ["00125", "Café Zürich", "Zürich", "15.320,75", "05.10.2026", "00452-C"],
      ["00126", "Östlund AB", "Göteborg", "2.499,99", "06.10.2026", "00453-D"],
      ["00127", "Nuñez y Hijos", "Sevilla", "310,40", "07.10.2026", "00454-E"],
      ["00128", "Dvořák s.r.o.", "Praha", "7.805,10", "08.10.2026", "00455-F"],
      ["00129", "Lukasz Sp. z o.o.", "Kraków", "1.020,00", "09.10.2026", "00456-G"],
      ["00130", "Renée & Fils", "Lyon", "4.512,30", "10.10.2026", "00457-H"],
    ],
    repairedRows: [
      ["Kundennummer", "Name", "Stadt", "Umsatz", "Datum", "Artikelnummer"],
      ["00123", "Müller GmbH", "Köln", "1.234,50", "03.10.2026", "00450-A"],
      ["00124", "Schäfer & Söhne", "München", "987,00", "04.10.2026", "00451-B"],
      ["00125", "Café Zürich", "Zürich", "15.320,75", "05.10.2026", "00452-C"],
      ["00126", "Östlund AB", "Göteborg", "2.499,99", "06.10.2026", "00453-D"],
      ["00127", "Nuñez y Hijos", "Sevilla", "310,40", "07.10.2026", "00454-E"],
      ["00128", "Dvořák s.r.o.", "Praha", "7.805,10", "08.10.2026", "00455-F"],
      ["00129", "Lukasz Sp. z o.o.", "Kraków", "1.020,00", "09.10.2026", "00456-G"],
      ["00130", "Renée & Fils", "Lyon", "4.512,30", "10.10.2026", "00457-H"],
    ],
    kinds: ["text", "text", "text", "number", "text", "text"],
  };
}

function getEffectiveRows(ds: ReturnType<typeof buildMockDataset>): string[][] {
  return ds.repairedRows.map((row, r) =>
    row.map((cell, c) => mockState.manualEdits.get(`${r}:${c}`) ?? cell),
  );
}

const MOCK_EXCEL_RISK_RE =
  /^(?:0\d+|\d{16,}|[+-]?\d+(?:[.,]\d+)?[eE][+-]?\d+|\d{1,4}[-/.]\d{1,2}(?:[-/.]\d{1,4})?)$/;

function mockSummary(): DocumentSummaryDto {
  const ds = buildMockDataset(mockState.preset);
  const rows = getEffectiveRows(ds);
  const width = ds.kinds.length;
  const columns: ColumnInfo[] = Array.from({ length: width }, (_, idx) => {
    const detectedKind = ds.kinds[idx] ?? "text";
    const kind = mockState.kindOverrides.get(idx) ?? detectedKind;
    const colCells = rows.slice(1).map((r) => r[idx] ?? "");
    const atRisk =
      kind === "text"
        ? colCells.filter((v) => MOCK_EXCEL_RISK_RE.test(v)).length
        : 0;
    const formulaRiskCount = colCells.filter((v) => /^[\s\t]*[=+@]/.test(v)).length;
    return {
      index: idx,
      name: ds.headers[idx] ?? `Column ${idx + 1}`,
      kind,
      detectedKind,
      mark: ds.decimalMark,
      atRisk,
      risks: atRisk > 0 ? 1 : 0,
      nonEmpty: colCells.filter((v) => v.trim().length > 0).length,
      emptyCount: colCells.filter((v) => v.trim().length === 0).length,
      uniqueCount: new Set(colCells).size,
      numericCount: detectedKind === "number" ? colCells.length : 0,
      formulaRiskCount,
      sampleValues: colCells.slice(0, 4),
    };
  });

  const cellsAtRisk = columns.filter((c) => c.kind === "text").reduce((a, b) => a + b.atRisk, 0);
  const formulaRisks = columns.reduce((a, b) => a + b.formulaRiskCount, 0);
  const repairedMarkers = mockState.preset === "mojibakeAndQuotes" ? 4 : 0;

  return {
    path: ds.fileName,
    fileName: ds.fileName,
    isSample: true,
    sourceBytesLen: 612,
    dataRows: rows.length - 1,
    totalRows: rows.length,
    columnCount: width,
    encodingName: ds.encodingName,
    hadBom: false,
    encodingNote: ds.encodingNote,
    wasLossy: false,
    invalidSequences: 0,
    delimiter: ds.delimiter,
    delimiterName: ds.delimiterName,
    delimiterDetected: true,
    decimalMark: ds.decimalMark,
    lineEndingLabel: "CRLF (Windows)",
    mixedLineEndings: false,
    suggestedHasHeader: true,
    repairedMarkers,
    cellsAtRisk,
    riskyColumns: columns.filter((c) => c.atRisk > 0).length,
    formulaRisks,
    duplicateRows: 0,
    raggedRows: mockState.preset === "mojibakeAndQuotes" ? 2 : 0,
    emptyLines: 0,
    hadStrayQuotes: mockState.preset === "mojibakeAndQuotes",
    endedInsideQuote: false,
    ambiguousRows: mockState.preset === "mojibakeAndQuotes" ? 1 : 0,
    manualEditsCount: mockState.manualEdits.size,
    canExportXlsx: true,
    problems: [
      ...(cellsAtRisk > 0
        ? [
            `Excel would alter ${cellsAtRisk} cell(s) (leading zeros, dates, or long numbers) if opened directly as CSV. CSVRescue keeps them as text.`,
          ]
        : []),
      ...(formulaRisks > 0
        ? [`${formulaRisks} cell(s) start with spreadsheet formula triggers (=, +, -, @).`]
        : []),
    ],
    recovery: {
      strategy: mockState.settings.repairStrategy,
      modalWidth: 5,
      maxWidth: width,
      shortRows: mockState.preset === "mojibakeAndQuotes" ? 1 : 0,
      wideRows: mockState.preset === "mojibakeAndQuotes" ? 1 : 0,
      alignedRows: 0,
      ambiguousRows: mockState.preset === "mojibakeAndQuotes" ? 1 : 0,
      manualEditsCount: mockState.manualEdits.size,
      mojibakeCellsCount: repairedMarkers,
      diagnostics: [],
      cellRepairs: [],
    },
    validation: {
      duplicateRowCount: 0,
      duplicateRowIndices: [],
      duplicateHeaders: [],
      emptyCellCount: 0,
      whitespaceIssueCount: 0,
      controlCharCount: 0,
      formulaInjectionCount: formulaRisks,
      formulaRowIndices: formulaRisks > 0 ? [1, 2, 3, 4] : [],
      typeOutlierCount: 0,
      mixedLineEndings: false,
      lossyDecode: false,
      findings: [
        ...(formulaRisks > 0
          ? [
              {
                severity: "security" as const,
                category: "Spreadsheet Formula Risk",
                count: formulaRisks,
                summary: `${formulaRisks} cell(s) begin with formula-triggering characters.`,
                sampleLocations: ["Row 2, Col 4", "Row 3, Col 4"],
              },
            ]
          : []),
      ],
    },
    columns,
  };
}

export const api = {
  async getBootstrap(): Promise<BootstrapPayload> {
    if (isTauriRuntime()) {
      return invoke("get_bootstrap");
    }
    return {
      config: mockState.config,
      currentDocument: mockSummary(),
      history: mockState.history,
      encodingChoices: DEFAULT_ENCODINGS,
      version: "2.0.0",
    };
  },

  async setConfig(newConfig: AppConfig): Promise<AppConfig> {
    if (isTauriRuntime()) {
      return invoke("set_config", { newConfig });
    }
    mockState.config = newConfig;
    return newConfig;
  },

  async pickOpenFile(): Promise<string | null> {
    if (!isTauriRuntime()) return null;
    const selected = await open({
      multiple: false,
      title: "Open CSV or delimited text file",
      filters: [
        { name: "CSV / Delimited Text", extensions: ["csv", "tsv", "txt", "dat"] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    return typeof selected === "string" ? selected : null;
  },

  async pickSaveFile(defaultPath: string, format: "xlsx" | "csv" | "md" | "json"): Promise<string | null> {
    if (!isTauriRuntime()) return defaultPath;
    const filters =
      format === "xlsx"
        ? [{ name: "Excel Workbook", extensions: ["xlsx"] }]
        : format === "csv"
          ? [{ name: "CSV File", extensions: ["csv"] }]
          : format === "md"
            ? [{ name: "Markdown Report", extensions: ["md"] }]
            : [{ name: "JSON Report", extensions: ["json"] }];
    const selected = await save({ defaultPath, filters });
    return typeof selected === "string" ? selected : null;
  },

  async openFile(path: string): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("open_file", { path });
    }
    return mockSummary();
  },

  async loadSample(preset: SamplePreset): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("load_sample", { preset });
    }
    mockState.preset = preset;
    mockState.manualEdits.clear();
    mockState.kindOverrides.clear();
    return mockSummary();
  },

  async updateSettings(settings: CsvSettings): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("update_settings", { settings });
    }
    mockState.settings = settings;
    return mockSummary();
  },

  async queryPreview(query: PreviewQuery): Promise<PreviewPageDto> {
    if (isTauriRuntime()) {
      return invoke("query_preview", { query });
    }
    const sum = mockSummary();
    const ds = buildMockDataset(mockState.preset);
    const effective = getEffectiveRows(ds);
    const q = (query.search ?? "").trim().toLowerCase();

    const mappedRows: PreviewRowDto[] = effective.map((row, rIdx) => {
      const cells: PreviewCellDto[] = row.map((val, cIdx) => {
        const orig = ds.rawRows[rIdx]?.[cIdx] ?? "";
        const isEdit = mockState.manualEdits.has(`${rIdx}:${cIdx}`);
        const isFormula = rIdx > 0 && /^[\s\t]*[=+@]/.test(val);
        const colKind = sum.columns[cIdx]?.kind ?? "text";
        const isExcelRisk =
          rIdx > 0 &&
          colKind === "text" &&
          MOCK_EXCEL_RISK_RE.test(val);

        if (isEdit) {
          return {
            value: val,
            originalValue: orig,
            status: "manualEdit",
            badgeText: "EDITED",
            confidence: "high",
            tooltip: "Manual user edit",
          };
        }
        if (orig !== val) {
          return {
            value: val,
            originalValue: orig,
            status: "repairedMojibake",
            badgeText: "UTF-8",
            confidence: "high",
            tooltip: "Double-encoded UTF-8 repaired",
          };
        }
        if (isFormula) {
          return {
            value: val,
            originalValue: null,
            status: "formulaRisk",
            badgeText: "FORMULA",
            confidence: null,
            tooltip: "Starts with spreadsheet formula trigger",
          };
        }
        if (isExcelRisk) {
          return {
            value: val,
            originalValue: null,
            status: "excelRisk",
            badgeText: "TEXT SAFE",
            confidence: null,
            tooltip: "Protected as Text against Excel coercion",
          };
        }
        return {
          value: val,
          originalValue: null,
          status: "clean",
          badgeText: null,
          confidence: null,
          tooltip: null,
        };
      });
      return {
        rowIndex: rIdx,
        sourceLine: rIdx + 1,
        isHeader: rIdx === 0,
        isRagged: mockState.preset === "mojibakeAndQuotes" && (rIdx === 3 || rIdx === 5),
        isDuplicate: false,
        isAmbiguous: mockState.preset === "mojibakeAndQuotes" && rIdx === 3,
        hasStrayQuote: mockState.preset === "mojibakeAndQuotes" && rIdx === 2,
        fieldCount: row.length,
        diagnostics: [],
        cells,
      };
    });

    const filtered = mappedRows.filter((row) => {
      if (q && !row.cells.some((c) => c.value.toLowerCase().includes(q))) return false;
      if (query.filter === "all") return true;
      if (query.filter === "excelRiskOnly")
        return row.cells.some((c) => c.status === "excelRisk");
      if (query.filter === "formulaRiskOnly")
        return row.cells.some((c) => c.status === "formulaRisk");
      if (query.filter === "repairedOnly")
        return row.cells.some((c) => c.status === "repairedMojibake") || row.isRagged;
      if (query.filter === "manualEditsOnly")
        return row.cells.some((c) => c.status === "manualEdit");
      if (query.filter === "problemsOnly")
        return (
          row.isRagged ||
          row.cells.some((c) => c.status !== "clean")
        );
      return false;
    });

    return {
      totalMatching: filtered.length,
      totalRows: effective.length,
      offset: query.offset,
      limit: query.limit,
      headers: sum.columns.map((c) => ({
        index: c.index,
        name: c.name,
        kind: c.kind,
        atRisk: c.atRisk,
        formulaRiskCount: c.formulaRiskCount,
      })),
      rows: filtered.slice(query.offset, query.offset + query.limit),
    };
  },

  async getRowComparison(rowIndex: number): Promise<RowComparisonDto> {
    if (isTauriRuntime()) {
      return invoke("get_row_comparison", { rowIndex });
    }
    const ds = buildMockDataset(mockState.preset);
    const sum = mockSummary();
    const rows = getEffectiveRows(ds);
    const recRow = rows[rowIndex] ?? [];
    const origRow = ds.rawRows[rowIndex] ?? [];
    return {
      rowIndex,
      sourceLine: rowIndex + 1,
      isHeader: rowIndex === 0,
      originalFieldCount: origRow.length,
      recoveredFieldCount: recRow.length,
      diagnostics: [],
      cells: recRow.map((val, cIdx) => {
        const orig = origRow[cIdx] ?? "";
        const isManual = mockState.manualEdits.has(`${rowIndex}:${cIdx}`);
        return {
          colIndex: cIdx,
          columnName: sum.columns[cIdx]?.name ?? `Column ${cIdx + 1}`,
          columnKind: sum.columns[cIdx]?.kind ?? "text",
          originalValue: orig,
          recoveredValue: val,
          status: isManual ? "manualEdit" : orig !== val ? "repairedMojibake" : "clean",
          isManuallyEdited: isManual,
          explanation: isManual ? "Manual override applied" : orig !== val ? "Repaired UTF-8 mojibake" : null,
          confidence: orig !== val || isManual ? "high" : null,
          excelRisk: null,
          formulaRisk: null,
        };
      }),
    };
  },

  async setCellOverride(rowIndex: number, colIndex: number, value: string): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("set_cell_override", { rowIndex, colIndex, value });
    }
    mockState.manualEdits.set(`${rowIndex}:${colIndex}`, value);
    return mockSummary();
  },

  async revertCellOverride(rowIndex: number, colIndex: number): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("revert_cell_override", { rowIndex, colIndex });
    }
    mockState.manualEdits.delete(`${rowIndex}:${colIndex}`);
    return mockSummary();
  },

  async setColumnKind(colIndex: number, kind: ColumnKind): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("set_column_kind", { colIndex, kind });
    }
    mockState.kindOverrides.set(colIndex, kind);
    return mockSummary();
  },

  async setAllColumnsKind(kind: ColumnKind): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("set_all_columns_kind", { kind });
    }
    const sum = mockSummary();
    sum.columns.forEach((c) => mockState.kindOverrides.set(c.index, kind));
    return mockSummary();
  },

  async resetColumnsAuto(): Promise<DocumentSummaryDto> {
    if (isTauriRuntime()) {
      return invoke("reset_columns_auto");
    }
    mockState.kindOverrides.clear();
    return mockSummary();
  },

  async getSuggestedExportPath(extension: string): Promise<string> {
    if (isTauriRuntime()) {
      return invoke("get_suggested_export_path", { extension });
    }
    const ds = buildMockDataset(mockState.preset);
    return ds.fileName.replace(/\.csv$/i, `-clean${extension}`);
  },

  async previewExport(options: ExportOptions, previewRows = 20): Promise<ExportDryRun> {
    if (isTauriRuntime()) {
      return invoke("preview_export", { options, previewRows });
    }
    const ds = buildMockDataset(mockState.preset);
    const rows = getEffectiveRows(ds).slice(0, previewRows);
    const delim = options.delimiter || ",";
    const lines = rows.map((r) => r.join(delim));
    return {
      previewText: lines.join("\n"),
      summary: {
        rowsWritten: rows.length,
        columnsWritten: rows[0]?.length ?? 0,
        bytesWritten: lines.join("\n").length,
        decimalsConverted: 0,
        excelTextWrapped: 0,
        formulasSanitized: 0,
        lossyChars: 0,
      },
      warnings: [],
    };
  },

  async exportCsv(targetPath: string, options: ExportOptions): Promise<ExportSummary> {
    if (isTauriRuntime()) {
      return invoke("export_csv", { targetPath, options });
    }
    return {
      rowsWritten: 9,
      columnsWritten: 6,
      bytesWritten: 412,
      decimalsConverted: 0,
      excelTextWrapped: 0,
      formulasSanitized: 0,
      lossyChars: 0,
    };
  },

  async exportXlsx(
    targetPath: string,
    sanitizeFormulas: boolean,
    selectedColumns: number[] | null,
  ): Promise<ExportSummary> {
    if (isTauriRuntime()) {
      return invoke("export_xlsx", { targetPath, sanitizeFormulas, selectedColumns });
    }
    return {
      rowsWritten: 9,
      columnsWritten: selectedColumns?.length ?? 6,
      bytesWritten: 4820,
      decimalsConverted: 0,
      excelTextWrapped: 0,
      formulasSanitized: 0,
      lossyChars: 0,
    };
  },

  async exportRecoveryReport(targetPath: string, markdown: boolean): Promise<number> {
    if (isTauriRuntime()) {
      return invoke("export_recovery_report", { targetPath, markdown });
    }
    return 1024;
  },

  async getHistory(): Promise<HistoryEntry[]> {
    if (isTauriRuntime()) {
      return invoke("get_history");
    }
    return mockState.history;
  },

  async clearHistory(): Promise<boolean> {
    if (isTauriRuntime()) {
      return invoke("clear_history");
    }
    mockState.history = [];
    return true;
  },

  async openInExplorer(path: string): Promise<boolean> {
    if (isTauriRuntime()) {
      return invoke("open_in_explorer", { path });
    }
    void path;
    return true;
  },

  async openDataFolder(): Promise<boolean> {
    if (isTauriRuntime()) {
      return invoke("open_data_folder");
    }
    return true;
  },
};
