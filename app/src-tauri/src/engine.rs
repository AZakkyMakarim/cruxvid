//! Engine: the public seam (Analyze, Export). Orchestrates the concern modules; holds no UI knowledge.

use crate::{caption::{self, CaptionStore, CaptionStyle, ClipCaption}, clips, export, gemini::GeminiClient, project::{Project, ProjectStore}, source};
use std::path::{Path, PathBuf};

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
    output_dir: PathBuf,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            gemini: GeminiClient::new(&cfg.gemini_base_url, &cfg.model),
            captions: CaptionStore::new(cfg.data_dir.clone()),
            store: ProjectStore::new(cfg.data_dir),
            output_dir: cfg.output_dir,
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
        export::export_clip_with(&p.source, clip, &self.output_dir, Some(&ass))
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
