//! Topic and Clip rules: Topics are content only, a Clip is 60-120 s, cut on sentence boundaries
//! inside one Topic, never overlapping. Short Topics merge with an adjacent Topic or are dropped;
//! long Topics are trimmed to their most complete sub-range.

use crate::gemini::{Label, Section};
use crate::transcript::Transcript;
use serde::{Deserialize, Serialize};

pub const MIN_SECS: f64 = 60.0;
pub const MAX_SECS: f64 = 120.0;
/// Tolerance when matching a Gemini time to a Transcript sentence boundary.
const EPS: f64 = 0.05;

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

/// Content section(s) after merging short neighbours; `section` carries the stronger part's text and score.
struct Topic {
    start: f64,
    end: f64,
    section: Section,
}

impl Topic {
    fn len(&self) -> f64 {
        self.end - self.start
    }
}

/// Whole sentences inside [a, b].
fn inside(sentences: &[(f64, f64)], a: f64, b: f64) -> impl Iterator<Item = &(f64, f64)> {
    sentences.iter().filter(move |(s, e)| *s >= a - EPS && *e <= b + EPS)
}

/// Moves a range inward to whole sentences. No whole sentence inside (e.g. an unpunctuated Transcript): unchanged.
fn snap(sentences: &[(f64, f64)], a: f64, b: f64) -> (f64, f64) {
    let mut it = inside(sentences, a, b);
    match (it.next(), it.last()) {
        (Some(&(s, e)), None) => (s, e),
        (Some(&(s, _)), Some(&(_, e))) => (s, e),
        _ => (a, b),
    }
}

/// A Topic longer than 120 s: Gemini's named best range if it is valid, else whole sentences from the Topic start.
fn trim(t: &Topic, sentences: &[(f64, f64)], (a, b): (f64, f64)) -> (f64, f64) {
    if let (Some(bs), Some(be)) = (t.section.best_start, t.section.best_end) {
        let (bs, be) = snap(sentences, bs, be);
        if bs >= a - EPS && be <= b + EPS && (MIN_SECS..=MAX_SECS).contains(&(be - bs)) {
            return (bs, be);
        }
    }
    let end = inside(sentences, a, b).map(|&(_, e)| e).take_while(|e| e - a <= MAX_SECS).last();
    (a, end.unwrap_or(a + MAX_SECS))
}

