//! Centralized error type for `csvrescue-core`.

use serde::Serialize;
use std::fmt;

/// Result alias for CSVRescue engine operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Domain and I/O errors produced by the CSVRescue engine.
#[derive(Debug)]
pub enum Error {
    FileNotFound(String),
    FileTooLarge {
        size_bytes: u64,
        max_bytes: u64,
    },
    SameFileOverwrite(String),
    ExcelLimitExceeded(String),
    EncodingError(String),
    InvalidInput(String),
    Busy(String),
    Cancelled,
    Io {
        context: &'static str,
        source: std::io::Error,
    },
}

impl Error {
    pub fn io(context: &'static str, source: std::io::Error) -> Self {
        Self::Io { context, source }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(path) => write!(f, "That file does not exist: {path}"),
            Self::FileTooLarge {
                size_bytes,
                max_bytes,
            } => write!(
                f,
                "The file is {} MB; CSVRescue opens files up to {} MB.",
                size_bytes / (1024 * 1024),
                max_bytes / (1024 * 1024)
            ),
            Self::SameFileOverwrite(path) => write!(
                f,
                "CSVRescue never overwrites the file it opened ({path}). Choose another name."
            ),
            Self::ExcelLimitExceeded(msg) => f.write_str(msg),
            Self::EncodingError(msg) => write!(f, "Encoding error: {msg}"),
            Self::InvalidInput(msg) => f.write_str(msg),
            Self::Busy(msg) => f.write_str(msg),
            Self::Cancelled => f.write_str("The operation was cancelled."),
            Self::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
