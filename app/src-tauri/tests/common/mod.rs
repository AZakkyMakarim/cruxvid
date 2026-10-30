//! Shared test helpers: fixture paths and a fake Gemini HTTP server that replays the recorded
//! responses in `shared/fixtures` (the only faked boundary; ffmpeg/ffprobe are real).
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shared/fixtures")
}

pub struct FakeGemini {
    pub base_url: String,
}

impl FakeGemini {
    /// Serves forever on a background thread; routes on the `CRUXVID_TASK:` marker in the prompt.
    pub fn start() -> Self {
        Self::start_slow(std::time::Duration::ZERO)
    }

    /// Like `start`, but every answer takes `delay` (so a test can change things while a Gemini call is in flight).
    pub fn start_slow(delay: std::time::Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                std::thread::spawn(move || {
                    let body = read_request_body(&mut s);
                    std::thread::sleep(delay);
                    let file = if body.contains("CRUXVID_TASK: transcribe") {
                        "gemini_transcribe_response.json"
                    } else if body.contains("CRUXVID_TASK: translate") {
                        "gemini_translate_response.json"
                    } else if body.contains("CRUXVID_TASK: describe") {
                        "gemini_describe_response.json"
                    } else if body.contains("CRUXVID_TASK: speakers") {
                        "gemini_speakers_response.json"
                    } else {
                        "gemini_analyze_response.json"
                    };
                    let payload = std::fs::read_to_string(fixtures().join(file)).unwrap();
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        payload.len(),
                        payload
                    );
                    let _ = s.write_all(resp.as_bytes());
                });
            }
        });
        FakeGemini { base_url }
    }
}

fn read_request_body(s: &mut std::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buf);
        if let Some(i) = text.find("\r\n\r\n") {
            let len = text[..i]
                .to_lowercase()
                .lines()
                .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                .unwrap_or(0);
            if buf.len() >= i + 4 + len {
                return String::from_utf8_lossy(&buf[i + 4..]).into_owned();
            }
        }
    }
    String::new()
}
