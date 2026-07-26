//! Platform paths for Thor user data (always branded `Thor`, never `.thor`).

use std::path::PathBuf;

/// Thor application data directory.
///
/// - macOS: `~/Library/Application Support/Thor`
/// - Windows: `%APPDATA%/Thor`
/// - Linux: `$XDG_DATA_HOME/Thor` or `~/.local/share/Thor`
///
/// Override with `THOR_DATA_DIR`.
pub fn thor_data_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("THOR_DATA_DIR") {
        return PathBuf::from(override_dir);
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    #[cfg(target_os = "macos")]
    {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Thor")
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata).join("Thor")
        } else {
            PathBuf::from(home)
                .join("AppData")
                .join("Roaming")
                .join("Thor")
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg).join("Thor")
        } else {
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("Thor")
        }
    }
}

/// Isolated Chromium user-data profile under the Thor data directory.
pub fn thor_chromium_profile_dir() -> PathBuf {
    thor_data_dir().join("ChromiumProfile")
}
