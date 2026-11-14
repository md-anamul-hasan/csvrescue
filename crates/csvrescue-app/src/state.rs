use csvrescue_core::core::CsvDocument;
use csvrescue_core::logging::Logger;
use csvrescue_core::storage::{AppConfig, ConfigStore, HistoryStore};
use csvrescue_core::AppPaths;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

pub struct AppState {
    pub paths: AppPaths,
    pub doc: Mutex<Option<CsvDocument>>,
    pub config_store: ConfigStore,
    pub config: Mutex<AppConfig>,
    pub history_store: HistoryStore,
    pub logger: Logger,
    pub cancel: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        let paths = AppPaths::from_env()
            .unwrap_or_else(|_| AppPaths::with_root(std::env::temp_dir().join("CSVRescue")));
        let config_store = ConfigStore::new(&paths);
        let config = config_store.load();
        let history_store = HistoryStore::from_app_paths(&paths);
        let logger = Logger::from_app_paths(&paths);

        Self {
            paths,
            doc: Mutex::new(None),
            config_store,
            config: Mutex::new(config),
            history_store,
            logger,
            cancel: AtomicBool::new(false),
        }
    }
}
