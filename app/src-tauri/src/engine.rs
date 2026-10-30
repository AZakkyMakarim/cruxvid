//! Engine: the public seam (Analyze, Export). Orchestrates the concern modules; holds no UI knowledge.

use crate::{caption::{self, CaptionStore, CaptionStyle, ClipCaption}, clips, export, gemini::{self, GeminiClient}, project::{Project, ProjectStore}, settings::{self, KeyStore}, source};
use serde::Serialize;
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub output_dir: PathBuf,
    pub gemini_base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub has_key: bool,
    pub model: String,
}

/// Rough Gemini token use for analyzing one Source (see `estimate_tokens`).
#[derive(Debug, Clone, Serialize)]
pub struct TokenEstimate {
    pub duration: f64,
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

pub struct Engine {
    gemini_base_url: String,
    model: Mutex<String>,
    /// `None` = no key management (key is neither required nor sent); production always passes a store.
    keys: Option<Arc<dyn KeyStore>>,
    timeout: Duration,
    data_dir: PathBuf,
    store: ProjectStore,
    captions: CaptionStore,
    output_dir: PathBuf,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self::build(cfg, None)
    }

    pub fn with_key_store(cfg: EngineConfig, keys: Arc<dyn KeyStore>) -> Self {
        Self::build(cfg, Some(keys))
    }

    fn build(cfg: EngineConfig, keys: Option<Arc<dyn KeyStore>>) -> Self {
        Self {
            gemini_base_url: cfg.gemini_base_url,
            model: Mutex::new(settings::load_model(&cfg.data_dir).unwrap_or(cfg.model)),
            keys,
            timeout: gemini::DEFAULT_TIMEOUT,
            store: ProjectStore::new(cfg.data_dir.clone()),
            captions: CaptionStore::new(cfg.data_dir.clone()),
            data_dir: cfg.data_dir,
            output_dir: cfg.output_dir,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn ai_status(&self) -> AiStatus {
        let has_key = self.keys.as_ref().is_some_and(|k| matches!(k.get(), Ok(Some(_))));
        AiStatus { has_key, model: self.model.lock().unwrap().clone() }
    }

    pub fn set_api_key(&self, key: &str) -> Result<(), String> {
        let key = key.trim();
        if key.is_empty() {
            return Err("API key tidak boleh kosong.".into());
        }
        self.keys.as_ref().ok_or("Penyimpanan key tidak tersedia.")?.set(key)
    }

    pub fn clear_api_key(&self) -> Result<(), String> {
        self.keys.as_ref().map_or(Ok(()), |k| k.clear())
    }

    pub fn set_model(&self, model: &str) -> Result<(), String> {
        let model = model.trim();
        if model.is_empty() {
            return Err("Nama model tidak boleh kosong.".into());
        }
        settings::save_model(&self.data_dir, model)?;
        *self.model.lock().unwrap() = model.to_string();
        Ok(())
    }

    /// Before analysis: rough tokens for the Source's duration. Audio is 32 tokens/s on Gemini; the
    /// Transcript comes back as word-level JSON (~40 tokens/s of speech) and is re-sent as text for the
    /// Topic pass (~5 tokens/s). shortcut: fixed rates, real use depends on speech density.
    pub fn estimate_tokens(&self, path: &Path) -> Result<TokenEstimate, String> {
        let duration = source::probe_local(path)?.duration;
        let input = (duration * (32.0 + 5.0)).ceil() as u64;
        let output = (duration * 40.0).ceil() as u64;
        Ok(TokenEstimate { duration, input, output, total: input + output })
    }

    fn client(&self) -> Result<GeminiClient, String> {
        let key = match &self.keys {
            None => None,
            Some(k) => Some(k.get()?.ok_or(gemini::MSG_NO_KEY)?),
        };
        let model = self.model.lock().unwrap().clone();
        Ok(GeminiClient::new(&self.gemini_base_url, &model).with_key(key).with_timeout(self.timeout))
    }

    /// Source file -> Transcript -> Clips, saved as a Project. Errors are user-readable.
    pub fn analyze(&self, path: &Path) -> Result<Project, String> {
        let source = source::probe_local(path)?;
        let gemini = self.client()?;
        let transcript = gemini.transcribe(&source)?;
        let analysis = gemini.analyze(&transcript)?;
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
