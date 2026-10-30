//! Seam: title/description per Clip through the Engine: present after analysis, manual edits persist in the
//! Project, Regenerate replaces one field of one Clip, and a Gemini failure leaves the old text intact.
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::{clip_text::TextField, engine::{Engine, EngineConfig}};
use std::path::Path;

fn engine(dir: &Path, base_url: &str) -> Engine {
    Engine::new(EngineConfig { data_dir: dir.join("data"), output_dir: dir.join("out"), gemini_base_url: base_url.into(), model: "test-model".into() })
}

#[test]
fn every_clip_has_title_and_description_after_analysis() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path(), &FakeGemini::start().base_url).analyze(&fixtures().join("source.mp4")).unwrap();
    assert!(!p.clips.is_empty());
    assert!(p.clips.iter().all(|c| !c.title.is_empty() && !c.description.is_empty()), "{:?}", p.clips);
}

#[test]
fn manual_edit_is_saved_in_the_project_and_touches_only_that_clip() {
    let tmp = tempfile::tempdir().unwrap();
    let g = FakeGemini::start();
    let e = engine(tmp.path(), &g.base_url);
    let before = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let id = &before.clips[1].id;

    let clip = e.edit_clip_text(&before.id, id, "  My own title ", "My own description").unwrap();
    assert_eq!((clip.title.as_str(), clip.description.as_str()), ("My own title", "My own description"));

    let reopened = engine(tmp.path(), &g.base_url).load_project(&before.id).unwrap();
    for (a, b) in before.clips.iter().zip(&reopened.clips) {
        if &a.id == id {
            assert_eq!((b.title.as_str(), b.description.as_str()), ("My own title", "My own description"));
        } else {
            assert_eq!(a, b);
        }
    }
    assert!(e.edit_clip_text(&before.id, id, "   ", "x").is_err(), "a Clip needs a title");
    assert!(e.edit_clip_text(&before.id, "nope", "t", "d").unwrap_err().contains("not found"));
}

#[test]
fn regenerate_replaces_only_the_asked_field_of_only_that_clip() {
    let tmp = tempfile::tempdir().unwrap();
    let g = FakeGemini::start();
    let e = engine(tmp.path(), &g.base_url);
    let before = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let id = &before.clips[0].id;
    e.edit_clip_text(&before.id, id, "Kept title", &before.clips[0].description).unwrap();

    let clip = e.regenerate_clip_text(&before.id, id, TextField::Description).unwrap();

    assert_eq!((clip.title.as_str(), clip.description.as_str()), ("Kept title", "Regenerated description for the clip."));
    let after = e.load_project(&before.id).unwrap();
    assert_eq!(after.clips[0], clip);
    assert_eq!(after.clips[1..], before.clips[1..]);
    let t = e.regenerate_clip_text(&before.id, id, TextField::Title).unwrap();
    assert_eq!(t.title, "Regenerated title");
}

#[test]
fn regenerate_without_network_reports_an_error_and_keeps_the_old_text() {
    let tmp = tempfile::tempdir().unwrap();
    let before = engine(tmp.path(), &FakeGemini::start().base_url).analyze(&fixtures().join("source.mp4")).unwrap();
    let dead = engine(tmp.path(), "http://127.0.0.1:9"); // nothing listens on the discard port

    let err = dead.regenerate_clip_text(&before.id, &before.clips[0].id, TextField::Description).unwrap_err();

    assert!(err.contains("Jaringan"), "{err}");
    assert_eq!(dead.load_project(&before.id).unwrap(), before);
}

#[test]
fn an_edit_and_a_trim_made_while_regenerate_waits_for_gemini_are_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let before = engine(tmp.path(), &FakeGemini::start().base_url).analyze(&fixtures().join("source.mp4")).unwrap();
    let slow = std::sync::Arc::new(engine(tmp.path(), &FakeGemini::start_slow(std::time::Duration::from_millis(1500)).base_url));
    let first = before.clips[0].clone();

    let worker = {
        let (e, pid, id) = (slow.clone(), before.id.clone(), first.id.clone());
        std::thread::spawn(move || e.regenerate_clip_text(&pid, &id, TextField::Description).unwrap())
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    slow.edit_clip_text(&before.id, &first.id, "Edited meanwhile", "x").unwrap();
    cruxvid_lib::clip_range::set_range(&slow, &before.id, &first.id, first.start + 1.0, first.end).unwrap();
    worker.join().unwrap();

    let c = &slow.load_project(&before.id).unwrap().clips[0];
    assert_eq!((c.title.as_str(), c.description.as_str(), c.start), ("Edited meanwhile", "Regenerated description for the clip.", first.start + 1.0));
}
