//! Small file helpers shared by the JSON stores: atomic writes and a damaged-file-aware read.

use serde::{de::DeserializeOwned, Serialize};
use std::{path::Path, sync::{atomic::{AtomicU32, Ordering}, Mutex, MutexGuard}};

/// Locks `m` even when a thread panicked while holding it (the protected data is files, not the guard).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Writes to a temp file next to `path`, then renames it over `path`, so a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.{}-{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T, what: &str) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    write_atomic(path, &json).map_err(|e| format!("Cannot save {what}: {e}"))
}

/// A missing file is the default value; a file that exists but cannot be read or parsed is an error
/// (so a read-modify-write never replaces a damaged file with an almost empty one).
pub fn read_json<T: DeserializeOwned + Default>(path: &Path, what: &str) -> Result<T, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("The {what} file is damaged: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(format!("Cannot read the {what} file: {e}")),
    }
}
