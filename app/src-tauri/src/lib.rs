pub mod clips;
mod commands;
pub mod config;
pub mod engine;
pub mod export;
pub mod faces;
pub mod gemini;
pub mod layout;
pub mod layout_store;
pub mod project;
pub mod source;
mod tools;
pub mod transcript;

use config::AppConfig;
use engine::{Engine, EngineConfig};
use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppConfig::from_env(|k| std::env::var(k).ok()))
        .setup(|app| {
            let cfg = app.state::<AppConfig>().inner().clone();
            let paths = app.path();
            let engine = Engine::new(EngineConfig {
                data_dir: cfg.data_dir.map_or_else(|| paths.app_data_dir(), Ok)?,
                output_dir: cfg.output_dir.map_or_else(|| paths.video_dir().map(|d| d.join("Cruxvid")), Ok)?,
                gemini_base_url: cfg.gemini_base_url,
                model: gemini::DEFAULT_MODEL.to_string(),
            });
            app.manage(Arc::new(engine));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::analyze_source,
            commands::export_clip,
            commands::layout_presets,
            commands::get_layout,
            commands::set_layout_preset,
            commands::shift_layout_pane,
            commands::get_clip_frame
        ])
        .run(tauri::generate_context!())
        .expect("error while running cruxvid");
}
