//! Engine: the public seam (Analyze, Export). Orchestrates the concern modules; holds no UI knowledge.

use crate::{clips, export, faces, gemini::GeminiClient, layout::Layout, layout_store::{self, LayoutStore}, project::{Project, ProjectStore}, source};
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
    layouts: LayoutStore,
    output_dir: PathBuf,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            gemini: GeminiClient::new(&cfg.gemini_base_url, &cfg.model),
            layouts: LayoutStore::new(cfg.data_dir.clone()),
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
        let layout = self.clip_layout(project_id, clip_id)?;
        let vf = layout.filter(export::export_fps(p.source.fps));
        export::export_clip_with(&p.source, clip, &self.output_dir, Some(&vf))
    }

    /// The Clip's Layout. First call detects faces in the Clip and picks the initial preset; the result is saved.
    pub fn clip_layout(&self, project_id: &str, clip_id: &str) -> Result<Layout, String> {
        let p = self.store.load(project_id)?;
        let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        if let Some(l) = self.layouts.get(project_id, clip_id)? {
            return Ok(l);
        }
        let found = faces::detect_clip_faces(&p.source, clip.start, clip.end)?;
        let layout = Layout::auto(&found, p.source.width, p.source.height);
        self.layouts.put(project_id, clip_id, &layout)?;
        Ok(layout)
    }

    /// Another preset for the Clip; no new detection, no render.
    pub fn set_layout_preset(&self, project_id: &str, clip_id: &str, preset: &str) -> Result<Layout, String> {
        let layout = self.clip_layout(project_id, clip_id)?.with_preset(preset)?;
        self.layouts.put(project_id, clip_id, &layout)?;
        Ok(layout)
    }

    /// Manual crop adjustment: moves one pane's crop window (Source-relative units).
    pub fn shift_layout_pane(&self, project_id: &str, clip_id: &str, pane: usize, dx: f64, dy: f64) -> Result<Layout, String> {
        let mut layout = self.clip_layout(project_id, clip_id)?;
        layout.shift_pane(pane, dx, dy)?;
        self.layouts.put(project_id, clip_id, &layout)?;
        Ok(layout)
    }

    /// JPEG from the middle of the Clip for the Layout preview.
    pub fn clip_frame(&self, project_id: &str, clip_id: &str) -> Result<Vec<u8>, String> {
        let p = self.store.load(project_id)?;
        let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        layout_store::preview_jpeg(&p.source, (clip.start + clip.end) / 2.0)
    }
}
