//! Exporter: Clip -> 9:16 MP4 (1080x1920 H.264 High/AAC, CFR, VBR ~8 Mbps, faststart).
//! Spec and flags proven in docs/spikes/b2t-2-ffmpeg-ass.md. Layout and caption burn-in come later (center crop for now).

use crate::{clips::Clip, source::Source, tools};
use std::path::{Path, PathBuf};

pub fn export_clip(source: &Source, clip: &Clip, out_dir: &Path) -> Result<PathBuf, String> {
    export_clip_with(source, clip, out_dir, None)
}

/// `layout_vf`: ffmpeg graph from the Clip's Layout (see `layout::Layout::filter`); None = centre crop.
pub fn export_clip_with(source: &Source, clip: &Clip, out_dir: &Path, layout_vf: Option<&str>) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Cannot create output folder: {e}"))?;
    let out = out_dir.join(file_name(&source.title, clip));
    let part = out.with_extension("part");
    let fps = export_fps(source.fps);
    let result = tools::command("ffmpeg")
        .args(["-y", "-v", "error", "-ss", &format!("{:.3}", clip.start), "-i", &source.path])
        .args(["-t", &format!("{:.3}", clip.end - clip.start)])
        .args(["-vf", &format!("{},fps={fps}", layout_vf.unwrap_or("scale=-2:1920,crop=1080:1920")), "-fps_mode", "cfr"])
        .args(["-c:v", "libx264", "-profile:v", "high", "-pix_fmt", "yuv420p", "-preset", "veryfast"])
        .args(["-b:v", "8M", "-maxrate", "10M", "-bufsize", "16M", "-g", &(fps * 2).to_string()])
        .args(["-c:a", "aac", "-b:a", "128k", "-ar", "48000", "-movflags", "+faststart", "-f", "mp4"])
        .arg(&part)
        .output()
        .map_err(|e| format!("Cannot run ffmpeg (is it installed?): {e}"))?;
    if !result.status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Export failed: {}", String::from_utf8_lossy(&result.stderr).trim()));
    }
    std::fs::rename(&part, &out).map_err(|e| format!("Cannot save export: {e}"))?;
    Ok(out)
}

/// Follows the Source frame rate, snapped to 30 or 60.
pub(crate) fn export_fps(source_fps: f64) -> u32 {
    if source_fps > 45.0 { 60 } else { 30 }
}

/// `<source title>_<NN>_<clip title>.mp4` with characters Windows forbids replaced.
fn file_name(source_title: &str, clip: &Clip) -> String {
    let clean = |s: &str| -> String {
        let s: String = s.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
        s.trim().trim_end_matches('.').chars().take(80).collect()
    };
    format!("{}_{:02}_{}.mp4", clean(source_title), clip.number, clean(&clip.title))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(title: &str) -> Clip {
        Clip { id: "clip-3".into(), number: 3, start: 0.0, end: 60.0, title: title.into(), description: String::new(), hook_score: 0, hook_reason: String::new() }
    }

    #[test]
    fn file_name_is_source_number_title_and_windows_safe() {
        assert_eq!(file_name("My Podcast", &clip("Why: a/b?")), "My Podcast_03_Why_ a_b_.mp4");
    }

    #[test]
    fn fps_snaps_to_30_or_60() {
        assert_eq!(export_fps(29.97), 30);
        assert_eq!(export_fps(25.0), 30);
        assert_eq!(export_fps(59.94), 60);
    }
}
