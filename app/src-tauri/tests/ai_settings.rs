//! Seam 1 for cruxvid-b2t.7: API key, model setting, token estimate and error mapping through the Engine,
//! against a fake Gemini that checks the key (the only faked boundary).
mod common;

use common::fixtures;
use cruxvid_lib::{
    engine::{Engine, EngineConfig},
    settings::MemoryKeyStore,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Seen = Arc<Mutex<Vec<(String, String, String)>>>; // (path, key header, body)

/// Fake Gemini keyed by the `x-goog-api-key` header: valid-key -> recorded fixtures, bad-key -> 401,
/// invalid-400 -> 400 API_KEY_INVALID (what real Gemini sends), quota-key -> 429, slow-key -> never answers.
struct KeyedGemini {
    base_url: String,
    seen: Seen,
}

impl KeyedGemini {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = log.clone();
                std::thread::spawn(move || serve(stream, log));
            }
        });
        Self { base_url, seen }
    }
}

fn serve(mut s: std::net::TcpStream, log: Seen) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    let (head, body) = loop {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buf).into_owned();
        if let Some(i) = text.find("\r\n\r\n") {
            let head = text[..i].to_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                .unwrap_or(0);
            if buf.len() >= i + 4 + len {
                break (head, String::from_utf8_lossy(&buf[i + 4..]).into_owned());
            }
        }
    };
    let path = head.lines().next().unwrap().split(' ').nth(1).unwrap().to_string();
    let key = head
        .lines()
        .find_map(|l| l.strip_prefix("x-goog-api-key:").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    log.lock().unwrap().push((path, key.clone(), body.clone()));
    let (status, payload) = match key.as_str() {
        "valid-key" => {
            let f = if body.contains("CRUXVID_TASK: transcribe") { "gemini_transcribe_response.json" } else { "gemini_analyze_response.json" };
            ("200 OK", std::fs::read_to_string(fixtures().join(f)).unwrap())
        }
        "bad-key" => ("401 Unauthorized", r#"{"error":{"code":401,"status":"UNAUTHENTICATED"}}"#.into()),
        "invalid-400" => ("400 Bad Request", r#"{"error":{"code":400,"message":"API key not valid.","details":[{"reason":"API_KEY_INVALID"}]}}"#.into()),
        "quota-key" => ("429 Too Many Requests", r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED"}}"#.into()),
        _ => {
            std::thread::sleep(Duration::from_secs(5)); // slow-key: caller must time out
            return;
        }
    };
    let _ = s.write_all(
        format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()).as_bytes(),
    );
}

fn engine(dir: &Path, gemini: &KeyedGemini) -> Engine {
    Engine::with_key_store(
        EngineConfig {
            data_dir: dir.join("data"),
            output_dir: dir.join("out"),
            gemini_base_url: gemini.base_url.clone(),
            model: "default-model".into(),
        },
        Arc::new(MemoryKeyStore::default()),
    )
    .with_timeout(Duration::from_millis(500))
}

fn analyze_err(e: &Engine) -> String {
    e.analyze(&fixtures().join("source.mp4")).unwrap_err()
}

#[test]
fn analyze_without_key_asks_for_it_and_sends_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let g = KeyedGemini::start();
    let e = engine(tmp.path(), &g);
    assert!(!e.ai_status().has_key);
    assert!(analyze_err(&e).contains("belum diisi"));
    assert!(g.seen.lock().unwrap().is_empty());
}

#[test]
fn valid_key_analyzes_and_is_sent_as_header_with_audio_and_never_stored_in_files() {
    let tmp = tempfile::tempdir().unwrap();
    let g = KeyedGemini::start();
    let e = engine(tmp.path(), &g);
    e.set_api_key("  valid-key \n").unwrap();
    assert!(e.ai_status().has_key);

    let p = e.analyze(&fixtures().join("source.mp4")).unwrap();
    assert_eq!(p.clips.len(), 4);

    let seen = g.seen.lock().unwrap();
    assert!(seen.iter().all(|(path, key, _)| key == "valid-key" && !path.contains("valid-key")), "key only in header");
    assert!(seen[0].2.contains("inline_data") && seen[0].2.contains("audio/mp3"), "transcribe sends the audio");
    drop(seen);
    // Not in any file under the data dir (Project files, settings).
    for entry in walk(&tmp.path().join("data")) {
        assert!(!std::fs::read_to_string(&entry).unwrap_or_default().contains("valid-key"), "{entry:?}");
    }
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p))
        } else {
            out.push(p)
        }
    }
    out
}

#[test]
fn each_failure_has_its_own_message_and_never_leaks_the_key() {
    let tmp = tempfile::tempdir().unwrap();
    let g = KeyedGemini::start();
    let e = engine(tmp.path(), &g);
    for (key, phrase) in [
        ("bad-key", "key tidak valid"),
        ("invalid-400", "key tidak valid"),
        ("quota-key", "kuota habis"),
        ("slow-key", "jaringan bermasalah"),
    ] {
        e.set_api_key(key).unwrap();
        let err = analyze_err(&e);
        assert!(err.to_lowercase().contains(phrase), "{key}: {err}");
        assert!(!err.contains(key), "{key} leaked: {err}");
    }
    // Connection refused is a network problem too.
    let dead = Engine::with_key_store(
        EngineConfig {
            data_dir: tmp.path().join("d2"),
            output_dir: tmp.path().join("o2"),
            gemini_base_url: "http://127.0.0.1:1".into(),
            model: "m".into(),
        },
        Arc::new(MemoryKeyStore::default()),
    );
    dead.set_api_key("valid-key").unwrap();
    assert!(analyze_err(&dead).to_lowercase().contains("jaringan bermasalah"));
    // The engine keeps working after errors.
    e.set_api_key("valid-key").unwrap();
    assert!(e.analyze(&fixtures().join("source.mp4")).is_ok());
}

#[test]
fn model_is_editable_used_in_requests_and_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let g = KeyedGemini::start();
    let e = engine(tmp.path(), &g);
    assert_eq!(e.ai_status().model, "default-model");
    e.set_model(" gemini-2.5-flash ").unwrap();
    assert!(e.set_model("  ").is_err());
    e.set_api_key("valid-key").unwrap();
    e.analyze(&fixtures().join("source.mp4")).unwrap();
    assert!(g.seen.lock().unwrap().iter().all(|(path, _, _)| path.contains("/models/gemini-2.5-flash:")));
    assert_eq!(engine(tmp.path(), &g).ai_status().model, "gemini-2.5-flash");
}

#[test]
fn clearing_the_key_goes_back_to_asking() {
    let tmp = tempfile::tempdir().unwrap();
    let g = KeyedGemini::start();
    let e = engine(tmp.path(), &g);
    e.set_api_key("valid-key").unwrap();
    e.clear_api_key().unwrap();
    assert!(!e.ai_status().has_key);
    assert!(analyze_err(&e).contains("belum diisi"));
    assert!(e.set_api_key("   ").is_err());
}

#[test]
fn token_estimate_scales_with_source_duration_and_needs_no_key() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engine(tmp.path(), &KeyedGemini::start());
    let est = e.estimate_tokens(&fixtures().join("source.mp4")).unwrap(); // 600 s
    assert!((est.duration - 600.0).abs() < 0.5);
    assert_eq!(est.total, est.input + est.output);
    assert!(est.input > 600 * 32, "audio alone is 32 tokens/s: {est:?}");
    assert!(est.output > 0);
    assert!(e.estimate_tokens(&tmp.path().join("nope.mp4")).unwrap_err().contains("not found"));
}
