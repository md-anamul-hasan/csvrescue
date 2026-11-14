//! CSVRescue core engine.
//!
//! All CSV decoding, mojibake repair, delimiter/format detection, streaming parsing,
//! malformed record recovery, validation, formula-injection analysis, and CSV/XLSX
//! serialization live in this crate. The Tauri desktop shell (`csvrescue`) is strictly
//! an IPC/window wrapper.

pub mod analysis;
pub mod core;
pub mod detector;
pub mod encoding;
pub mod errors;
pub mod export;
pub mod logging;
pub mod parser;
pub mod preview;
pub mod recovery;
pub mod repair;
pub mod security;
pub mod storage;
pub mod validation;

pub use crate::core::{CsvDocument, CsvSettings, DocumentSummaryDto, SamplePreset};
pub use errors::{Error, Result};

/// Product version from workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Local storage paths under `%APPDATA%\CSVRescue`.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data: std::path::PathBuf,
}

impl AppPaths {
    pub fn from_env() -> Result<Self> {
        let appdata = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| Error::InvalidInput("The APPDATA folder is not set.".into()))?;
        Ok(Self {
            data: appdata.join("CSVRescue"),
        })
    }

    pub fn with_root(root: impl Into<std::path::PathBuf>) -> Self {
        Self { data: root.into() }
    }

    pub fn settings(&self) -> std::path::PathBuf {
        self.data.join("settings.json")
    }

    pub fn history(&self) -> std::path::PathBuf {
        self.data.join("history.json")
    }

    pub fn logs(&self) -> std::path::PathBuf {
        self.data.join("logs")
    }

    pub fn error_log(&self) -> std::path::PathBuf {
        self.data.join("error.log")
    }
}
