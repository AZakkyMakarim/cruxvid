//! Runtime config with a "test mode" (ticket cruxvid-b2t.4, story 57).
//!
//! The Gemini base URL and the Source resolver are overridable ONLY when
//! `CRUXVID_TEST_MODE=1`. Without it the override vars are ignored, so a
//! stray env var can never point a normal launch at a fake server.

use serde::Serialize;

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
}

impl AppConfig {
    /// `env` is injected so tests do not touch the process environment.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Self {
        let test_mode = env("CRUXVID_TEST_MODE").as_deref() == Some("1");
        let mut cfg = AppConfig {
            test_mode,
            gemini_base_url: DEFAULT_GEMINI_BASE_URL.to_string(),
            source_resolver: SourceResolver::Real,
        };
        if test_mode {
            if let Some(url) = env("CRUXVID_GEMINI_BASE_URL").filter(|u| !u.is_empty()) {
                cfg.gemini_base_url = url;
            }
            if env("CRUXVID_SOURCE_RESOLVER").as_deref() == Some("fake") {
                cfg.source_resolver = SourceResolver::Fake;
            }
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
    fn empty_url_override_keeps_default() {
        let c = cfg(&[("CRUXVID_TEST_MODE", "1"), ("CRUXVID_GEMINI_BASE_URL", "")]);
        assert_eq!(c.gemini_base_url, DEFAULT_GEMINI_BASE_URL);
    }
}
