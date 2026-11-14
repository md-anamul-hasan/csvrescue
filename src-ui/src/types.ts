export type ThemePreference = "system" | "dark" | "light";
export type SamplePreset = "europeanErp" | "mojibakeAndQuotes" | "formulaAndTypes";
export type ColumnKind = "text" | "number";
export type DecimalMark = "auto" | "dot" | "comma";
export type EscapeMode = "doubledQuote" | "backslash";
export type RepairStrategy = "preserve" | "alignColumns";
export type ConfidenceLevel = "high" | "medium" | "low" | "ambiguous";
export type RowFilter =
  | "all"
  | "problemsOnly"
  | "repairedOnly"
  | "excelRiskOnly"
  | "formulaRiskOnly"
  | "manualEditsOnly"
  | "duplicateRowsOnly";
export type CellStatus =
  | "clean"
  | "repairedMojibake"
  | "repairedStructure"
  | "manualEdit"
  | "excelRisk"
  | "formulaRisk";
export type OutputEncoding = "utf8" | "utf16LeBom" | "windows1252";
export type LineEnding = "crlf" | "lf" | "cr";
export type FindingSeverity = "info" | "warning" | "security";

export interface AppConfig {
  theme: ThemePreference;
  defaultOutDelimiter: string;
  defaultUtf8Bom: boolean;
  defaultDecimalAsDot: boolean;
  defaultExcelProtect: boolean;
  defaultSanitizeFormulas: boolean;
  defaultRepairDoubleEncoding: boolean;
  recentFiles: string[];
}

export interface CsvSettings {
  encoding: string;
  delimiter: string | null;
  mark: DecimalMark;
  hasHeader: boolean;
  repairDoubleEncoding: boolean;
  skipEmptyLines: boolean;
  escapeMode: EscapeMode;
  repairStrategy: RepairStrategy;
}

export interface ColumnInfo {
  index: number;
  name: string;
  kind: ColumnKind;
  detectedKind: ColumnKind;
  mark: DecimalMark;
  atRisk: number;
  risks: number;
  nonEmpty: number;
  emptyCount: number;
  uniqueCount: number;
  numericCount: number;
  formulaRiskCount: number;
  sampleValues: string[];
}

export interface RowDiagnostic {
  rowIndex: number;
  sourceLine: number;
  kind: string;
  originalFieldCount: number;
  expectedFieldCount: number;
  confidence: ConfidenceLevel;
  ambiguous: boolean;
  explanation: string;
}

export interface CellDiagnostic {
  row: number;
  col: number;
  original: string;
  repaired: string;
  kind: "mojibake" | "structuralMerge" | "manualOverride";
  confidence: ConfidenceLevel;
  explanation: string;
}

export interface RecoverySummary {
  strategy: RepairStrategy;
  modalWidth: number;
  maxWidth: number;
  shortRows: number;
  wideRows: number;
  alignedRows: number;
  ambiguousRows: number;
  manualEditsCount: number;
  mojibakeCellsCount: number;
  diagnostics: RowDiagnostic[];
  cellRepairs: CellDiagnostic[];
}

export interface ValidationFinding {
  severity: FindingSeverity;
  category: string;
  count: number;
  summary: string;
  sampleLocations: string[];
}

export interface ValidationReport {
  duplicateRowCount: number;
  duplicateRowIndices: number[];
  duplicateHeaders: string[];
  emptyCellCount: number;
  whitespaceIssueCount: number;
  controlCharCount: number;
  formulaInjectionCount: number;
  formulaRowIndices: number[];
  typeOutlierCount: number;
  mixedLineEndings: boolean;
  lossyDecode: boolean;
  findings: ValidationFinding[];
}

export interface DocumentSummaryDto {
  path: string;
  fileName: string;
  isSample: boolean;
  sourceBytesLen: number;
  dataRows: number;
  totalRows: number;
  columnCount: number;
  encodingName: string;
  hadBom: boolean;
  encodingNote: string;
  wasLossy: boolean;
  invalidSequences: number;
  delimiter: string;
  delimiterName: string;
  delimiterDetected: boolean;
  decimalMark: DecimalMark;
  lineEndingLabel: string;
  mixedLineEndings: boolean;
  suggestedHasHeader: boolean;
  repairedMarkers: number;
  cellsAtRisk: number;
  riskyColumns: number;
  formulaRisks: number;
  duplicateRows: number;
  raggedRows: number;
  emptyLines: number;
  hadStrayQuotes: boolean;
  endedInsideQuote: boolean;
  ambiguousRows: number;
  manualEditsCount: number;
  canExportXlsx: boolean;
  problems: string[];
  recovery: RecoverySummary;
  validation: ValidationReport;
  columns: ColumnInfo[];
}

export interface PreviewQuery {
  offset: number;
  limit: number;
  search: string | null;
  filter: RowFilter;
}

export interface PreviewCellDto {
  value: string;
  originalValue: string | null;
  status: CellStatus;
  badgeText: string | null;
  confidence: ConfidenceLevel | null;
  tooltip: string | null;
}

export interface PreviewRowDto {
  rowIndex: number;
  sourceLine: number;
  isHeader: boolean;
  isRagged: boolean;
  isDuplicate: boolean;
  isAmbiguous: boolean;
  hasStrayQuote: boolean;
  fieldCount: number;
  diagnostics: RowDiagnostic[];
  cells: PreviewCellDto[];
}

export interface PreviewHeaderDto {
  index: number;
  name: string;
  kind: ColumnKind;
  atRisk: number;
  formulaRiskCount: number;
}

export interface PreviewPageDto {
  totalMatching: number;
  totalRows: number;
  offset: number;
  limit: number;
  headers: PreviewHeaderDto[];
  rows: PreviewRowDto[];
}

export interface CellComparisonDto {
  colIndex: number;
  columnName: string;
  columnKind: ColumnKind;
  originalValue: string;
  recoveredValue: string;
  status: CellStatus;
  isManuallyEdited: boolean;
  explanation: string | null;
  confidence: ConfidenceLevel | null;
  excelRisk: string | null;
  formulaRisk: string | null;
}

export interface RowComparisonDto {
  rowIndex: number;
  sourceLine: number;
  isHeader: boolean;
  originalFieldCount: number;
  recoveredFieldCount: number;
  diagnostics: RowDiagnostic[];
  cells: CellComparisonDto[];
}

export interface ExportOptions {
  delimiter: string;
  encoding: OutputEncoding;
  lineEnding: LineEnding;
  utf8Bom: boolean;
  decimalAsDot: boolean;
  excelTextProtect: boolean;
  sanitizeFormulas: boolean;
  hasHeader: boolean;
  selectedColumns: number[] | null;
  selectedRows: number[] | null;
}

export interface ExportSummary {
  rowsWritten: number;
  columnsWritten: number;
  bytesWritten: number;
  decimalsConverted: number;
  excelTextWrapped: number;
  formulasSanitized: number;
  lossyChars: number;
}

export interface ExportDryRun {
  previewText: string;
  summary: ExportSummary;
  warnings: string[];
}

export interface HistoryEntry {
  id: string;
  timestamp: string;
  sourcePath: string;
  outputPath: string;
  format: string;
  encodingName: string;
  delimiter: string;
  rowsWritten: number;
  columnsWritten: number;
  mojibakeRepaired: number;
  formulasSanitized: number;
}

export interface BootstrapPayload {
  config: AppConfig;
  currentDocument: DocumentSummaryDto | null;
  history: HistoryEntry[];
  encodingChoices: string[];
  version: string;
}
