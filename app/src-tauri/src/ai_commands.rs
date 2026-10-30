//! Tauri commands for the AI settings (API key, model) and the pre-analysis token estimate.

use crate::engine::{AiStatus, Engine, TokenEstimate};
use std::{path::PathBuf, sync::Arc};
use tauri::State;

#[tauri::command]
pub fn ai_status(engine: State<'_, Arc<Engine>>) -> AiStatus {
    engine.ai_status()
}

#[tauri::command]
pub fn save_api_key(engine: State<'_, Arc<Engine>>, key: String) -> Result<(), String> {
    engine.set_api_key(&key)
}

#[tauri::command]
pub fn clear_api_key(engine: State<'_, Arc<Engine>>) -> Result<(), String> {
    engine.clear_api_key()
}

#[tauri::command]
pub fn set_model(engine: State<'_, Arc<Engine>>, model: String) -> Result<(), String> {
    engine.set_model(&model)
}

#[tauri::command]
pub async fn estimate_tokens(engine: State<'_, Arc<Engine>>, path: String) -> Result<TokenEstimate, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.estimate_tokens(&PathBuf::from(path)))
        .await
        .map_err(|e| e.to_string())?
}
