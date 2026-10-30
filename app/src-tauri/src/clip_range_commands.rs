//! Tauri commands for manual Clips, trimming and preview (cruxvid-b2t.17). None calls Gemini.

use crate::{clip_range::{self, Added, Preview, RangeEdit, RangeWarning}, engine::Engine, project::Project};
use std::{path::PathBuf, sync::Arc};
use tauri::{Manager, State};

/// A Project for a local Source without analysis (no API key needed).
#[tauri::command]
pub async fn create_manual_project(engine: State<'_, Arc<Engine>>, path: String) -> Result<Project, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clip_range::new_manual_project(&e, &PathBuf::from(path))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn add_manual_clip(engine: State<'_, Arc<Engine>>, project_id: String, start: f64, end: f64) -> Result<Added, String> {
    clip_range::add_manual_clip(&engine, &project_id, start, end)
}

#[tauri::command]
pub fn set_clip_range(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, start: f64, end: f64) -> Result<RangeEdit, String> {
    clip_range::set_range(&engine, &project_id, &clip_id, start, end)
}

/// For showing the warning while the user is still typing a range.
#[tauri::command]
pub fn range_warning(start: f64, end: f64) -> Option<RangeWarning> {
    clip_range::warning(start, end)
}

/// The file and range to play. Only this one file is opened up to the webview's asset protocol.
#[tauri::command]
pub async fn clip_preview(app: tauri::AppHandle, engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String) -> Result<Preview, String> {
    let e = engine.inner().clone();
    let preview = tauri::async_runtime::spawn_blocking(move || clip_range::preview(&e, &project_id, &clip_id)).await.map_err(|e| e.to_string())??;
    app.asset_protocol_scope().allow_file(&preview.path).map_err(|e| e.to_string())?;
    Ok(preview)
}
