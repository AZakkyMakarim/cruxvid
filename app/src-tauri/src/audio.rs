//! Audio track of a Source, shrunk for Gemini (mono 16 kHz MP3, ~24 kbps = 3 KB/s).

use crate::tools;
use std::path::Path;

/// Gemini's inline request limit is 20 MB total; base64 adds a third, so keep the raw audio well under.
const MAX_BYTES: usize = 14_000_000;

/// shortcut: inline upload caps a Source at about 90 minutes of audio, switch to the Files API for longer ones.
pub fn extract_mp3(path: &Path) -> Result<Vec<u8>, String> {
    let out = tools::command("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-b:a", "24k", "-f", "mp3", "pipe:1"])
        .output()
        .map_err(|e| format!("Cannot run ffmpeg (is it installed?): {e}"))?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err("Cannot read the audio of this video (does it have a sound track?).".into());
    }
    if out.stdout.len() > MAX_BYTES {
        return Err("This video is too long to send to Gemini in one piece (limit is about 90 minutes).".into());
    }
    Ok(out.stdout)
}
