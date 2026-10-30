//! Tauri commands: thin async wrappers over the Engine (blocking work runs off the UI thread).

use crate::{caption::{CaptionStyle, ClipCaption}, config::AppConfig, engine::Engine, project::Project};
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

#[tauri::command]
pub fn caption_presets() -> Vec<CaptionStyle> {
    Engine::caption_presets()
}

#[tauri::command]
pub fn get_caption(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String) -> Result<ClipCaption, String> {
    engine.caption(&project_id, &clip_id)
}

#[tauri::command]
pub fn set_caption_style(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, style: CaptionStyle) -> Result<ClipCaption, String> {
    engine.set_caption_style(&project_id, &clip_id, style)
}

#[tauri::command]
pub fn correct_caption_word(engine: State<'_, Arc<Engine>>, project_id: String, index: usize, text: String) -> Result<String, String> {
    engine.correct_caption_word(&project_id, index, &text)
}