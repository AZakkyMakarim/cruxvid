//! Caption: style presets, per-word corrections and the ASS file Export burns in.
//! ASS template is shared with Android (shared/ass, ADR 0002); format proven in docs/spikes/b2t-2-ffmpeg-ass.md.

use crate::{clips::Clip, transcript::Transcript};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, HashMap}, path::PathBuf};

const TEMPLATE: &str = include_str!("../../../shared/ass/caption.ass.tmpl");
/// Platform UI (captions, buttons) covers roughly the bottom quarter of a 1080x1920 frame.
pub const SAFE_MARGIN_V: u32 = 480;
const LINE_WORDS: usize = 4;
const LINE_CHARS: usize = 28;

/// Preset values plus the few parameters the user may change. Colors are `#RRGGBB`; sizes are output pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionStyle {
    pub preset: String,
    pub font: String,
    pub size: u32,
    pub color: String,
    pub highlight: String,
    pub outline: u32,
    /// Distance from the bottom edge to the text (position).
    pub margin_v: u32,
}

pub fn presets() -> Vec<CaptionStyle> {
    let p = |preset: &str, font: &str, size, color: &str, highlight: &str, outline| CaptionStyle {
        preset: preset.into(),
        font: font.into(),
        size,
        color: color.into(),
        highlight: highlight.into(),
        outline,
        margin_v: SAFE_MARGIN_V,
    };
    vec![
        p("classic", "Arial", 68, "#FFFFFF", "#FFE600", 6),
        p("bold", "Impact", 84, "#FFFFFF", "#39FF14", 8),
        p("soft", "Segoe UI", 60, "#F5F5F5", "#4FC3F7", 3),
        p("pop", "Arial Black", 64, "#FFFFFF", "#FF4081", 5),
    ]
}

impl CaptionStyle {
    pub fn validate(&self) -> Result<(), String> {
        let hex = |c: &str| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit());
        if !hex(&self.color) || !hex(&self.highlight) {
            return Err("Caption color must look like #RRGGBB.".into());
        }
        if self.font.trim().is_empty() || self.font.len() > 64 || self.font.contains([',', '\r', '\n', '{', '}']) {
            return Err("Caption font name is not valid.".into());
        }
        if !(30..=140).contains(&self.size) {
            return Err("Caption size must be 30-140.".into());
        }
        if self.outline > 12 || self.margin_v > 1500 {
            return Err("Caption outline must be 0-12 and position 0-1500.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaptionWord {
    /// Index into the Transcript words (what corrections are keyed by).
    pub index: usize,
    pub text: String,
    pub original: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipCaption {
    pub style: CaptionStyle,
    pub words: Vec<CaptionWord>,
}

/// Per Project: style per Clip, corrections per Transcript word. Stored beside project.json.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CaptionState {
    styles: HashMap<String, CaptionStyle>,
    corrections: BTreeMap<usize, String>,
}

impl CaptionState {
    pub fn style(&self, clip_id: &str) -> CaptionStyle {
        self.styles.get(clip_id).cloned().unwrap_or_else(|| presets().remove(0))
    }

    pub fn set_style(&mut self, clip_id: &str, style: CaptionStyle) -> Result<(), String> {
        style.validate()?;
        self.styles.insert(clip_id.into(), style);
        Ok(())
    }

    /// Returns the text now shown. Empty (or unchanged) text restores the original word.
    pub fn correct(&mut self, transcript: &Transcript, index: usize, text: &str) -> Result<String, String> {
        let original = &transcript.words.get(index).ok_or_else(|| format!("Word not found: {index}"))?.text;
        let text = clean(text);
        if text.is_empty() || text == *original {
            self.corrections.remove(&index);
            return Ok(original.clone());
        }
        self.corrections.insert(index, text.clone());
        Ok(text)
    }

    pub fn view(&self, transcript: &Transcript, clip: &Clip) -> ClipCaption {
        let words = transcript
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| w.start >= clip.start && w.start < clip.end)
            .map(|(index, w)| CaptionWord {
                index,
                text: self.corrections.get(&index).cloned().unwrap_or_else(|| w.text.clone()),
                original: w.text.clone(),
                start: w.start,
                end: w.end,
            })
            .collect();
        ClipCaption { style: self.style(&clip.id), words }
    }
}

/// Trim, drop characters that ASS would read as markup, collapse whitespace.
fn clean(text: &str) -> String {
    let t: String = text.chars().filter(|c| !matches!(c, '{' | '}' | '\\')).collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub struct CaptionStore {
    root: PathBuf,
}

impl CaptionStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { root: data_dir.join("projects") }
    }

