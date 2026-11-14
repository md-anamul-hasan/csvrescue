import type { DocumentSummaryDto, RowFilter } from "../types";

interface RescueGaugeProps {
  doc: DocumentSummaryDto;
  activeFilter: RowFilter;
  onChangeFilter: (filter: RowFilter) => void;
}

export function RescueGauge({ doc, activeFilter, onChangeFilter }: RescueGaugeProps) {
  const totalCells = Math.max(doc.dataRows * Math.max(doc.columnCount, 1), 1);
  const repairedCells = doc.recovery.mojibakeCellsCount + doc.recovery.alignedRows;
  const formulaCells = doc.formulaRisks;
  const riskCells = doc.cellsAtRisk;

  const pctFormula = Math.min(100, (formulaCells / totalCells) * 100);
  const pctRisk = Math.min(100 - pctFormula, (riskCells / totalCells) * 100);
  const pctRepaired = Math.min(100 - pctFormula - pctRisk, (repairedCells / totalCells) * 100);
  const pctClean = Math.max(0, 100 - pctFormula - pctRisk - pctRepaired);

  const pills: Array<{ id: RowFilter; label: string; count?: number }> = [
    { id: "all", label: "All Rows", count: doc.totalRows },
    { id: "problemsOnly", label: "All Issues" },
    {
      id: "repairedOnly",
      label: "Repaired / Ragged",
      count: repairedCells + doc.raggedRows,
    },
    { id: "excelRiskOnly", label: "Excel Risk", count: doc.cellsAtRisk },
    { id: "formulaRiskOnly", label: "Formula Risk", count: doc.formulaRisks },
    { id: "manualEditsOnly", label: "Manual Edits", count: doc.manualEditsCount },
    { id: "duplicateRowsOnly", label: "Duplicates", count: doc.duplicateRows },
  ];

  return (
    <section className="gauge-card" aria-label="Document integrity breakdown and row filters">
      <div className="gauge-top">
        <div className="gauge-metrics">
          <span className="gauge-stat">
            <span className="swatch clean" />
            <span>Clean</span>
            <strong>{Math.round(pctClean)}%</strong>
          </span>
          <span className="gauge-stat">
            <span className="swatch repaired" />
            <span>Repaired</span>
            <strong>{repairedCells.toLocaleString()}</strong>
          </span>
          <span className="gauge-stat">
            <span className="swatch warning" />
            <span>Excel Text-Safe</span>
            <strong>{doc.cellsAtRisk.toLocaleString()}</strong>
          </span>
          <span className="gauge-stat">
            <span className="swatch danger" />
            <span>Formula Triggers</span>
            <strong>{doc.formulaRisks.toLocaleString()}</strong>
          </span>
        </div>

        <span className="mono" style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
          {doc.encodingName}
          {doc.hadBom ? " (BOM)" : ""} · {doc.delimiterName} · {doc.lineEndingLabel}
        </span>
      </div>

      <div className="gauge-bar-track" role="img" aria-label="Proportion of clean, repaired, Excel risk, and formula trigger cells">
        {pctClean > 0 && <div className="gauge-seg clean" style={{ width: `${pctClean}%` }} />}
        {pctRepaired > 0 && <div className="gauge-seg repaired" style={{ width: `${pctRepaired}%` }} />}
        {pctRisk > 0 && <div className="gauge-seg warning" style={{ width: `${pctRisk}%` }} />}
        {pctFormula > 0 && <div className="gauge-seg danger" style={{ width: `${pctFormula}%` }} />}
      </div>

      <div className="filter-pills" role="tablist" aria-label="Filter preview rows">
        {pills.map((pill) => {
          const isActive = activeFilter === pill.id;
          return (
            <button
              key={pill.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              className={`filter-pill ${isActive ? "active" : ""}`}
              onClick={() => onChangeFilter(pill.id)}
            >
              <span>{pill.label}</span>
              {typeof pill.count === "number" && pill.count > 0 && (
                <span className="mono" style={{ fontSize: "11px" }}>
                  {pill.count.toLocaleString()}
                </span>
              )}
            </button>
          );
        })}
      </div>
    </section>
  );
}
