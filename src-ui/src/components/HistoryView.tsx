import { api } from "../api";
import type { HistoryEntry } from "../types";
import { FolderIcon } from "./Icons";

interface HistoryViewProps {
  history: HistoryEntry[];
  onOpenSource: (path: string) => Promise<void>;
  onClearHistory: () => Promise<void>;
}

export function HistoryView({
  history,
  onOpenSource,
  onClearHistory,
}: HistoryViewProps) {
  return (
    <div className="section-stack">
      <div className="panel">
        <div className="panel-head">
          <div>
            <h2 className="panel-title">Export & Operation History</h2>
            <span style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
              Stored locally in %APPDATA%\CSVRescue\history.json. Up to 100 recent exports are retained.
            </span>
          </div>
          {history.length > 0 && (
            <button type="button" className="btn btn-sm btn-danger" onClick={() => void onClearHistory()}>
              Clear History
            </button>
          )}
        </div>

        {history.length === 0 ? (
          <p style={{ margin: 0, color: "var(--text-secondary)" }}>
            No exports have been recorded yet. Save an Excel workbook or clean CSV to log an entry here.
          </p>
        ) : (
          <div className="table-scroll">
            <table className="std-table">
              <thead>
                <tr>
                  <th>Timestamp</th>
                  <th>Format</th>
                  <th>Output File</th>
                  <th>Rows × Cols</th>
                  <th>Repaired</th>
                  <th>Sanitized</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {history.map((item) => (
                  <tr key={item.id}>
                    <td className="mono" style={{ whiteSpace: "nowrap" }}>
                      {item.timestamp}
                    </td>
                    <td>
                      <span className="kind-pill text">{item.format}</span>
                    </td>
                    <td className="mono" title={item.outputPath} style={{ maxWidth: "260px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {item.outputPath}
                    </td>
                    <td className="mono">
                      {item.rowsWritten.toLocaleString()} × {item.columnsWritten}
                    </td>
                    <td className="mono">{item.mojibakeRepaired}</td>
                    <td className="mono">{item.formulasSanitized}</td>
                    <td>
                      <div style={{ display: "flex", gap: "6px" }}>
                        <button
                          type="button"
                          className="btn btn-sm"
                          onClick={() => void api.openInExplorer(item.outputPath)}
                          title="Reveal exported file in Windows Explorer"
                        >
                          <FolderIcon size={13} />
                          <span>Reveal</span>
                        </button>
                        {item.sourcePath && !item.sourcePath.startsWith("sample-") && (
                          <button
                            type="button"
                            className="btn btn-sm"
                            onClick={() => void onOpenSource(item.sourcePath)}
                          >
                            Re-open Source
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
