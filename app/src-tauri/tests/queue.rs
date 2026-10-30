//! Seam: the Export queue and output folder through the public Engine API (real ffmpeg, faked Gemini).
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::engine::{Engine, EngineConfig};
use cruxvid_lib::queue::Status;
use std::path::Path;

fn engine(dir: &Path, gemini: &FakeGemini) -> Engine {
    Engine::new(EngineConfig {
        data_dir: dir.join("data"),
        output_dir: dir.join("out"),
        gemini_base_url: gemini.base_url.clone(),
        model: "test-model".into(),
    })
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<_> = std::fs::read_dir(dir).map(|d| d.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

#[test]
fn three_clips_export_in_order_one_fails_and_retry_reruns_only_it() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let ids: Vec<String> = p.clips.iter().take(3).map(|c| c.id.clone()).collect();
    e.fail_once(vec![ids[1].clone()]);

    let queued = e.enqueue_exports(&p.id, &ids).unwrap();
    assert!(queued.iter().all(|j| j.status == Status::Waiting));
    e.run_export_queue(&p.id).unwrap();

    let jobs = e.export_jobs(&p.id).unwrap();
    let st: Vec<_> = jobs.iter().map(|j| (j.clip_id.as_str(), j.status)).collect();
    assert_eq!(st, [(ids[0].as_str(), Status::Done), (ids[1].as_str(), Status::Failed), (ids[2].as_str(), Status::Done)]);
    assert!(jobs[1].error.as_deref().is_some_and(|m| !m.is_empty()));
    assert_eq!(files(&tmp.path().join("out")), ["source_01_First topic.mp4", "source_03_Fourth topic.mp4"]);
    let first = std::fs::metadata(tmp.path().join("out/source_01_First topic.mp4")).unwrap().modified().unwrap();

    let retried = e.retry_failed_exports(&p.id).unwrap();
    assert_eq!(retried.iter().map(|j| j.status).collect::<Vec<_>>(), [Status::Done, Status::Waiting, Status::Done]);
    e.run_export_queue(&p.id).unwrap();

    assert!(e.export_jobs(&p.id).unwrap().iter().all(|j| j.status == Status::Done));
    assert_eq!(files(&tmp.path().join("out")).len(), 3);
    let after = std::fs::metadata(tmp.path().join("out/source_01_First topic.mp4")).unwrap().modified().unwrap();
    assert_eq!(first, after, "successful jobs are not exported again");
}

#[test]
fn output_folder_is_chosen_persisted_and_opened() {
    let tmp = tempfile::tempdir().unwrap();
    let gemini = FakeGemini::start();
    let e = engine(tmp.path(), &gemini);
    assert_eq!(e.output_dir(), tmp.path().join("out"));
    assert!(e.set_output_dir("  ").is_err());
    assert!(e.set_output_dir("relative/dir").is_err());

    let chosen = tmp.path().join("chosen");
    e.set_output_dir(chosen.to_str().unwrap()).unwrap();
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    e.enqueue_exports(&p.id, &[p.clips[0].id.clone()]).unwrap();
    e.run_export_queue(&p.id).unwrap();
    assert_eq!(files(&chosen), ["source_01_First topic.mp4"]);

    assert_eq!(engine(tmp.path(), &gemini).output_dir(), chosen, "setting survives a restart");

    let record = tmp.path().join("opened.txt");
    e.open_output_dir(Some(&record)).unwrap();
    assert_eq!(std::fs::read_to_string(&record).unwrap().trim(), chosen.to_string_lossy());
}

#[test]
fn jobs_left_running_by_a_closed_app_come_back_as_waiting() {
    let tmp = tempfile::tempdir().unwrap();
    let gemini = FakeGemini::start();
    let e = engine(tmp.path(), &gemini);
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let ids: Vec<String> = p.clips.iter().take(2).map(|c| c.id.clone()).collect();
    e.enqueue_exports(&p.id, &ids).unwrap();
    // Simulate the app closing mid-export: the file says running.
    let file = tmp.path().join("data/projects").join(&p.id).join("queue.json");
    let text = std::fs::read_to_string(&file).unwrap().replacen("\"waiting\"", "\"running\"", 1);
    std::fs::write(&file, text).unwrap();

    let reopened = engine(tmp.path(), &gemini);
    let st: Vec<_> = reopened.export_jobs(&p.id).unwrap().iter().map(|j| j.status).collect();
    assert_eq!(st, [Status::Waiting, Status::Waiting]);
}
