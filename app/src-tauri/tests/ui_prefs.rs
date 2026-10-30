//! cruxvid-b2t.18: UI language and theme are saved in the data dir and survive a restart.
use cruxvid_lib::{
    engine::{Engine, EngineConfig},
    settings::UiPrefs,
};
use std::path::Path;

fn engine(dir: &Path) -> Engine {
    Engine::new(EngineConfig {
        data_dir: dir.to_path_buf(),
        output_dir: dir.join("out"),
        gemini_base_url: "http://127.0.0.1:1".into(),
        model: "m".into(),
    })
}

fn prefs(language: &str, theme: &str) -> UiPrefs {
    UiPrefs { language: language.into(), theme: theme.into() }
}

#[test]
fn fresh_install_has_no_choice_so_the_ui_follows_the_system() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(engine(dir.path()).ui_prefs(), prefs("", ""));
}

#[test]
fn choice_survives_restart_and_keeps_the_model_setting() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path());
    e.set_model("gemini-x").unwrap();
    e.set_ui_prefs(prefs("en", "dark")).unwrap();
    let restarted = engine(dir.path());
    assert_eq!(restarted.ui_prefs(), prefs("en", "dark"));
    assert_eq!(restarted.ai_status().model, "gemini-x");
    restarted.set_ui_prefs(prefs("id", "")).unwrap(); // back to "follow system" for the theme
    assert_eq!(engine(dir.path()).ui_prefs(), prefs("id", ""));
}

#[test]
fn unknown_language_or_theme_is_rejected_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let e = engine(dir.path());
    e.set_ui_prefs(prefs("en", "light")).unwrap();
    assert!(e.set_ui_prefs(prefs("fr", "light")).is_err());
    assert!(e.set_ui_prefs(prefs("en", "sepia")).is_err());
    assert_eq!(engine(dir.path()).ui_prefs(), prefs("en", "light"));
}
