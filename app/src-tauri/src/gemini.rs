//! Gemini client. Tracer scope: plain-http generateContent against a configurable base URL.
//! API key, model setting, token estimate and https come with cruxvid-b2t.7.

use crate::{source::Source, transcript::Transcript};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

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
}

impl GeminiClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self { base_url: base_url.trim_end_matches('/').to_string(), model: model.to_string() }
    }

    /// shortcut: sends no audio (local-file upload needs the Files API, cruxvid-b2t.7); only the prompt and Source facts.
    pub fn transcribe(&self, source: &Source) -> Result<Transcript, String> {
        self.generate(format!("{TRANSCRIBE_PROMPT}\nVideo: {} ({:.0} s)\n", source.title, source.duration))
    }

    pub fn analyze(&self, transcript: &Transcript) -> Result<Analysis, String> {
        self.generate(format!("{ANALYZE_PROMPT}\n{}", transcript.as_prompt_text()))
    }

    fn generate<T: DeserializeOwned>(&self, prompt: String) -> Result<T, String> {
        let url = format!("{}/v1beta/models/{}:generateContent", self.base_url, self.model);
        let body = json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": { "responseMimeType": "application/json" },
        });
        let resp = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| match e {
                ureq::Error::Status(code, _) => format!("Gemini returned HTTP {code}."),
                e => format!("Cannot reach Gemini at {}: {e}", self.base_url),
            })?
            .into_string()
            .map_err(|e| format!("Cannot read Gemini response: {e}"))?;
        let env: Value = serde_json::from_str(&resp).map_err(|e| format!("Gemini response is not JSON: {e}"))?;
        let text = env["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or("Gemini response has no text.")?;
        serde_json::from_str(text).map_err(|e| format!("Gemini answer does not match the expected shape: {e}"))
    }
}
