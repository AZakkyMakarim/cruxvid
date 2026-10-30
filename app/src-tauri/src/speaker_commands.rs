//! Tauri commands for scrubbing a followed Layout (cruxvid-b2t.14): the Layout and a frame at a given Clip time.

use crate::{engine::Engine, layout::Layout};
use std::sync::Arc;
use tauri::State;

/// The Clip's Layout `t` seconds into the Clip.
#[tauri::command]
pub async fn get_layout_at(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, t: f64) -> Result<Layout, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.clip_layout_at(&project_id, &clip_id, t)).await.map_err(|e| e.to_string())?
}

/// JPEG bytes (ArrayBuffer in JS) of the Source `t` seconds into the Clip.
#[tauri::command]
pub async fn get_clip_frame_at(engine: State<'_, Arc<Engine>>, project_id: String, clip_id: String, t: f64) -> Result<tauri::ipc::Response, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.clip_frame_at(&project_id, &clip_id, t).map(tauri::ipc::Response::new))
        .await
        .map_err(|e| e.to_string())?
}
