//! Tauri commands for Clip title/description (cruxvid-b2t.10). Regenerate calls Gemini, so it runs off the UI thread.

use crate::{clip_text::TextField, clips::Clip, engine::Engine};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn edit_clip_text(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, title: String, description: String) -> Result<Clip, String> {
    engine.edit_clip_text(&project_id, &clip_id, &title, &description)
}

#[tauri::command]
pub async fn regenerate_clip_text(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, field: TextField) -> Result<Clip, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.regenerate_clip_text(&project_id, &clip_id, field))
        .await
        .map_err(|e| e.to_string())?
}
