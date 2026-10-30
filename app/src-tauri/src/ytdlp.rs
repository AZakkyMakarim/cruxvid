//! Source from a link, through the yt-dlp binary: metadata, captions, audio and stream-cut of a Clip range.
//! Decisions from docs/spikes/b2t-1-stream-cut.md: pin the format to protocol=https, give ffmpeg ranged requests
//! (else it drains the whole track), check the cut with ffprobe and fall back to a padded range.
//! For a link Source, `Source::path` holds the URL; the cut files live under the data dir.

use crate::{source::Source, tools, transcript::Transcript, vtt};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

pub const MSG_LIVE: &str = "Siaran ini masih live. Tunggu sampai siarannya selesai, lalu tempel link rekamannya.";

/// https (DASH) first: the default YouTube pick is an HLS stream that is very slow to cut. The last
/// alternatives serve Twitch (HLS only) and sites without height info.
const FORMAT: &str = "bv*[height<=1080][protocol=https]+ba[protocol=https]/bv*[height<=1080]+ba/b[height<=1080]/b";
const FFMPEG_RANGED: &str = "ffmpeg_i:-multiple_requests 1 -request_size 2000000";
const DAY_SECS: u64 = 24 * 60 * 60;
const UPDATE_TIMEOUT: Duration = Duration::from_secs(60);
const PAD_SECS: f64 = 10.0;
/// Allowed difference between the wanted and the real length of a cut.
const CUT_TOLERANCE: f64 = 1.0;

pub fn is_link(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("http://") || s.starts_with("https://")
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkInfo {
    pub title: String,
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    /// Language of the platform captions to fetch; `None` = no usable captions (use audio).
    pub caption_lang: Option<String>,
}

/// yt-dlp `-J` output -> what we need. A live or not-yet-finished stream is rejected.
pub fn parse_info(json: &[u8]) -> Result<LinkInfo, String> {
    let j: Value = serde_json::from_slice(json).map_err(|_| "yt-dlp gave no video information for this link.".to_string())?;
    let status = j["live_status"].as_str().unwrap_or("");
    if j["is_live"] == true || matches!(status, "is_live" | "is_upcoming" | "post_live") {
        return Err(MSG_LIVE.into());
    }
    let duration = j["duration"].as_f64().filter(|d| *d > 0.0).ok_or("This link has no video with a known length.")?;
    Ok(LinkInfo {
        title: j["title"].as_str().unwrap_or("video").to_string(),
        duration,
        fps: j["fps"].as_f64().filter(|f| *f > 0.0).unwrap_or(30.0),
        width: j["width"].as_u64().unwrap_or(0) as u32,
        height: j["height"].as_u64().unwrap_or(0) as u32,
        caption_lang: pick_caption_lang(&j),
    })
}

/// Hand-made captions first (the video's own language, else the first); automatic ones only in the video's language.
fn pick_caption_lang(j: &Value) -> Option<String> {
    let keys = |k: &str| -> Vec<String> {
        j[k].as_object().map_or(vec![], |o| o.iter().filter(|(l, v)| *l != "live_chat" && v.as_array().is_some_and(|a| !a.is_empty())).map(|(l, _)| l.clone()).collect())
    };
    let lang = j["language"].as_str().unwrap_or("");
    let manual = keys("subtitles");
    if let Some(l) = manual.iter().find(|l| *l == lang).or(manual.first()) {
        return Some(l.clone());
    }
    let auto = keys("automatic_captions");
    let wanted = if lang.is_empty() { "en" } else { lang };
    [format!("{wanted}-orig"), wanted.to_string()].into_iter().find(|l| auto.contains(l))
}

/// A stream-cut result: `offset` is where the Clip starts inside `path` (0 unless the padded fallback was used).
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    pub path: PathBuf,
    pub offset: f64,
}

pub struct YtDlp {
    bin: PathBuf,
    /// Holds the unix time of the last update attempt.
    stamp: PathBuf,
    /// `yt-dlp -U` is killed after this long (a stalled network must not hang an analysis or export).
    update_timeout: Duration,
}

impl YtDlp {
    pub fn new(bin: PathBuf, stamp: PathBuf) -> Self {
        Self { bin, stamp, update_timeout: UPDATE_TIMEOUT }
    }

    pub fn with_update_timeout(mut self, timeout: Duration) -> Self {
        self.update_timeout = timeout;
        self
    }

    pub fn with_bin(mut self, bin: PathBuf) -> Self {
        self.bin = bin;
        self
    }

