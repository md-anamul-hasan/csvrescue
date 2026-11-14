import type { DocumentSummaryDto, ThemePreference } from "../types";
import { ColumnsIcon, HistoryIcon, SettingsIcon, TableIcon } from "./Icons";

export type NavTab = "workbench" | "columns" | "history" | "settings";

interface SidebarProps {
  activeTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  doc: DocumentSummaryDto | null;
  historyCount: number;
  version: string;
  theme: ThemePreference;
  onChangeTheme: (theme: ThemePreference) => void;
}

export function Sidebar({
  activeTab,
  onSelectTab,
  doc,
  historyCount,
  version,
  theme,
  onChangeTheme,
}: SidebarProps) {
  const issueBadge = doc ? doc.problems.length + doc.formulaRisks : 0;

  return (
    <aside className="sidebar" aria-label="Primary navigation">
      <div>
        <div className="sidebar-brand">
          <img src="/icon.svg" alt="" className="sidebar-logo" />
          <div className="sidebar-title-wrap">
            <span className="sidebar-title">CSVRescue</span>
            <span className="sidebar-version">v{version}</span>
          </div>
        </div>

        <nav className="sidebar-nav">
          <button
            type="button"
            className={`nav-item ${activeTab === "workbench" ? "active" : ""}`}
            aria-current={activeTab === "workbench" ? "page" : undefined}
            onClick={() => onSelectTab("workbench")}
          >
            <TableIcon size={16} />
            <span>Rescue & Preview</span>
          </button>

          <button
            type="button"
            className={`nav-item ${activeTab === "columns" ? "active" : ""}`}
            aria-current={activeTab === "columns" ? "page" : undefined}
            onClick={() => onSelectTab("columns")}
          >
            <ColumnsIcon size={16} />
            <span>Columns & Checks</span>
            {doc && issueBadge > 0 && (
              <span className="nav-badge warn">{issueBadge}</span>
            )}
          </button>

          <button
            type="button"
            className={`nav-item ${activeTab === "history" ? "active" : ""}`}
            aria-current={activeTab === "history" ? "page" : undefined}
            onClick={() => onSelectTab("history")}
          >
            <HistoryIcon size={16} />
            <span>History</span>
            {historyCount > 0 && <span className="nav-badge">{historyCount}</span>}
          </button>

          <button
            type="button"
            className={`nav-item ${activeTab === "settings" ? "active" : ""}`}
            aria-current={activeTab === "settings" ? "page" : undefined}
            onClick={() => onSelectTab("settings")}
          >
            <SettingsIcon size={16} />
            <span>Settings & Guide</span>
          </button>
        </nav>
      </div>

      <div className="sidebar-footer">
        {doc && (
          <div className="doc-pill" title={doc.path}>
            <span className="doc-pill-name">{doc.fileName}</span>
            <span className="doc-pill-meta">
              {doc.dataRows.toLocaleString()} rows × {doc.columnCount} cols
            </span>
          </div>
        )}

        <div className="theme-switch" role="group" aria-label="Color theme">
          {(["system", "dark", "light"] as ThemePreference[]).map((pref) => (
            <button
              key={pref}
              type="button"
              className={`theme-btn ${theme === pref ? "active" : ""}`}
              aria-pressed={theme === pref}
              onClick={() => onChangeTheme(pref)}
            >
              {pref.charAt(0).toUpperCase() + pref.slice(1)}
            </button>
          ))}
        </div>
      </div>
    </aside>
  );
}
