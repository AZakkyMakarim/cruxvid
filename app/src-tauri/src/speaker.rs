//! Active-speaker follow (cruxvid-b2t.14). Decision from the b2t.3 spike: mouth motion alone is not reliable,
//! so Gemini watches the Clip and reports who talks when and where that person's face is (`position`).
//! This module turns those turns + the detected faces into the keys a Layout follows.

use crate::{clips::Clip, gemini::GeminiClient, layout::{Key, Rect}, source::Source, tools};
use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;

const PROMPT: &str = include_str!("../../../shared/prompts/speakers.txt");
/// Gemini's inline request limit is 20 MB; base64 adds a third.
const MAX_BYTES: usize = 14_000_000;

/// Turns shorter than this are interjections ("yeah"): following them would make the crop flap (spike result).
pub const MIN_TURN: f64 = 1.0;
/// A reported position farther than this from every face is not trusted (speaker out of frame, or a false match).
pub const MAX_OFFSET: f64 = 0.2;

/// One stretch of one speaker, seconds from the Clip start. `position` = horizontal centre of that speaker's face
/// as a fraction of the frame width (0 = left edge); None = the speaker is not visible.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Turn {
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub position: Option<f64>,
}

#[derive(Deserialize)]
struct Turns {
    turns: Vec<Turn>,
}

/// Asks Gemini who speaks when in `clip`. Only that range is sent, as a small copy (360p, 5 fps, mono audio).
pub fn detect_turns(gemini: &GeminiClient, source: &Source, clip: &Clip) -> Result<Vec<Turn>, String> {
    let out = tools::command("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{:.3}", clip.start), "-i", &source.path, "-t", &format!("{:.3}", clip.end - clip.start)])
        .args(["-vf", "scale=-2:360,fps=5", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "35", "-pix_fmt", "yuv420p"])
        .args(["-c:a", "aac", "-b:a", "24k", "-ac", "1", "-ar", "16000", "-movflags", "frag_keyframe+empty_moov", "-f", "mp4", "pipe:1"])
        .output()
        .map_err(|e| format!("Cannot run ffmpeg (is it installed?): {e}"))?;
    if !out.status.success() || out.stdout.is_empty() || out.stdout.len() > MAX_BYTES {
        return Err(format!("Cannot prepare the Clip for speaker detection: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let video = json!({ "inline_data": { "mime_type": "video/mp4", "data": base64::engine::general_purpose::STANDARD.encode(out.stdout) } });
    let turns: Turns = gemini.generate(vec![json!({ "text": PROMPT }), video])?;
    Ok(turns.turns)
}

/// Where the crop should sit over the Clip: one key per change of speaker. A turn that is too short, has no
/// visible speaker, or matches no face keeps the previous crop. Empty = no turn could be placed (caller falls back).
pub fn follow_keys(faces: &[Rect], turns: &[Turn]) -> Vec<Key> {
    let mut sorted = turns.to_vec();
    sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut keys: Vec<Key> = Vec::new();
    for t in sorted.iter().filter(|t| t.end - t.start >= MIN_TURN) {
        let Some(p) = t.position else { continue };
        let near = faces.iter().map(|f| f.x + f.w / 2.0).min_by(|a, b| (a - p).abs().total_cmp(&(b - p).abs()));
        let Some(cx) = near.filter(|cx| (cx - p).abs() <= MAX_OFFSET) else { continue };
        match keys.last() {
            None => keys.push(Key { t: 0.0, cx }),
            Some(k) if k.cx == cx => {}
            Some(_) => keys.push(Key { t: t.start, cx }),
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(cx: f64) -> Rect {
        Rect { x: cx - 0.05, y: 0.3, w: 0.1, h: 0.1 }
    }
    fn turn(start: f64, end: f64, position: Option<f64>) -> Turn {
        Turn { start, end, position }
    }
    fn at(keys: &[Key]) -> Vec<(f64, f64)> {
        keys.iter().map(|k| (k.t, (k.cx * 100.0).round() / 100.0)).collect()
    }

    #[test]
    fn a_change_of_speaker_becomes_a_key_at_the_start_of_the_new_turn() {
        let faces = [face(0.25), face(0.75)];
        let turns = [turn(5.0, 20.0, Some(0.27)), turn(20.0, 40.0, Some(0.72)), turn(40.0, 50.0, Some(0.24))];
        // The crop is on the first speaker from the very start of the Clip.
        assert_eq!(at(&follow_keys(&faces, &turns)), [(0.0, 0.25), (20.0, 0.75), (40.0, 0.25)]);
    }

    #[test]
    fn same_speaker_twice_in_a_row_and_unsorted_turns_make_no_extra_keys() {
        let faces = [face(0.25), face(0.75)];
        let turns = [turn(20.0, 30.0, Some(0.75)), turn(0.0, 10.0, Some(0.25)), turn(10.0, 20.0, Some(0.25))];
        assert_eq!(at(&follow_keys(&faces, &turns)), [(0.0, 0.25), (20.0, 0.75)]);
    }

    #[test]
    fn speaker_out_of_frame_or_too_short_or_far_from_every_face_keeps_the_previous_crop() {
        let faces = [face(0.25), face(0.75)];
        let turns = [
            turn(0.0, 10.0, Some(0.25)),
            turn(10.0, 20.0, None),       // off camera: hold
            turn(20.0, 20.5, Some(0.75)), // interjection: ignore
            turn(21.0, 30.0, Some(0.5)),  // 0.25 from both faces: not trusted
        ];
        assert_eq!(at(&follow_keys(&faces, &turns)), [(0.0, 0.25)]);
    }

    #[test]
    fn nothing_placeable_gives_no_keys() {
        assert!(follow_keys(&[face(0.25), face(0.75)], &[turn(0.0, 10.0, None)]).is_empty());
        assert!(follow_keys(&[], &[turn(0.0, 10.0, Some(0.5))]).is_empty());
        assert!(follow_keys(&[face(0.5)], &[]).is_empty());
    }
}
