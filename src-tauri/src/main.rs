#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use hc_ipc::state::{AppState, SharedState};
use hc_ipc::commands;
use hc_ipc::handlers;
use std::sync::Arc;
use tokio::sync::Mutex;

fn main() {
    let state: SharedState = Arc::new(Mutex::new(AppState::new()));

    tauri::Builder::default()
        .manage(state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            let s = state.clone();
            tokio::spawn(async move {
                s.lock().await.tauri_app = Some(handle);
                handlers::spawn_event_handler(s.clone()).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::new_project,
            commands::open_project,
            commands::save_project,
            commands::list_devices,
            commands::start_runtime,
            commands::stop_runtime,
            commands::runtime_status,
            commands::write_tag,
            commands::query_trend,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
