//! Tauri commands for the Export queue and output folder (kept apart from commands.rs).

use crate::{config::AppConfig, engine::Engine, queue::Job};
use std::sync::Arc;
use tauri::State;

/// Runs the queue on its own thread; the UI polls `export_jobs`.
/// The worker re-checks for waiting jobs before it lets go (queue.rs), so a job enqueued meanwhile is not left behind.
fn start(engine: &Arc<Engine>, project_id: String) {
    let e = engine.clone();
    std::thread::spawn(move || {
        let _ = e.run_export_queue(&project_id);
    });
}

#[tauri::command]
pub fn export_clips(engine: State<'_, Arc<Engine>>, project_id: String, clip_ids: Vec<String>) -> Result<Vec<Job>, String> {
    let jobs = engine.enqueue_exports(&project_id, &clip_ids)?;
    start(engine.inner(), project_id);
    Ok(jobs)
}

#[tauri::command]
pub fn retry_failed_exports(engine: State<'_, Arc<Engine>>, project_id: String) -> Result<Vec<Job>, String> {
    let jobs = engine.retry_failed_exports(&project_id)?;
    start(engine.inner(), project_id);
    Ok(jobs)
}

#[tauri::command]
pub fn export_jobs(engine: State<'_, Arc<Engine>>, project_id: String) -> Result<Vec<Job>, String> {
    engine.export_jobs(&project_id)
}

#[tauri::command]
pub fn get_output_dir(engine: State<'_, Arc<Engine>>) -> String {
    engine.output_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn set_output_dir(engine: State<'_, Arc<Engine>>, dir: String) -> Result<String, String> {
    engine.set_output_dir(&dir).map(|p| p.to_string_lossy().into_owned())
}

/// Test mode (with a data dir) records the folder instead of launching Explorer.
#[tauri::command]
pub fn open_output_dir(engine: State<'_, Arc<Engine>>, cfg: State<'_, AppConfig>) -> Result<String, String> {
    let record = cfg.test_mode.then(|| cfg.data_dir.as_ref().map(|d| d.join("opened-folder.txt"))).flatten();
    engine.open_output_dir(record.as_deref())?;
    Ok(engine.output_dir().to_string_lossy().into_owned())
}
