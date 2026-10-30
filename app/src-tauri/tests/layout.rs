//! Seam 1 for the Layout engine: real YuNet/ONNX and real ffmpeg on the face fixtures, faked Gemini only.
//! Exported videos are checked by running face detection on them again: where the face lands is the Layout.
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::engine::{Engine, EngineConfig};
use cruxvid_lib::{faces, source};
use std::path::{Path, PathBuf};

fn engine(dir: &Path, gemini: &FakeGemini) -> Engine {
    Engine::new(EngineConfig {
        data_dir: dir.join("data"),
        output_dir: dir.join("out"),
        gemini_base_url: gemini.base_url.clone(),
        model: "test-model".into(),
    })
}

/// Face centre of an exported file, in 0..1 of the 1080x1920 frame, and its width.
fn face_in(video: &Path) -> (f64, f64, f64) {
    let s = source::probe_local(video).unwrap();
    assert_eq!((s.width, s.height), (1080, 1920));
    let faces = faces::detect_clip_faces(&s, 0.5, 2.0).unwrap();
    assert_eq!(faces.len(), 1, "exactly one face in the exported frame: {faces:?}");
    let f = faces[0];
    (f.x + f.w / 2.0, f.y + f.h / 2.0, f.w)
}

fn first_clip(e: &Engine, source: &str) -> (String, String) {
    let p = e.analyze(&fixtures().join(source)).unwrap();
    (p.id, p.clips[0].id.clone())
}

#[test]
fn one_face_clip_starts_in_focus_on_that_face_and_export_follows() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let (pid, cid) = first_clip(&e, "face.mp4");

    let l = e.clip_layout(&pid, &cid).unwrap();

    assert_eq!((l.preset.as_str(), l.faces, l.notice.as_deref()), ("focus", 1, None));
    let face = l.face.unwrap();
    assert!((face.x + face.w / 2.0 - 0.29).abs() < 0.04, "face is left of centre: {face:?}");
    // The Fokus window is centred on the face (not on the frame centre).
    let c = l.panes[0].crop;
    assert!(((c.x + c.w / 2.0) - (face.x + face.w / 2.0)).abs() < 0.01);

    let out: PathBuf = e.export(&pid, &cid).unwrap();
    let (cx, cy, w) = face_in(&out);
    assert!((cx - 0.5).abs() < 0.08, "face centred horizontally in the export, got {cx}");
    assert!(cy > 0.1 && cy < 0.5 && w > 0.1, "{cy} {w}");
}

#[test]
fn stream_with_webcam_box_switches_to_split_person_and_game_and_export_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let (pid, cid) = first_clip(&e, "webcam.mp4");
    let auto = e.clip_layout(&pid, &cid).unwrap();
    assert_eq!((auto.preset.as_str(), auto.faces), ("focus", 1));

    let l = e.set_layout_preset(&pid, &cid, "webcam_game").unwrap();
    assert_eq!(l.preset, "webcam_game");
    assert_eq!(l.panes.len(), 2);
    assert_eq!(l.face, auto.face, "switching preset reuses the detection");
    let moved = e.shift_layout_pane(&pid, &cid, 0, 0.01, 0.0).unwrap();
    assert!((moved.panes[0].crop.x - l.panes[0].crop.x - 0.01).abs() < 1e-9 || moved.panes[0].crop.x + moved.panes[0].crop.w >= 1.0 - 1e-9);

    let out = e.export(&pid, &cid).unwrap();

    // The face is now in the webcam box on top: upper third, large (3 face-widths fill the 1080 width).
    let (_, cy, w) = face_in(&out);
    assert!(cy < 640.0 / 1920.0, "face in the top webcam box, got {cy}");
    assert!(w > 0.2, "face fills the webcam box, got {w}");
    // The game pane below is the stream, not a second face: exactly one face was found above.
}

#[test]
fn clip_without_a_face_uses_full_with_a_notice_and_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let (pid, cid) = first_clip(&e, "source.mp4");

    let l = e.clip_layout(&pid, &cid).unwrap();

    assert_eq!((l.preset.as_str(), l.faces, l.notice.as_deref()), ("full", 0, Some("no_face")));
    let out = e.export(&pid, &cid).unwrap();
    let s = source::probe_local(&out).unwrap();
    assert_eq!((s.width, s.height), (1080, 1920));
}

#[test]
fn layout_edits_survive_reopening_the_project() {
    let tmp = tempfile::tempdir().unwrap();
    let gemini = FakeGemini::start();
    let (pid, cid) = first_clip(&engine(tmp.path(), &gemini), "face.mp4");
    let e = engine(tmp.path(), &gemini);
    e.set_layout_preset(&pid, &cid, "stack").unwrap();
    let shifted = e.shift_layout_pane(&pid, &cid, 1, 0.02, 0.0).unwrap();

    let reopened = engine(tmp.path(), &gemini).clip_layout(&pid, &cid).unwrap();

    assert_eq!(reopened, shifted);
    assert!(e.set_layout_preset(&pid, &cid, "nope").is_err());
    assert!(e.clip_layout(&pid, "missing").is_err());
}

#[test]
fn preview_frame_is_a_jpeg_of_the_clip() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let (pid, cid) = first_clip(&e, "face.mp4");
    let jpg = e.clip_frame(&pid, &cid).unwrap();
    assert_eq!(&jpg[..3], &[0xFF, 0xD8, 0xFF]);
}
