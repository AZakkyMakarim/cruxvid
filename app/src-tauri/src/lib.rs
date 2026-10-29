pub mod config;

use config::AppConfig;

#[tauri::command]
fn get_config(cfg: tauri::State<AppConfig>) -> AppConfig {
    cfg.inner().clone()
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppConfig::from_env(|k| std::env::var(k).ok()))
        .invoke_handler(tauri::generate_handler![get_config])
        .run(tauri::generate_context!())
        .expect("error while running cruxvid");
}