    /// `project_id` must already be validated (the Engine loads the Project first).
    pub fn load(&self, project_id: &str) -> CaptionState {
        std::fs::read(self.root.join(project_id).join("captions.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Read-modify-write of captions.json under one lock; a damaged file is an error, never replaced by an empty one.
    pub fn update<T>(&self, project_id: &str, f: impl FnOnce(&mut CaptionState) -> Result<T, String>) -> Result<T, String> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = crate::fsutil::lock(&LOCK);
        let file = self.root.join(project_id).join("captions.json");
        let mut state: CaptionState = crate::fsutil::read_json(&file, "caption")?;
        let out = f(&mut state)?;
        crate::fsutil::write_json(&file, &state, "caption")?;
        Ok(out)
    }
}

/// `#RRGGBB` -> ASS `BBGGRR`.
fn bgr(hex: &str) -> String {
    format!("{}{}{}", &hex[5..7], &hex[3..5], &hex[1..3]).to_uppercase()
}

fn ass_time(t: f64) -> String {
    let cs = (t.max(0.0) * 100.0).round() as u64;
    format!("{}:{:02}:{:02}.{:02}", cs / 360_000, cs / 6000 % 60, cs / 100 % 60, cs % 100)
}

/// ASS file: one Dialogue event per word, showing the current line with the active word highlighted.
pub fn ass(clip: &Clip, caption: &ClipCaption) -> String {
    let s = &caption.style;
    let mut out = TEMPLATE
        .replace("{{FONT}}", &s.font)
        .replace("{{SIZE}}", &s.size.to_string())
        .replace("{{COLOR}}", &format!("&H00{}", bgr(&s.color)))
        .replace("{{OUTLINE}}", &s.outline.to_string())
        .replace("{{MARGIN_V}}", &s.margin_v.to_string());
    let words = &caption.words;
    let dur = clip.end - clip.start;
    let rel = |t: f64| (t - clip.start).clamp(0.0, dur);
    let (hl, base) = (format!("{{\\c&H{}&}}", bgr(&s.highlight)), format!("{{\\c&H{}&}}", bgr(&s.color)));
    let mut i = 0;
    while i < words.len() {
        let mut n = 1;
        let mut chars = words[i].text.chars().count();
        while i + n < words.len() && n < LINE_WORDS && chars + 1 + words[i + n].text.chars().count() <= LINE_CHARS {
            chars += 1 + words[i + n].text.chars().count();
            n += 1;
        }
        for a in i..i + n {
            // Hold until the next word of the line starts so the line does not flicker in the gaps.
            let end = if a + 1 < i + n { words[a + 1].start } else { words[a].end };
            let (start, end) = (rel(words[a].start), rel(end));
            if end <= start {
                continue;
            }
            let line: Vec<String> = (i..i + n)
                .map(|j| if j == a { format!("{hl}{}{base}", words[j].text) } else { words[j].text.clone() })
                .collect();
            out.push_str(&format!("Dialogue: 0,{},{},Default,,0,0,0,,{}\r\n", ass_time(start), ass_time(end), line.join(" ")));
        }
        i += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Word;

    fn transcript() -> Transcript {
        let words = ["alpha", "bravo", "charlie", "delta", "echo.", "foxtrot"]
            .iter()
            .enumerate()
            .map(|(i, t)| Word { text: (*t).into(), start: 10.0 + i as f64, end: 10.8 + i as f64 })
            .collect();
        Transcript { language: "en".into(), words }
    }

    fn clip() -> Clip {
        Clip { id: "clip-1".into(), number: 1, start: 10.0, end: 70.0, title: "t".into(), description: String::new(), hook_score: 0, hook_reason: String::new() }
    }

    fn dialogues(a: &str) -> Vec<&str> {
        a.lines().filter(|l| l.starts_with("Dialogue:")).collect()
    }

    #[test]
    fn ass_has_one_event_per_word_with_the_active_word_highlighted() {
        let mut st = CaptionState::default();
        let mut style = presets().remove(0);
        style.highlight = "#FF8000".into();
        st.set_style("clip-1", style).unwrap();
        let a = ass(&clip(), &st.view(&transcript(), &clip()));
        let d = dialogues(&a);
        assert_eq!(d.len(), 6);
        assert!(a.contains("Style: Default,Arial,68,&H00FFFFFF"));
        // 4-word lines; second event highlights "bravo"; times are clip-relative and held until the next word.
        assert_eq!(d[1], "Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,alpha {\\c&H0080FF&}bravo{\\c&HFFFFFF&} charlie delta");
        assert!(d[4].ends_with("{\\c&H0080FF&}echo.{\\c&HFFFFFF&} foxtrot"));
        assert!(d[3].contains("{\\c&H0080FF&}delta"));
        assert!(d[5].starts_with("Dialogue: 0,0:00:05.00,0:00:05.80"));
    }

    #[test]
    fn corrections_reach_the_ass_and_empty_text_restores_the_word() {
        let t = transcript();
        let mut st = CaptionState::default();
        assert_eq!(st.correct(&t, 1, " {\\i1}Brav o ").unwrap(), "i1Brav o");
        assert!(ass(&clip(), &st.view(&t, &clip())).contains("i1Brav o"));
        assert_eq!(st.correct(&t, 1, "  ").unwrap(), "bravo");
        assert!(!ass(&clip(), &st.view(&t, &clip())).contains("Brav"));
        assert!(st.correct(&t, 6, "x").is_err());
    }

    #[test]
    fn words_outside_the_clip_are_not_captioned_and_events_stay_inside_it() {
        let mut c = clip();
        c.start = 11.0;
        c.end = 13.5;
        let v = CaptionState::default().view(&transcript(), &c);
        assert_eq!(v.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["bravo", "charlie", "delta"]);
        let a = ass(&c, &v);
        assert!(dialogues(&a).last().unwrap().contains("0:00:02.50"), "last event clamped to clip end");
    }

    #[test]
    fn validation_rejects_bad_colors_sizes_and_fonts() {
        let ok = presets().remove(0);
        assert!(ok.validate().is_ok());
        for bad in [
            CaptionStyle { color: "#GG0000".into(), ..ok.clone() },
            CaptionStyle { highlight: "FFF".into(), ..ok.clone() },
            CaptionStyle { font: "A,B".into(), ..ok.clone() },
            CaptionStyle { size: 500, ..ok.clone() },
            CaptionStyle { margin_v: 5000, ..ok.clone() },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
