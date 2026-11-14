import { useCallback, useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, isTauriRuntime } from "./api";
import { ColumnsValidationView } from "./components/ColumnsValidationView";
import { ExportModal } from "./components/ExportModal";
import { HistoryView } from "./components/HistoryView";
import { InspectorDrawer } from "./components/InspectorDrawer";
import { SettingsGuideView } from "./components/SettingsGuideView";
import { NavTab, Sidebar } from "./components/Sidebar";
import { WorkbenchView } from "./components/WorkbenchView";
import type {
  AppConfig,
  ColumnKind,
  CsvSettings,
  DocumentSummaryDto,
  HistoryEntry,
  PreviewPageDto,
  RowComparisonDto,
  RowFilter,
  SamplePreset,
  ThemePreference,
} from "./types";

const DEFAULT_CONFIG: AppConfig = {
  theme: "system",
  defaultOutDelimiter: ",",
  defaultUtf8Bom: true,
  defaultDecimalAsDot: false,
  defaultExcelProtect: false,
  defaultSanitizeFormulas: false,
  defaultRepairDoubleEncoding: true,
  recentFiles: [],
};

const DEFAULT_SETTINGS: CsvSettings = {
  encoding: "Auto",
  delimiter: null,
  mark: "auto",
  hasHeader: true,
  repairDoubleEncoding: true,
  skipEmptyLines: true,
  escapeMode: "doubledQuote",
  repairStrategy: "preserve",
};

const PAGE_LIMIT = 120;

