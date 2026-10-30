//! Engine: the public seam (Analyze, Export). Orchestrates the concern modules; holds no UI knowledge.

use crate::{caption::{self, CaptionStore, CaptionStyle, ClipCaption}, clips, export, gemini::GeminiClient, project::{Project, ProjectStore}, source};
use crate::{output_folder::OutputFolder, queue::{Job, JobQueue}};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub output_dir: PathBuf,
    pub gemini_base_url: String,
    pub model: String,
}

pub struct Engine {
    gemini: GeminiClient,
    store: ProjectStore,
    captions: CaptionStore,
    output: OutputFolder,
    queue: JobQueue,
    /// Test mode: Clips whose next queued Export fails once (see `fail_once`).
    fail_once: Mutex<Vec<String>>,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        let output = OutputFolder::new(&cfg.data_dir, cfg.output_dir);
        Self {
            gemini: GeminiClient::new(&cfg.gemini_base_url, &cfg.model),
            captions: CaptionStore::new(cfg.data_dir.clone()),
            queue: JobQueue::new(cfg.data_dir.clone()),
            store: ProjectStore::new(cfg.data_dir),
            output,
            fail_once: Mutex::new(Vec::new()),
        }
    }

    /// Source file -> Transcript -> Clips, saved as a Project. Errors are user-readable.
    pub fn analyze(&self, path: &Path) -> Result<Project, String> {
        let source = source::probe_local(path)?;
        let transcript = self.gemini.transcribe(&source)?;
        let analysis = self.gemini.analyze(&transcript)?;
        let project = Project {
            id: ProjectStore::new_id(),
            clips: clips::build_clips(&analysis.sections),
            source,
            transcript,
        };
        self.store.save(&project)?;
        Ok(project)
    }

    pub fn load_project(&self, id: &str) -> Result<Project, String> {
        self.store.load(id)
    }

    /// Renders one Clip of a saved Project into the output folder; returns the MP4 path.
    pub fn export(&self, project_id: &str, clip_id: &str) -> Result<PathBuf, String> {
        let p = self.store.load(project_id)?;
        let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        let ass = caption::ass(clip, &self.captions.load(project_id).view(&p.transcript, clip));
        export::export_clip_with(&p.source, clip, &self.output.get(), Some(&ass))
    }

    pub fn output_dir(&self) -> PathBuf {
        self.output.get()
    }

    pub fn set_output_dir(&self, dir: &str) -> Result<PathBuf, String> {
        self.output.set(dir)
    }

    /// Opens the output folder in Explorer, or records it in `record` (test mode).
    pub fn open_output_dir(&self, record: Option<&Path>) -> Result<(), String> {
        self.output.open(record)
    }

    /// Test mode only: these Clips fail their first queued Export, so retry can be exercised.
    pub fn fail_once(&self, clip_ids: Vec<String>) {
        *self.fail_once.lock().unwrap() = clip_ids;
    }

    pub fn enqueue_exports(&self, project_id: &str, clip_ids: &[String]) -> Result<Vec<Job>, String> {
        let p = self.store.load(project_id)?;
        if let Some(bad) = clip_ids.iter().find(|id| !p.clips.iter().any(|c| &c.id == *id)) {
            return Err(format!("Clip not found: {bad}"));
        }
        self.queue.enqueue(project_id, clip_ids)
    }

    pub fn retry_failed_exports(&self, project_id: &str) -> Result<Vec<Job>, String> {
        self.store.load(project_id)?;
        self.queue.retry_failed(project_id)
    }

    pub fn export_jobs(&self, project_id: &str) -> Result<Vec<Job>, String> {
        self.store.load(project_id)?;
        Ok(self.queue.jobs(project_id))
    }

    /// Runs the waiting jobs one by one and returns when none are left. A failed job does not stop the rest.
    pub fn run_export_queue(&self, project_id: &str) -> Result<(), String> {
        self.store.load(project_id)?;
        self.queue.run(project_id, |clip_id| {
            let mut scheduled = self.fail_once.lock().unwrap();
            if let Some(i) = scheduled.iter().position(|c| c == clip_id) {
                scheduled.remove(i);
                return Err("Scheduled test failure".to_string());
            }
            drop(scheduled);
            self.export(project_id, clip_id).map(|p| p.to_string_lossy().into_owned())
        })?;
        Ok(())
    }

    pub fn caption_presets() -> Vec<CaptionStyle> {
        caption::presets()
    }

    /// The Clip's Caption style and its words (with corrections applied).
    pub fn caption(&self, project_id: &str, clip_id: &str) -> Result<ClipCaption, String> {
        let p = self.store.load(project_id)?;
        let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        Ok(self.captions.load(project_id).view(&p.transcript, clip))
    }

    pub fn set_caption_style(&self, project_id: &str, clip_id: &str, style: CaptionStyle) -> Result<ClipCaption, String> {
        self.store.load(project_id)?; // validates the id
        let mut state = self.captions.load(project_id);
        state.set_style(clip_id, style)?;
        self.captions.save(project_id, &state)?;
        self.caption(project_id, clip_id)
    }

    /// Corrects one Transcript word; returns the text now shown (the original when `text` is empty).
    pub fn correct_caption_word(&self, project_id: &str, index: usize, text: &str) -> Result<String, String> {
        let p = self.store.load(project_id)?;
        let mut state = self.captions.load(project_id);
        let shown = state.correct(&p.transcript, index, text)?;
        self.captions.save(project_id, &state)?;
        Ok(shown)
    }
}
