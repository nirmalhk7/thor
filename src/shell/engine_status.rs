//! Honest content-engine capability reporting (FS-003).
//!
//! Product default = Thor-owned Chromium (downstream / snapshot) with self-CDP Labs.
//! Legacy dual-window = AppKit + separate Chromium process (`THOR_LEGACY_DUAL_WINDOW=1`).

use serde_json::{json, Value};

use crate::engine::product::{discover_owned_chromium_path, legacy_dual_window_requested};

/// Process-wide engine status for UI / IPC consumers.
pub fn engine_status() -> Value {
    let legacy = legacy_dual_window_requested();
    let owned = discover_owned_chromium_path();
    let (shell, milestone, freeze, freeze_reason) = if legacy {
        (
            "legacy-dual-window",
            "M2-legacy",
            "soft-cdp",
            "Legacy dual-window: CDP Page.setWebLifecycleState; transitional only.",
        )
    } else if owned.is_some() {
        (
            "chromium-downstream",
            "M3-downstream",
            "chromium-hooks+self-cdp",
            "Product: Thor-owned Chromium; prefer WebContents freeze/discard (see browser/isolation/MAPPING.md); Labs via self-CDP.",
        )
    } else {
        (
            "chromium-downstream-missing",
            "M3-downstream",
            "unavailable",
            "Owned Chromium not found — run ./scripts/bootstrap_chromium_snapshot.sh or build_chromium.sh.",
        )
    };

    json!({
        "brand": "Thor",
        "backend": "chromium",
        "product": "chromium-downstream",
        "owned_chromium": owned.as_ref().map(|p| p.display().to_string()),
        "cdp": true,
        "self_cdp": !legacy,
        "intercept": false,
        "intercept_reason": "CDP Fetch interception UI not wired yet.",
        "freeze": freeze,
        "freeze_reason": freeze_reason,
        "mv3": false,
        "sync": false,
        "milestone": milestone,
        "shell": shell,
    })
}

pub fn engine_status_script() -> String {
    format!(
        "window.__thorEngineStatus = {}; if (window.__thorOnEngineStatus) window.__thorOnEngineStatus(window.__thorEngineStatus);",
        engine_status()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_honest_chromium_downstream() {
        let s = engine_status();
        assert_eq!(s["backend"], "chromium");
        assert_eq!(s["cdp"], true);
        assert_eq!(s["product"], "chromium-downstream");
        assert_eq!(s["intercept"], false);
        assert!(s["shell"].as_str().unwrap().contains("chromium") || s["shell"] == "legacy-dual-window");
    }
}
