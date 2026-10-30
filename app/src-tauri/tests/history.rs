//! Seam: Project history through the public API: list saved Projects, reopen one with every edit and
//! the queue restored, with no Gemini call (the reopening Engine points at a dead Gemini address).
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::arrange::ClipOrder;
use cruxvid_lib::engine::{Engine, EngineConfig};
use cruxvid_lib::{history, queue::Status};
use std::path::Path;

fn engine(dir: &Path, base_url: &str) -> Engine {
    Engine::new(EngineConfig { data_dir: dir.join("data"), output_dir: dir.join("out"), gemini_base_url: base_url.into(), model: "test-model".into() })
}

#[test]
fn reopening_restores_every_edit_without_calling_gemini() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start().base_url);
    let p = e.analyze(&fixtures().join("face.mp4")).unwrap();
    let clip = p.clips[0].id.clone();

    e.edit_clip_text(&p.id, &clip, "My own title", "My own description").unwrap();
    let mut style = Engine::caption_presets()[1].clone();
    style.color = "#FF0000".into();
    e.set_caption_style(&p.id, &clip, style.clone()).unwrap();
    let idx = e.caption(&p.id, &clip).unwrap().words[2].index;
    e.correct_caption_word(&p.id, idx, "Zulu").unwrap();
    e.set_layout_preset(&p.id, &clip, "full").unwrap();
    history::save_view(&e, &p.id, ClipOrder::Hook, 80).unwrap();
    e.enqueue_exports(&p.id, &[clip.clone()]).unwrap();
    // The app closed mid-export.
    let file = tmp.path().join("data/projects").join(&p.id).join("queue.json");
    std::fs::write(&file, std::fs::read_to_string(&file).unwrap().replace("\"waiting\"", "\"running\"")).unwrap();

    // "Close and reopen": a new Engine whose Gemini address refuses connections.
    let reopened = engine(tmp.path(), "http://127.0.0.1:1");
    let list = history::list(&reopened);
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].id.as_str(), list[0].title.as_str(), list[0].damaged), (p.id.as_str(), "face", false));
    assert!(list[0].created_ms > 1_700_000_000_000, "date comes from the Project id");

    let opened = history::open(&reopened, &p.id).unwrap();
    let c = opened.project.clips.iter().find(|c| c.id == clip).unwrap();
    assert_eq!((c.title.as_str(), c.description.as_str()), ("My own title", "My own description"));
    assert_eq!(opened.project.transcript, p.transcript);
    assert_eq!((opened.view.order, opened.view.threshold), (ClipOrder::Hook, 80));
    assert_eq!(opened.jobs.iter().map(|j| (j.clip_id.as_str(), j.status)).collect::<Vec<_>>(), [(clip.as_str(), Status::Waiting)]);
    let cap = reopened.caption(&p.id, &clip).unwrap();
    assert_eq!((cap.style, cap.words[2].text.as_str()), (style, "Zulu"));
    assert_eq!(reopened.clip_layout(&p.id, &clip).unwrap().preset, "full");
}

#[test]
fn a_new_project_opens_with_default_order_and_threshold() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start().base_url);
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let v = history::open(&e, &p.id).unwrap().view;
    assert_eq!((v.order, v.threshold), (ClipOrder::Timeline, 50));
}

#[test]
fn a_damaged_project_is_listed_and_fails_to_open_without_hiding_the_others() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start().base_url);
    let good = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let bad_dir = tmp.path().join("data/projects/p1700000000000");
    std::fs::create_dir_all(&bad_dir).unwrap();
    std::fs::write(bad_dir.join("project.json"), "{ not json").unwrap();
    std::fs::create_dir_all(tmp.path().join("data/projects/empty-folder")).unwrap();

    let list = history::list(&e);
    assert_eq!(list.iter().map(|s| (s.id.as_str(), s.damaged)).collect::<Vec<_>>(), [(good.id.as_str(), false), ("p1700000000000", true)]);
    assert!(history::open(&e, "p1700000000000").unwrap_err().contains("damaged"));
    assert!(history::open(&e, &good.id).is_ok());
    assert!(history::open(&e, "../evil").is_err());
}
