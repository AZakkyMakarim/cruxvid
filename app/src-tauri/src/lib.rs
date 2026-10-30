mod ai_commands;
pub mod arrange;
pub mod audio;
pub mod caption;
pub mod clip_text;
mod clip_text_commands;
pub mod clip_range;
mod clip_range_commands;
pub mod clips;
mod commands;
pub mod config;
pub mod engine;
pub mod export;
pub mod faces;
pub mod fsutil;
pub mod gemini;
pub mod history;
mod export_queue_commands;
pub mod layout;
pub mod layout_store;
pub mod output_folder;
pub mod project;
pub mod queue;
pub mod settings;
pub mod source;
pub mod speaker;
mod speaker_commands;
mod tools;
pub mod transcript;
pub mod translate;
mod translate_commands;
pub mod vtt;
pub mod ytdlp;

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
            let keys = settings::key_store_for(&cfg);
            let timeout = cfg.gemini_timeout_ms.map_or(gemini::DEFAULT_TIMEOUT, std::time::Duration::from_millis);
            let engine = Engine::with_key_store(
                EngineConfig {
                    data_dir: cfg.data_dir.map_or_else(|| paths.app_data_dir(), Ok)?,
                    output_dir: cfg.output_dir.map_or_else(|| paths.video_dir().map(|d| d.join("Cruxvid")), Ok)?,
                    gemini_base_url: cfg.gemini_base_url,
                    model: gemini::DEFAULT_MODEL.to_string(),
                },
                keys,
            )
            .with_timeout(timeout);
            let engine = match cfg.ytdlp_path {
                Some(p) => engine.with_ytdlp(p),
                None => engine,
            };
            if cfg.test_mode {
                let ids = std::env::var("CRUXVID_FAIL_ONCE_CLIPS").unwrap_or_default();
                engine.fail_once(ids.split(',').filter(|s| !s.is_empty()).map(String::from).collect());
            }
            app.manage(Arc::new(engine));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::analyze_source,
            commands::export_clip,
            ai_commands::ai_status,
            ai_commands::save_api_key,
            ai_commands::clear_api_key,
            ai_commands::set_model,
            ai_commands::estimate_tokens,
            ai_commands::get_ui_prefs,
            ai_commands::set_ui_prefs,
            commands::caption_presets,
            commands::get_caption,
            commands::set_caption_style,
            commands::correct_caption_word,
            commands::layout_presets,
            commands::get_layout,
            commands::set_layout_preset,
            commands::shift_layout_pane,
            commands::get_clip_frame,
            export_queue_commands::export_clips,
            export_queue_commands::retry_failed_exports,
            export_queue_commands::export_jobs,
            export_queue_commands::get_output_dir,
            export_queue_commands::set_output_dir,
            export_queue_commands::open_output_dir,
            arrange::arrange_clips,
            clip_text_commands::edit_clip_text,
            clip_text_commands::regenerate_clip_text,
            speaker_commands::get_layout_at,
            speaker_commands::get_clip_frame_at,
            history::list_projects,
            history::open_project,
            translate_commands::set_clip_language,
            translate_commands::get_clip_language,
            clip_range_commands::create_manual_project,
            clip_range_commands::add_manual_clip,
            clip_range_commands::set_clip_range,
            clip_range_commands::range_warning,
            clip_range_commands::clip_preview
        ])
        .run(tauri::generate_context!())
        .expect("error while running cruxvid");
}