    fn run(&self, args: &[&str]) -> Result<Output, String> {
        tools::command(&self.bin.to_string_lossy())
            .args(args)
            .output()
            .map_err(|e| format!("yt-dlp tidak bisa dijalankan ({}): {e}. Pasang yt-dlp atau taruh yt-dlp.exe di folder data Cruxvid.", self.bin.display()))
    }

    /// `yt-dlp -U`; true when it succeeded in time.
    fn update(&self) -> bool {
        let Ok(mut child) = tools::command(&self.bin.to_string_lossy()).arg("-U").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return false };
        let deadline = Instant::now() + self.update_timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    }

    /// Updates at most once a day (an attempt counts, so an offline machine is not retried every call).
    fn daily_update(&self) {
        let now = unix_now();
        let last = std::fs::read_to_string(&self.stamp).ok().and_then(|s| s.trim().parse().ok());
        if update_due(last, now) {
            if let Some(dir) = self.stamp.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&self.stamp, now.to_string());
            self.update();
        }
    }

    /// Runs yt-dlp; when it fails in a way an older version explains, updates it (whatever the daily limit) and tries
    /// once more. A private, removed or live video fails the same way after an update, so those are reported at once.
    fn run_retry(&self, args: &[&str]) -> Result<Output, String> {
        self.daily_update();
        let first = self.run(args)?;
        if first.status.success() {
            return Ok(first);
        }
        if version_related(&first) && self.update() {
            let second = self.run(args)?;
            if second.status.success() {
                return Ok(second);
            }
            return Err(failure_message(&second));
        }
        Err(failure_message(&first))
    }

    /// Link -> Source (path = the URL) and Transcript: platform captions when there are some, else the audio
    /// is downloaded into `work` and handed to `transcribe`. Live streams are rejected before any download.
    pub fn resolve(&self, url: &str, work: &Path, transcribe: impl FnOnce(&Source) -> Result<Transcript, String>) -> Result<(Source, Transcript), String> {
        let info = self.info(url)?;
        let source = Source { path: url.to_string(), title: info.title, duration: info.duration, fps: info.fps, width: info.width, height: info.height };
        let captions = info.caption_lang.as_deref().and_then(|lang| self.captions(url, lang, work));
        let transcript = match captions {
            Some(t) => t,
            None => transcribe(&Source { path: self.audio(url, work)?.to_string_lossy().into_owned(), ..source.clone() })?,
        };
        Ok((source, transcript))
    }

    pub fn info(&self, url: &str) -> Result<LinkInfo, String> {
        parse_info(&self.run_retry(&["-J", "--no-playlist", "-f", FORMAT, "--", url])?.stdout)
    }

    /// Platform captions as a Transcript; `None` when there are none or fetching/reading them fails (then use audio).
    pub fn captions(&self, url: &str, lang: &str, dir: &Path) -> Option<Transcript> {
        std::fs::create_dir_all(dir).ok()?;
        let out = dir.join("cap");
        let args = ["--skip-download", "--write-subs", "--write-auto-subs", "--sub-langs", lang, "--sub-format", "vtt", "--no-playlist", "-o", &out.to_string_lossy(), "--", url];
        if !self.run(&args).ok()?.status.success() {
            return None;
        }
        let file = first_file(dir, "cap", Some("vtt"))?;
        let t = vtt::parse(&std::fs::read_to_string(file).ok()?, lang);
        (!t.words.is_empty()).then_some(t)
    }

    /// The audio track only, into `dir`.
    pub fn audio(&self, url: &str, dir: &Path) -> Result<PathBuf, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create folder: {e}"))?;
        let out = dir.join("audio.%(ext)s");
        self.run_retry(&["--no-playlist", "-f", "ba[protocol=https]/ba/b", "-o", &out.to_string_lossy(), "--", url])?;
        first_file(dir, "audio", None).ok_or_else(|| "yt-dlp downloaded no audio for this link.".to_string())
    }

    /// Downloads only `start..end` of the stream into `dir/<name>.mp4` (or `<name>.pad.mp4` after the fallback)
    /// and returns it; a cut already on disk is reused. `source_duration` caps the padded range.
    pub fn cut(&self, url: &str, start: f64, end: f64, source_duration: f64, dir: &Path, name: &str) -> Result<Cut, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create folder: {e}"))?;
        let want = end - start;
        let (ps, pe) = ((start - PAD_SECS).max(0.0), (end + PAD_SECS).min(source_duration));
        let exact = dir.join(format!("{name}.mp4"));
        let padded = dir.join(format!("{name}.pad.mp4"));
        if cut_is_good(&exact, want) {
            return Ok(Cut { path: exact, offset: 0.0 });
        }
        if cut_is_good(&padded, pe - ps) {
            return Ok(Cut { path: padded, offset: start - ps });
        }
        self.download_range(url, start, end, dir, &exact)?;
        if cut_is_good(&exact, want) {
            return Ok(Cut { path: exact, offset: 0.0 });
        }
        self.download_range(url, ps, pe, dir, &padded)?;
        if cut_is_good(&padded, pe - ps) {
            return Ok(Cut { path: padded, offset: start - ps });
        }
        let _ = std::fs::remove_file(&padded);
        Err("Potongan Clip dari link ini gagal diverifikasi (durasi atau suara tidak sesuai). Coba lagi.".into())
    }

    fn download_range(&self, url: &str, start: f64, end: f64, dir: &Path, out: &Path) -> Result<(), String> {
        let tmp = dir.join(format!("{}.tmp", out.file_stem().unwrap_or_default().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| format!("Cannot create folder: {e}"))?;
        let template = tmp.join("cut.%(ext)s");
        let section = format!("*{start:.3}-{end:.3}");
        let result = self.run_retry(&[
            "--no-playlist", "-f", FORMAT, "--merge-output-format", "mp4",
            "--download-sections", &section, "--downloader-args", FFMPEG_RANGED,
            "-o", &template.to_string_lossy(), "--", url,
        ]);
        let moved = result.and_then(|_| {
            let file = first_file(&tmp, "cut", None).ok_or("yt-dlp produced no file for this Clip.")?;
            let _ = std::fs::remove_file(out);
            std::fs::rename(file, out).map_err(|e| format!("Cannot save the cut: {e}"))
        });
        let _ = std::fs::remove_dir_all(&tmp);
        moved
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn update_due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|l| now.saturating_sub(l) >= DAY_SECS)
}

