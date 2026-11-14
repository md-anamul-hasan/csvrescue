use crate::state::AppState;
use csvrescue_core::analysis::ColumnKind;
use csvrescue_core::core::{CsvDocument, CsvSettings, DocumentSummaryDto, SamplePreset};
use csvrescue_core::encoding::ENCODING_CHOICES;
use csvrescue_core::errors::Error;
use csvrescue_core::export::{
    preview_export as core_preview_export, save_csv, save_recovery_report, save_xlsx, ExportDryRun,
    ExportOptions, ExportSummary,
};
use csvrescue_core::logging::current_local_timestamp;
use csvrescue_core::preview::{
    get_row_comparison as core_get_row_comparison, query_preview as core_query_preview,
    PreviewPageDto, PreviewQuery, RowComparisonDto,
};
use csvrescue_core::security::{open_with_shell, reveal_in_explorer};
use csvrescue_core::storage::{AppConfig, HistoryEntry};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::State;

static ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub config: AppConfig,
    pub current_document: Option<DocumentSummaryDto>,
    pub history: Vec<HistoryEntry>,
    pub encoding_choices: Vec<String>,
    pub version: &'static str,
}

#[tauri::command]
pub fn get_bootstrap(state: State<'_, AppState>) -> Result<BootstrapPayload, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let history = state.history_store.list();
    let current_document = state
        .doc
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(CsvDocument::summary_dto);
    let encoding_choices = ENCODING_CHOICES.iter().map(|s| (*s).to_owned()).collect();

    Ok(BootstrapPayload {
        config,
        current_document,
        history,
        encoding_choices,
        version: csvrescue_core::VERSION,
    })
}

#[tauri::command]
pub fn set_config(new_config: AppConfig, state: State<'_, AppState>) -> Result<AppConfig, String> {
    let clean = new_config.sanitized();
    state.config_store.save(&clean).map_err(|e| e.to_string())?;
    let mut guard = state.config.lock().map_err(|e| e.to_string())?;
    *guard = clean.clone();
    Ok(clean)
}

#[tauri::command]
pub fn open_file(path: String, state: State<'_, AppState>) -> Result<DocumentSummaryDto, String> {
    let repair_double_encoding = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.default_repair_double_encoding
    };
    let settings = CsvSettings {
        repair_double_encoding,
        ..CsvSettings::default()
    };

    let doc = CsvDocument::load(Path::new(&path), settings).map_err(|e| {
        state.logger.error("open", &format!("{path}: {e}"));
        e.to_string()
    })?;
    let summary = doc.summary_dto();

    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.push_recent(&path);
        let _ = state.config_store.save(&cfg);
    }

    state.logger.info(
        "open",
        &format!(
            "Opened {} ({} rows, {} cols, encoding {}, delimiter '{}')",
            summary.file_name,
            summary.data_rows,
            summary.column_count,
            summary.encoding_name,
            summary.delimiter_name
        ),
    );

    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    *doc_guard = Some(doc);

    Ok(summary)
}

#[tauri::command]
pub fn load_sample(
    preset: SamplePreset,
    state: State<'_, AppState>,
) -> Result<DocumentSummaryDto, String> {
    let repair_double_encoding = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.default_repair_double_encoding
    };
    let settings = CsvSettings {
        repair_double_encoding,
        ..CsvSettings::default()
    };

    let doc = CsvDocument::from_sample_preset(preset, settings).map_err(|e| e.to_string())?;
    let summary = doc.summary_dto();

    state
        .logger
        .info("sample", &format!("Loaded sample {}", summary.file_name));

    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    *doc_guard = Some(doc);

    Ok(summary)
}

#[tauri::command]
pub fn update_settings(
    settings: CsvSettings,
    state: State<'_, AppState>,
) -> Result<DocumentSummaryDto, String> {
    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_mut()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    doc.update_settings(settings).map_err(|e| e.to_string())?;
    Ok(doc.summary_dto())
}

#[tauri::command]
pub fn query_preview(
    query: PreviewQuery,
    state: State<'_, AppState>,
) -> Result<PreviewPageDto, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    Ok(core_query_preview(doc, &query))
}

#[tauri::command]
pub fn get_row_comparison(
    row_index: usize,
    state: State<'_, AppState>,
) -> Result<RowComparisonDto, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    core_get_row_comparison(doc, row_index).ok_or_else(|| {
        Error::InvalidInput(format!("Row {} does not exist.", row_index + 1)).to_string()
    })
}

#[tauri::command]
pub fn set_cell_override(
    row_index: usize,
    col_index: usize,
    value: String,
    state: State<'_, AppState>,
) -> Result<DocumentSummaryDto, String> {
    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_mut()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    doc.set_manual_cell_edit(row_index, col_index, value);
    Ok(doc.summary_dto())
}

#[tauri::command]
pub fn revert_cell_override(
    row_index: usize,
    col_index: usize,
    state: State<'_, AppState>,
) -> Result<DocumentSummaryDto, String> {
    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_mut()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    doc.revert_manual_cell_edit(row_index, col_index);
    Ok(doc.summary_dto())
}

