//! Persistent settings, recent file list, and export operation history in `%APPDATA%\CSVRescue`.

use crate::errors::Result;
use crate::security;
use crate::AppPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// UI theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

/// Persisted application defaults stored in `%APPDATA%\CSVRescue\settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub theme: ThemePreference,
    pub default_out_delimiter: char,
    pub default_utf8_bom: bool,
    pub default_decimal_as_dot: bool,
    pub default_excel_protect: bool,
    pub default_sanitize_formulas: bool,
    pub default_repair_double_encoding: bool,
    pub recent_files: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            default_out_delimiter: ',',
            default_utf8_bom: true,
            default_decimal_as_dot: false,
            default_excel_protect: false,
            default_sanitize_formulas: false,
            default_repair_double_encoding: true,
            recent_files: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn sanitized(mut self) -> Self {
        if !matches!(self.default_out_delimiter, ',' | ';' | '\t' | '|') {
            self.default_out_delimiter = ',';
        }
        self.recent_files.truncate(10);
        self
    }

    pub fn push_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| !p.eq_ignore_ascii_case(path));
        self.recent_files.insert(0, path.to_owned());
        self.recent_files.truncate(10);
    }
}

/// Record of an analyzed/exported file stored locally in `%APPDATA%\CSVRescue\history.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: String,
    pub source_path: String,
    pub output_path: String,
    pub format: String,
    pub encoding_name: String,
    pub delimiter: char,
    pub rows_written: usize,
    pub columns_written: usize,
    pub mojibake_repaired: usize,
    pub formulas_sanitized: usize,
}

/// Loads and saves `settings.json`.
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(app_paths: &AppPaths) -> Self {
        Self {
            path: app_paths.settings(),
        }
    }

    pub fn load(&self) -> AppConfig {
        match fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str::<AppConfig>(&raw)
                .map(AppConfig::sanitized)
                .unwrap_or_default(),
            Err(_) => AppConfig::default(),
        }
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        let clean = config.clone().sanitized();
        let json = serde_json::to_string_pretty(&clean)
            .map_err(|e| crate::errors::Error::InvalidInput(format!("JSON error: {e}")))?;
        security::atomic_write_bytes(&self.path, json.as_bytes())
    }
}

/// Loads and appends export history records (`history.json`).
pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    pub fn from_app_paths(app_paths: &AppPaths) -> Self {
        Self {
            path: app_paths.history(),
        }
    }

    pub fn list(&self) -> Vec<HistoryEntry> {
        match fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    pub fn append(&self, entry: HistoryEntry) -> Result<()> {
        let mut all = self.list();
        all.insert(0, entry);
        all.truncate(100);
        let json = serde_json::to_string_pretty(&all)
            .map_err(|e| crate::errors::Error::InvalidInput(format!("JSON error: {e}")))?;
        security::atomic_write_bytes(&self.path, json.as_bytes())
    }

    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .map_err(|e| crate::errors::Error::io("Could not clear history", e))?;
        }
        Ok(())
    }
}
