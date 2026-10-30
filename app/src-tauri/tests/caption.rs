//! Seam: Caption style, word corrections and burn-in through the public Engine API (real ffmpeg, faked Gemini).
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::caption::SAFE_MARGIN_V;
use cruxvid_lib::engine::{Engine, EngineConfig};
use std::path::Path;
use std::process::Command;

fn engine(dir: &Path, gemini: &FakeGemini) -> Engine {
    Engine::new(EngineConfig {
        data_dir: dir.join("data"),
        output_dir: dir.join("out"),
        gemini_base_url: gemini.base_url.clone(),
        model: "test-model".into(),
    })
}

#[test]
fn presets_are_several_and_default_position_is_in_the_safe_zone() {
    let presets = Engine::caption_presets();
    assert!(presets.len() >= 3);
    assert!(presets.iter().all(|p| p.margin_v >= SAFE_MARGIN_V), "{presets:?}");
}

#[test]
fn style_and_word_corrections_persist_and_invalid_input_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let gemini = FakeGemini::start();
    let e = engine(tmp.path(), &gemini);
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let clip = &p.clips[0];

    let cap = e.caption(&p.id, &clip.id).unwrap();
    assert_eq!(cap.style, Engine::caption_presets()[0], "default is the first preset");
    assert!(cap.words.len() > 50 && cap.words.iter().all(|w| w.start >= clip.start && w.start < clip.end));

    let mut style = Engine::caption_presets()[1].clone();
    style.color = "#FF0000".into();
    style.margin_v = 700;
    e.set_caption_style(&p.id, &clip.id, style.clone()).unwrap();
    let bad = |s: cruxvid_lib::caption::CaptionStyle| e.set_caption_style(&p.id, &clip.id, s).unwrap_err();
    assert!(bad(cruxvid_lib::caption::CaptionStyle { color: "red".into(), ..style.clone() }).contains("color"));
    assert!(bad(cruxvid_lib::caption::CaptionStyle { size: 5, ..style.clone() }).contains("size"));

    let idx = cap.words[2].index;
    assert_eq!(e.correct_caption_word(&p.id, idx, "  Zulu ").unwrap(), "Zulu");
    // Empty correction is not saved: the word goes back to the original.
    let original = cap.words[2].original.clone();
    let again = engine(tmp.path(), &gemini); // fresh Engine: state came from disk
    let reloaded = again.caption(&p.id, &clip.id).unwrap();
    assert_eq!((reloaded.style.clone(), reloaded.words[2].text.as_str()), (style, "Zulu"));
    assert_eq!(again.correct_caption_word(&p.id, idx, "   ").unwrap(), original);
    assert_eq!(again.caption(&p.id, &clip.id).unwrap().words[2].text, original);
    assert!(again.correct_caption_word(&p.id, 999_999, "x").is_err());
}

/// Raw RGB 1080x1920 frame of an exported file.
fn frame(video: &Path, t: f64) -> Vec<u8> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &t.to_string(), "-i"])
        .arg(video)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .unwrap();
    assert_eq!(out.stdout.len(), 1080 * 1920 * 3, "{}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn mean_abs_diff(a: &[u8], b: &[u8], y0: usize, y1: usize) -> f64 {
    let (a, b) = (&a[y0 * 3240..y1 * 3240], &b[y0 * 3240..y1 * 3240]);
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as f64).sum::<f64>() / a.len() as f64
}

#[test]
fn export_burns_caption_with_highlight_and_the_corrected_word() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let clip = &p.clips[0];
    let first = e.caption(&p.id, &clip.id).unwrap().words[0].clone();
    let mut style = Engine::caption_presets()[0].clone();
    style.highlight = "#FF00FF".into();
    e.set_caption_style(&p.id, &clip.id, style).unwrap();
    let t = first.start - clip.start + 0.3; // inside the first word

    let before = e.export(&p.id, &clip.id).unwrap();
    let before_frame = frame(&before, t);
    let magenta = before_frame.chunks(3).skip(1300 * 1080).take(400 * 1080).filter(|p| p[0] > 230 && p[1] < 40 && p[2] > 230).count();
    assert!(magenta > 200, "highlight colour not burned in: {magenta} px");

    e.correct_caption_word(&p.id, first.index, "ZULU").unwrap();
    let after = e.export(&p.id, &clip.id).unwrap(); // same file name: overwrites
    let after_frame = frame(&after, t);
    let band = mean_abs_diff(&before_frame, &after_frame, 1000, 1800);
    let control = mean_abs_diff(&before_frame, &after_frame, 100, 500);
    assert!(band > control + 2.0, "corrected word not visible: band {band} control {control}");
}
