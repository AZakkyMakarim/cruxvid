//! AI settings: the Gemini API key (OS credential store) and the model name (plain file in the data dir).
//! The key never touches Project files, config files or logs.

use std::{path::Path, sync::Mutex};

pub trait KeyStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, key: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

/// In-memory store for tests and test mode, so they never touch the real credential store.
#[derive(Default)]
pub struct MemoryKeyStore(Mutex<Option<String>>);

impl MemoryKeyStore {
    pub fn with_key(key: &str) -> Self {
        Self(Mutex::new(Some(key.to_string())))
    }
}

impl KeyStore for MemoryKeyStore {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn set(&self, key: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(key.to_string());
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

/// Windows Credential Manager (via the `keyring` crate).
pub struct OsKeyStore;

impl OsKeyStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new("cruxvid", "gemini-api-key").map_err(|e| format!("Cannot open the credential store: {e}"))
    }
}

impl KeyStore for OsKeyStore {
    fn get(&self) -> Result<Option<String>, String> {
        match Self::entry()?.get_password() {
            Ok(k) => Ok(Some(k)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Cannot read the API key from the credential store: {e}")),
        }
    }
    fn set(&self, key: &str) -> Result<(), String> {
        Self::entry()?.set_password(key).map_err(|e| format!("Cannot save the API key to the credential store: {e}"))
    }
    fn clear(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Cannot remove the API key from the credential store: {e}")),
        }
    }
}

/// Test mode never touches the real credential store.
pub fn key_store_for(cfg: &crate::config::AppConfig) -> std::sync::Arc<dyn KeyStore> {
    match (&cfg.test_mode, cfg.test_api_key.as_deref()) {
        (true, Some(k)) if !k.is_empty() => std::sync::Arc::new(MemoryKeyStore::with_key(k)),
        (true, _) => std::sync::Arc::new(MemoryKeyStore::default()),
        _ => std::sync::Arc::new(OsKeyStore),
    }
}

const SETTINGS_FILE: &str = "settings.json";

/// The saved model name, if the user ever changed it.
pub fn load_model(data_dir: &Path) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(data_dir.join(SETTINGS_FILE)).ok()?).ok()?;
    v["model"].as_str().map(str::to_string).filter(|m| !m.is_empty())
}

pub fn save_model(data_dir: &Path, model: &str) -> Result<(), String> {
    let file = data_dir.join(SETTINGS_FILE);
    // Merge into the existing file so settings owned by other features survive.
    let mut v: serde_json::Value = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| serde_json::json!({}));
    v["model"] = model.into();
    std::fs::create_dir_all(data_dir)
        .and_then(|_| std::fs::write(&file, v.to_string()))
        .map_err(|e| format!("Cannot save settings: {e}"))
}
