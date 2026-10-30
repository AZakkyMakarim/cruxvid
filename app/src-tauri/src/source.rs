//! Source resolver, local-file half. Links (yt-dlp) arrive with cruxvid-b2t.8.

use crate::tools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    pub title: String,
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
}

/// Accepts a local file only if ffprobe finds a video stream; otherwise a user-readable error.
pub fn probe_local(path: &Path) -> Result<Source, String> {
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }
    let not_video = || format!("{} is not a video file.", path.file_name().unwrap_or_default().to_string_lossy());
    let out = tools::command("ffprobe")
        .args(["-v", "error", "-show_streams", "-show_format", "-of", "json"])
        .arg(path)
        .output()
        .map_err(|e| format!("Cannot run ffprobe (is ffmpeg installed?): {e}"))?;
    if !out.status.success() {
        return Err(not_video());
    }
    let j: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let v = j["streams"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["codec_type"] == "video"))
        .ok_or_else(not_video)?;
    Ok(Source {
        path: path.to_string_lossy().into_owned(),
        title: path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        duration: j["format"]["duration"].as_str().and_then(|d| d.parse().ok()).ok_or_else(not_video)?,
        fps: parse_rate(v["avg_frame_rate"].as_str().unwrap_or("")).ok_or_else(not_video)?,
        width: v["width"].as_u64().unwrap_or(0) as u32,
        height: v["height"].as_u64().unwrap_or(0) as u32,
    })
}

fn parse_rate(r: &str) -> Option<f64> {
    let (n, d) = r.split_once('/')?;
    let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
    (d > 0.0 && n > 0.0).then(|| n / d)
}

#[cfg(test)]
mod tests {
    use super::parse_rate;

    #[test]
    fn parses_rational_frame_rates() {
        assert_eq!(parse_rate("30/1"), Some(30.0));
        assert!((parse_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_rate("0/0"), None);
        assert_eq!(parse_rate("x"), None);
    }
}
