//! Seam: Topic/Clip rules on the shared fixture Transcript (real sentence boundaries) and the recorded
//! Gemini analysis, which holds an opening, a sponsor and a closing.
mod common;

use common::fixtures;
use cruxvid_lib::clips::{build_clips, Clip};
use cruxvid_lib::gemini::Analysis;
use cruxvid_lib::transcript::Transcript;
use serde_json::{json, Value};

fn transcript() -> Transcript {
    serde_json::from_slice(&std::fs::read(fixtures().join("transcript.json")).unwrap()).unwrap()
}

fn recorded() -> Analysis {
    let env: Value = serde_json::from_slice(&std::fs::read(fixtures().join("gemini_analyze_response.json")).unwrap()).unwrap();
    serde_json::from_str(env["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap()).unwrap()
}

fn analysis(sections: Value) -> Analysis {
    serde_json::from_value(json!({ "sections": sections })).unwrap()
}

fn check_rules(clips: &[Clip]) {
    let t = transcript();
    let sentences = t.sentences();
    for c in clips {
        assert!((60.0..=120.0).contains(&(c.end - c.start)), "{c:?}");
        assert!(sentences.iter().any(|s| (s.0 - c.start).abs() < 0.01), "start on a sentence start: {c:?}");
        assert!(sentences.iter().any(|s| (s.1 - c.end).abs() < 0.01), "end on a sentence end: {c:?}");
    }
    assert!(clips.windows(2).all(|w| w[0].end <= w[1].start));
}

#[test]
fn recorded_analysis_yields_no_opening_sponsor_or_closing_clip() {
    let clips = build_clips(&recorded().sections, &transcript());
    check_rules(&clips);
    let titles: Vec<_> = clips.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["First topic", "Second topic", "Fourth topic", "Fifth topic"]);
    // opening 0-44.4, sponsor 400-439.4, closing 560-599.4
    assert!(clips.iter().all(|c| c.start >= 45.0 && c.end <= 559.4 && !(c.start < 439.4 && c.end > 400.0)));
}

#[test]
fn rule_cases_on_the_fixture_transcript() {
    let one = |s: Value| build_clips(&analysis(json!([s])).sections, &transcript());
    let content = |a: f64, b: f64| json!({ "label": "content", "start": a, "end": b, "title": "x", "hook_score": 70 });

    // A Topic longer than 120 s is trimmed to the sub-range Gemini names.
    let mut long = content(100.0, 300.0);
    long["best_start"] = json!(150.0);
    long["best_end"] = json!(244.4);
    let trimmed = one(long);
    check_rules(&trimmed);
    assert_eq!((trimmed[0].start, trimmed[0].end), (150.0, 244.4));

    // Without one it is still cut to 60-120 s on sentence boundaries.
    let cut = one(content(100.0, 300.0));
    check_rules(&cut);
    assert_eq!(cut.len(), 1);

    // Edge case: a 40 s Topic with no related neighbour yields no Clip.
    assert!(one(content(100.0, 139.4)).is_empty());
    // Mid-roll between two short Topics keeps them apart.
    let split = build_clips(
        &analysis(json!([content(0.0, 39.4), { "label": "sponsor", "start": 40.0, "end": 79.4 }, content(80.0, 119.4)])).sections,
        &transcript(),
    );
    assert!(split.is_empty());
    // Two short adjacent Topics merge into one Clip.
    let merged = build_clips(&analysis(json!([content(0.0, 44.4), content(45.0, 84.4)])).sections, &transcript());
    check_rules(&merged);
    assert_eq!((merged[0].start, merged[0].end), (0.0, 84.4));
}
