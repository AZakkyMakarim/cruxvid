//! Per-Project Layout persistence: `<data_dir>/projects/<project id>/layouts.json`, Clip id -> Layout.
//! Kept beside (not inside) project.json so Layout work never touches the Project/Clip model.
//! The caller validates the project id (by loading the Project first).

use crate::layout::Layout;
use std::{collections::HashMap, path::PathBuf};

pub struct LayoutStore {
    root: PathBuf,
}

impl LayoutStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { root: data_dir.join("projects") }
    }

    fn file(&self, project_id: &str) -> PathBuf {
        self.root.join(project_id).join("layouts.json")
    }

    pub fn get(&self, project_id: &str, clip_id: &str) -> Result<Option<Layout>, String> {
        let Ok(bytes) = std::fs::read(self.file(project_id)) else { return Ok(None) };
        let mut all: HashMap<String, Layout> = serde_json::from_slice(&bytes).map_err(|e| format!("Layout file is damaged: {e}"))?;
        Ok(all.remove(clip_id))
    }

    pub fn put(&self, project_id: &str, clip_id: &str, layout: &Layout) -> Result<(), String> {
        let file = self.file(project_id);
        let mut all: HashMap<String, Layout> =
            std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        all.insert(clip_id.to_string(), layout.clone());
        let json = serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?;
        std::fs::write(&file, json).map_err(|e| format!("Cannot save layout: {e}"))
    }
}

/// A 640-wide JPEG of the Source at `at` seconds: what the Layout preview draws its panes from.
pub fn preview_jpeg(source: &crate::source::Source, at: f64) -> Result<Vec<u8>, String> {
    let out = crate::tools::command("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{at:.3}"), "-i", &source.path, "-frames:v", "1", "-vf", "scale=640:-2"])
        .args(["-f", "image2pipe", "-c:v", "mjpeg", "-q:v", "4", "pipe:1"])
        .output()
        .map_err(|e| format!("Cannot run ffmpeg (is it installed?): {e}"))?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(format!("Cannot read a preview frame: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_round_trip_per_clip() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("projects/p1")).unwrap();
        let s = LayoutStore::new(tmp.path().to_path_buf());
        assert_eq!(s.get("p1", "clip-1").unwrap(), None);
        let l = Layout::auto(&[], 1920, 1080);
        s.put("p1", "clip-1", &l).unwrap();
        s.put("p1", "clip-2", &l.with_preset("focus").unwrap()).unwrap();
        assert_eq!(s.get("p1", "clip-1").unwrap(), Some(l));
        assert_eq!(s.get("p1", "clip-2").unwrap().unwrap().preset, "focus");
    }
}
