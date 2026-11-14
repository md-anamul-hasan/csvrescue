import type { ColumnKind, DocumentSummaryDto } from "../types";

interface ColumnsValidationViewProps {
  doc: DocumentSummaryDto | null;
  onSetColumnKind: (colIndex: number, kind: ColumnKind) => Promise<void>;
  onSetAllColumnsKind: (kind: ColumnKind) => Promise<void>;
  onResetColumnsAuto: () => Promise<void>;
}

export function ColumnsValidationView({
  doc,
  onSetColumnKind,
  onSetAllColumnsKind,
  onResetColumnsAuto,
}: ColumnsValidationViewProps) {
  if (!doc) {
    return (
      <div className="section-stack">
        <div className="panel">
          <h2 className="panel-title">Columns & Validation</h2>
          <p style={{ margin: 0, color: "var(--text-secondary)" }}>
            Open a CSV file or select a sample dataset in Rescue & Preview to inspect columns and validation findings.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="section-stack">
      <div className="panel">
        <div className="panel-head">
          <div>
            <h2 className="panel-title">Column Types & Excel Risk Analysis</h2>
            <span style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
              Click Text / Number to override how a column is stored in Excel (.xlsx) and numeric conversions.
            </span>
          </div>
          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
            <button type="button" className="btn btn-sm" onClick={() => void onSetAllColumnsKind("text")}>
              Set All Text
            </button>
            <button type="button" className="btn btn-sm" onClick={() => void onResetColumnsAuto()}>
              Reset Auto-Detect
            </button>
          </div>
        </div>

        <div className="table-scroll">
          <table className="std-table">
            <thead>
              <tr>
                <th>#</th>
                <th>Column Name</th>
                <th>Export Type</th>
                <th>Detected</th>
                <th>Non-Empty</th>
                <th>Excel Risks</th>
                <th>Formula Triggers</th>
                <th>Sample Values</th>
              </tr>
            </thead>
            <tbody>
              {doc.columns.map((col) => {
                const nextKind: ColumnKind = col.kind === "text" ? "number" : "text";
                return (
                  <tr key={col.index}>
                    <td className="mono">{col.index + 1}</td>
                    <td>
                      <strong>{col.name}</strong>
                    </td>
                    <td>
                      <button
                        type="button"
                        className={`kind-pill ${col.kind}`}
                        onClick={() => void onSetColumnKind(col.index, nextKind)}
                        title="Click to toggle Text / Number"
                      >
                        {col.kind.toUpperCase()}
                      </button>
                    </td>
                    <td className="mono" style={{ color: "var(--text-secondary)" }}>
                      {col.detectedKind}
                    </td>
                    <td className="mono">
                      {col.nonEmpty.toLocaleString()}
                      {col.emptyCount > 0 ? ` (${col.emptyCount} empty)` : ""}
                    </td>
                    <td>
                      {col.atRisk > 0 ? (
                        <span className="cell-badge excelRisk">{col.atRisk} protected</span>
                      ) : (
                        <span style={{ color: "var(--text-muted)" }}>None</span>
                      )}
                    </td>
                    <td>
                      {col.formulaRiskCount > 0 ? (
                        <span className="cell-badge formulaRisk">{col.formulaRiskCount} risky</span>
                      ) : (
                        <span style={{ color: "var(--text-muted)" }}>0</span>
                      )}
                    </td>
                    <td className="mono" style={{ fontSize: "11.5px", color: "var(--text-secondary)" }}>
                      {col.sampleValues.slice(0, 3).join(" · ")}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2 className="panel-title">Validation & Security Diagnostics</h2>
          <span className="mono" style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
            Duplicate rows: {doc.validation.duplicateRowCount} · Empty cells:{" "}
            {doc.validation.emptyCellCount.toLocaleString()} · Whitespace padding:{" "}
            {doc.validation.whitespaceIssueCount} · Control chars: {doc.validation.controlCharCount}
          </span>
        </div>

        {doc.validation.findings.length === 0 ? (
          <p style={{ margin: 0, color: "var(--status-clean-text)" }}>
            No structural anomalies, lossy decode sequences, or formula injection risks were found.
          </p>
        ) : (
          <div className="finding-list">
            {doc.validation.findings.map((finding, idx) => (
              <div key={idx} className={`finding-card ${finding.severity}`}>
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                  <strong>
                    [{finding.severity.toUpperCase()}] {finding.category}
                  </strong>
                  <span className="mono" style={{ fontSize: "11.5px", color: "var(--text-secondary)" }}>
                    {finding.count.toLocaleString()} occurrence(s)
                  </span>
                </div>
                <div style={{ fontSize: "12.5px" }}>{finding.summary}</div>
                {finding.sampleLocations.length > 0 && (
                  <div className="mono" style={{ fontSize: "11.5px", color: "var(--text-muted)" }}>
                    Locations: {finding.sampleLocations.join(", ")}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
