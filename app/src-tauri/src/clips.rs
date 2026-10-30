//! Clip builder. Tracer scope: a content section that is already 60-120 s becomes a Clip.
//! Trimming long Topics, merging short ones and snapping to sentences is cruxvid-b2t.9.

use crate::gemini::{Label, Section};
use serde::{Deserialize, Serialize};

pub const MIN_SECS: f64 = 60.0;
pub const MAX_SECS: f64 = 120.0;

/// Non-destructive definition of a segment; only Export produces a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    /// 1-based position in timeline order (used in the Export file name).
    pub number: u32,
    pub start: f64,
    pub end: f64,
    pub title: String,
    pub description: String,
    pub hook_score: u8,
    pub hook_reason: String,
}

pub fn build_clips(sections: &[Section]) -> Vec<Clip> {
    let mut content: Vec<&Section> = sections
        .iter()
        .filter(|s| s.label == Label::Content && (MIN_SECS..=MAX_SECS).contains(&(s.end - s.start)))
        .collect();
    content.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut clips: Vec<Clip> = Vec::new();
    for s in content {
        if clips.last().is_some_and(|prev| s.start < prev.end) {
            continue; // no overlap between Clips
        }
        let number = clips.len() as u32 + 1;
        clips.push(Clip {
            id: format!("clip-{number}"),
            number,
            start: s.start,
            end: s.end,
            title: if s.title.is_empty() { format!("Clip {number}") } else { s.title.clone() },
            description: s.description.clone(),
            hook_score: s.hook_score.min(100),
            hook_reason: s.hook_reason.clone(),
        });
    }
    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(label: Label, start: f64, end: f64) -> Section {
        Section { label, start, end, title: "t".into(), description: String::new(), hook_score: 70, hook_reason: String::new() }
    }

    #[test]
    fn only_content_of_60_to_120_seconds_becomes_a_clip_in_timeline_order() {
        let clips = build_clips(&[
            sec(Label::Content, 300.0, 380.0),
            sec(Label::Opening, 0.0, 90.0),
            sec(Label::Sponsor, 90.0, 200.0),
            sec(Label::Content, 200.0, 259.0),  // 59 s: too short
            sec(Label::Content, 10.0, 70.0),    // exactly 60 s
            sec(Label::Content, 400.0, 521.0),  // 121 s: too long
        ]);
        let spans: Vec<_> = clips.iter().map(|c| (c.id.as_str(), c.number, c.start, c.end)).collect();
        assert_eq!(spans, [("clip-1", 1, 10.0, 70.0), ("clip-2", 2, 300.0, 380.0)]);
    }

    #[test]
    fn overlapping_clips_keep_the_earlier_one() {
        let clips = build_clips(&[sec(Label::Content, 0.0, 80.0), sec(Label::Content, 70.0, 150.0)]);
        assert_eq!(clips.len(), 1);
    }
}
