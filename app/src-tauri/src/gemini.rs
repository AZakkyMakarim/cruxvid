//! Gemini client: generateContent against a configurable base URL, API key in a header (never in the URL).

use crate::{audio, source::Source, transcript::Transcript};
use base64::Engine as _;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::time::Duration;

pub const MSG_NO_KEY: &str = "API key Gemini belum diisi. Masukkan key di Settings untuk memakai fitur AI.";
const MSG_BAD_KEY: &str = "Key tidak valid: Gemini menolak API key ini. Periksa key di Settings.";
const MSG_QUOTA: &str = "Kuota habis: batas penggunaan Gemini tercapai. Coba lagi nanti atau pakai model lain di Settings.";
const MSG_NETWORK: &str = "Jaringan bermasalah: tidak bisa menghubungi Gemini. Periksa koneksi internet lalu coba lagi.";

/// Analysis of a long Source is slow; this only guards against a hung connection.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

pub const DEFAULT_MODEL: &str = "gemini-2.0-flash";

const TRANSCRIBE_PROMPT: &str = include_str!("../../../shared/prompts/transcribe.txt");
const ANALYZE_PROMPT: &str = include_str!("../../../shared/prompts/analyze.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    Opening,
    Content,
    Sponsor,
    Closing,
    Idle,
}

/// One labelled stretch of the Source (shape: shared/schema/analysis.schema.json).
#[derive(Debug, Clone, Deserialize)]
pub struct Section {
    pub label: Label,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub hook_score: u8,
    #[serde(default)]
    pub hook_reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Analysis {
    pub sections: Vec<Section>,
}

pub struct GeminiClient {
    base_url: String,
    model: String,
    key: Option<String>,
    timeout: Duration,
}

impl GeminiClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self { base_url: base_url.trim_end_matches('/').to_string(), model: model.to_string(), key: None, timeout: DEFAULT_TIMEOUT }
    }

    pub fn with_key(mut self, key: Option<String>) -> Self {
        self.key = key;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The Source's audio goes inline with the prompt.
    pub fn transcribe(&self, source: &Source) -> Result<Transcript, String> {
        let mp3 = audio::extract_mp3(std::path::Path::new(&source.path))?;
        let audio = json!({ "inline_data": { "mime_type": "audio/mp3", "data": base64::engine::general_purpose::STANDARD.encode(mp3) } });
        self.generate(vec![json!({ "text": format!("{TRANSCRIBE_PROMPT}\nVideo: {} ({:.0} s)\n", source.title, source.duration) }), audio])
    }

    pub fn analyze(&self, transcript: &Transcript) -> Result<Analysis, String> {
        self.generate(vec![json!({ "text": format!("{ANALYZE_PROMPT}\n{}", transcript.as_prompt_text()) })])
    }

    fn generate<T: DeserializeOwned>(&self, parts: Vec<Value>) -> Result<T, String> {
        let url = format!("{}/v1beta/models/{}:generateContent", self.base_url, self.model);
        let body = json!({
            "contents": [{ "parts": parts }],
            "generationConfig": { "responseMimeType": "application/json" },
        });
        let mut req = ureq::post(&url).timeout(self.timeout).set("Content-Type", "application/json");
        if let Some(k) = &self.key {
            req = req.set("x-goog-api-key", k);
        }
        let resp = req
            .send_string(&body.to_string())
            .map_err(|e| match e {
                ureq::Error::Status(code, r) => status_message(code, &r.into_string().unwrap_or_default()),
                _ => MSG_NETWORK.to_string(), // transport errors only: they never carry the key
            })?
            .into_string()
            .map_err(|_| MSG_NETWORK.to_string())?;
        let env: Value = serde_json::from_str(&resp).map_err(|e| format!("Gemini response is not JSON: {e}"))?;
        let text = env["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or("Gemini response has no text.")?;
        serde_json::from_str(text).map_err(|e| format!("Gemini answer does not match the expected shape: {e}"))
    }
}

/// Real Gemini answers a bad key with HTTP 400 + API_KEY_INVALID (some paths use 401/403).
fn status_message(code: u16, body: &str) -> String {
    match code {
        401 | 403 => MSG_BAD_KEY.into(),
        400 if body.contains("API_KEY_INVALID") => MSG_BAD_KEY.into(),
        429 => MSG_QUOTA.into(),
        code => format!("Gemini returned HTTP {code}."),
    }
}
