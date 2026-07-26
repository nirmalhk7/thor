//! Honest content-engine capability reporting (FS-003).
//!
//! M2 live shell = AppKit chrome + headful Chromium content via CDP (dual-window).
//! CEF single-window embed and Fetch intercept UI are still later.

use serde_json::{json, Value};

/// Process-wide engine status for UI / IPC consumers.
pub fn engine_status() -> Value {
    json!({
        "brand": "Thor",
        "backend": "chromium",
        "cdp": true,
        "intercept": false,
        "intercept_reason": "CDP Fetch interception UI not wired yet (M2+).",
        "freeze": "soft-cdp",
        "freeze_reason": "Background tabs soft-freeze via CDP Page.setWebLifecycleState when available; bring-to-front on activate.",
        "mv3": false,
        "sync": false,
        "milestone": "M2",
        "shell": "dual-window",
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
    fn status_is_honest_chromium_m2() {
        let s = engine_status();
        assert_eq!(s["backend"], "chromium");
        assert_eq!(s["cdp"], true);
        assert_eq!(s["intercept"], false);
        assert_eq!(s["milestone"], "M2");
        assert_eq!(s["shell"], "dual-window");
    }
}
