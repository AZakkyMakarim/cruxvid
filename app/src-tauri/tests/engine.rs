//! Seam 1: the public Engine API (Analyze, Export) against real ffmpeg/ffprobe and a faked Gemini.
mod common;

use common::{fixtures, FakeGemini};
use cruxvid_lib::engine::{Engine, EngineConfig};
use serde_json::Value;
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
fn analyze_local_source_lists_clips_and_saves_project() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());

    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();

    assert_eq!(p.source.title, "source");
    assert!((p.source.duration - 600.0).abs() < 0.5);
    assert_eq!(p.source.fps, 30.0);
    // Four content topics are 60-120 s; "Too short topic" (54 s) yields no Clip; opening/sponsor/closing never do.
    let titles: Vec<_> = p.clips.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["First topic", "Second topic", "Fourth topic", "Fifth topic"]);
    for c in &p.clips {
        assert!((60.0..=120.0).contains(&(c.end - c.start)), "{c:?}");
    }
    assert!(p.clips.windows(2).all(|w| w[0].end <= w[1].start), "timeline order, no overlap");
    assert_eq!((p.clips[0].start, p.clips[0].end, p.clips[0].hook_score), (45.0, 124.4, 82));
    assert_eq!(p.transcript.words.len(), 600);

    // Project survives a reload from disk.
    assert_eq!(e.load_project(&p.id).unwrap(), p);
}

#[test]
fn analyze_rejects_non_video_with_clear_error_and_engine_keeps_working() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let not_video = tmp.path().join("notes.txt");
    std::fs::write(&not_video, "hello").unwrap();

    let err = e.analyze(&not_video).unwrap_err();
    assert!(err.contains("not a video"), "{err}");
    assert!(e.analyze(&tmp.path().join("missing.mp4")).unwrap_err().contains("not found"));
    assert!(e.analyze(&fixtures().join("source.mp4")).is_ok());
}

#[test]
fn export_clip_is_9x16_h264_aac_cfr_with_clip_duration() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &FakeGemini::start());
    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    let clip = &p.clips[0];

    let out = e.export(&p.id, &clip.id).unwrap();

    assert_eq!(out.parent().unwrap(), tmp.path().join("out"));
    assert_eq!(out.file_name().unwrap(), "source_01_First topic.mp4");
    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-show_format", "-of", "json"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(probe.status.success(), "ffprobe failed");
    let j: Value = serde_json::from_slice(&probe.stdout).unwrap();
    let streams = j["streams"].as_array().unwrap();
    let v = streams.iter().find(|s| s["codec_type"] == "video").unwrap();
    let a = streams.iter().find(|s| s["codec_type"] == "audio").unwrap();
    assert_eq!((v["codec_name"].as_str(), v["profile"].as_str(), v["pix_fmt"].as_str()), (Some("h264"), Some("High"), Some("yuv420p")));
    assert_eq!((v["width"].as_i64(), v["height"].as_i64()), (Some(1080), Some(1920)));
    assert_eq!(v["r_frame_rate"], "30/1");
    assert_eq!(v["avg_frame_rate"], "30/1", "constant frame rate");
    assert_eq!(a["codec_name"], "aac");
    let dur: f64 = j["format"]["duration"].as_str().unwrap().parse().unwrap();
    assert!((dur - (clip.end - clip.start)).abs() < 0.2, "duration {dur}");
    let bitrate: f64 = j["format"]["bit_rate"].as_str().unwrap().parse().unwrap();
    assert!(bitrate < 12_000_000.0, "bitrate {bitrate}");
    // faststart: the moov atom precedes mdat.
    let head = std::fs::read(&out).unwrap();
    let pos = |tag: &[u8]| head.windows(4).position(|w| w == tag).unwrap();
    assert!(pos(b"moov") < pos(b"mdat"));
}

#[test]
fn shared_fixtures_match_json_schemas() {
    let shared = fixtures().join("..");
    let load = |p: &str| -> Value { serde_json::from_slice(&std::fs::read(shared.join(p)).unwrap()).unwrap() };
    let envelope_text = |p: &str| -> Value {
        let env = load(p);
        serde_json::from_str(env["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap()).unwrap()
    };
    for (schema, instance) in [
        ("schema/transcript.schema.json", load("fixtures/transcript.json")),
        ("schema/transcript.schema.json", envelope_text("fixtures/gemini_transcribe_response.json")),
        ("schema/analysis.schema.json", envelope_text("fixtures/gemini_analyze_response.json")),
    ] {
        let validator = jsonschema::validator_for(&load(schema)).unwrap();
        assert!(validator.is_valid(&instance), "{schema} rejects fixture");
    }
    assert_eq!(load("fixtures/transcript.json"), envelope_text("fixtures/gemini_transcribe_response.json"));
}
