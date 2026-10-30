//! Exporter: Clip -> 9:16 MP4 (1080x1920 H.264 High/AAC, CFR, VBR ~8 Mbps, faststart).
//! Spec and flags proven in docs/spikes/b2t-2-ffmpeg-ass.md. Layout and caption burn-in come later (center crop for now).

use crate::{clips::Clip, source::Source, tools};
use std::path::{Path, PathBuf};

pub fn export_clip(source: &Source, clip: &Clip, out_dir: &Path) -> Result<PathBuf, String> {
    export_clip_with(source, clip, out_dir, None, None)
}

/// `layout_vf`: ffmpeg graph from the Clip's Layout (see `layout::Layout::filter`); None = centre crop.
/// `ass`: optional ASS file content burned into the 1080x1920 result (Caption).
pub fn export_clip_with(source: &Source, clip: &Clip, out_dir: &Path, layout_vf: Option<&str>, ass: Option<&str>) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Cannot create output folder: {e}"))?;
    let out = reserve(out_dir, &file_name(&source.title, clip))?;
    let part = out.with_extension("part");
    let fps = export_fps(source.fps);
    // Unique per call: two exports of the same Clip id (other Projects, parallel tests) must not share a file.
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let ass_file = std::env::temp_dir().join(format!("cruxvid-{}-{}-{}.ass", std::process::id(), clip.id, SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let mut vf = format!("{},fps={fps}", layout_vf.unwrap_or("scale=-2:1920,crop=1080:1920"));
    if let Some(ass) = ass {
        std::fs::write(&ass_file, ass).map_err(|e| format!("Cannot write caption file: {e}"))?;
        vf.push_str(&ass_filter(&ass_file));
    }
    let result = tools::command("ffmpeg")
        .args(["-y", "-v", "error", "-ss", &format!("{:.3}", clip.start), "-i", &source.path])
        .args(["-t", &format!("{:.3}", clip.end - clip.start)])
        .args(["-vf", &vf, "-fps_mode", "cfr"])
        .args(["-c:v", "libx264", "-profile:v", "high", "-pix_fmt", "yuv420p", "-preset", "veryfast"])
        .args(["-b:v", "8M", "-maxrate", "10M", "-bufsize", "16M", "-g", &(fps * 2).to_string()])
        .args(["-c:a", "aac", "-b:a", "128k", "-ar", "48000", "-movflags", "+faststart", "-f", "mp4"])
        .arg(&part)
        .output();
    let _ = std::fs::remove_file(&ass_file);
    let result = result.map_err(|e| format!("Cannot run ffmpeg (is it installed?): {e}"));
    let saved = match result {
        Ok(r) if r.status.success() => std::fs::rename(&part, &out).map_err(|e| format!("Cannot save export: {e}")),
        Ok(r) => Err(format!("Export failed: {}", String::from_utf8_lossy(&r.stderr).trim())),
        Err(e) => Err(e),
    };
    if saved.is_err() {
        let _ = std::fs::remove_file(&part);
        let _ = std::fs::remove_file(&out); // the empty placeholder `reserve` made
    }
    saved.map(|()| out)
}

/// Claims a file name that does not exist yet by creating it empty: `name`, else `name (2)`, `name (3)`...
/// An export never overwrites another Clip's file (same source title and clip number in another Project).
fn reserve(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let stem = name.strip_suffix(".mp4").unwrap_or(name);
    for n in 1u32.. {
        let path = dir.join(if n == 1 { name.to_string() } else { format!("{stem} ({n}).mp4") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("Cannot create the export file: {e}")),
        }
    }
    unreachable!()
}

/// `,ass=...` filter with ffmpeg filtergraph escaping; fonts come from the Windows folder so the look is deterministic.
fn ass_filter(file: &Path) -> String {
    let q = |p: &str| format!("'{}'", p.replace('\\', "/").replace(':', "\\:").replace('\'', "'\\''"));
    let fonts = std::env::var("WINDIR").map_or("C:/Windows/Fonts".to_string(), |w| format!("{w}/Fonts"));
    format!(",ass={}:fontsdir={}", q(&file.to_string_lossy()), q(&fonts))
}

/// Follows the Source frame rate, snapped to 30 or 60.
pub(crate) fn export_fps(source_fps: f64) -> u32 {
    if source_fps > 45.0 { 60 } else { 30 }
}

/// `<source title>_<NN>_<clip title>.mp4` with characters Windows forbids replaced. `reserve` keeps the name unique.
fn file_name(source_title: &str, clip: &Clip) -> String {
    format!("{}_{:02}_{}.mp4", clean_part(source_title), clip.number, clean_part(&clip.title))
}

/// One part of a file name: no forbidden characters, no leading/trailing dots or spaces, not a Windows device name.
fn clean_part(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).take(80).collect();
    let s = s.trim_matches(|c| c == ' ' || c == '.');
    let device = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (device.len() == 4 && (device.starts_with("COM") || device.starts_with("LPT")) && device.ends_with(|c: char| c.is_ascii_digit() && c != '0'));
    if reserved { format!("_{s}") } else { s.to_string() }
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
    fn name_parts_are_not_windows_device_names_nor_end_in_dots_or_spaces() {
        for (given, shown) in [("CON", "_CON"), ("nul.txt", "_nul.txt"), ("Com1", "_Com1"), ("LPT9", "_LPT9"), ("COM0", "COM0"), ("CONSOLE", "CONSOLE"), ("Title. . ", "Title"), ("  ..x", "x")] {
            assert_eq!(clean_part(given), shown, "{given:?}");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number_instead_of_being_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let names: Vec<_> = (0..3).map(|_| reserve(dir.path(), "a_01_b.mp4").unwrap().file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a_01_b.mp4", "a_01_b (2).mp4", "a_01_b (3).mp4"]);
    }

    #[test]
    fn ass_filter_escapes_the_windows_drive_colon_and_quotes() {
        let f = ass_filter(Path::new("C:\\Users\\o'neil\\a.ass"));
        assert!(f.starts_with(",ass='C\\:/Users/o'\\''neil/a.ass':fontsdir='"), "{f}");
    }

    #[test]
    fn fps_snaps_to_30_or_60() {
        assert_eq!(export_fps(29.97), 30);
        assert_eq!(export_fps(25.0), 30);
        assert_eq!(export_fps(59.94), 60);
    }
}
