use crate::state::SharedState;
use hc_core::model::*;
use hc_storage::project;

#[tauri::command]
pub async fn new_project(_state: tauri::State<'_, SharedState>, name: String) -> Result<Project, String> {
    let mut project = Project::default();
    project.name = name.clone();
    let base = std::env::temp_dir().join("hc_projects").join(project::project_dir_name(&name));
    project::create_project(&base, &project).map_err(|e| e.to_string())?;
    project.path = Some(base);
    Ok(project)
}

#[tauri::command]
pub async fn open_project(_state: tauri::State<'_, SharedState>, path: String) -> Result<Project, String> {
    project::load_project(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_project(_state: tauri::State<'_, SharedState>, project: Project) -> Result<(), String> {
    if let Some(ref p) = project.path {
        project::save_project_file(p, &project).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_devices(_state: tauri::State<'_, SharedState>) -> Result<Vec<Device>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn start_runtime(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.lock().await.runtime.start(None).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_runtime(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.lock().await.runtime.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn runtime_status(state: tauri::State<'_, SharedState>) -> Result<RuntimeStatus, String> {
    Ok(state.lock().await.runtime.status().await)
}

#[tauri::command]
pub async fn write_tag(_state: tauri::State<'_, SharedState>, _tag_id: String, _value: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn query_trend(state: tauri::State<'_, SharedState>, tag_ids: Vec<String>, from_ms: i64, to_ms: i64, max_points: u32) -> Result<Vec<Sample>, String> {
    let s = state.lock().await;
    if let Some(ref db) = s.history {
        db.query_trend(&tag_ids, from_ms, to_ms, max_points).map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    }
}
