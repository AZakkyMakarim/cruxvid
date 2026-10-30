//! Seam 1 for cruxvid-b2t.8: a link Source through the public Engine API, with a fake yt-dlp executable
//! (examples/fake_ytdlp.rs, serves shared/fixtures/source.mp4) and the fake Gemini. No real network, no real yt-dlp.
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::engine::{Engine, EngineConfig};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const URL: &str = "https://www.youtube.com/watch?v=fake";
const CAPTIONS: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nHello from the captions.\n\n00:00:04.000 --> 00:00:06.000\nSecond line here.\n";

struct Fake {
    exe: PathBuf,
}

impl Fake {
    /// `extra` is merged into the fake's config (see examples/fake_ytdlp.rs).
    fn new(dir: &Path, extra: Value) -> Self {
        let fake_dir = dir.join("fake");
        std::fs::create_dir_all(&fake_dir).unwrap();
        let built = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("examples").join("fake_ytdlp.exe");
        let exe = fake_dir.join("yt-dlp.exe");
        std::fs::copy(&built, &exe).unwrap_or_else(|e| panic!("build the example first (`cargo test` does): {built:?}: {e}"));
        let vtt = fake_dir.join("captions.vtt");
        std::fs::write(&vtt, CAPTIONS).unwrap();
        let mut cfg = json!({ "fixture": fixtures().join("source.mp4"), "captions": vtt });
        for (k, v) in extra.as_object().unwrap() {
            cfg[k] = v.clone();
        }
        if cfg["captions"].is_null() {
            cfg.as_object_mut().unwrap().remove("captions");
        }
        std::fs::write(exe.with_extension("json"), cfg.to_string()).unwrap();
        Fake { exe }
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.exe.with_extension("state").join("calls.log")).unwrap_or_default().lines().map(String::from).collect()
    }

    fn count(&self, what: &str) -> usize {
        self.calls().iter().filter(|c| c.contains(what)).count()
    }
}

fn engine(dir: &Path, gemini: &FakeGemini, fake: &Fake) -> Engine {
    Engine::new(EngineConfig {
        data_dir: dir.join("data"),
        output_dir: dir.join("out"),
        gemini_base_url: gemini.base_url.clone(),
        model: "test-model".into(),
    })
    .with_ytdlp(fake.exe.clone())
}

/// Pretend yt-dlp was already updated today, so only a failure can trigger an update.
fn mark_updated_today(dir: &Path) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(dir.join("data").join("ytdlp-update.txt"), now.to_string()).unwrap();
}

fn duration(p: &Path) -> f64 {
    let o = Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"]).arg(p).output().unwrap();
    String::from_utf8_lossy(&o.stdout).trim().parse().unwrap()
}

#[test]
fn link_with_captions_gets_a_transcript_from_them_and_clips() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({}));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);

    let p = e.analyze(Path::new(URL)).unwrap();

    assert_eq!((p.source.title.as_str(), p.source.path.as_str(), p.source.duration), ("Fake VOD", URL, 600.0));
    let words: Vec<_> = p.transcript.words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(words, ["Hello", "from", "the", "captions.", "Second", "line", "here."]);
    assert_eq!(p.transcript.language, "en");
    assert_eq!(p.clips.len(), 4, "Clips come from the Gemini analysis");
    assert_eq!(fake.count("--download-sections"), 0, "analysis downloads no video");
    assert_eq!(fake.count("-f ba"), 0, "captions exist, so no audio download");
    assert_eq!(e.load_project(&p.id).unwrap(), p);
}

#[test]
fn link_without_captions_is_transcribed_from_its_audio() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({ "captions": null }));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);

    let p = e.analyze(Path::new(URL)).unwrap();

    assert_eq!(p.transcript.words.len(), 600, "the fake Gemini's transcript");
    assert_eq!(fake.count("-f ba"), 1);
    assert_eq!(p.clips.len(), 4);
}

