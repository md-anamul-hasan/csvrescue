//! Local structured JSON-lines logger with size-based rotation and plain `error.log` fallback.

use crate::AppPaths;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize)]
struct LogRecord<'a> {
    ts: &'a str,
    level: &'a str,
    scope: &'a str,
    msg: &'a str,
}

/// Thread-safe local logger writing under `%APPDATA%\CSVRescue\logs`.
pub struct Logger {
    log_file: PathBuf,
    error_file: PathBuf,
    guard: Mutex<()>,
}

impl Logger {
    pub fn from_app_paths(paths: &AppPaths) -> Self {
        Self {
            log_file: paths.logs().join("csvrescue.log"),
            error_file: paths.error_log(),
            guard: Mutex::new(()),
        }
    }

    pub fn info(&self, scope: &str, msg: &str) {
        self.write_line("info", scope, msg);
    }

    pub fn warn(&self, scope: &str, msg: &str) {
        self.write_line("warn", scope, msg);
    }

    pub fn error(&self, scope: &str, msg: &str) {
        self.write_line("error", scope, msg);
        self.append_error_log(scope, msg);
    }

    fn write_line(&self, level: &str, scope: &str, msg: &str) {
        let Ok(_lock) = self.guard.lock() else {
            return;
        };
        if let Some(parent) = self.log_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(meta) = fs::metadata(&self.log_file) {
            if meta.len() >= MAX_LOG_BYTES {
                let rotated = self.log_file.with_extension("log.1");
                let _ = fs::remove_file(&rotated);
                let _ = fs::rename(&self.log_file, &rotated);
            }
        }
        let ts = current_local_timestamp();
        let rec = LogRecord {
            ts: &ts,
            level,
            scope,
            msg,
        };
        if let Ok(line) = serde_json::to_string(&rec) {
            if let Ok(mut f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.log_file)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    fn append_error_log(&self, scope: &str, msg: &str) {
        if let Some(parent) = self.error_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.error_file)
        {
            let ts = current_local_timestamp();
            let _ = writeln!(f, "[{ts}] [{scope}] {msg}");
        }
    }
}

/// Returns current local wall-clock timestamp formatted as `YYYY-MM-DD HH:MM:SS`.
#[cfg(windows)]
pub fn current_local_timestamp() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut st = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    // SAFETY: `st` is a valid mutable stack struct passed to Win32 `GetLocalTime`.
    unsafe {
        GetLocalTime(&mut st);
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

#[cfg(not(windows))]
pub fn current_local_timestamp() -> String {
    "1970-01-01 00:00:00".to_owned()
}
