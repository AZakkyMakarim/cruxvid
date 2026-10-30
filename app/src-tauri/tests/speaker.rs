//! Seam 1 for active-speaker follow: real YuNet/ONNX and real ffmpeg on the two-person fixture, faked Gemini only.
//! Fixture: two_faces.mp4 (faces at about 0.26 and 0.75 of the width); gemini_speakers_response.json says the left
//! person talks 0-18 s, the right 18-40 s, someone off camera 40-46 s (plus a 0.5 s interjection), right to 60 s, left after.
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::engine::{Engine, EngineConfig};
use cruxvid_lib::{faces, source};
use std::path::Path;

fn engine(dir: &Path, gemini_url: &str) -> Engine {
    Engine::new(EngineConfig {
        data_dir: dir.join("data"),
        output_dir: dir.join("out"),
        gemini_base_url: gemini_url.into(),
        model: "test-model".into(),
    })
}

fn first_clip(e: &Engine) -> (String, String) {
    let p = e.analyze(&fixtures().join("two_faces.mp4")).unwrap();
    (p.id, p.clips[0].id.clone())
}

/// Horizontal centre (0..1) of the single face in an exported frame at `t` seconds, None when not exactly one face.
fn face_x_at(video: &Path, t: f64) -> Option<f64> {
    let s = source::probe_local(video).unwrap();
    let f = faces::detect_clip_faces(&s, t - 0.2, t + 0.2).unwrap();
    (f.len() == 1).then(|| f[0].x + f[0].w / 2.0)
}

#[test]
fn two_faces_start_in_focus_and_the_crop_follows_who_speaks() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start().base_url);
    let (pid, cid) = first_clip(&e);

    let l = e.clip_layout(&pid, &cid).unwrap();

    assert_eq!((l.preset.as_str(), l.faces, l.notice.as_deref()), ("focus", 2, Some("follow_speaker")));
    let centre = |t: f64| {
        let c = l.at(t).panes[0].crop;
        c.x + c.w / 2.0
    };
    // Left speaker, right speaker from 18 s (eased), off-camera turn and interjection keep the right crop, left again at 60 s.
    assert!((centre(9.0) - 0.26).abs() < 0.04, "{}", centre(9.0));
    assert!(centre(18.4) > 0.3 && centre(18.4) < 0.7, "moving at {}", centre(18.4));
    assert!((centre(30.0) - 0.75).abs() < 0.04 && (centre(43.0) - 0.75).abs() < 0.04 && (centre(46.2) - 0.75).abs() < 0.04);
    assert!((centre(70.0) - 0.26).abs() < 0.04);
    assert_eq!(e.clip_layout(&pid, &cid).unwrap(), l, "saved: no second Gemini call or detection needed");

    // Export: whoever speaks is the centred face of the 9:16 video; a fixed crop could only centre one of them.
    let out = e.export(&pid, &cid).unwrap();
    for t in [9.0, 30.0, 43.0, 70.0] {
        let x = face_x_at(&out, t).unwrap_or_else(|| panic!("one face at {t} s"));
        assert!((x - 0.5).abs() < 0.08, "face at {x} when {t} s");
    }
}

#[test]
fn manual_correction_still_applies_on_top_of_the_followed_crop() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start().base_url);
    let (pid, cid) = first_clip(&e);
    let auto = e.clip_layout(&pid, &cid).unwrap();

    let moved = e.shift_layout_pane(&pid, &cid, 0, 0.05, 0.0).unwrap();
    let cx = |l: &cruxvid_lib::layout::Layout, t: f64| l.at(t).panes[0].crop.x;
    assert!((cx(&moved, 9.0) - cx(&auto, 9.0) - 0.05).abs() < 1e-9 && (cx(&moved, 30.0) - cx(&auto, 30.0) - 0.05).abs() < 1e-9);

    let out = e.export(&pid, &cid).unwrap();
    // The window moved right by 5% of the width (96 px of 608), so the speaker sits left of centre in the video.
    let x = face_x_at(&out, 9.0).unwrap();
    assert!(x < 0.42 && x > 0.25, "face at {x}");

    // Choosing the split by hand still works, and Focus brings the following back.
    let stack = e.set_layout_preset(&pid, &cid, "stack").unwrap();
    assert_eq!((stack.preset.as_str(), stack.panes.len(), stack.notice), ("stack", 2, None));
    assert_eq!(e.set_layout_preset(&pid, &cid, "focus").unwrap().follow, auto.follow);
}

#[test]
fn without_usable_speaker_info_two_faces_fall_back_to_full_with_the_notice() {
    let tmp = tempfile::tempdir().unwrap();
    let (pid, cid) = first_clip(&engine(tmp.path(), &FakeGemini::start().base_url));

    // Same Project, but nothing listens at this Gemini address, so asking for the turns fails.
    let l = engine(tmp.path(), "http://127.0.0.1:9").clip_layout(&pid, &cid).unwrap();

    assert_eq!((l.preset.as_str(), l.faces, l.notice.as_deref()), ("full", 2, Some("multiple_faces")));
    assert!(l.follow.is_empty());

    // The fallback is not a user choice: once Gemini answers, the same Clip follows the speaker.
    let working = engine(tmp.path(), &FakeGemini::start().base_url);
    let again = working.clip_layout(&pid, &cid).unwrap();
    assert_eq!((again.preset.as_str(), again.notice.as_deref()), ("focus", Some("follow_speaker")));
}

#[test]
fn a_layout_the_user_picked_after_a_fallback_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let (pid, cid) = first_clip(&engine(tmp.path(), &FakeGemini::start().base_url));
    let offline = engine(tmp.path(), "http://127.0.0.1:9");
    offline.set_layout_preset(&pid, &cid, "stack").unwrap();

    let later = engine(tmp.path(), &FakeGemini::start().base_url).clip_layout(&pid, &cid).unwrap();

    assert_eq!((later.preset.as_str(), later.notice), ("stack", None));
}
