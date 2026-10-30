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
    /// (start, end) of every sentence; a sentence ends at a word ending in . ? ! (trailing words form a last one).
    pub fn sentences(&self) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        let mut open: Option<f64> = None;
        for w in &self.words {
            let start = *open.get_or_insert(w.start);
            if w.text.ends_with(['.', '?', '!']) {
                out.push((start, w.end));
                open = None;
            }
        }
        if let (Some(start), Some(w)) = (open, self.words.last()) {
            out.push((start, w.end));
        }
        out
    }

    /// One `[start] sentence` line per sentence (a word ending in . ? !), the form sent to Gemini.
    pub fn as_prompt_text(&self) -> String {
        let mut out = String::new();
        let mut line_start = true;
        for w in &self.words {
            if line_start {
                out.push_str(&format!("[{:.1}] ", w.start));
                line_start = false;
            }
            out.push_str(&w.text);
            if w.text.ends_with(['.', '?', '!']) {
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
    fn sentences_span_first_word_start_to_terminating_word_end() {
        let w = |t: &str, s: f64| Word { text: t.into(), start: s, end: s + 0.5 };
        let t = Transcript { language: "en".into(), words: vec![w("Hi", 0.0), w("there.", 0.6), w("Bye!", 2.0), w("and", 3.0)] };
        assert_eq!(t.sentences(), [(0.0, 1.1), (2.0, 2.5), (3.0, 3.5)]);
    }
}
