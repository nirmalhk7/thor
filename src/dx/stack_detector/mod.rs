mod rules;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TechBadge {
    pub name: String,
    pub category: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RenderingMode {
    Ssr,
    Csr,
    Hybrid,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevComponentLocation {
    pub component_name: String,
    pub file_path: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetectedStack {
    pub url: String,
    pub badges: Vec<TechBadge>,
    pub server_header: Option<String>,
    pub rendering_mode: RenderingMode,
    pub dev_components: Vec<DevComponentLocation>,
}

pub struct TechStackDetector;

impl TechStackDetector {
    /// Detect tech stack from URL, meta generator tag, HTML content snippet, or response server header
    pub fn detect(url: &str, html_snippet: &str, server_header: Option<&str>) -> DetectedStack {
        let mut headers = HashMap::with_capacity(server_header.map_or(0, |_| 1));
        if let Some(srv) = server_header {
            headers.insert("server".to_string(), srv.to_string());
        }
        Self::detect_with_headers(url, html_snippet, &headers)
    }

    /// Detect tech stack with full inspection of DOM meta/script tags and HTTP headers map
    pub fn detect_with_headers(
        url: &str,
        html_snippet: &str,
        headers: &HashMap<String, String>,
    ) -> DetectedStack {
        let html_lower = html_snippet.to_lowercase();
        let url_lower = url.to_lowercase();

        let server_header = headers
            .get("server")
            .or_else(|| headers.get("Server"))
            .cloned();

        let x_powered_by = headers
            .get("x-powered-by")
            .or_else(|| headers.get("X-Powered-By"))
            .cloned();

        let mut badges = rules::detect(
            html_snippet,
            &html_lower,
            &url_lower,
            server_header.as_deref(),
            x_powered_by.as_deref(),
        );

        let rendering_mode = detect_rendering_mode(&html_lower, &badges);

        let dev_components = extract_dev_components(html_snippet);

        if badges.is_empty() {
            if url_lower.contains("github.com") {
                badges.push(TechBadge {
                    name: "Ruby / Go / React".into(),
                    category: "Stack".into(),
                    icon: "🐙".into(),
                });
            } else {
                badges.push(TechBadge {
                    name: "HTML5 / JavaScript".into(),
                    category: "Web".into(),
                    icon: "🌐".into(),
                });
            }
        }

        DetectedStack {
            url: url.to_string(),
            badges,
            server_header,
            rendering_mode,
            dev_components,
        }
    }
}

fn detect_rendering_mode(html_lower: &str, badges: &[TechBadge]) -> RenderingMode {
    if html_lower.contains("__next_data__")
        || html_lower.contains("__nuxt")
        || html_lower.contains("window.__initial_state__")
        || html_lower.contains("data-reactroot")
    {
        RenderingMode::Ssr
    } else if html_lower.contains("id=\"root\"")
        || html_lower.contains("id=\"app\"")
        || html_lower.contains("<div id=\"app\"></div>")
    {
        RenderingMode::Csr
    } else if badges.iter().any(|b| b.category == "Framework") {
        RenderingMode::Hybrid
    } else {
        RenderingMode::Unknown
    }
}

fn extract_dev_components(html_snippet: &str) -> Vec<DevComponentLocation> {
    // Extract IDE component file locations (e.g. data-source-file="src/App.tsx:42")
    let mut dev_components = Vec::new();
    for marker in &["data-source-file=\"", "data-file=\"", "_jsxFileName=\""] {
        let mut search_idx = 0;
        while let Some(pos) = html_snippet[search_idx..].find(marker) {
            let start = search_idx + pos + marker.len();
            if let Some(end) = html_snippet[start..].find('"') {
                let raw_val = &html_snippet[start..start + end];
                let parts: Vec<&str> = raw_val.split(':').collect();
                let file_path = parts.first().unwrap_or(&raw_val).to_string();
                let line: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                let comp_name = std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Component")
                    .to_string();

                dev_components.push(DevComponentLocation {
                    component_name: comp_name,
                    file_path,
                    line,
                });
            }
            search_idx = start;
            if dev_components.len() >= 10 {
                break;
            }
        }
    }

    dev_components
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_nextjs_react_tailwind() {
        let html = r#"<script id="__NEXT_DATA__">{"props":{}}</script><div class="flex items-center"></div>"#;
        let stack = TechStackDetector::detect("https://nextjs.org", html, Some("Vercel"));
        assert!(stack.badges.iter().any(|b| b.name == "Next.js"));
        assert!(stack.badges.iter().any(|b| b.name.contains("React")));
        assert!(stack.badges.iter().any(|b| b.name == "Tailwind CSS"));
    }

    #[test]
    fn test_detect_rust_wasm() {
        let html = r#"<script src="app_bg.wasm"></script>"#;
        let stack = TechStackDetector::detect("https://docs.rs", html, None);
        assert!(stack.badges.iter().any(|b| b.name.contains("Rust")));
    }

    #[test]
    fn test_detect_with_headers_and_meta() {
        let html =
            r#"<meta name="generator" content="WordPress 6.2"><div class="wp-content"></div>"#;
        let mut headers = HashMap::new();
        headers.insert("x-powered-by".to_string(), "PHP/8.1".to_string());
        let stack = TechStackDetector::detect_with_headers("https://example.com", html, &headers);
        assert!(stack.badges.iter().any(|b| b.name.contains("WordPress")));
    }
}