/// Failures an update can fix: the site changed (extraction, signatures) or rejects an old client (HTTP 400/403).
fn version_related(o: &Output) -> bool {
    let err = String::from_utf8_lossy(&o.stderr).to_lowercase();
    ["unable to extract", "please report this issue", "signature", "nsig", "player response", "http error 400", "http error 403", "update yt-dlp"].iter().any(|s| err.contains(s))
}

/// The most useful line of yt-dlp's complaint, in a user-readable sentence.
fn failure_message(o: &Output) -> String {
    let err = String::from_utf8_lossy(&o.stderr);
    let line = err.lines().rev().find(|l| l.contains("ERROR:")).or_else(|| err.lines().rev().find(|l| !l.trim().is_empty())).unwrap_or("unknown error");
    format!("Tidak bisa mengambil video dari link ini: {}", line.trim().trim_start_matches("ERROR:").trim())
}

/// Finds `<stem>.<ext>` style output of yt-dlp (its extension is not known upfront), ignoring partial files.
fn first_file(dir: &Path, stem: &str, ext: Option<&str>) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        name.starts_with(&format!("{stem}.")) && !name.ends_with(".part") && !name.ends_with(".ytdl") && ext.is_none_or(|e| name.ends_with(&format!(".{e}")))
    })
}

fn cut_is_good(path: &Path, want: f64) -> bool {
    path.is_file()
        && tools::command("ffprobe")
            .args(["-v", "error", "-show_entries", "stream=codec_type", "-show_entries", "format=duration", "-of", "json"])
            .arg(path)
            .output()
            .is_ok_and(|o| o.status.success() && cut_json_ok(&o.stdout, want))
}