pub fn build_clips(sections: &[Section], transcript: &Transcript) -> Vec<Clip> {
    let mut sorted: Vec<&Section> = sections.iter().filter(|s| s.end > s.start).collect();
    sorted.sort_by(|a, b| a.start.total_cmp(&b.start));

    // Merge pass: a short Topic joins the adjacent Topic (nothing but content between them) if the result fits.
    let mut topics: Vec<Topic> = Vec::new();
    let mut prev_content = false;
    for s in sorted {
        let is_content = s.label == Label::Content;
        if is_content {
            let t = Topic { start: s.start, end: s.end, section: s.clone() };
            match topics.last_mut() {
                Some(last) if prev_content && (last.len() < MIN_SECS || t.len() < MIN_SECS) && t.end.max(last.end) - last.start <= MAX_SECS => {
                    last.end = last.end.max(t.end);
                    if t.section.hook_score > last.section.hook_score {
                        last.section = t.section;
                    }
                }
                _ => topics.push(t),
            }
        }
        prev_content = is_content;
    }

    let sentences = transcript.sentences();
    let mut clips: Vec<Clip> = Vec::new();
    for t in topics {
        let mut range = snap(&sentences, t.start, t.end);
        if range.1 - range.0 > MAX_SECS + EPS {
            range = trim(&t, &sentences, range);
        }
        let (start, end) = range;
        if !(MIN_SECS..=MAX_SECS + EPS).contains(&(end - start)) || clips.last().is_some_and(|prev| start < prev.end) {
            continue; // too short even after merging, or overlapping the previous Clip
        }
        let number = clips.len() as u32 + 1;
        let s = t.section;
        clips.push(Clip {
            id: format!("clip-{number}"),
            number,
            start,
            end,
            title: if s.title.is_empty() { format!("Clip {number}") } else { s.title },
            description: s.description,
            hook_score: s.hook_score.min(100),
            hook_reason: s.hook_reason,
        });
    }
    clips
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Word;

    /// 5 s sentence slots over 10 minutes: sentence k spans [5k, 5k+4.4].
    fn transcript() -> Transcript {
        let words = (0..120).map(|k| Word { text: "word.".into(), start: k as f64 * 5.0, end: k as f64 * 5.0 + 4.4 }).collect();
        Transcript { language: "en".into(), words }
    }

    fn sec(label: Label, start: f64, end: f64) -> Section {
        Section { label, start, end, title: "t".into(), description: String::new(), hook_score: 70, hook_reason: String::new(), best_start: None, best_end: None }
    }

    fn spans(sections: &[Section]) -> Vec<(f64, f64)> {
        build_clips(sections, &transcript()).iter().map(|c| (c.start, c.end)).collect()
    }

    #[test]
    fn only_content_of_60_to_120_seconds_becomes_a_clip_in_timeline_order() {
        let clips = build_clips(
            &[
                sec(Label::Content, 300.0, 379.4),
                sec(Label::Opening, 0.0, 89.4),
                sec(Label::Sponsor, 90.0, 199.4),
                sec(Label::Content, 200.0, 254.4), // 54 s: too short, no neighbour to merge with
                sec(Label::Content, 10.0, 74.4),   // 64 s
                sec(Label::Content, 400.0, 524.4), // 124 s: trimmed
            ],
            &transcript(),
        );
        let spans: Vec<_> = clips.iter().map(|c| (c.id.as_str(), c.number, c.start, c.end)).collect();
        assert_eq!(spans, [("clip-1", 1, 10.0, 74.4), ("clip-2", 2, 300.0, 379.4), ("clip-3", 3, 400.0, 519.4)]);
    }

    #[test]
    fn overlapping_clips_keep_the_earlier_one() {
        assert_eq!(spans(&[sec(Label::Content, 0.0, 79.4), sec(Label::Content, 70.0, 149.4)]).len(), 1);
    }

    #[test]
    fn cuts_snap_inward_to_sentence_boundaries() {
        // 47..126 -> first sentence starting at/after 47 is 50, last sentence ending at/before 126 ends 124.4.
        assert_eq!(spans(&[sec(Label::Content, 47.0, 126.0)]), [(50.0, 124.4)]);
    }

    #[test]
    fn a_topic_that_shrinks_under_60_s_when_snapped_is_dropped() {
        assert!(spans(&[sec(Label::Content, 47.0, 109.0)]).is_empty()); // 50..104.4
    }

    #[test]
    fn long_topic_without_a_best_range_keeps_whole_sentences_from_its_start() {
        assert_eq!(spans(&[sec(Label::Content, 100.0, 300.0)]), [(100.0, 219.4)]);
    }

    #[test]
    fn long_topic_is_trimmed_to_the_best_range_gemini_names() {
        let mut s = sec(Label::Content, 100.0, 300.0);
        s.best_start = Some(150.0);
        s.best_end = Some(244.4);
        assert_eq!(spans(&[s]), [(150.0, 244.4)]);
    }

    #[test]
    fn an_invalid_best_range_is_ignored() {
        let mut s = sec(Label::Content, 100.0, 300.0);
        s.best_start = Some(50.0); // outside the Topic
        s.best_end = Some(140.0);
        assert_eq!(spans(&[s]), [(100.0, 219.4)]);
    }

    #[test]
    fn short_topic_merges_with_an_adjacent_topic_when_the_result_fits() {
        assert_eq!(spans(&[sec(Label::Content, 0.0, 44.4), sec(Label::Content, 45.0, 84.4)]), [(0.0, 84.4)]);
    }

    #[test]
    fn merged_topic_takes_the_title_and_hook_of_the_stronger_part() {
        let mut strong = sec(Label::Content, 45.0, 84.4);
        (strong.title, strong.hook_score, strong.hook_reason) = ("strong".into(), 90, "why".into());
        let clips = build_clips(&[sec(Label::Content, 0.0, 44.4), strong], &transcript());
        assert_eq!((clips[0].title.as_str(), clips[0].hook_score, clips[0].hook_reason.as_str()), ("strong", 90, "why"));
    }

    #[test]
    fn short_topic_is_dropped_when_alone_or_separated_by_a_sponsor_or_too_big_to_merge() {
        assert!(spans(&[sec(Label::Content, 0.0, 39.4)]).is_empty());
        assert!(spans(&[sec(Label::Content, 0.0, 39.4), sec(Label::Sponsor, 40.0, 79.4), sec(Label::Content, 80.0, 119.4)]).is_empty());
        // 100 s + 40 s would be 140 s: the long one stays, the short one goes.
        assert_eq!(spans(&[sec(Label::Content, 0.0, 99.4), sec(Label::Content, 100.0, 139.4)]), [(0.0, 99.4)]);
    }

    #[test]
    fn hook_score_is_capped_at_100() {
        let mut s = sec(Label::Content, 0.0, 79.4);
        s.hook_score = 250;
        assert_eq!(build_clips(&[s], &transcript())[0].hook_score, 100);
    }
}