#[test]
fn export_of_a_link_clip_downloads_only_its_range_and_is_9x16() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({}));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);
    let p = e.analyze(Path::new(URL)).unwrap();
    let clip = &p.clips[0];

    let out = e.export(&p.id, &clip.id).unwrap();

    assert_eq!(out.file_name().unwrap(), "Fake VOD_01_First topic.mp4");
    assert!((duration(&out) - (clip.end - clip.start)).abs() < 0.3, "{}", duration(&out));
    let cuts: Vec<_> = fake.calls().into_iter().filter(|c| c.contains("--download-sections")).collect();
    assert_eq!(cuts.len(), 1);
    let c = &cuts[0];
    assert!(c.contains("--download-sections *45.000-124.400"), "{c}");
    assert!(c.contains("protocol=https"), "format pinned to https: {c}");
    assert!(c.contains("ffmpeg_i:-multiple_requests 1 -request_size 2000000"), "ranged ffmpeg requests: {c}");
    assert!(c.ends_with(URL), "{c}");
    // Exporting again reuses the cut.
    e.export(&p.id, &clip.id).unwrap();
    assert_eq!(fake.count("--download-sections"), 1);
}

#[test]
fn a_bad_cut_is_retried_with_a_padded_range_and_export_still_starts_at_the_clip() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({ "bad_first_cut": true }));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);
    let p = e.analyze(Path::new(URL)).unwrap();
    let clip = &p.clips[0];

    let out = e.export(&p.id, &clip.id).unwrap();

    let cuts: Vec<_> = fake.calls().into_iter().filter(|c| c.contains("--download-sections")).collect();
    assert_eq!(cuts.len(), 2, "{cuts:?}");
    assert!(cuts[0].contains("*45.000-124.400"));
    assert!(cuts[1].contains("*35.000-134.400"), "10 s padding each side: {}", cuts[1]);
    assert!((duration(&out) - (clip.end - clip.start)).abs() < 0.3, "{}", duration(&out));
}

#[test]
fn a_cut_that_never_verifies_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({ "bad_cuts": true }));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);
    let p = e.analyze(Path::new(URL)).unwrap();

    let err = e.export(&p.id, &p.clips[0].id).unwrap_err();

    assert!(err.contains("gagal diverifikasi"), "{err}");
    assert_eq!(fake.count("--download-sections"), 2, "exact range, then padded, then give up");
}

#[test]
fn a_live_link_is_rejected_before_anything_is_downloaded() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({ "live": true }));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);

    let err = e.analyze(Path::new("https://www.twitch.tv/somechannel")).unwrap_err();

    assert!(err.contains("live"), "{err}");
    assert_eq!(fake.count("--download-sections") + fake.count("--skip-download") + fake.count("-f ba"), 0);
    assert!(e.estimate_tokens(Path::new(URL)).unwrap_err().contains("live"), "also rejected at the token estimate");
}

#[test]
fn yt_dlp_is_updated_at_most_once_a_day() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({}));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);

    e.analyze(Path::new(URL)).unwrap();
    e.analyze(Path::new(URL)).unwrap();

    assert_eq!(fake.count("-U"), 1);
    assert!(fake.calls()[0] == "-U", "the update runs before the first extraction: {:?}", fake.calls());
}

#[test]
fn failed_extraction_updates_yt_dlp_and_retries_once() {
    let tmp = tempfile::tempdir().unwrap();
    mark_updated_today(tmp.path());
    let fake = Fake::new(tmp.path(), json!({ "broken_until_update": true }));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);

    let p = e.analyze(Path::new(URL)).unwrap();

    assert_eq!(p.source.title, "Fake VOD");
    let calls = fake.calls();
    assert!(calls[0].starts_with("-J") && calls[1] == "-U" && calls[2].starts_with("-J"), "{calls:?}");
}

#[test]
fn extraction_that_stays_broken_gives_a_clear_error_after_one_retry() {
    let tmp = tempfile::tempdir().unwrap();
    mark_updated_today(tmp.path());
    let fake = Fake::new(tmp.path(), json!({ "always_broken": true }));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);

    let err = e.analyze(Path::new(URL)).unwrap_err();

    assert!(err.contains("Unable to extract player response"), "{err}");
    assert_eq!(fake.calls().len(), 3, "one try, one update, one retry: {:?}", fake.calls());
}

#[test]
fn a_missing_yt_dlp_is_explained() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({}));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake).with_ytdlp(tmp.path().join("nope").join("yt-dlp.exe"));
    let err = e.analyze(Path::new(URL)).unwrap_err();
    assert!(err.contains("yt-dlp tidak bisa dijalankan"), "{err}");
}

#[test]
fn token_estimate_for_a_link_uses_its_length() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = Fake::new(tmp.path(), json!({}));
    let e = engine(tmp.path(), &FakeGemini::start(), &fake);
    assert_eq!(e.estimate_tokens(Path::new(URL)).unwrap().duration, 600.0);
}
