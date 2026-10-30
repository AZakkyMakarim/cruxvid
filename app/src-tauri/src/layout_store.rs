//! Per-Project Layout persistence: `<data_dir>/projects/<project id>/layouts.json`, Clip id -> Layout.
//! Kept beside (not inside) project.json so Layout work never touches the Project/Clip model.
//! The caller validates the project id (by loading the Project first).

use crate::{fsutil, layout::Layout};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

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
        let mut all: HashMap<String, Layout> = fsutil::read_json(&self.file(project_id), "layout")?;
        Ok(all.remove(clip_id))
    }

    /// Read-modify-write of layouts.json under one lock (shortcut: one lock for all Projects). A damaged file is an error, never an empty map.
    fn update(&self, project_id: &str, f: impl FnOnce(&mut HashMap<String, Layout>)) -> Result<(), String> {
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = fsutil::lock(&LOCK);
        let file = self.file(project_id);
        let mut all: HashMap<String, Layout> = fsutil::read_json(&file, "layout")?;
        f(&mut all);
        fsutil::write_json(&file, &all, "layout")
    }

    pub fn put(&self, project_id: &str, clip_id: &str, layout: &Layout) -> Result<(), String> {
        self.update(project_id, |all| {
            all.insert(clip_id.to_string(), layout.clone());
        })
    }

    pub fn remove(&self, project_id: &str, clip_id: &str) -> Result<(), String> {
        self.update(project_id, |all| {
            all.remove(clip_id);
        })
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

    #[test]
    fn concurrent_puts_all_survive_and_a_damaged_file_is_never_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("projects/p1")).unwrap();
        let s = std::sync::Arc::new(LayoutStore::new(tmp.path().to_path_buf()));
        let l = Layout::auto(&[], 1920, 1080);
        let threads: Vec<_> = (0..8)
            .map(|t| {
                let (s, l) = (s.clone(), l.clone());
                std::thread::spawn(move || (0..10).for_each(|i| s.put("p1", &format!("c{t}-{i}"), &l).unwrap()))
            })
            .collect();
        threads.into_iter().for_each(|t| t.join().unwrap());
        let all: HashMap<String, Layout> = serde_json::from_slice(&std::fs::read(s.file("p1")).unwrap()).unwrap();
        assert_eq!(all.len(), 80);

        std::fs::write(s.file("p1"), "{ not json").unwrap();
        assert!(s.put("p1", "x", &l).unwrap_err().contains("damaged"));
        assert_eq!(std::fs::read_to_string(s.file("p1")).unwrap(), "{ not json");
    }
}
