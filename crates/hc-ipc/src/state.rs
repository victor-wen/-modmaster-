use hc_runtime::runtime::Runtime;
use hc_storage::history::HistoryDb;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub runtime: Runtime,
    pub history: Option<HistoryDb>,
    pub tauri_app: Option<tauri::AppHandle>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            runtime: Runtime::new(),
            history: None,
            tauri_app: None,
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;