export function App() {
  const [activeTab, setActiveTab] = useState<NavTab>("workbench");
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [encodingChoices, setEncodingChoices] = useState<string[]>([]);
  const [version, setVersion] = useState<string>("2.0.0");
  const [doc, setDoc] = useState<DocumentSummaryDto | null>(null);
  const [settings, setSettings] = useState<CsvSettings>(DEFAULT_SETTINGS);
  const [history, setHistory] = useState<HistoryEntry[]>([]);

  const [filter, setFilter] = useState<RowFilter>("all");
  const [search, setSearch] = useState<string>("");
  const [pageOffset, setPageOffset] = useState<number>(0);
  const [preview, setPreview] = useState<PreviewPageDto | null>(null);

  const [inspectedRow, setInspectedRow] = useState<RowComparisonDto | null>(null);
  const [exportOpen, setExportOpen] = useState<boolean>(false);
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [toast, setToast] = useState<{ kind: "success" | "error"; text: string } | null>(null);

  const showToast = useCallback((kind: "success" | "error", text: string) => {
    setToast({ kind, text });
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 5000);
    return () => clearTimeout(t);
  }, [toast]);

  // Apply theme preference to <html>
  useEffect(() => {
    const apply = () => {
      const resolved =
        config.theme === "system"
          ? window.matchMedia("(prefers-color-scheme: light)").matches
            ? "light"
            : "dark"
          : config.theme;
      document.documentElement.setAttribute("data-theme", resolved);
    };
    apply();
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [config.theme]);

  const refreshPreview = useCallback(
    async (nextFilter = filter, nextSearch = search, nextOffset = pageOffset) => {
      if (!doc) {
        setPreview(null);
        return;
      }
      try {
        const page = await api.queryPreview({
          offset: nextOffset,
          limit: PAGE_LIMIT,
          search: nextSearch.trim() ? nextSearch : null,
          filter: nextFilter,
        });
        setPreview(page);
      } catch (err) {
        showToast("error", err instanceof Error ? err.message : String(err));
      }
    },
    [doc, filter, search, pageOffset, showToast],
  );

  useEffect(() => {
    void refreshPreview();
  }, [refreshPreview]);

  // Initial bootstrap: load config, history, CLI-opened doc or default sample
  useEffect(() => {
    let mounted = true;
    void (async () => {
      try {
        const boot = await api.getBootstrap();
        if (!mounted) return;
        setConfig(boot.config);
        setEncodingChoices(boot.encodingChoices);
        setVersion(boot.version);
        setHistory(boot.history);

        const initSettings: CsvSettings = {
          ...DEFAULT_SETTINGS,
          repairDoubleEncoding: boot.config.defaultRepairDoubleEncoding,
        };
        setSettings(initSettings);

        if (boot.currentDocument) {
          setDoc(boot.currentDocument);
        } else {
          const sampleDoc = await api.loadSample("europeanErp");
          if (mounted) setDoc(sampleDoc);
        }
      } catch (err) {
        if (mounted) {
          showToast("error", err instanceof Error ? err.message : String(err));
        }
      }
    })();
    return () => {
      mounted = false;
    };
  }, [showToast]);

  const handleOpenPath = useCallback(
    async (path: string) => {
      try {
        const nextDoc = await api.openFile(path);
        const boot = await api.getBootstrap();
        setConfig(boot.config);
        setDoc(nextDoc);
        setSettings({
          ...DEFAULT_SETTINGS,
          repairDoubleEncoding: boot.config.defaultRepairDoubleEncoding,
        });
        setFilter("all");
        setSearch("");
        setPageOffset(0);
        setInspectedRow(null);
        setActiveTab("workbench");
      } catch (err) {
        showToast("error", err instanceof Error ? err.message : String(err));
      }
    },
    [showToast],
  );

  const handleOpenPicker = useCallback(async () => {
    const picked = await api.pickOpenFile();
    if (picked) {
      await handleOpenPath(picked);
    }
  }, [handleOpenPath]);

  // Global keyboard shortcuts (Ctrl+O, Ctrl+E)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        void handleOpenPicker();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "e") {
        if (doc) {
          e.preventDefault();
          setExportOpen(true);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [doc, handleOpenPicker]);

  // Tauri native drag-and-drop file drop listener
  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        unlisten = await getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === "enter" || event.payload.type === "over") {
            setIsDragging(true);
          } else if (event.payload.type === "leave") {
            setIsDragging(false);
          } else if (event.payload.type === "drop") {
            setIsDragging(false);
            const droppedPath = event.payload.paths[0];
            if (droppedPath) {
              void handleOpenPath(droppedPath);
            }
          }
        });
      } catch {
        // Safe ignore in environments without webview drag-drop
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [handleOpenPath]);

  const handleLoadSample = async (preset: SamplePreset) => {
    try {
      const nextDoc = await api.loadSample(preset);
      setDoc(nextDoc);
      setSettings({
        ...DEFAULT_SETTINGS,
        repairDoubleEncoding: config.defaultRepairDoubleEncoding,
      });
      setFilter("all");
      setSearch("");
      setPageOffset(0);
      setInspectedRow(null);
      setActiveTab("workbench");
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleChangeSettings = async (next: CsvSettings) => {
    setSettings(next);
    try {
      const updated = await api.updateSettings(next);
      setDoc(updated);
      setPageOffset(0);
      setInspectedRow(null);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleInspectRow = async (rowIndex: number) => {
    try {
      const comparison = await api.getRowComparison(rowIndex);
      setInspectedRow(comparison);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleSaveCell = async (rowIndex: number, colIndex: number, value: string) => {
    try {
      const updated = await api.setCellOverride(rowIndex, colIndex, value);
      setDoc(updated);
      const comparison = await api.getRowComparison(rowIndex);
      setInspectedRow(comparison);
      showToast("success", `Updated cell (${rowIndex + 1}, ${colIndex + 1})`);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleRevertCell = async (rowIndex: number, colIndex: number) => {
    try {
      const updated = await api.revertCellOverride(rowIndex, colIndex);
      setDoc(updated);
      const comparison = await api.getRowComparison(rowIndex);
      setInspectedRow(comparison);
      showToast("success", `Reverted manual edit at (${rowIndex + 1}, ${colIndex + 1})`);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleSetColumnKind = async (colIndex: number, kind: ColumnKind) => {
    try {
      const updated = await api.setColumnKind(colIndex, kind);
      setDoc(updated);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleSetAllColumnsKind = async (kind: ColumnKind) => {
    try {
      const updated = await api.setAllColumnsKind(kind);
      setDoc(updated);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleResetColumnsAuto = async () => {
    try {
      const updated = await api.resetColumnsAuto();
      setDoc(updated);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleUpdateConfig = async (next: AppConfig) => {
    try {
      const saved = await api.setConfig(next);
      setConfig(saved);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    }
  };

  const handleChangeTheme = (theme: ThemePreference) => {
    void handleUpdateConfig({ ...config, theme });
  };

  const handleClearHistory = async () => {
    await api.clearHistory();
    setHistory([]);
    showToast("success", "Export history cleared");
  };

  return (
    <div className="app-shell">
      <Sidebar
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        doc={doc}
        historyCount={history.length}
        version={version}
        theme={config.theme}
        onChangeTheme={handleChangeTheme}
      />

      <main className="main-content">
        {activeTab === "workbench" && (
          <WorkbenchView
            doc={doc}
            settings={settings}
            encodingChoices={encodingChoices}
            recentFiles={config.recentFiles}
            preview={preview}
            filter={filter}
            search={search}
            pageOffset={pageOffset}
            pageLimit={PAGE_LIMIT}
            selectedRowIndex={inspectedRow?.rowIndex ?? null}
            isDraggingFile={isDragging}
            onOpenFilePicker={handleOpenPicker}
            onOpenPath={handleOpenPath}
            onLoadSample={handleLoadSample}
            onChangeSettings={handleChangeSettings}
            onChangeFilter={(f) => {
              setFilter(f);
              setPageOffset(0);
            }}
            onChangeSearch={(q) => {
              setSearch(q);
              setPageOffset(0);
            }}
            onChangeOffset={setPageOffset}
            onInspectRow={handleInspectRow}
            onToggleColumnKind={handleSetColumnKind}
            onOpenExport={() => setExportOpen(true)}
          />
        )}

        {activeTab === "columns" && (
          <ColumnsValidationView
            doc={doc}
            onSetColumnKind={handleSetColumnKind}
            onSetAllColumnsKind={handleSetAllColumnsKind}
            onResetColumnsAuto={handleResetColumnsAuto}
          />
        )}

        {activeTab === "history" && (
          <HistoryView
            history={history}
            onOpenSource={handleOpenPath}
            onClearHistory={handleClearHistory}
          />
        )}

        {activeTab === "settings" && (
          <SettingsGuideView
            config={config}
            version={version}
            onUpdateConfig={handleUpdateConfig}
          />
        )}
      </main>

      <InspectorDrawer
        row={inspectedRow}
        onClose={() => setInspectedRow(null)}
        onSaveCell={handleSaveCell}
        onRevertCell={handleRevertCell}
      />

      {doc && (
        <ExportModal
          isOpen={exportOpen}
          doc={doc}
          config={config}
          hasHeader={settings.hasHeader}
          onClose={() => setExportOpen(false)}
          onExported={async (msg) => {
            showToast("success", msg);
            setHistory(await api.getHistory());
          }}
          onError={(errMsg) => showToast("error", errMsg)}
        />
      )}

      {toast && (
        <div className={`toast-bar ${toast.kind}`} role="status" aria-live="polite">
          <span>{toast.text}</span>
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => setToast(null)}
            aria-label="Dismiss notification"
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}
export default App;
