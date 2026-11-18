//! Browser executable discovery for standalone Labs/CDP tooling.
//! Thor itself always enters the embedded CEF runtime.

use std::path::{Path, PathBuf};

use super::chromium_runtime::THOR_CHROMIUM_PATH_ENV;
use crate::paths::{thor_bundled_chromium_path, thor_data_dir};

/// Allow discovering system Chrome/Chromium for Labs and development only.
pub const THOR_ALLOW_SYSTEM_CHROME_ENV: &str = "THOR_ALLOW_SYSTEM_CHROME";
/// Resolve an owned browser executable for standalone CDP tooling.
pub fn discover_owned_chromium_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(THOR_CHROMIUM_PATH_ENV) {
        let p = PathBuf::from(path);
        if executable_exists(&p) {
            return Some(p);
        }
    }

    owned_chromium_candidates()
        .into_iter()
        .find(|candidate| executable_exists(candidate))
}

fn executable_exists(p: &Path) -> bool {
    p.is_file()
}

/// Candidate paths for owned browser executables used by standalone CDP tooling.
pub fn owned_chromium_candidates() -> Vec<PathBuf> {
    vec![
        thor_bundled_chromium_path(),
        thor_data_dir()
            .join("ChromiumCache")
            .join("Chromium.app")
            .join("Contents")
            .join("MacOS")
            .join("Chromium"),
    ]
}

pub fn allow_system_chrome() -> bool {
    matches!(
        std::env::var(THOR_ALLOW_SYSTEM_CHROME_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_candidates_include_cache_and_frameworks() {
        let c = owned_chromium_candidates();
        assert!(c
            .iter()
            .any(|p| p.to_string_lossy().contains("ChromiumCache")));
        assert!(c.iter().any(|p| p.to_string_lossy().contains("Frameworks")));
    }

    #[test]
    fn lifecycle_hook_names_are_documented() {
        use crate::engine::freezing::ChromiumLifecycleHooks;
        assert!(ChromiumLifecycleHooks::FREEZE.contains("Frozen"));
        assert!(ChromiumLifecycleHooks::DISCARD.contains("discard"));
        assert!(ChromiumLifecycleHooks::EXPIRE.contains("Close"));
    }
}