#[tauri::command]
pub fn set_column_kind(
    col_index: usize,
    kind: ColumnKind,
    state: State<'_, AppState>,
) -> Result<DocumentSummaryDto, String> {
    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_mut()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    if !doc.set_column_kind(col_index, kind) {
        return Err(
            Error::InvalidInput(format!("Column {} not found.", col_index + 1)).to_string(),
        );
    }
    Ok(doc.summary_dto())
}

#[tauri::command]
pub fn set_all_columns_kind(
    kind: ColumnKind,
    state: State<'_, AppState>,
) -> Result<DocumentSummaryDto, String> {
    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_mut()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    doc.set_all_columns_kind(kind);
    Ok(doc.summary_dto())
}

#[tauri::command]
pub fn reset_columns_auto(state: State<'_, AppState>) -> Result<DocumentSummaryDto, String> {
    let mut doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_mut()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    doc.reset_columns_auto();
    Ok(doc.summary_dto())
}

#[tauri::command]
pub fn get_suggested_export_path(
    extension: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    Ok(doc
        .suggested_name(&extension)
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub fn preview_export(
    options: ExportOptions,
    preview_rows: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ExportDryRun, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    Ok(core_preview_export(
        doc,
        &options,
        preview_rows.unwrap_or(25),
    ))
}

#[tauri::command]
pub fn export_csv(
    target_path: String,
    options: ExportOptions,
    state: State<'_, AppState>,
) -> Result<ExportSummary, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    let dest = Path::new(&target_path);
    let src_opt = (!doc.is_sample).then(|| Path::new(&doc.path));
    let summary = save_csv(dest, src_opt, doc.rows(), &doc.columns, &options).map_err(|e| {
        state
            .logger
            .error("export_csv", &format!("{target_path}: {e}"));
        e.to_string()
    })?;

    let entry = HistoryEntry {
        id: next_history_id(),
        timestamp: current_local_timestamp(),
        source_path: doc.path.clone(),
        output_path: target_path.clone(),
        format: "CSV".to_owned(),
        encoding_name: options.encoding.label().to_owned(),
        delimiter: options.delimiter,
        rows_written: summary.rows_written,
        columns_written: summary.columns_written,
        mojibake_repaired: doc.recovery.mojibake_cells_count,
        formulas_sanitized: summary.formulas_sanitized,
    };
    let _ = state.history_store.append(entry);

    state.logger.info(
        "export_csv",
        &format!(
            "Exported {} ({} rows, {} bytes)",
            target_path, summary.rows_written, summary.bytes_written
        ),
    );

    Ok(summary)
}

#[tauri::command]
pub fn export_xlsx(
    target_path: String,
    sanitize_formulas: bool,
    selected_columns: Option<Vec<usize>>,
    state: State<'_, AppState>,
) -> Result<ExportSummary, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    let dest = Path::new(&target_path);
    let src_opt = (!doc.is_sample).then(|| Path::new(&doc.path));
    state.cancel.store(false, Ordering::Relaxed);
    let summary = save_xlsx(
        dest,
        src_opt,
        doc.rows(),
        &doc.columns,
        doc.settings.has_header,
        sanitize_formulas,
        selected_columns.as_deref(),
        Some(&state.cancel),
    )
    .map_err(|e| {
        state
            .logger
            .error("export_xlsx", &format!("{target_path}: {e}"));
        e.to_string()
    })?;

    let entry = HistoryEntry {
        id: next_history_id(),
        timestamp: current_local_timestamp(),
        source_path: doc.path.clone(),
        output_path: target_path.clone(),
        format: "XLSX".to_owned(),
        encoding_name: "UTF-8 (OpenXML)".to_owned(),
        delimiter: doc.delimiter,
        rows_written: summary.rows_written,
        columns_written: summary.columns_written,
        mojibake_repaired: doc.recovery.mojibake_cells_count,
        formulas_sanitized: summary.formulas_sanitized,
    };
    let _ = state.history_store.append(entry);

    state.logger.info(
        "export_xlsx",
        &format!(
            "Exported {} ({} rows, {} bytes)",
            target_path, summary.rows_written, summary.bytes_written
        ),
    );

    Ok(summary)
}

#[tauri::command]
pub fn export_recovery_report(
    target_path: String,
    markdown: bool,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let doc_guard = state.doc.lock().map_err(|e| e.to_string())?;
    let doc = doc_guard
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("No active document loaded.".into()).to_string())?;

    let bytes =
        save_recovery_report(Path::new(&target_path), doc, markdown).map_err(|e| e.to_string())?;

    state.logger.info(
        "export_report",
        &format!("Exported recovery report to {target_path}"),
    );

    Ok(bytes)
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>) -> Result<bool, String> {
    state.cancel.store(true, Ordering::Relaxed);
    Ok(true)
}

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    Ok(state.history_store.list())
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<bool, String> {
    state.history_store.clear().map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<bool, String> {
    reveal_in_explorer(Path::new(&path)).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn open_data_folder(state: State<'_, AppState>) -> Result<bool, String> {
    fs::create_dir_all(&state.paths.data).map_err(|e| e.to_string())?;
    open_with_shell(&state.paths.data).map_err(|e| e.to_string())?;
    Ok(true)
}

fn next_history_id() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("hist-{ms}-{seq}")
}
