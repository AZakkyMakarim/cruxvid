//! External tools (ffmpeg, ffprobe). One place to spawn them so every call behaves the same.

use std::process::Command;

/// `name` is resolved from PATH. shortcut: no bundled sidecar yet, ship ffmpeg as a Tauri sidecar when installers are built.
pub fn command(name: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(name);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash from the GUI app
    }
    c
}
