//! Project store: one JSON file per Project under `<data_dir>/projects/<id>/`. Local only.

use crate::{clips::Clip, source::Source, transcript::Transcript};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub source: Source,
    pub transcript: Transcript,
    pub clips: Vec<Clip>,
}

pub struct ProjectStore {
    root: PathBuf,
}

impl ProjectStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { root: data_dir.join("projects") }
    }

    fn file(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("Invalid project id: {id}"));
        }
        Ok(self.root.join(id).join("project.json"))
    }

    pub fn new_id() -> String {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        format!("p{ms}")
    }

    pub fn save(&self, p: &Project) -> Result<(), String> {
        let file = self.file(&p.id)?;
        std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| format!("Cannot save project: {e}"))?;
        let json = serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?;
        std::fs::write(&file, json).map_err(|e| format!("Cannot save project: {e}"))
    }

    pub fn load(&self, id: &str) -> Result<Project, String> {
        let bytes = std::fs::read(self.file(id)?).map_err(|_| format!("Project not found: {id}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("Project file is damaged: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ids_that_could_escape_the_store() {
        let s = ProjectStore::new(PathBuf::from("x"));
        assert!(s.load("../evil").is_err());
        assert!(s.load("").is_err());
        assert!(s.load("p1").unwrap_err().contains("not found"));
    }
}
