use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TechBadge {
    pub name: String,
    pub category: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetectedStack {
    pub url: String,
    pub badges: Vec<TechBadge>,
    pub server_header: Option<String>,
}

pub struct TechStackDetector;

impl TechStackDetector {
    /// Detect tech stack from URL, meta generator tag, HTML content snippet, or response server header
    pub fn detect(url: &str, html_snippet: &str, server_header: Option<&str>) -> DetectedStack {
        let mut headers = HashMap::new();
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
        let mut badges = Vec::new();
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

        // 1. Next.js & React
        if html_lower.contains("__next_data__")
            || html_lower.contains("_next/static")
            || url_lower.contains("nextjs.org")
            || x_powered_by.as_deref().map_or(false, |s| s.to_lowercase().contains("next.js"))
        {
            badges.push(TechBadge {
                name: "Next.js".into(),
                category: "Framework".into(),
                icon: "▲".into(),
            });
            badges.push(TechBadge {
                name: "React 18".into(),
                category: "UI".into(),
                icon: "⚛️".into(),
            });
        } else if html_lower.contains("react") || html_lower.contains("data-reactroot") {
            badges.push(TechBadge {
                name: "React".into(),
                category: "UI".into(),
                icon: "⚛️".into(),
            });
        }

        // 2. Vue.js & Nuxt
        if html_lower.contains("__nuxt") || html_lower.contains("_nuxt/") {
            badges.push(TechBadge {
                name: "Nuxt".into(),
                category: "Framework".into(),
                icon: "💚".into(),
            });
            badges.push(TechBadge {
                name: "Vue.js".into(),
                category: "UI".into(),
                icon: "🟢".into(),
            });
        } else if html_lower.contains("data-v-") || html_lower.contains("vue") {
            badges.push(TechBadge {
                name: "Vue.js".into(),
                category: "UI".into(),
                icon: "🟢".into(),
            });
        }

        // 3. Svelte & SvelteKit
        if html_lower.contains("__svelte") || html_lower.contains(".svelte") || html_lower.contains("sveltekit") {
            badges.push(TechBadge {
                name: "SvelteKit / Svelte".into(),
                category: "Framework".into(),
                icon: "🧡".into(),
            });
        }

        // 4. Angular
        if html_lower.contains("ng-version") || html_lower.contains("ng-app") || html_lower.contains("angular") {
            badges.push(TechBadge {
                name: "Angular".into(),
                category: "Framework".into(),
                icon: "🅰️".into(),
            });
        }

        // 5. Astro
        if html_lower.contains("astro") || html_lower.contains("data-astro-cid") {
            badges.push(TechBadge {
                name: "Astro".into(),
                category: "Framework".into(),
                icon: "🚀".into(),
            });
        }

        // 6. Rust & WebAssembly
        if html_lower.contains(".wasm")
            || html_lower.contains("wasm_bindgen")
            || html_lower.contains("yew")
            || html_lower.contains("leptos")
            || url_lower.contains("docs.rs")
        {
            badges.push(TechBadge {
                name: "Rust (Wasm)".into(),
                category: "Language".into(),
                icon: "🦀".into(),
            });
        }

        // 7. Tailwind CSS & Bootstrap
        if html_lower.contains("tailwindcss")
            || html_lower.contains("tailwind")
            || html_snippet.contains("flex items-center")
        {
            badges.push(TechBadge {
                name: "Tailwind CSS".into(),
                category: "Styling".into(),
                icon: "🎨".into(),
            });
        } else if html_lower.contains("bootstrap") {
            badges.push(TechBadge {
                name: "Bootstrap".into(),
                category: "Styling".into(),
                icon: "🟣".into(),
            });
        }

        // 8. TypeScript & htmx & Alpine.js
        if html_lower.contains("typescript") || html_lower.contains(".ts") {
            badges.push(TechBadge {
                name: "TypeScript".into(),
                category: "Language".into(),
                icon: "🔷".into(),
            });
        }

        if html_lower.contains("htmx") || html_lower.contains("hx-get") || html_lower.contains("hx-post") {
            badges.push(TechBadge {
                name: "htmx".into(),
                category: "Library".into(),
                icon: "⚡".into(),
            });
        }

        if html_lower.contains("alpine") || html_lower.contains("x-data") {
            badges.push(TechBadge {
                name: "Alpine.js".into(),
                category: "Library".into(),
                icon: "🏔️".into(),
            });
        }

        // 9. Meta Generator Tag Inspection
        if html_lower.contains("wordpress") || html_lower.contains("wp-content") {
            badges.push(TechBadge {
                name: "WordPress (PHP)".into(),
                category: "CMS".into(),
                icon: "📝".into(),
            });
        } else if html_lower.contains("hugo") {
            badges.push(TechBadge {
                name: "Hugo (Go)".into(),
                category: "SSG".into(),
                icon: "🐹".into(),
            });
        } else if html_lower.contains("jekyll") {
            badges.push(TechBadge {
                name: "Jekyll (Ruby)".into(),
                category: "SSG".into(),
                icon: "💎".into(),
            });
        }

        // 10. Backend Languages & Frameworks from Headers
        if let Some(srv) = &server_header {
            let srv_lower = srv.to_lowercase();
            if srv_lower.contains("gunicorn") || srv_lower.contains("uvicorn") || srv_lower.contains("django") {
                badges.push(TechBadge {
                    name: "Python (Django)".into(),
                    category: "Backend".into(),
                    icon: "🐍".into(),
                });
            } else if srv_lower.contains("puma") || srv_lower.contains("phusion") {
                badges.push(TechBadge {
                    name: "Ruby on Rails".into(),
                    category: "Backend".into(),
                    icon: "💎".into(),
                });
            } else if srv_lower.contains("express") || srv_lower.contains("node") || srv_lower.contains("vercel") {
                badges.push(TechBadge {
                    name: "Node.js / Vercel".into(),
                    category: "Runtime".into(),
                    icon: "🟢".into(),
                });
            }
        }

        if let Some(pow) = &x_powered_by {
            let pow_lower = pow.to_lowercase();
            if pow_lower.contains("express") {
                if !badges.iter().any(|b| b.name.contains("Node.js")) {
                    badges.push(TechBadge {
                        name: "Node.js / Express".into(),
                        category: "Backend".into(),
                        icon: "🟢".into(),
                    });
                }
            } else if pow_lower.contains("php") || pow_lower.contains("laravel") {
                if !badges.iter().any(|b| b.name.contains("WordPress")) {
                    badges.push(TechBadge {
                        name: "PHP / Laravel".into(),
                        category: "Backend".into(),
                        icon: "🐘".into(),
                    });
                }
            }
        }

        // Default fallback if no specific framework detected
        if badges.is_empty() {
            if url_lower.contains("github") {
                badges.push(TechBadge { name: "Ruby / Go / React".into(), category: "Stack".into(), icon: "🐙".into() });
            } else if url_lower.contains("google") {
                badges.push(TechBadge { name: "C++ / Closure / Go".into(), category: "Stack".into(), icon: "🔍".into() });
            } else {
                badges.push(TechBadge { name: "HTML5 / JavaScript".into(), category: "Stack".into(), icon: "⚡".into() });
            }
        }

        DetectedStack {
            url: url.to_string(),
            badges,
            server_header,
        }
    }
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
        let html = r#"<meta name="generator" content="WordPress 6.2"><div class="wp-content"></div>"#;
        let mut headers = HashMap::new();
        headers.insert("x-powered-by".to_string(), "PHP/8.1".to_string());
        let stack = TechStackDetector::detect_with_headers("https://example.com", html, &headers);
        assert!(stack.badges.iter().any(|b| b.name.contains("WordPress")));
    }
}

