//! Tauri commands: thin async wrappers over the Engine (blocking work runs off the UI thread).

use crate::{config::AppConfig, engine::Engine, project::Project};
use std::{path::PathBuf, sync::Arc};
use tauri::State;

#[tauri::command]
pub fn get_config(cfg: State<AppConfig>) -> AppConfig {
    cfg.inner().clone()
}

#[tauri::command]
pub async fn analyze_source(engine: State<'_, Arc<Engine>>, path: String) -> Result<Project, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.analyze(&PathBuf::from(path)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_clip(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String) -> Result<String, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.export(&project_id, &clip_id).map(|p| p.to_string_lossy().into_owned()))
        .await
        .map_err(|e| e.to_string())?
}
