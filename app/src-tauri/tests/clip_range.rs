//! Seam for cruxvid-b2t.17: manual Clips, trimming and preview through the public API, with no Gemini
//! (the Engine points at an address nothing listens on) and no API key.
mod common;

use common::fixtures;
use cruxvid_lib::clip_range::{self, RangeWarning};
use cruxvid_lib::engine::{Engine, EngineConfig};
use cruxvid_lib::layout::{Key, Layout};
use std::path::Path;

fn engine(dir: &Path) -> Engine {
    Engine::new(EngineConfig { data_dir: dir.join("data"), output_dir: dir.join("out"), gemini_base_url: "http://127.0.0.1:1".into(), model: "m".into() })
}

fn duration(file: &Path) -> f64 {
    let out = std::process::Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"]).arg(file).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[test]
fn a_manual_clip_is_trimmed_and_exported_without_a_key_or_analysis() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let p = clip_range::new_manual_project(&e, &fixtures().join("source.mp4")).unwrap();
    assert!(p.clips.is_empty() && p.transcript.words.is_empty());

    // 90 s Clip, no warning.
    let added = clip_range::add_manual_clip(&e, &p.id, 100.0, 190.0).unwrap();
    assert_eq!(added.warning, None);
    // Start 5 s earlier.
    let edit = clip_range::set_range(&e, &p.id, &added.clip_id, 95.0, 190.0).unwrap();
    assert_eq!((edit.clip.start, edit.clip.end, edit.warning), (95.0, 190.0, None));

    // Persisted in the Project, so a reopened Project has the trimmed range.
    let reopened = engine(tmp.path()).load_project(&p.id).unwrap();
    assert_eq!((reopened.clips[0].start, reopened.clips[0].end), (95.0, 190.0));

    let file = e.export(&p.id, &added.clip_id).unwrap();
    assert!((duration(&file) - 95.0).abs() < 0.2, "export duration follows the final range");
}

#[test]
fn a_range_outside_60_to_120_seconds_warns_but_still_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let p = clip_range::new_manual_project(&e, &fixtures().join("source.mp4")).unwrap();
    let short = clip_range::add_manual_clip(&e, &p.id, 10.0, 20.0).unwrap();
    assert_eq!(short.warning, Some(RangeWarning::TooShort));
    let long = clip_range::set_range(&e, &p.id, &short.clip_id, 10.0, 150.0).unwrap();
    assert_eq!(long.warning, Some(RangeWarning::TooLong));
    let file = e.export(&p.id, &short.clip_id).unwrap();
    assert!((duration(&file) - 140.0).abs() < 0.2);
}

#[test]
fn manual_clips_are_numbered_in_timeline_order_and_get_their_own_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let p = clip_range::new_manual_project(&e, &fixtures().join("source.mp4")).unwrap();
    let late = clip_range::add_manual_clip(&e, &p.id, 300.0, 380.0).unwrap();
    let early = clip_range::add_manual_clip(&e, &p.id, 10.0, 90.0).unwrap();
    assert_ne!(late.clip_id, early.clip_id);
    let clips = &early.project.clips;
    assert_eq!(clips.iter().map(|c| (c.id.as_str(), c.number)).collect::<Vec<_>>(), [(early.clip_id.as_str(), 1), (late.clip_id.as_str(), 2)]);
    assert_eq!(clips[0].hook_score, 100, "stays visible at any Hook score threshold");
    assert_eq!(e.load_project(&p.id).unwrap().clips, *clips);
}

#[test]
fn bad_ranges_are_refused_and_nothing_is_saved() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let p = clip_range::new_manual_project(&e, &fixtures().join("source.mp4")).unwrap(); // 600 s
    let id = clip_range::add_manual_clip(&e, &p.id, 100.0, 190.0).unwrap().clip_id;
    for (s, en) in [(50.0, 50.0), (60.0, 50.0), (-1.0, 50.0), (590.0, 601.0), (f64::NAN, 10.0), (10.0, 10.5)] {
        assert!(clip_range::add_manual_clip(&e, &p.id, s, en).is_err(), "{s}-{en}");
        assert!(clip_range::set_range(&e, &p.id, &id, s, en).is_err(), "{s}-{en}");
    }
    assert!(clip_range::set_range(&e, &p.id, "nope", 1.0, 70.0).unwrap_err().contains("not found"));
    let c = &e.load_project(&p.id).unwrap().clips;
    assert_eq!((c.len(), c[0].start, c[0].end), (1, 100.0, 190.0));
}

#[test]
fn preview_names_exactly_the_clip_range_of_the_source() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let src = fixtures().join("source.mp4");
    let p = clip_range::new_manual_project(&e, &src).unwrap();
    let id = clip_range::add_manual_clip(&e, &p.id, 100.0, 190.0).unwrap().clip_id;
    clip_range::set_range(&e, &p.id, &id, 95.0, 190.0).unwrap();
    let v = clip_range::preview(&e, &p.id, &id).unwrap();
    assert_eq!((Path::new(&v.path), v.start, v.end), (src.as_path(), 95.0, 190.0));
}

#[test]
fn trimming_the_start_keeps_a_followed_layout_in_step_with_the_video() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path());
    let p = clip_range::new_manual_project(&e, &fixtures().join("source.mp4")).unwrap();
    let id = clip_range::add_manual_clip(&e, &p.id, 100.0, 190.0).unwrap().clip_id;
    let layout = Layout::auto(&[], 1920, 1080);
    let mut followed = layout.clone();
    followed.follow = vec![Key { t: 0.0, cx: 0.3 }, Key { t: 20.0, cx: 0.7 }];
    cruxvid_lib::layout_store::LayoutStore::new(e.data_dir().to_path_buf()).put(&p.id, &id, &followed).unwrap();
    clip_range::set_range(&e, &p.id, &id, 95.0, 190.0).unwrap(); // starts 5 s earlier
    let l = e.clip_layout(&p.id, &id).unwrap();
    assert_eq!(l.follow.iter().map(|k| k.t).collect::<Vec<_>>(), [5.0, 25.0]);
}

#[test]
fn a_file_that_is_not_a_video_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("notes.txt");
    std::fs::write(&f, "hi").unwrap();
    assert!(clip_range::new_manual_project(&engine(tmp.path()), &f).unwrap_err().contains("not a video file"));
}
