//! Tauri commands for per-Clip translation (cruxvid-b2t.12). Translating calls Gemini, so it runs off the UI thread.

use crate::{clips::Clip, engine::Engine};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn set_clip_language(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, language: String) -> Result<Clip, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.set_clip_language(&project_id, &clip_id, &language))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_clip_language(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String) -> Result<String, String> {
    engine.clip_language(&project_id, &clip_id)
}
