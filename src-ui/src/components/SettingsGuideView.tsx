import { api } from "../api";
import type { AppConfig, ThemePreference } from "../types";
import { FolderIcon } from "./Icons";

interface SettingsGuideViewProps {
  config: AppConfig;
  version: string;
  onUpdateConfig: (next: AppConfig) => Promise<void>;
}

export function SettingsGuideView({
  config,
  version,
  onUpdateConfig,
}: SettingsGuideViewProps) {
  return (
    <div className="section-stack">
      <div className="panel">
        <div className="panel-head">
          <div>
            <h2 className="panel-title">Default Export & Recovery Preferences</h2>
            <span style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
              Persisted in %APPDATA%\CSVRescue\settings.json. Zero telemetry, 100% offline.
            </span>
          </div>
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => void api.openDataFolder()}
          >
            <FolderIcon size={14} />
            <span>Open AppData Folder</span>
          </button>
        </div>

        <div className="options-grid">
          <label className="control-group">
            <span className="control-label">Color Theme</span>
            <select
              className="select-input"
              value={config.theme}
              onChange={(e) =>
                void onUpdateConfig({ ...config, theme: e.target.value as ThemePreference })
              }
            >
              <option value="system">System (Windows Default)</option>
              <option value="dark">Dark Slate</option>
              <option value="light">Light Porcelain</option>
            </select>
          </label>

          <label className="control-group">
            <span className="control-label">Default CSV Output Delimiter</span>
            <select
              className="select-input"
              value={config.defaultOutDelimiter}
              onChange={(e) =>
                void onUpdateConfig({ ...config, defaultOutDelimiter: e.target.value })
              }
            >
              <option value=",">Comma (,)</option>
              <option value=";">Semicolon (;)</option>
              <option value={"\t"}>Tab (\t)</option>
              <option value="|">Pipe (|)</option>
            </select>
          </label>
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={config.defaultRepairDoubleEncoding}
              onChange={(e) =>
                void onUpdateConfig({ ...config, defaultRepairDoubleEncoding: e.target.checked })
              }
            />
            <span>Automatically repair double-encoded UTF-8 mojibake (e.g. Ã© → é)</span>
          </label>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={config.defaultUtf8Bom}
              onChange={(e) =>
                void onUpdateConfig({ ...config, defaultUtf8Bom: e.target.checked })
              }
            />
            <span>Include UTF-8 BOM in CSV exports by default (recommended for Excel)</span>
          </label>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={config.defaultDecimalAsDot}
              onChange={(e) =>
                void onUpdateConfig({ ...config, defaultDecimalAsDot: e.target.checked })
              }
            />
            <span>Normalize comma decimals (12,50) to dot decimals (12.50) in numeric CSV columns</span>
          </label>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={config.defaultExcelProtect}
              onChange={(e) =>
                void onUpdateConfig({ ...config, defaultExcelProtect: e.target.checked })
              }
            />
            <span>Wrap risky Text values as ="..." when exporting CSV for Excel</span>
          </label>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={config.defaultSanitizeFormulas}
              onChange={(e) =>
                void onUpdateConfig({ ...config, defaultSanitizeFormulas: e.target.checked })
              }
            />
            <span>Neutralize spreadsheet formula injection triggers (= + - @) on export</span>
          </label>
        </div>
      </div>

      <div className="panel">
        <h2 className="panel-title">CSVRescue Engine Reference (v{version})</h2>
        <div className="table-scroll">
          <table className="std-table">
            <thead>
              <tr>
                <th>Symptom</th>
                <th>Cause</th>
                <th>How CSVRescue Protects Your Data</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><strong>Accents became Ã© or â€™</strong></td>
                <td>UTF-8 bytes misread as Windows-1252 and re-saved as UTF-8.</td>
                <td>Deterministic CP1252 → UTF-8 reversal only where strictly reversible; leaves valid letters untouched.</td>
              </tr>
              <tr>
                <td><strong>Everything collapsed into Column A</strong></td>
                <td>European semicolon (;) or tab delimiter ignored by Excel locale.</td>
                <td>Auto-scores comma, semicolon, tab, pipe, and colon delimiters across the leading 200 records.</td>
              </tr>
              <tr>
                <td><strong>00123 became 123 / ID turned into E+16</strong></td>
                <td>Excel auto-coerces leading zeros, 16-digit IDs, fractions (1/2), and dates (3-4).</td>
                <td>Flags all Excel risks and writes Text columns as explicit OOXML <span className="mono">inlineStr</span> in .xlsx.</td>
              </tr>
              <tr>
                <td><strong>Cells start with =CMD| or +SUM</strong></td>
                <td>Untrusted external input contains spreadsheet formula triggers.</td>
                <td>Highlights formula hazards and optionally prefixes &apos; during export without mutating clean negative numbers (-12.5).</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
