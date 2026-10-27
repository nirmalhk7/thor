//! Honest content-engine capability reporting (FS-003).
//!
//! Current product = embedded CEF. The separate Chromium process is legacy-only.

use serde_json::{json, Value};

use crate::engine::product::legacy_dual_window_requested;

/// Process-wide engine status for UI / IPC consumers.
pub fn engine_status() -> Value {
    let legacy = legacy_dual_window_requested();
    let (shell, backend, product, milestone, cdp, self_cdp, freeze, freeze_reason) = if legacy {
        (
            "legacy-dual-window",
            "chromium",
            "legacy-dual-window",
            "legacy",
            true,
            false,
            "soft-cdp",
            "Legacy dual-window: CDP Page.setWebLifecycleState; transitional only.",
        )
    } else {
        (
            "cef-native",
            "cef",
            "embedded-cef",
            "cef",
            false,
            false,
            "unreported",
            "The CEF host reports no completed freeze or discard operation.",
        )
    };

    json!({
        "brand": "Thor",
        "backend": backend,
        "product": product,
        "cdp": cdp,
        "self_cdp": self_cdp,
        "intercept": false,
        "intercept_reason": "Traffic interception is not reported by this engine-status surface.",
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
    fn status_matches_the_selected_browser_runtime() {
        let s = engine_status();
        if legacy_dual_window_requested() {
            assert_eq!(s["backend"], "chromium");
            assert_eq!(s["product"], "legacy-dual-window");
        } else {
            assert_eq!(s["backend"], "cef");
            assert_eq!(s["product"], "embedded-cef");
            assert_eq!(s["cdp"], false);
        }
        assert_eq!(s["intercept"], false);
        assert!(s["freeze_reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()));
    }
}
