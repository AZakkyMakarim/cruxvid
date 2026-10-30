//! Per-Clip Caption translation: one Clip's Caption, title and description go through Gemini; other Clips stay as they are.
//! The translation is stored beside project.json (translations.json); the Project keeps the shown title/description,
//! this file keeps the originals so "Original" restores them.

use crate::{
    caption::{CaptionStore, CaptionWord, ClipCaption},
    clips::Clip,
    gemini::GeminiClient,
    project::ProjectStore,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

const TRANSLATE_PROMPT: &str = include_str!("../../../shared/prompts/translate.txt");

/// Gemini's answer (shape: shared/schema/translate.schema.json).
#[derive(Deserialize)]
struct Translated {
    text: String,
    title: String,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Translation {
    pub language: String,
    pub original_title: String,
    pub original_description: String,
    pub words: Vec<TimedWord>,
}

/// Per Project, keyed by Clip id.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Translations(BTreeMap<String, Translation>);

impl Translations {
    /// Language of the Clip's Caption when translated; `None` = the Transcript's own.
    pub fn language(&self, clip_id: &str) -> Option<&str> {
        self.0.get(clip_id).map(|t| t.language.as_str())
    }

    pub fn get(&self, clip_id: &str) -> Option<&Translation> {
        self.0.get(clip_id)
    }
}

pub struct TranslationStore {
    root: PathBuf,
}

impl TranslationStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { root: data_dir.join("projects") }
    }

    /// `project_id` must already be validated (the Engine loads the Project first).
    pub fn load(&self, project_id: &str) -> Translations {
        std::fs::read(self.root.join(project_id).join("translations.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(&self, project_id: &str, t: &Translations) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(t).map_err(|e| e.to_string())?;
        std::fs::write(self.root.join(project_id).join("translations.json"), json).map_err(|e| format!("Cannot save translation: {e}"))
    }
}

/// Language names/codes go into the prompt, so only letters, spaces and hyphens pass.
fn clean_language(language: &str) -> Result<String, String> {
    let l = language.trim();
    if !(2..=35).contains(&l.chars().count()) || !l.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '-') {
        return Err("Bahasa tidak valid.".into());
    }
    Ok(l.to_string())
}

/// Spreads the translated words over the span of the original words, by letter count (their own timing is unknown).
fn retime(text: &str, source: &[CaptionWord]) -> Vec<TimedWord> {
    // ASS would read these as markup.
    let tokens: Vec<String> = text.split_whitespace().map(|t| t.chars().filter(|c| !matches!(c, '{' | '}' | '\\')).collect::<String>()).filter(|t| !t.is_empty()).collect();
    let (Some(first), Some(last)) = (source.first(), source.last()) else { return Vec::new() };
    let total: usize = tokens.iter().map(|t| t.chars().count()).sum();
    let span = last.end - first.start;
    let mut done = 0;
    tokens
        .into_iter()
        .map(|text| {
            let start = first.start + span * done as f64 / total as f64;
            done += text.chars().count();
            TimedWord { end: first.start + span * done as f64 / total as f64, start, text }
        })
        .collect()
}

/// The Caption as shown and exported: the translated words when the Clip has a translation, else `cap` as is.
pub fn overlay(mut cap: ClipCaption, t: Option<&Translation>) -> ClipCaption {
    if let Some(t) = t {
        // shortcut: translated words are not Transcript words, so they carry no index and cannot be corrected one by one.
        cap.words = t.words.iter().map(|w| CaptionWord { index: usize::MAX, text: w.text.clone(), original: w.text.clone(), start: w.start, end: w.end }).collect();
    }
    cap
}

/// Sets the language of one Clip's Caption, title and description. "" restores the originals.
/// Gemini is asked before anything is written, so a failure leaves the Clip as it was.
pub fn set_language(
    projects: &ProjectStore,
    store: &TranslationStore,
    captions: &CaptionStore,
    gemini: impl FnOnce() -> Result<GeminiClient, String>,
    project_id: &str,
    clip_id: &str,
    language: &str,
) -> Result<Clip, String> {
    let mut p = projects.load(project_id)?;
    let mut all = store.load(project_id);
    let i = p.clips.iter().position(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
    if language.trim().is_empty() {
        if let Some(t) = all.0.remove(clip_id) {
            p.clips[i].title = t.original_title;
            p.clips[i].description = t.original_description;
        }
    } else {
        let language = clean_language(language)?;
        let clip = &p.clips[i];
        // Always translate from the originals, never from an earlier translation.
        let (title, description) = all.get(clip_id).map_or((clip.title.clone(), clip.description.clone()), |t| (t.original_title.clone(), t.original_description.clone()));
        let source = captions.load(project_id).view(&p.transcript, clip);
        let caption: Vec<&str> = source.words.iter().map(|w| w.text.as_str()).collect();
        let ask = format!("{TRANSLATE_PROMPT}\nLanguage: {language}\nCaption:\n{}\nTitle:\n{title}\nDescription:\n{description}\n", caption.join(" "));
        let new: Translated = gemini()?.generate_json(&ask)?;
        let words = retime(&new.text, &source.words);
        if new.title.trim().is_empty() || (words.is_empty() && !source.words.is_empty()) {
            return Err("Terjemahan dari Gemini kosong. Coba lagi.".into());
        }
        let clip = &mut p.clips[i];
        clip.title = new.title.trim().to_string();
        clip.description = new.description.trim().to_string();
        all.0.insert(clip_id.into(), Translation { language, original_title: title, original_description: description, words });
    }
    store.save(project_id, &all)?;
    projects.save(&p)?;
    Ok(p.clips[i].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_words_fill_the_original_span_in_proportion_to_their_length() {
        let w = |s, e| CaptionWord { index: 0, text: "x".into(), original: "x".into(), start: s, end: e };
        let t = retime("ab {cdef}", &[w(10.0, 11.0), w(12.0, 16.0)]);
        let spans: Vec<(&str, f64, f64)> = t.iter().map(|w| (w.text.as_str(), w.start, w.end)).collect();
        assert_eq!(spans, [("ab", 10.0, 12.0), ("cdef", 12.0, 16.0)]);
        assert!(retime("anything", &[]).is_empty());
    }
}
