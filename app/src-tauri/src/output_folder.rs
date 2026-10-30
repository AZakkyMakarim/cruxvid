//! Output folder for Exports: chosen in Settings, kept across restarts, and opened from the UI.

use std::{path::{Path, PathBuf}, sync::Mutex};

pub struct OutputFolder {
    file: PathBuf,
    dir: Mutex<PathBuf>,
}

impl OutputFolder {
    /// `default` is used until the user picks a folder.
    pub fn new(data_dir: &Path, default: PathBuf) -> Self {
        let file = data_dir.join("output_dir.txt");
        let saved = std::fs::read_to_string(&file).ok().map(|s| PathBuf::from(s.trim())).filter(|p| p.is_absolute());
        Self { file, dir: Mutex::new(saved.unwrap_or(default)) }
    }

    pub fn get(&self) -> PathBuf {
        self.dir.lock().unwrap().clone()
    }

    pub fn set(&self, dir: &str) -> Result<PathBuf, String> {
        let dir = PathBuf::from(dir.trim());
        if !dir.is_absolute() {
            return Err("Choose a full folder path (for example C:\\Videos\\Cruxvid).".into());
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot use that folder: {e}"))?;
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Cannot save setting: {e}"))?;
        }
        std::fs::write(&self.file, dir.to_string_lossy().as_bytes()).map_err(|e| format!("Cannot save setting: {e}"))?;
        *self.dir.lock().unwrap() = dir.clone();
        Ok(dir)
    }

    /// Shows the folder in Explorer. `record`: write the path there instead (test mode, no real Explorer).
    pub fn open(&self, record: Option<&Path>) -> Result<(), String> {
        let dir = self.get();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create output folder: {e}"))?;
        match record {
            Some(file) => std::fs::write(file, dir.to_string_lossy().as_bytes()).map_err(|e| e.to_string()),
            None => std::process::Command::new("explorer").arg(&dir).spawn().map(drop).map_err(|e| format!("Cannot open folder: {e}")),
        }
    }
}
