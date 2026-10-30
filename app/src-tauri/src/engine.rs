//! Engine: the public seam (Analyze, Export). Orchestrates the concern modules; holds no UI knowledge.

use crate::{clips, export, gemini::GeminiClient, project::{Project, ProjectStore}, source};
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
    output_dir: PathBuf,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            gemini: GeminiClient::new(&cfg.gemini_base_url, &cfg.model),
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
        export::export_clip(&p.source, clip, &self.output_dir)
    }
}
