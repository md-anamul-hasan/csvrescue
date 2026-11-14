#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use commands::*;
use csvrescue_core::core::{CsvDocument, CsvSettings};
use state::AppState;

fn secure_windows_environment() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::LibraryLoader::{
            SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32,
        };
        // Restrict DLL search path strictly to %WINDIR%\System32 to thwart DLL preloading/planting
        SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
    }
}

fn main() {
    secure_windows_environment();

    let app_state = AppState::new();

    // If a file path was passed on the command line (e.g. Windows Explorer "Open with"), preload it
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let candidate = &args[1];
        let path = std::path::Path::new(candidate);
        if path.is_file() {
            let repair_double = app_state
                .config
                .lock()
                .map(|c| c.default_repair_double_encoding)
                .unwrap_or(true);
            let settings = CsvSettings {
                repair_double_encoding: repair_double,
                ..CsvSettings::default()
            };
            if let Ok(doc) = CsvDocument::load(path, settings) {
                if let Ok(mut doc_guard) = app_state.doc.lock() {
                    *doc_guard = Some(doc);
                }
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_bootstrap,
            set_config,
            open_file,
            load_sample,
            update_settings,
            query_preview,
            get_row_comparison,
            set_cell_override,
            revert_cell_override,
            set_column_kind,
            set_all_columns_kind,
            reset_columns_auto,
            get_suggested_export_path,
            preview_export,
            export_csv,
            export_xlsx,
            export_recovery_report,
            cancel_operation,
            get_history,
            clear_history,
            open_in_explorer,
            open_data_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running CSVRescue");
}
