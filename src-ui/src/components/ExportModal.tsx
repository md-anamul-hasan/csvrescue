import { useEffect, useState } from "react";
import { api } from "../api";
import type {
  AppConfig,
  DocumentSummaryDto,
  ExportDryRun,
  ExportOptions,
  LineEnding,
  OutputEncoding,
} from "../types";
import { CheckIcon, CloseIcon, ExportIcon } from "./Icons";

interface ExportModalProps {
  isOpen: boolean;
  doc: DocumentSummaryDto;
  config: AppConfig;
  hasHeader: boolean;
  onClose: () => void;
  onExported: (message: string) => Promise<void>;
  onError: (message: string) => void;
}

type ExportTab = "xlsx" | "csv" | "report";

export function ExportModal({
  isOpen,
  doc,
  config,
  hasHeader,
  onClose,
  onExported,
  onError,
}: ExportModalProps) {
  const [tab, setTab] = useState<ExportTab>(doc.canExportXlsx ? "xlsx" : "csv");
  const [delimiter, setDelimiter] = useState<string>(config.defaultOutDelimiter || ",");
  const [encoding, setEncoding] = useState<OutputEncoding>("utf8");
  const [lineEnding, setLineEnding] = useState<LineEnding>("crlf");
  const [utf8Bom, setUtf8Bom] = useState<boolean>(config.defaultUtf8Bom);
  const [decimalAsDot, setDecimalAsDot] = useState<boolean>(config.defaultDecimalAsDot);
  const [excelTextProtect, setExcelTextProtect] = useState<boolean>(config.defaultExcelProtect);
  const [sanitizeFormulas, setSanitizeFormulas] = useState<boolean>(config.defaultSanitizeFormulas);
  const [reportMarkdown, setReportMarkdown] = useState<boolean>(true);
  const [selectedCols, setSelectedCols] = useState<number[]>(doc.columns.map((c) => c.index));
  const [dryRun, setDryRun] = useState<ExportDryRun | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setSelectedCols(doc.columns.map((c) => c.index));
  }, [doc]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [isOpen, onClose]);

  const buildOptions = (): ExportOptions => ({
    delimiter,
    encoding,
    lineEnding,
    utf8Bom,
    decimalAsDot,
    excelTextProtect,
    sanitizeFormulas,
    hasHeader,
    selectedColumns:
      selectedCols.length === doc.columns.length ? null : [...selectedCols].sort((a, b) => a - b),
    selectedRows: null,
  });

  useEffect(() => {
    if (!isOpen || tab === "report") return;
    let active = true;
    const opts = buildOptions();
    void api.previewExport(opts, 18).then((res) => {
      if (active) setDryRun(res);
    });
    return () => {
      active = false;
    };
  }, [
    isOpen,
    tab,
    delimiter,
    encoding,
    lineEnding,
    utf8Bom,
    decimalAsDot,
    excelTextProtect,
    sanitizeFormulas,
    hasHeader,
    selectedCols,
  ]);

  if (!isOpen) return null;

  const toggleColumn = (idx: number) => {
    setSelectedCols((prev) =>
      prev.includes(idx) ? (prev.length > 1 ? prev.filter((c) => c !== idx) : prev) : [...prev, idx],
    );
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      if (tab === "xlsx") {
        const suggested = await api.getSuggestedExportPath(".xlsx");
        const target = await api.pickSaveFile(suggested, "xlsx");
        if (!target) return;
        const colsArg =
          selectedCols.length === doc.columns.length
            ? null
            : [...selectedCols].sort((a, b) => a - b);
        const res = await api.exportXlsx(target, sanitizeFormulas, colsArg);
        await onExported(
          `Saved Excel workbook (${res.rowsWritten.toLocaleString()} rows, ${(res.bytesWritten / 1024).toFixed(1)} KB) to ${target}`,
        );
        onClose();
      } else if (tab === "csv") {
        const suggested = await api.getSuggestedExportPath(".csv");
        const target = await api.pickSaveFile(suggested, "csv");
        if (!target) return;
        const res = await api.exportCsv(target, buildOptions());
        await onExported(
          `Saved clean CSV (${res.rowsWritten.toLocaleString()} rows, ${(res.bytesWritten / 1024).toFixed(1)} KB) to ${target}`,
        );
        onClose();
      } else {
        const ext = reportMarkdown ? ".md" : ".json";
        const suggested = await api.getSuggestedExportPath(`-report${ext}`);
        const target = await api.pickSaveFile(suggested, reportMarkdown ? "md" : "json");
        if (!target) return;
        const bytes = await api.exportRecoveryReport(target, reportMarkdown);
        await onExported(`Saved recovery report (${bytes.toLocaleString()} bytes) to ${target}`);
        onClose();
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label="Export recovered data"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal-panel">
        <div className="modal-header">
          <h2>Export Recovered Output</h2>
          <button type="button" className="btn btn-sm" onClick={onClose} aria-label="Close export modal">
            <CloseIcon size={15} />
            <span>Close</span>
          </button>
        </div>

        <div className="modal-body">
          <div className="tabs-row" role="tablist">
            <button
              type="button"
              role="tab"
              aria-selected={tab === "xlsx"}
              className={`tab-btn ${tab === "xlsx" ? "active" : ""}`}
              disabled={!doc.canExportXlsx}
              onClick={() => setTab("xlsx")}
            >
              Excel Workbook (.xlsx)
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === "csv"}
              className={`tab-btn ${tab === "csv" ? "active" : ""}`}
              onClick={() => setTab("csv")}
            >
              Clean CSV (.csv)
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={tab === "report"}
              className={`tab-btn ${tab === "report" ? "active" : ""}`}
              onClick={() => setTab("report")}
            >
              Recovery Report (.md / .json)
            </button>
          </div>

          {tab === "xlsx" && (
            <div style={{ fontSize: "12.5px", color: "var(--text-secondary)" }}>
              Writes a native OpenXML spreadsheet where all Text columns (such as{" "}
              <span className="mono">00123</span>, 16-digit IDs, dates, and <span className="mono">1E5</span>)
              are stored as explicit text cells (<span className="mono">inlineStr</span>), preserving leading
              zeros and exact formatting in Microsoft Excel.
            </div>
          )}

          {tab === "csv" && (
            <div className="options-grid">
              <label className="control-group">
                <span className="control-label">Output Delimiter</span>
                <select
                  className="select-input"
                  value={delimiter}
                  onChange={(e) => setDelimiter(e.target.value)}
                >
                  <option value=",">Comma (,)</option>
                  <option value=";">Semicolon (;)</option>
                  <option value={"\t"}>Tab (\t)</option>
                  <option value="|">Pipe (|)</option>
                </select>
              </label>

              <label className="control-group">
                <span className="control-label">Output Encoding</span>
                <select
                  className="select-input"
                  value={encoding}
                  onChange={(e) => setEncoding(e.target.value as OutputEncoding)}
                >
                  <option value="utf8">UTF-8</option>
                  <option value="utf16LeBom">UTF-16 LE (with BOM)</option>
                  <option value="windows1252">Windows-1252 (Western Europe)</option>
                </select>
              </label>

              <label className="control-group">
                <span className="control-label">Line Ending</span>
                <select
                  className="select-input"
                  value={lineEnding}
                  onChange={(e) => setLineEnding(e.target.value as LineEnding)}
                >
                  <option value="crlf">CRLF (Windows)</option>
                  <option value="lf">LF (Unix / macOS)</option>
                  <option value="cr">CR (Classic Mac)</option>
                </select>
              </label>
            </div>
          )}

          {tab !== "report" && (
            <div style={{ display: "flex", flexWrap: "wrap", gap: "14px" }}>
              {tab === "csv" && (
                <>
                  <label className="checkbox-row">
                    <input
                      type="checkbox"
                      checked={utf8Bom}
                      onChange={(e) => setUtf8Bom(e.target.checked)}
                    />
                    <span>Include UTF-8 BOM (helps older Excel)</span>
                  </label>
                  <label className="checkbox-row">
                    <input
                      type="checkbox"
                      checked={decimalAsDot}
                      onChange={(e) => setDecimalAsDot(e.target.checked)}
                    />
                    <span>Convert number decimal commas to dots</span>
                  </label>
                  <label className="checkbox-row">
                    <input
                      type="checkbox"
                      checked={excelTextProtect}
                      onChange={(e) => setExcelTextProtect(e.target.checked)}
                    />
                    <span>Wrap risky CSV cells as ="..." for Excel</span>
                  </label>
                </>
              )}

              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={sanitizeFormulas}
                  onChange={(e) => setSanitizeFormulas(e.target.checked)}
                />
                <span>
                  Neutralize formula injection triggers (prefix &apos; on{" "}
                  <span className="mono">= + - @</span>)
                </span>
              </label>
            </div>
          )}

          {tab !== "report" && (
            <div className="control-group">
              <span className="control-label">
                Columns to Include ({selectedCols.length} / {doc.columns.length})
              </span>
              <div className="col-check-list">
                {doc.columns.map((col) => (
                  <label key={col.index} className="checkbox-row" style={{ height: "24px" }}>
                    <input
                      type="checkbox"
                      checked={selectedCols.includes(col.index)}
                      onChange={() => toggleColumn(col.index)}
                    />
                    <span>{col.name}</span>
                  </label>
                ))}
              </div>
            </div>
          )}

          {tab === "report" && (
            <div className="options-grid">
              <label className="checkbox-row">
                <input
                  type="radio"
                  name="reportFmt"
                  checked={reportMarkdown}
                  onChange={() => setReportMarkdown(true)}
                />
                <span>Markdown Diagnostic Report (.md)</span>
              </label>
              <label className="checkbox-row">
                <input
                  type="radio"
                  name="reportFmt"
                  checked={!reportMarkdown}
                  onChange={() => setReportMarkdown(false)}
                />
                <span>Machine-Readable JSON Report (.json)</span>
              </label>
            </div>
          )}

          {tab !== "report" && dryRun && (
            <div className="control-group">
              <span className="control-label">
                Dry-Run Serialization Preview ({dryRun.summary.rowsWritten.toLocaleString()} total rows ·{" "}
                {dryRun.summary.formulasSanitized} formulas sanitized · {dryRun.summary.decimalsConverted}{" "}
                decimals normalized)
              </span>
              {dryRun.warnings.length > 0 && (
                <div className="diagnostics-banner">
                  {dryRun.warnings.map((w, idx) => (
                    <div key={idx} className="diagnostics-banner-item">
                      {w}
                    </div>
                  ))}
                </div>
              )}
              <pre className="preview-box">{dryRun.previewText}</pre>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <span style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
            Source file <span className="mono">{doc.fileName}</span> will never be overwritten.
          </span>
          <div style={{ display: "flex", gap: "8px" }}>
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="button" className="btn btn-primary" disabled={saving} onClick={handleSave}>
              {saving ? <CheckIcon size={15} /> : <ExportIcon size={15} />}
              <span>
                {saving
                  ? "Saving..."
                  : tab === "xlsx"
                    ? "Save .xlsx Workbook..."
                    : tab === "csv"
                      ? "Save Clean .csv..."
                      : "Save Recovery Report..."}
              </span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
