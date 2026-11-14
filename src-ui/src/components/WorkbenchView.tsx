import { useState } from "react";
import type {
  ColumnKind,
  CsvSettings,
  DecimalMark,
  DocumentSummaryDto,
  PreviewPageDto,
  RepairStrategy,
  RowFilter,
  SamplePreset,
} from "../types";
import { AlertIcon, ExportIcon, FileOpenIcon, SearchIcon } from "./Icons";
import { RescueGauge } from "./RescueGauge";

interface WorkbenchViewProps {
  doc: DocumentSummaryDto | null;
  settings: CsvSettings;
  encodingChoices: string[];
  recentFiles: string[];
  preview: PreviewPageDto | null;
  filter: RowFilter;
  search: string;
  pageOffset: number;
  pageLimit: number;
  selectedRowIndex: number | null;
  isDraggingFile: boolean;
  onOpenFilePicker: () => Promise<void>;
  onOpenPath: (path: string) => Promise<void>;
  onLoadSample: (preset: SamplePreset) => Promise<void>;
  onChangeSettings: (next: CsvSettings) => Promise<void>;
  onChangeFilter: (filter: RowFilter) => void;
  onChangeSearch: (q: string) => void;
  onChangeOffset: (offset: number) => void;
  onInspectRow: (rowIndex: number) => Promise<void>;
  onToggleColumnKind: (colIndex: number, nextKind: ColumnKind) => Promise<void>;
  onOpenExport: () => void;
}