/// Video and audio present and the length within a second of what was asked (spike finding 4).
fn cut_json_ok(json: &[u8], want: f64) -> bool {
    let Ok(j) = serde_json::from_slice::<Value>(json) else { return false };
    let has = |t: &str| j["streams"].as_array().is_some_and(|s| s.iter().any(|s| s["codec_type"] == t));
    let dur: Option<f64> = j["format"]["duration"].as_str().and_then(|d| d.parse().ok());
    has("video") && has("audio") && dur.is_some_and(|d| (d - want).abs() <= CUT_TOLERANCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(extra: &str) -> Result<LinkInfo, String> {
        parse_info(format!(r#"{{"title":"T","duration":600,"fps":30,"width":1280,"height":720{extra}}}"#).as_bytes())
    }

    #[test]
    fn only_http_links_are_links() {
        assert!(is_link(" https://youtu.be/x "));
        assert!(is_link("http://twitch.tv/videos/1"));
        for no in ["C:\\v\\a.mp4", "ftp://x", "-fsomething", "", "youtube.com/x"] {
            assert!(!is_link(no), "{no}");
        }
    }

    #[test]
    fn live_and_unfinished_streams_are_rejected_with_a_clear_message() {
        for extra in [r#","is_live":true"#, r#","live_status":"is_live""#, r#","live_status":"is_upcoming""#, r#","live_status":"post_live""#] {
            assert_eq!(info(extra).unwrap_err(), MSG_LIVE, "{extra}");
        }
        assert!(MSG_LIVE.contains("live"));
        assert!(info(r#","live_status":"was_live","is_live":false"#).is_ok(), "a finished stream is a normal Source");
        assert!(info(r#","live_status":"not_live""#).is_ok());
    }

    #[test]
    fn info_without_a_length_is_an_error_not_a_pass() {
        assert!(parse_info(br#"{"title":"x"}"#).is_err());
        assert!(parse_info(b"").is_err());
    }

    #[test]
    fn info_fields_and_defaults() {
        let i = parse_info(br#"{"title":"Talk","duration":61.5}"#).unwrap();
        assert_eq!((i.title.as_str(), i.duration, i.fps, i.caption_lang), ("Talk", 61.5, 30.0, None));
    }

    #[test]
    fn caption_language_prefers_hand_made_then_original_automatic() {
        let lang = |j: &str| pick_caption_lang(&serde_json::from_str(j).unwrap());
        let en = r#"[{"ext":"vtt"}]"#;
        assert_eq!(lang(&format!(r#"{{"language":"id","subtitles":{{"en":{en},"id":{en}}}}}"#)).as_deref(), Some("id"));
        assert_eq!(lang(&format!(r#"{{"subtitles":{{"fr":{en},"live_chat":{en}}}}}"#)).as_deref(), Some("fr"));
        assert_eq!(lang(&format!(r#"{{"subtitles":{{"live_chat":{en}}}}}"#)), None);
        assert_eq!(lang(&format!(r#"{{"language":"en","automatic_captions":{{"en":{en},"en-orig":{en},"de":{en}}}}}"#)).as_deref(), Some("en-orig"));
        assert_eq!(lang(&format!(r#"{{"automatic_captions":{{"en":{en}}}}}"#)).as_deref(), Some("en"));
        assert_eq!(lang(&format!(r#"{{"language":"id","automatic_captions":{{"en":{en}}}}}"#)), None, "no automatic captions in the spoken language");
        assert_eq!(lang("{}"), None);
    }

    #[test]
    fn update_is_due_once_a_day() {
        assert!(update_due(None, 5));
        assert!(!update_due(Some(1_000), 1_000 + DAY_SECS - 1));
        assert!(update_due(Some(1_000), 1_000 + DAY_SECS));
        assert!(!update_due(Some(5_000), 1_000), "clock set back: not due");
    }

    #[test]
    fn a_cut_needs_audio_video_and_the_right_length() {
        let j = |streams: &str, dur: &str| format!(r#"{{"streams":[{streams}],"format":{{"duration":"{dur}"}}}}"#);
        let v = r#"{"codec_type":"video"}"#;
        let a = r#"{"codec_type":"audio"}"#;
        assert!(cut_json_ok(j(&format!("{v},{a}"), "90.014").as_bytes(), 90.0));
        assert!(!cut_json_ok(j(&format!("{v},{a}"), "88.9").as_bytes(), 90.0));
        assert!(!cut_json_ok(j(v, "90.0").as_bytes(), 90.0), "no audio (spike finding 4)");
        assert!(!cut_json_ok(j(a, "90.0").as_bytes(), 90.0));
        assert!(!cut_json_ok(b"garbage", 90.0));
    }

    #[test]
    fn only_failures_an_update_can_fix_are_version_related() {
        let out = |e: &str| Output { status: std::process::ExitStatus::default(), stdout: vec![], stderr: e.as_bytes().to_vec() };
        assert!(version_related(&out("ERROR: [youtube] abc: Unable to extract player response; please report this issue")));
        assert!(version_related(&out("ERROR: unable to download video data: HTTP Error 403: Forbidden")));
        for no in ["ERROR: [youtube] abc: Private video. Sign in if you've been granted access", "ERROR: Video unavailable. This video has been removed", "ERROR: HTTP Error 404: Not Found", "ERROR: This live event will begin in 2 hours"] {
            assert!(!version_related(&out(no)), "{no}");
        }
    }

    #[test]
    fn failure_message_quotes_the_error_line() {
        let out = Output { status: std::process::ExitStatus::default(), stdout: vec![], stderr: b"[x] warn\nERROR: [twitch:stream] nasa: The channel is not currently live\n".to_vec() };
        assert_eq!(failure_message(&out), "Tidak bisa mengambil video dari link ini: [twitch:stream] nasa: The channel is not currently live");
    }
}
