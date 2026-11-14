import { useEffect, useState } from "react";
import type { RowComparisonDto } from "../types";
import { CloseIcon } from "./Icons";

interface InspectorDrawerProps {
  row: RowComparisonDto | null;
  onClose: () => void;
  onSaveCell: (rowIndex: number, colIndex: number, value: string) => Promise<void>;
  onRevertCell: (rowIndex: number, colIndex: number) => Promise<void>;
}

export function InspectorDrawer({
  row,
  onClose,
  onSaveCell,
  onRevertCell,
}: InspectorDrawerProps) {
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [busyCol, setBusyCol] = useState<number | null>(null);

  useEffect(() => {
    if (!row) return;
    const next: Record<number, string> = {};
    for (const c of row.cells) {
      next[c.colIndex] = c.recoveredValue;
    }
    setDrafts(next);
  }, [row]);

  useEffect(() => {
    if (!row) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [row, onClose]);

  if (!row) return null;

  return (
    <div
      className="drawer-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label={`Row ${row.rowIndex + 1} inspection and repair`}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="inspector-drawer">
        <div className="drawer-header">
          <div>
            <h2>
              Row {row.rowIndex + 1} {row.isHeader ? "(Header Row)" : ""}
            </h2>
            <span className="mono" style={{ fontSize: "11.5px", color: "var(--text-secondary)" }}>
              Source line {row.sourceLine} · {row.originalFieldCount} original fields →{" "}
              {row.recoveredFieldCount} fields
            </span>
          </div>
          <button
            type="button"
            className="btn btn-sm"
            onClick={onClose}
            aria-label="Close row inspector"
          >
            <CloseIcon size={15} />
            <span>Close</span>
          </button>
        </div>

        <div className="drawer-body">
          {row.diagnostics.length > 0 && (
            <div className="diagnostics-banner">
              {row.diagnostics.map((d, i) => (
                <div key={i} className="diagnostics-banner-item">
                  <strong>[{d.confidence.toUpperCase()}]</strong>
                  <span>{d.explanation}</span>
                </div>
              ))}
            </div>
          )}

          {row.cells.map((cell) => {
            const draft = drafts[cell.colIndex] ?? cell.recoveredValue;
            const changedFromCurrent = draft !== cell.recoveredValue;
            const changedFromOriginal = cell.originalValue !== cell.recoveredValue;

            return (
              <div key={cell.colIndex} className="diff-card">
                <div className="diff-card-head">
                  <span>
                    {cell.columnName}{" "}
                    <span className="mono" style={{ color: "var(--text-muted)", fontSize: "11px" }}>
                      ({cell.columnKind.toUpperCase()})
                    </span>
                  </span>
                  {cell.status !== "clean" && (
                    <span className={`cell-badge ${cell.status}`}>
                      {cell.isManuallyEdited
                        ? "EDITED"
                        : cell.status === "repairedMojibake"
                          ? "UTF-8 FIXED"
                          : cell.status === "repairedStructure"
                            ? "ALIGNED"
                            : cell.status === "formulaRisk"
                              ? "FORMULA TRIGGER"
                              : "TEXT SAFE"}
                    </span>
                  )}
                </div>

                {changedFromOriginal && (
                  <div className="diff-pair">
                    <span className="diff-key">Original</span>
                    <div className="diff-val">{cell.originalValue || "(empty)"}</div>
                  </div>
                )}

                <div className="diff-pair">
                  <span className="diff-key">Recovered</span>
                  <div className="diff-val">{cell.recoveredValue || "(empty)"}</div>
                </div>

                {(cell.explanation || cell.excelRisk || cell.formulaRisk) && (
                  <div style={{ fontSize: "11.5px", color: "var(--text-secondary)" }}>
                    {[cell.explanation, cell.excelRisk, cell.formulaRisk].filter(Boolean).join(" · ")}
                  </div>
                )}

                <div className="edit-row">
                  <input
                    type="text"
                    className="text-input mono"
                    aria-label={`Edit value for ${cell.columnName}`}
                    value={draft}
                    onChange={(e) =>
                      setDrafts((prev) => ({ ...prev, [cell.colIndex]: e.target.value }))
                    }
                  />
                  <button
                    type="button"
                    className="btn btn-sm btn-primary"
                    disabled={!changedFromCurrent || busyCol === cell.colIndex}
                    onClick={async () => {
                      setBusyCol(cell.colIndex);
                      try {
                        await onSaveCell(row.rowIndex, cell.colIndex, draft);
                      } finally {
                        setBusyCol(null);
                      }
                    }}
                  >
                    Apply
                  </button>
                  {cell.isManuallyEdited && (
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={busyCol === cell.colIndex}
                      onClick={async () => {
                        setBusyCol(cell.colIndex);
                        try {
                          await onRevertCell(row.rowIndex, cell.colIndex);
                        } finally {
                          setBusyCol(null);
                        }
                      }}
                    >
                      Revert
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
