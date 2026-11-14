//! File validation, overwrite protection, CSV formula-injection guards, and atomic writes.

use crate::analysis;
use crate::errors::{Error, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum file size opened in standard document mode (300 MiB, matching CSVRescue 1.x).
pub const MAX_BYTES: u64 = 300 * 1024 * 1024;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Formula-triggering prefixes detected by [`detect_formula_risk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormulaHazard {
    pub trigger_char: char,
    pub has_leading_whitespace: bool,
}

/// Validates that `path` points to a readable regular file within [`MAX_BYTES`].
pub fn validate_source_file(path: &Path) -> Result<u64> {
    if path.as_os_str().is_empty() {
        return Err(Error::InvalidInput("Choose a file first.".into()));
    }
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::FileNotFound(path.display().to_string()));
        }
        Err(e) => return Err(Error::io("Could not inspect the source file", e)),
    };
    if !meta.is_file() {
        return Err(Error::InvalidInput(format!(
            "The selected path is not a regular file: {}",
            path.display()
        )));
    }
    let len = meta.len();
    if len > MAX_BYTES {
        return Err(Error::FileTooLarge {
            size_bytes: len,
            max_bytes: MAX_BYTES,
        });
    }
    Ok(len)
}

/// Checks whether two paths refer to the same file on Windows (case-insensitive, normalized,
/// and canonicalized when both targets exist on disk).
pub fn is_same_file(source: &Path, target: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (fs::canonicalize(source), fs::canonicalize(target)) {
        if normalize_lexical(&a).eq_ignore_ascii_case(&normalize_lexical(&b)) {
            return true;
        }
    }
    let abs_a = absolutize_lexical(source);
    let abs_b = absolutize_lexical(target);
    abs_a.eq_ignore_ascii_case(&abs_b)
}

fn absolutize_lexical(path: &Path) -> String {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };
    normalize_lexical(&joined)
}

fn normalize_lexical(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => {
                let s = p.as_os_str().to_string_lossy();
                let trimmed = s.strip_prefix(r"\\?\").unwrap_or(&s);
                parts.push(trimmed.to_owned());
            }
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            Component::Normal(seg) => parts.push(seg.to_string_lossy().into_owned()),
        }
    }
    if parts.is_empty() {
        String::new()
    } else if parts.len() == 1 {
        format!("{}\\", parts[0])
    } else {
        format!("{}\\{}", parts[0], parts[1..].join("\\"))
    }
}

/// Detects potential spreadsheet formula injection in a cell value.
///
/// Flags cells starting with `=`, `+`, `@`, or `-` (including after leading ASCII whitespace or
/// tab/CR/LF control characters), while allowing clean plain negative numbers like `-12.5` or
/// `-0,5` so legitimate numeric columns do not produce false positives.
pub fn detect_formula_risk(cell: &str) -> Option<FormulaHazard> {
    if cell.is_empty() {
        return None;
    }
    let trimmed =
        cell.trim_start_matches(|c: char| c.is_ascii_whitespace() || c.is_ascii_control());
    let first = trimmed.chars().next()?;
    if !matches!(first, '=' | '+' | '-' | '@') {
        return None;
    }
    let has_leading_ws = trimmed.len() != cell.len();
    // Clean plain negative numbers (e.g. "-12.5", "-0,5", "-100") are only dangerous if prefixed by
    // tab/control whitespace or followed by formula expressions like "-2+3" or "-@SUM".
    if first == '-' && !has_leading_ws && is_plain_negative_number(trimmed) {
        return None;
    }
    Some(FormulaHazard {
        trigger_char: first,
        has_leading_whitespace: has_leading_ws,
    })
}

fn is_plain_negative_number(s: &str) -> bool {
    analysis::try_parse_number(s, analysis::DecimalMark::Dot).is_some()
        || analysis::try_parse_number(s, analysis::DecimalMark::Comma).is_some()
}

/// Neutralizes spreadsheet formula triggers by prefixing `'`, returning `(output, was_sanitized)`.
pub fn sanitize_formula_cell(cell: &str) -> (String, bool) {
    if detect_formula_risk(cell).is_some() && !cell.starts_with('\'') {
        (format!("'{cell}"), true)
    } else {
        (cell.to_owned(), false)
    }
}

/// Guard that deletes a temporary sibling file if dropped before `persist` succeeds.
pub struct TempFileGuard {
    pub path: PathBuf,
    armed: bool,
}

impl TempFileGuard {
    pub fn new(dest: &Path) -> Self {
        let pid = std::process::id();
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let file_name = dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".to_owned());
        let tmp_name = format!(".{file_name}.{pid}.{seq}.tmp");
        let path = dest
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join(&tmp_name))
            .unwrap_or_else(|| PathBuf::from(&tmp_name));
        Self { path, armed: true }
    }

    pub fn persist(mut self, dest: &Path) -> Result<()> {
        move_replace_atomic(&self.path, dest)?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Atomically writes `bytes` to `dest` via a sibling `.tmp` file, `sync_all`, and atomic replace.
pub fn atomic_write_bytes(dest: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|e| Error::io("Could not create destination folder", e))?;
    }
    let guard = TempFileGuard::new(dest);
    {
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&guard.path)
            .map_err(|e| Error::io("Could not create temporary output file", e))?;
        f.write_all(bytes)
            .map_err(|e| Error::io("Could not write temporary output file", e))?;
        f.sync_all()
            .map_err(|e| Error::io("Could not flush output file to disk", e))?;
    }
    guard.persist(dest)
}

/// Creates and returns an open temporary file handle plus guard for streaming writers (e.g. `.xlsx` ZIP).
pub fn create_temp_file(dest: &Path) -> Result<(File, TempFileGuard)> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|e| Error::io("Could not create destination folder", e))?;
    }
    let guard = TempFileGuard::new(dest);
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&guard.path)
        .map_err(|e| Error::io("Could not create temporary output file", e))?;
    Ok((file, guard))
}

#[cfg(windows)]
fn move_replace_atomic(tmp: &Path, dest: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let from_w: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_w: Vec<u16> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Both buffers are NUL-terminated UTF-16 slices alive across the Win32 call.
    let ok = unsafe {
        MoveFileExW(
            from_w.as_ptr(),
            to_w.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        return Err(Error::io(
            "Could not finalize output file",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn move_replace_atomic(tmp: &Path, dest: &Path) -> Result<()> {
    fs::rename(tmp, dest).map_err(|e| Error::io("Could not finalize output file", e))
}

/// Opens a file or folder path using Windows ShellExecuteW.
#[cfg(windows)]
pub fn open_with_shell(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Valid NUL-terminated UTF-16 strings for ShellExecuteW.
    let res = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if res <= 32 {
        return Err(Error::InvalidInput(format!(
            "Could not open '{}'",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn open_with_shell(_path: &Path) -> Result<()> {
    Err(Error::InvalidInput(
        "Shell open is supported on Windows only.".into(),
    ))
}

/// Reveals a file in Windows Explorer (`explorer.exe /select,"<path>"`).
#[cfg(windows)]
pub fn reveal_in_explorer(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound(path.display().to_string()));
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|e| Error::io("Could not open Windows Explorer", e))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn reveal_in_explorer(_path: &Path) -> Result<()> {
    Err(Error::InvalidInput(
        "Explorer reveal is supported on Windows only.".into(),
    ))
}
