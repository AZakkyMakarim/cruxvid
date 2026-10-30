//! Project history: list the saved Projects and reopen one with everything it had (Clips with their
//! edits, Clip order/threshold, queue). Captions and Layouts are saved per Project and load on demand,
//! so reopening never calls Gemini.

use crate::{arrange::{ClipOrder, DEFAULT_THRESHOLD}, engine::Engine, project::{Project, ProjectStore}, queue::{Job, Status}};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tauri::State;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    /// The Source's title; the id when the Project file is damaged.
    pub title: String,
    /// Creation time (ms since epoch), taken from the Project id; 0 when the id has none.
    pub created_ms: u64,
    pub damaged: bool,
}

/// The Clip list view the user left: order and Hook score threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewState {
    pub order: ClipOrder,
    pub threshold: u8,
}

impl Default for ViewState {
    fn default() -> Self {
        Self { order: ClipOrder::Timeline, threshold: DEFAULT_THRESHOLD }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenedProject {
    pub project: Project,
    pub view: ViewState,
    pub jobs: Vec<Job>,
}

fn view_file(engine: &Engine, id: &str) -> PathBuf {
    engine.data_dir().join("projects").join(id).join("view.json")
}

/// Saved Projects, newest first. A folder without a Project file is not a Project; an unreadable file is listed as damaged.
/// shortcut: parses every project.json (Transcript included) per call, add an index file if the list gets slow.
pub fn list(engine: &Engine) -> Vec<ProjectSummary> {
    let store = ProjectStore::new(engine.data_dir().to_path_buf());
    let Ok(dir) = std::fs::read_dir(engine.data_dir().join("projects")) else { return Vec::new() };
    let mut out: Vec<ProjectSummary> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("project.json").is_file())
        .map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let created_ms = id.strip_prefix('p').and_then(|ms| ms.parse().ok()).unwrap_or(0);
            match store.load(&id) {
                Ok(p) => ProjectSummary { id, title: p.source.title, created_ms, damaged: false },
                Err(_) => ProjectSummary { title: id.clone(), id, created_ms, damaged: true },
            }
        })
        .collect();
    out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then(a.id.cmp(&b.id)));
    out
}

pub fn save_view(engine: &Engine, id: &str, order: ClipOrder, threshold: u8) -> Result<(), String> {
    engine.load_project(id)?; // validates the id
    let json = serde_json::to_vec(&ViewState { order, threshold }).map_err(|e| e.to_string())?;
    std::fs::write(view_file(engine, id), json).map_err(|e| format!("Cannot save view: {e}"))
}

/// The Project and its saved view and queue. A missing or unreadable view file means the defaults.
pub fn open(engine: &Engine, id: &str) -> Result<OpenedProject, String> {
    let project = engine.load_project(id)?;
    let view = std::fs::read(view_file(engine, id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    Ok(OpenedProject { project, view, jobs: engine.export_jobs(id)? })
}

#[tauri::command]
pub fn list_projects(engine: State<'_, Arc<Engine>>) -> Vec<ProjectSummary> {
    list(&engine)
}

/// Opens a Project; Exports the closed app left waiting or running continue on their own.
#[tauri::command]
pub fn open_project(engine: State<'_, Arc<Engine>>, id: String) -> Result<OpenedProject, String> {
    let opened = open(&engine, &id)?;
    if opened.jobs.iter().any(|j| j.status == Status::Waiting) {
        let e = engine.inner().clone();
        std::thread::spawn(move || {
            let _ = e.run_export_queue(&id);
        });
    }
    Ok(opened)
}
