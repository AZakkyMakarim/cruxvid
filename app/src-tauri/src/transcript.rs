//! Transcript: word-level timestamps (shape: shared/schema/transcript.schema.json).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub language: String,
    pub words: Vec<Word>,
}

impl Transcript {
    /// One `[start] sentence` line per sentence (a word ending in . ? !), the form sent to Gemini.
    pub fn as_prompt_text(&self) -> String {
        let mut out = String::new();
        let mut line_start = true;
        let mut in_line = 0;
        for w in &self.words {
            if line_start {
                out.push_str(&format!("[{:.1}] ", w.start));
                line_start = false;
                in_line = 0;
            }
            out.push_str(&w.text);
            in_line += 1;
            // Platform captions often have no punctuation: cap the line so the timestamps stay dense.
            if w.text.ends_with(['.', '?', '!']) || in_line >= 20 {
                out.push('\n');
                line_start = true;
            } else {
                out.push(' ');
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_text_has_one_timestamped_line_per_sentence() {
        let w = |t: &str, s: f64| Word { text: t.into(), start: s, end: s + 0.5 };
        let t = Transcript { language: "en".into(), words: vec![w("Hi", 0.0), w("there.", 0.6), w("Bye!", 2.0)] };
        assert_eq!(t.as_prompt_text(), "[0.0] Hi there.\n[2.0] Bye!\n");
    }

    #[test]
    fn unpunctuated_captions_still_get_a_timestamp_every_few_words() {
        let words = (0..45).map(|i| Word { text: format!("w{i}"), start: i as f64, end: i as f64 + 0.5 }).collect();
        let text = Transcript { language: "en".into(), words }.as_prompt_text();
        let starts: Vec<_> = text.lines().map(|l| l.split(']').next().unwrap()).collect();
        assert_eq!(starts, ["[0.0", "[20.0", "[40.0"]);
    }
}
