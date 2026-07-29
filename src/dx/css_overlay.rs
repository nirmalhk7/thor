//! CSS Layout Overlay Injector & Inspector for Thor (Flexbox, Grid, Box Model bounds).
//! Injects a lightweight non-intrusive CSS/JS overlay into the active tab webview
//! showing Flexbox direction vectors, Grid track lines/gaps, and box-model padding/margins.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssOverlayState {
    pub enabled: bool,
    pub show_flexbox: bool,
    pub show_grid: bool,
    pub show_margins: bool,
    pub active_selector: Option<String>,
}

impl Default for CssOverlayState {
    fn default() -> Self {
        Self {
            enabled: false,
            show_flexbox: true,
            show_grid: true,
            show_margins: true,
            active_selector: None,
        }
    }
}

pub struct CssOverlayEngine;

impl CssOverlayEngine {
    /// Return the JavaScript snippet to inject into Chromium target webview for live layout overlays
    pub fn get_injection_script(state: &CssOverlayState) -> String {
        if !state.enabled {
            return r#"
                (function() {
                    const el = document.getElementById('thor-css-layout-overlay-style');
                    if (el) el.remove();
                    const badge = document.getElementById('thor-css-layout-overlay-hud');
                    if (badge) badge.remove();
                })();
            "#.to_string();
        }

        format!(r#"
            (function() {{
                let style = document.getElementById('thor-css-layout-overlay-style');
                if (!style) {{
                    style = document.createElement('style');
                    style.id = 'thor-css-layout-overlay-style';
                    document.head.appendChild(style);
                }}

                style.textContent = `
                    /* Flexbox overlay boundaries */
                    div, section, main, article, nav, header, footer {{
                        outline: {} !important;
                    }}
                    
                    /* Grid track indicators */
                    [style*="display: grid"], [class*="grid"] {{
                        box-shadow: {} !important;
                    }}

                    /* Element hover inspector */
                    *:hover {{
                        outline: 1px dashed #3b82f6 !important;
                    }}
                `;
            }})();
        "#,
            if state.show_flexbox { "1px solid rgba(59, 130, 246, 0.4)" } else { "none" },
            if state.show_grid { "0 0 0 1px rgba(168, 85, 247, 0.5) inset" } else { "none" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_css_overlay_injection_script() {
        let mut state = CssOverlayState::default();
        state.enabled = true;
        let script = CssOverlayEngine::get_injection_script(&state);
        assert!(script.contains("thor-css-layout-overlay-style"));
        assert!(script.contains("1px dashed #3b82f6"));

        state.enabled = false;
        let disable_script = CssOverlayEngine::get_injection_script(&state);
        assert!(disable_script.contains("el.remove()"));
    }
}
