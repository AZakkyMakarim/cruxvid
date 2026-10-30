//! Runtime config with a "test mode" (ticket cruxvid-b2t.4, story 57).
//!
//! The Gemini base URL and the Source resolver are overridable ONLY when
//! `CRUXVID_TEST_MODE=1`. Without it the override vars are ignored, so a
//! stray env var can never point a normal launch at a fake server.

use serde::Serialize;
use std::path::PathBuf;

pub const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceResolver {
    /// Real resolver (yt-dlp sidecar).
    Real,
    /// Fake resolver for tests (serves a local fixture instead of downloading).
    Fake,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppConfig {
    pub test_mode: bool,
    pub gemini_base_url: String,
    pub source_resolver: SourceResolver,
    /// Test-mode only: Project store and Export output folders (else OS defaults, see `lib.rs`).
    #[serde(skip)]
    pub data_dir: Option<PathBuf>,
    #[serde(skip)]
    pub output_dir: Option<PathBuf>,
    /// Test-mode only: Gemini request timeout in ms (else the 300 s default).
    #[serde(skip)]
    pub gemini_timeout_ms: Option<u64>,
    /// Test-mode only: the API key the in-memory key store starts with. Unset -> "test-key" so UI flows that
    /// are not about keys just work; set empty -> no key (first-use dialog). Never read outside test mode.
    #[serde(skip)]
    pub test_api_key: Option<String>,
    /// Test-mode only: the yt-dlp executable to run for links (a fake one in tests).
    #[serde(skip)]
    pub ytdlp_path: Option<PathBuf>,
}

impl AppConfig {
    /// `env` is injected so tests do not touch the process environment.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Self {
        let test_mode = env("CRUXVID_TEST_MODE").as_deref() == Some("1");
        let mut cfg = AppConfig {
            test_mode,
            gemini_base_url: DEFAULT_GEMINI_BASE_URL.to_string(),
            source_resolver: SourceResolver::Real,
            data_dir: None,
            output_dir: None,
            gemini_timeout_ms: None,
            test_api_key: None,
            ytdlp_path: None,
        };
        if test_mode {
            if let Some(url) = env("CRUXVID_GEMINI_BASE_URL").filter(|u| !u.is_empty()) {
                cfg.gemini_base_url = url;
            }
            if env("CRUXVID_SOURCE_RESOLVER").as_deref() == Some("fake") {
                cfg.source_resolver = SourceResolver::Fake;
            }
            cfg.data_dir = env("CRUXVID_DATA_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
            cfg.output_dir = env("CRUXVID_OUTPUT_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
            cfg.gemini_timeout_ms = env("CRUXVID_GEMINI_TIMEOUT_MS").and_then(|v| v.parse().ok());
            cfg.ytdlp_path = env("CRUXVID_YTDLP").filter(|v| !v.is_empty()).map(PathBuf::from);
            cfg.test_api_key = Some(env("CRUXVID_TEST_API_KEY").unwrap_or_else(|| "test-key".into()));
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(vars: &[(&str, &str)]) -> AppConfig {
        let m: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        AppConfig::from_env(|k| m.get(k).cloned())
    }

    const OVERRIDES: [(&str, &str); 2] = [
        ("CRUXVID_GEMINI_BASE_URL", "http://127.0.0.1:9999"),
        ("CRUXVID_SOURCE_RESOLVER", "fake"),
    ];

    #[test]
    fn defaults_without_any_env() {
        let c = cfg(&[]);
        assert!(!c.test_mode);
        assert_eq!(c.gemini_base_url, DEFAULT_GEMINI_BASE_URL);
        assert_eq!(c.source_resolver, SourceResolver::Real);
    }

    #[test]
    fn test_mode_applies_overrides() {
        let mut vars = OVERRIDES.to_vec();
        vars.push(("CRUXVID_TEST_MODE", "1"));
        let c = cfg(&vars);
        assert!(c.test_mode);
        assert_eq!(c.gemini_base_url, "http://127.0.0.1:9999");
        assert_eq!(c.source_resolver, SourceResolver::Fake);
    }

    #[test]
    fn overrides_ignored_without_test_mode() {
        let c = cfg(&OVERRIDES);
        assert!(!c.test_mode);
        assert_eq!(c.gemini_base_url, DEFAULT_GEMINI_BASE_URL);
        assert_eq!(c.source_resolver, SourceResolver::Real);
    }

    #[test]
    fn test_mode_flag_must_be_exactly_one() {
        for v in ["0", "true", "", "yes"] {
            let mut vars = OVERRIDES.to_vec();
            vars.push(("CRUXVID_TEST_MODE", v));
            let c = cfg(&vars);
            assert!(!c.test_mode, "value {v:?} must not enable test mode");
            assert_eq!(c.gemini_base_url, DEFAULT_GEMINI_BASE_URL);
        }
    }

    #[test]
    fn dir_overrides_only_in_test_mode() {
        let dirs = [("CRUXVID_DATA_DIR", "D:/d"), ("CRUXVID_OUTPUT_DIR", "D:/o")];
        let off = cfg(&dirs);
        assert_eq!((off.data_dir, off.output_dir), (None, None));
        let mut vars = dirs.to_vec();
        vars.push(("CRUXVID_TEST_MODE", "1"));
        let on = cfg(&vars);
        assert_eq!(on.data_dir, Some(PathBuf::from("D:/d")));
        assert_eq!(on.output_dir, Some(PathBuf::from("D:/o")));
    }

    #[test]
    fn ai_test_overrides_only_in_test_mode() {
        let vars = [("CRUXVID_GEMINI_TIMEOUT_MS", "250"), ("CRUXVID_TEST_API_KEY", "")];
        let off = cfg(&vars);
        assert_eq!((off.gemini_timeout_ms, off.test_api_key), (None, None));
        let mut on_vars = vars.to_vec();
        on_vars.push(("CRUXVID_TEST_MODE", "1"));
        let on = cfg(&on_vars);
        assert_eq!((on.gemini_timeout_ms, on.test_api_key.as_deref()), (Some(250), Some("")));
        assert_eq!(cfg(&[("CRUXVID_TEST_MODE", "1")]).test_api_key.as_deref(), Some("test-key"));
    }

    #[test]
    fn ytdlp_override_only_in_test_mode() {
        let off = cfg(&[("CRUXVID_YTDLP", "D:/fake/yt-dlp.exe")]);
        assert_eq!(off.ytdlp_path, None);
        let on = cfg(&[("CRUXVID_TEST_MODE", "1"), ("CRUXVID_YTDLP", "D:/fake/yt-dlp.exe")]);
        assert_eq!(on.ytdlp_path, Some(PathBuf::from("D:/fake/yt-dlp.exe")));
    }

    #[test]
    fn empty_url_override_keeps_default() {
        let c = cfg(&[("CRUXVID_TEST_MODE", "1"), ("CRUXVID_GEMINI_BASE_URL", "")]);
        assert_eq!(c.gemini_base_url, DEFAULT_GEMINI_BASE_URL);
    }
}