export function WorkbenchView({
  doc,
  settings,
  encodingChoices,
  recentFiles,
  preview,
  filter,
  search,
  pageOffset,
  pageLimit,
  selectedRowIndex,
  isDraggingFile,
  onOpenFilePicker,
  onOpenPath,
  onLoadSample,
  onChangeSettings,
  onChangeFilter,
  onChangeSearch,
  onChangeOffset,
  onInspectRow,
  onToggleColumnKind,
  onOpenExport,
}: WorkbenchViewProps) {
  const [colWidths, setColWidths] = useState<Record<number, number>>({});

  const startResize = (colIdx: number, e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const th = (e.currentTarget as HTMLElement).parentElement;
    const startWidth = colWidths[colIdx] ?? th?.getBoundingClientRect().width ?? 160;
    const startX = e.clientX;

    const onMove = (ev: MouseEvent) => {
      const next = Math.max(85, Math.min(540, startWidth + (ev.clientX - startX)));
      setColWidths((prev) => ({ ...prev, [colIdx]: next }));
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  const totalMatching = preview?.totalMatching ?? 0;
  const currentPage = Math.floor(pageOffset / pageLimit) + 1;
  const totalPages = Math.max(1, Math.ceil(totalMatching / pageLimit));

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0 }}>
      <header className="workspace-header">
        <div className="header-left">
          <h1 className="header-title">
            {doc ? doc.fileName : "Rescue & Preview Workbench"}
          </h1>
          <p className="header-subtitle" title={doc?.path}>
            {doc
              ? `${doc.isSample ? "Built-in diagnostic sample" : doc.path} · ${formatSize(doc.sourceBytesLen)} · ${doc.dataRows.toLocaleString()} data rows × ${doc.columnCount} columns`
              : "Open a damaged CSV or choose a diagnostic sample below"}
          </p>
        </div>

        <div className="header-actions">
          <button type="button" className="btn" onClick={() => void onOpenFilePicker()}>
            <FileOpenIcon size={15} />
            <span>Open File…</span>
          </button>

          <select
            className="select-input"
            aria-label="Load diagnostic sample dataset"
            value=""
            onChange={(e) => {
              const val = e.target.value as SamplePreset | "";
              if (val) void onLoadSample(val);
            }}
          >
            <option value="" disabled>
              Load Sample…
            </option>
            <option value="europeanErp">European ERP (Windows-1252, ;, 00123)</option>
            <option value="mojibakeAndQuotes">Mojibake, Stray Quotes & Ragged Rows</option>
            <option value="formulaAndTypes">Formula Triggers & ID Risk Audit</option>
          </select>

          {recentFiles.length > 0 && (
            <select
              className="select-input"
              aria-label="Open recent file"
              value=""
              style={{ maxWidth: "160px" }}
              onChange={(e) => {
                if (e.target.value) void onOpenPath(e.target.value);
              }}
            >
              <option value="" disabled>
                Recent Files…
              </option>
              {recentFiles.map((path) => (
                <option key={path} value={path}>
                  {path.split(/[\\/]/).pop() ?? path}
                </option>
              ))}
            </select>
          )}

          <button
            type="button"
            className="btn btn-primary"
            disabled={!doc}
            onClick={onOpenExport}
          >
            <ExportIcon size={15} />
            <span>Export…</span>
          </button>
        </div>
      </header>

      {doc && (
        <section className="control-strip" aria-label="Decoding, delimiter, and recovery controls">
          <label className="control-group">
            <span className="control-label">Source Encoding</span>
            <select
              className="select-input"
              value={settings.encoding}
              onChange={(e) =>
                void onChangeSettings({ ...settings, encoding: e.target.value })
              }
            >
              <option value="Auto">Auto ({doc.encodingName})</option>
              {encodingChoices.map((enc) => (
                <option key={enc} value={enc}>
                  {enc}
                </option>
              ))}
            </select>
          </label>

          <label className="control-group">
            <span className="control-label">Delimiter</span>
            <select
              className="select-input"
              value={settings.delimiter ?? "Auto"}
              onChange={(e) =>
                void onChangeSettings({
                  ...settings,
                  delimiter: e.target.value === "Auto" ? null : e.target.value,
                })
              }
            >
              <option value="Auto">Auto ({doc.delimiterName})</option>
              <option value=",">Comma (,)</option>
              <option value=";">Semicolon (;)</option>
              <option value={"\t"}>Tab (\t)</option>
              <option value="|">Pipe (|)</option>
              <option value=":">Colon (:)</option>
            </select>
          </label>

          <label className="control-group">
            <span className="control-label">Decimal Mark</span>
            <select
              className="select-input"
              value={settings.mark}
              onChange={(e) =>
                void onChangeSettings({ ...settings, mark: e.target.value as DecimalMark })
              }
            >
              <option value="auto">Auto ({doc.decimalMark})</option>
              <option value="dot">Dot (1,234.56)</option>
              <option value="comma">Comma (1.234,56)</option>
            </select>
          </label>

          <label className="control-group">
            <span className="control-label">Ragged Row Strategy</span>
            <select
              className="select-input"
              value={settings.repairStrategy}
              onChange={(e) =>
                void onChangeSettings({
                  ...settings,
                  repairStrategy: e.target.value as RepairStrategy,
                })
              }
            >
              <option value="preserve">Preserve All Fields (Safe)</option>
              <option value="alignColumns">Realign Split Columns</option>
            </select>
          </label>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={settings.hasHeader}
              onChange={(e) =>
                void onChangeSettings({ ...settings, hasHeader: e.target.checked })
              }
            />
            <span>First Row is Header</span>
          </label>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={settings.repairDoubleEncoding}
              onChange={(e) =>
                void onChangeSettings({
                  ...settings,
                  repairDoubleEncoding: e.target.checked,
                })
              }
            />
            <span>Repair Double-UTF8</span>
          </label>
        </section>
      )}

      <div className="workbench-body">
        {!doc ? (
          <div className={`dropzone-card ${isDraggingFile ? "dragging" : ""}`}>
            <FileOpenIcon size={32} />
            <h2 style={{ margin: 0, fontSize: "16px" }}>Drop a CSV file anywhere in this window</h2>
            <p style={{ margin: 0, color: "var(--text-secondary)", maxWidth: "480px" }}>
              CSVRescue never modifies your original file. It detects encodings, repairs garbled text,
              surfaces leading-zero and formula hazards, and writes a clean workbook or CSV.
            </p>
            <div className="sample-buttons">
              <button type="button" className="btn btn-primary" onClick={() => void onOpenFilePicker()}>
                Open CSV File…
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => void onLoadSample("europeanErp")}
              >
                Sample: European ERP
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => void onLoadSample("mojibakeAndQuotes")}
              >
                Sample: Mojibake & Ragged
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => void onLoadSample("formulaAndTypes")}
              >
                Sample: Formula Injection Audit
              </button>
            </div>
          </div>
        ) : (
          <>
            <RescueGauge doc={doc} activeFilter={filter} onChangeFilter={onChangeFilter} />

            {doc.problems.length > 0 && (
              <div className="diagnostics-banner" role="region" aria-label="Detected issues">
                {doc.problems.slice(0, 3).map((problem, idx) => (
                  <div key={idx} className="diagnostics-banner-item">
                    <AlertIcon
                      size={14}
                      style={{ color: "var(--status-warning)", flexShrink: 0, marginTop: "2px" }}
                    />
                    <span>{problem}</span>
                  </div>
                ))}
              </div>
            )}

            <div className="grid-toolbar">
              <div className="search-wrap">
                <SearchIcon size={14} className="search-icon" />
                <input
                  type="search"
                  className="text-input"
                  aria-label="Filter grid rows by cell text"
                  placeholder="Search cells..."
                  value={search}
                  onChange={(e) => onChangeSearch(e.target.value)}
                />
              </div>

              <div className="pagination-wrap">
                <span className="mono">
                  {totalMatching.toLocaleString()} matching row(s)
                </span>
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={pageOffset === 0}
                  onClick={() => onChangeOffset(Math.max(0, pageOffset - pageLimit))}
                >
                  Prev
                </button>
                <span className="mono">
                  Page {currentPage} / {totalPages}
                </span>
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={pageOffset + pageLimit >= totalMatching}
                  onClick={() => onChangeOffset(pageOffset + pageLimit)}
                >
                  Next
                </button>
              </div>
            </div>

            <div className={`grid-container ${isDraggingFile ? "dragging" : ""}`}>
              <table className="data-grid" aria-label="Recovered CSV preview">
                <thead>
                  <tr>
                    <th className="row-num-col" scope="col">
                      #
                    </th>
                    {(preview?.headers ?? []).map((hdr) => {
                      const nextKind: ColumnKind = hdr.kind === "text" ? "number" : "text";
                      const customW = colWidths[hdr.index];
                      return (
                        <th
                          key={hdr.index}
                          scope="col"
                          style={customW ? { width: `${customW}px`, minWidth: `${customW}px` } : undefined}
                        >
                          <div className="th-inner">
                            <span className="th-title" title={hdr.name}>
                              {hdr.name}
                            </span>
                            <button
                              type="button"
                              className={`kind-pill ${hdr.kind}`}
                              title={`Column is exported as ${hdr.kind}. Click to switch to ${nextKind}.`}
                              onClick={() => void onToggleColumnKind(hdr.index, nextKind)}
                            >
                              {hdr.kind.toUpperCase()}
                            </button>
                          </div>
                          <div
                            className="col-resizer"
                            onMouseDown={(e) => startResize(hdr.index, e)}
                            aria-hidden="true"
                          />
                        </th>
                      );
                    })}
                  </tr>
                </thead>
                <tbody>
                  {(preview?.rows ?? []).length === 0 ? (
                    <tr>
                      <td
                        colSpan={Math.max((preview?.headers.length ?? 0) + 1, 2)}
                        style={{ textAlign: "center", padding: "24px", color: "var(--text-secondary)" }}
                      >
                        No rows match the active filter or search term.
                      </td>
                    </tr>
                  ) : (
                    (preview?.rows ?? []).map((row) => (
                      <tr
                        key={row.rowIndex}
                        className={[
                          row.isRagged ? "ragged-row" : "",
                          row.isHeader ? "header-preview-row" : "",
                          selectedRowIndex === row.rowIndex ? "selected-row" : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        onClick={() => void onInspectRow(row.rowIndex)}
                        title="Click row to compare original vs recovered values or edit cells"
                      >
                        <td className="row-num-cell">{row.rowIndex + 1}</td>
                        {row.cells.map((cell, cIdx) => {
                          const customW = colWidths[cIdx];
                          return (
                            <td
                              key={cIdx}
                              style={customW ? { width: `${customW}px`, maxWidth: `${customW}px` } : undefined}
                              title={cell.tooltip ?? undefined}
                            >
                              <div className="cell-wrap">
                                <span className="cell-value">{cell.value}</span>
                                {cell.badgeText && (
                                  <span className={`cell-badge ${cell.status}`}>
                                    {cell.badgeText}
                                  </span>
                                )}
                              </div>
                            </td>
                          );
                        })}
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
