//! Engine: the public seam (Analyze, Export). Orchestrates the concern modules; holds no UI knowledge.

use crate::{caption::{self, CaptionStore, CaptionStyle, ClipCaption}, clips, export, faces, gemini::{self, GeminiClient}, layout::Layout, layout_store::{self, LayoutStore}, project::{Project, ProjectStore}, settings::{self, KeyStore}, source, speaker, ytdlp};
use crate::{output_folder::OutputFolder, queue::{Job, JobQueue}};
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

fn default_ytdlp(data_dir: &Path) -> PathBuf {
    let own = data_dir.join("bin").join("yt-dlp.exe");
    if own.is_file() { own } else { PathBuf::from("yt-dlp") }
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
    layouts: LayoutStore,
    output: OutputFolder,
    queue: JobQueue,
    ytdlp: ytdlp::YtDlp,
    /// Two threads asking for the same Clip's cut must not download it twice at once.
    cut_lock: Mutex<()>,
    /// Test mode: Clips whose next queued Export fails once (see `fail_once`).
    fail_once: Mutex<Vec<String>>,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self::build(cfg, None)
    }

    pub fn with_key_store(cfg: EngineConfig, keys: Arc<dyn KeyStore>) -> Self {
        Self::build(cfg, Some(keys))
    }

    fn build(cfg: EngineConfig, keys: Option<Arc<dyn KeyStore>>) -> Self {
        let output = OutputFolder::new(&cfg.data_dir, cfg.output_dir);
        Self {
            gemini_base_url: cfg.gemini_base_url,
            model: Mutex::new(settings::load_model(&cfg.data_dir).unwrap_or(cfg.model)),
            keys,
            timeout: gemini::DEFAULT_TIMEOUT,
            store: ProjectStore::new(cfg.data_dir.clone()),
            captions: CaptionStore::new(cfg.data_dir.clone()),
            layouts: LayoutStore::new(cfg.data_dir.clone()),
            queue: JobQueue::new(cfg.data_dir.clone()),
            ytdlp: ytdlp::YtDlp::new(default_ytdlp(&cfg.data_dir), cfg.data_dir.join("ytdlp-update.txt")),
            cut_lock: Mutex::new(()),
            data_dir: cfg.data_dir,
            output,
            fail_once: Mutex::new(Vec::new()),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Which yt-dlp executable to run for link Sources (default: `<data dir>/bin/yt-dlp.exe`, else `yt-dlp` on PATH).
    pub fn with_ytdlp(mut self, bin: PathBuf) -> Self {
        self.ytdlp = self.ytdlp.with_bin(bin);
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
        let link = path.to_string_lossy();
        let duration = if ytdlp::is_link(&link) { self.ytdlp.info(link.trim())?.duration } else { source::probe_local(path)?.duration };
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
        let (source, transcript, gemini) = self.source_and_transcript(path)?;
        let analysis = gemini.analyze(&transcript)?;
        let project = Project {
            id: ProjectStore::new_id(),
            clips: clips::build_clips(&analysis.sections, &transcript),
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
        let (media, view) = self.clip_media(project_id, &p, clip)?;
        let vf = self.clip_layout(project_id, clip_id)?.filter(export::export_fps(media.fps));
        let ass = caption::ass(clip, &self.captions.load(project_id).view(&p.transcript, clip));
        export::export_clip_with(&media, &view, &self.output.get(), Some(&vf), Some(&ass))
    }

    /// Manual title/description edit, saved in the Project.
    pub fn edit_clip_text(&self, project_id: &str, clip_id: &str, title: &str, description: &str) -> Result<clips::Clip, String> {
        crate::clip_text::edit(&self.store, project_id, clip_id, title, description)
    }

    /// New AI text for one field of one Clip; on error the old text stays.
    pub fn regenerate_clip_text(&self, project_id: &str, clip_id: &str, field: crate::clip_text::TextField) -> Result<clips::Clip, String> {
        crate::clip_text::regenerate(&self.store, &self.client()?, project_id, clip_id, field)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
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

    /// The Clip's Layout. First call detects faces in the Clip and picks the initial preset; the result is saved.
    pub fn clip_layout(&self, project_id: &str, clip_id: &str) -> Result<Layout, String> {
        let p = self.store.load(project_id)?;
        let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        if let Some(l) = self.layouts.get(project_id, clip_id)? {
            return Ok(l);
        }
        let (media, view) = self.clip_media(project_id, &p, clip)?;
        let found = faces::detect_clip_faces(&media, view.start, view.end)?;
        if found.len() < 2 {
            let layout = Layout::auto(&found, media.width, media.height);
            self.layouts.put(project_id, clip_id, &layout)?;
            return Ok(layout);
        }
        // 2+ faces: follow whoever speaks. No key, no network or no placeable turn = Full + notice, which is
        // not saved (it is not a user choice), so the next open asks again.
        let keys = self.client().and_then(|g| speaker::detect_turns(&g, &media, &view)).map(|t| speaker::follow_keys(&found, &t)).unwrap_or_default();
        let layout = Layout::auto_following(&found, keys, media.width, media.height);
        if !layout.follow.is_empty() {
            self.layouts.put(project_id, clip_id, &layout)?;
        }
        Ok(layout)
    }

    /// The Clip's Layout as it looks `t` seconds into the Clip (the followed crop moves with the speaker).
    pub fn clip_layout_at(&self, project_id: &str, clip_id: &str, t: f64) -> Result<Layout, String> {
        Ok(self.clip_layout(project_id, clip_id)?.at(t))
    }

    /// JPEG of the Source `t` seconds into the Clip, for scrubbing the Layout preview.
    pub fn clip_frame_at(&self, project_id: &str, clip_id: &str, t: f64) -> Result<Vec<u8>, String> {
        let p = self.store.load(project_id)?;
        let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        let (media, view) = self.clip_media(project_id, &p, clip)?;
        layout_store::preview_jpeg(&media, view.start + t.clamp(0.0, view.end - view.start))
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
        let (media, view) = self.clip_media(project_id, &p, clip)?;
        layout_store::preview_jpeg(&media, (view.start + view.end) / 2.0)
    }

    /// The Source (a local file) and Clip times to render from. A link Source is stream-cut to just the
    /// Clip's range first, so the returned Clip's times are relative to that cut file (docs/spikes/b2t-1-stream-cut.md).
    fn clip_media(&self, project_id: &str, p: &Project, clip: &clips::Clip) -> Result<(source::Source, clips::Clip), String> {
        if !ytdlp::is_link(&p.source.path) {
            return Ok((p.source.clone(), clip.clone()));
        }
        let _one_cut_at_a_time = self.cut_lock.lock().unwrap();
        let name = format!("{}_{}_{}", clip.id, (clip.start * 1000.0).round(), (clip.end * 1000.0).round());
        let dir = self.data_dir.join("cuts").join(project_id);
        let cut = self.ytdlp.cut(&p.source.path, clip.start, clip.end, p.source.duration, &dir, &name)?;
        let mut media = source::probe_local(&cut.path)?;
        media.title = p.source.title.clone();
        let view = clips::Clip { start: cut.offset, end: cut.offset + (clip.end - clip.start), ..clip.clone() };
        Ok((media, view))
    }

    /// Link Source: metadata, then platform captions, else the audio through Gemini.
    fn source_and_transcript(&self, input: &Path) -> Result<(source::Source, crate::transcript::Transcript, GeminiClient), String> {
        let link = input.to_string_lossy();
        if !ytdlp::is_link(&link) {
            let source = source::probe_local(input)?;
            let gemini = self.client()?;
            let transcript = gemini.transcribe(&source)?;
            return Ok((source, transcript, gemini));
        }
        let gemini = self.client()?;
        let work = self.data_dir.join("work").join(ProjectStore::new_id());
        let resolved = self.ytdlp.resolve(link.trim(), &work, |audio| gemini.transcribe(audio));
        let _ = std::fs::remove_dir_all(&work);
        let (source, transcript) = resolved?;
        Ok((source, transcript, gemini))
    }
}
