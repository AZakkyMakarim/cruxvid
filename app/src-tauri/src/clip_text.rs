//! Title and description per Clip: manual edit and per-field Regenerate. Both write straight into the Project.
//! The text language follows the Clip's Caption language (its translation, if any).

use crate::{clips::Clip, gemini::GeminiClient, project::{Project, ProjectStore}, transcript::Transcript};
use serde::Deserialize;

const DESCRIBE_PROMPT: &str = include_str!("../../../shared/prompts/describe.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextField {
    Title,
    Description,
}

/// Gemini's answer (shape: shared/schema/describe.schema.json).
#[derive(Deserialize)]
struct Described {
    title: String,
    description: String,
}

/// Language of the Clip's Caption: its translation when it has one (translate.rs), else the Transcript's.
fn caption_language<'a>(transcript: &'a Transcript, translated: Option<&'a str>) -> &'a str {
    translated.unwrap_or(&transcript.language)
}

/// The prompt: task marker, target language, and only this Clip's spoken text.
fn prompt(transcript: &Transcript, clip: &Clip, translated: Option<&str>) -> String {
    let words: Vec<&str> = transcript.words.iter().filter(|w| w.start >= clip.start - 0.05 && w.end <= clip.end + 0.05).map(|w| w.text.as_str()).collect();
    format!("{DESCRIBE_PROMPT}\nLanguage: {}\nTranscript of the clip:\n{}\n", caption_language(transcript, translated), words.join(" "))
}

fn clip_mut<'a>(p: &'a mut Project, clip_id: &str) -> Result<&'a mut Clip, String> {
    p.clips.iter_mut().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))
}

pub fn edit(store: &ProjectStore, project_id: &str, clip_id: &str, title: &str, description: &str) -> Result<Clip, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Title tidak boleh kosong.".into());
    }
    let mut p = store.load(project_id)?;
    let clip = clip_mut(&mut p, clip_id)?;
    clip.title = title.to_string();
    clip.description = description.trim().to_string();
    let clip = clip.clone();
    store.save(&p)?;
    Ok(clip)
}

/// Asks Gemini for new text and replaces `field` of this Clip only. On any error nothing is saved.
/// `translated`: the Clip's Caption language when it was translated; the text is written in it.
pub fn regenerate(store: &ProjectStore, gemini: &GeminiClient, project_id: &str, clip_id: &str, field: TextField, translated: Option<&str>) -> Result<Clip, String> {
    let mut p = store.load(project_id)?;
    let ask = prompt(&p.transcript, p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?, translated);
    let new: Described = gemini.generate_json(&ask)?;
    let clip = clip_mut(&mut p, clip_id)?;
    match field {
        TextField::Title => clip.title = new.title.trim().to_string(),
        TextField::Description => clip.description = new.description.trim().to_string(),
    }
    let clip = clip.clone();
    store.save(&p)?;
    Ok(clip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Word;

    #[test]
    fn prompt_asks_in_the_caption_language_about_only_the_clips_words() {
        let w = |t: &str, s: f64| Word { text: t.into(), start: s, end: s + 0.5 };
        let t = Transcript { language: "id".into(), words: vec![w("before.", 0.0), w("Halo", 10.0), w("dunia.", 11.0), w("after.", 30.0)] };
        let clip = Clip { id: "c".into(), number: 1, start: 10.0, end: 11.5, title: String::new(), description: String::new(), hook_score: 0, hook_reason: String::new() };
        let p = prompt(&t, &clip, None);
        assert!(p.contains("CRUXVID_TASK: describe") && p.contains("Language: id") && p.contains("Halo dunia."), "{p}");
        assert!(!p.contains("before.") && !p.contains("after."), "{p}");
        // A translated Clip is described in its Caption language, still from its own spoken words.
        let p = prompt(&t, &clip, Some("en"));
        assert!(p.contains("Language: en") && !p.contains("Language: id") && p.contains("Halo dunia."), "{p}");
    }
}
