use std::collections::HashSet;
use thor::dx::cmd_palette::{
    get_default_bookmarks, parse_chrome_bookmarks, read_chrome_bookmarks_json,
    read_chrome_extensions, score_palette_item, CmdPalette, CmdPaletteMode, PaletteItem,
    PaletteItemType, SettingsRegistry, TabHistoryTracker,
};
use thor::dx::stack_detector::{DetectedStack, TechBadge, TechStackDetector};
use thor::net::locust::{LocustMetricScorer, LocustScoredMetrics, RequestMetric};
use thor::net::security_shield::SecurityShield;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // 1. SecurityShield Tracking Parameter Stripping Tests
    // =========================================================================

    #[test]
    fn test_security_shield_tracking_parameter_set_completeness() {
        let trackers: HashSet<&'static str> = SecurityShield::tracking_parameters();
        assert!(trackers.contains("utm_source"));
        assert!(trackers.contains("gclid"));
        assert!(trackers.contains("fbclid"));
        assert!(trackers.contains("msclkid"));
        assert!(trackers.contains("utm_medium"));
        assert!(trackers.contains("utm_campaign"));
        assert!(trackers.contains("yclid"));
        assert!(trackers.contains("si"));
        assert!(trackers.contains("igshid"));
    }

    #[test]
    fn test_security_shield_strip_all_four_required_trackers() {
        let dirty_url = "https://checkout.shop.com/pay?utm_source=newsletter&gclid=123456&fbclid=abcdef&msclkid=xyz789&cart_id=9999&currency=USD";
        let (clean_url, stripped) = SecurityShield::sanitize_url(dirty_url);
        assert!(stripped, "Expected tracking parameters to be detected and stripped");
        assert_eq!(clean_url, "https://checkout.shop.com/pay?cart_id=9999&currency=USD");
        assert!(!clean_url.contains("utm_source"));
        assert!(!clean_url.contains("gclid"));
        assert!(!clean_url.contains("fbclid"));
        assert!(!clean_url.contains("msclkid"));
    }

    #[test]
    fn test_security_shield_strip_individual_tracking_params() {
        let (url1, s1) = SecurityShield::sanitize_url("https://example.com/item?utm_source=twitter&ref_id=42");
        assert!(s1);
        assert_eq!(url1, "https://example.com/item?ref_id=42");

        let (url2, s2) = SecurityShield::sanitize_url("https://example.com/item?gclid=google123&session=active");
        assert!(s2);
        assert_eq!(url2, "https://example.com/item?session=active");

        let (url3, s3) = SecurityShield::sanitize_url("https://example.com/item?fbclid=fb456&lang=en");
        assert!(s3);
        assert_eq!(url3, "https://example.com/item?lang=en");

        let (url4, s4) = SecurityShield::sanitize_url("https://example.com/item?msclkid=ms789&tab=overview");
        assert!(s4);
        assert_eq!(url4, "https://example.com/item?tab=overview");
    }

    #[test]
    fn test_security_shield_case_insensitivity() {
        let dirty = "https://example.com/path?UTM_SOURCE=email&GCLID=999&FbClId=abc&MSCLKID=def&valid_key=true";
        let (clean, stripped) = SecurityShield::sanitize_url(dirty);
        assert!(stripped);
        assert_eq!(clean, "https://example.com/path?valid_key=true");
    }

    #[test]
    fn test_security_shield_urls_with_only_tracking_params() {
        let dirty = "https://example.com/landing?utm_source=google&gclid=123&fbclid=abc&msclkid=xyz";
        let (clean, stripped) = SecurityShield::sanitize_url(dirty);
        assert!(stripped);
        assert_eq!(clean, "https://example.com/landing");
    }

    #[test]
    fn test_security_shield_clean_url_unmodified() {
        let clean = "https://github.com/thor/thor?search=test&page=1";
        let (result, stripped) = SecurityShield::sanitize_url(clean);
        assert!(!stripped);
        assert_eq!(result, clean);
    }

    #[test]
    fn test_security_shield_url_without_query_string() {
        let clean = "https://thor.dev/documentation/index.html";
        let (result, stripped) = SecurityShield::sanitize_url(clean);
        assert!(!stripped);
        assert_eq!(result, clean);
    }

    #[test]
    fn test_security_shield_anti_tracking_headers() {
        let headers = SecurityShield::get_security_headers();
        assert!(headers.contains(&("DNT", "1")));
        assert!(headers.contains(&("Sec-GPC", "1")));
        assert!(headers.contains(&("X-Content-Type-Options", "nosniff")));
        assert!(headers.contains(&("X-Frame-Options", "SAMEORIGIN")));
    }

    // =========================================================================
    // 2. TechStackDetector Signature Matching Tests
    // =========================================================================

    #[test]
    fn test_tech_stack_detector_react_matching() {
        let html = r#"<!DOCTYPE html><html><body><div id="root" data-reactroot=""></div><script src="/static/js/react.js"></script></body></html>"#;
        let stack = TechStackDetector::detect("https://myreactapp.dev", html, None);
        assert_eq!(stack.url, "https://myreactapp.dev");
        assert!(stack.badges.iter().any(|b| b.name == "React" && b.category == "UI" && b.icon == "⚛️"));
    }

    #[test]
    fn test_tech_stack_detector_nextjs_matching() {
        let html = r#"<!DOCTYPE html><html><head><script id="__NEXT_DATA__" type="application/json">{"props":{}}</script></head><body><div id="__next"></div></body></html>"#;
        let stack = TechStackDetector::detect("https://nextjs.org", html, Some("Vercel"));
        assert!(stack.badges.iter().any(|b| b.name == "Next.js" && b.category == "Framework" && b.icon == "▲"));
        assert!(stack.badges.iter().any(|b| b.name == "React 18" && b.category == "UI" && b.icon == "⚛️"));
        assert_eq!(stack.server_header.as_deref(), Some("Vercel"));
    }

    #[test]
    fn test_tech_stack_detector_rust_wasm_matching() {
        let html = r#"<!DOCTYPE html><html><body><script type="module">import init from './app_bg.wasm'; init();</script></body></html>"#;
        let stack = TechStackDetector::detect("https://wasm-demo.org", html, None);
        assert!(stack.badges.iter().any(|b| b.name == "Rust (Wasm)" && b.category == "Language" && b.icon == "🦀"));

        let docs_stack = TechStackDetector::detect("https://docs.rs/tokio/latest", "<html></html>", None);
        assert!(docs_stack.badges.iter().any(|b| b.name == "Rust (Wasm)"));
    }

    #[test]
    fn test_tech_stack_detector_django_matching() {
        let html = r#"<!DOCTYPE html><html><body><h1>Django Portal</h1></body></html>"#;
        let stack = TechStackDetector::detect("https://django-app.com", html, Some("gunicorn/20.1.0 (Django 4.2)"));
        assert!(stack.badges.iter().any(|b| b.name == "Python (Django)" && b.category == "Backend" && b.icon == "🐍"));

        let uvicorn_stack = TechStackDetector::detect("https://fastapi-django.com", html, Some("uvicorn"));
        assert!(uvicorn_stack.badges.iter().any(|b| b.name == "Python (Django)"));
    }

    #[test]
    fn test_tech_stack_detector_vue_and_nuxt_matching() {
        // Pure Vue test
        let vue_html = r#"<div id="app" data-v-123456>Vue Component</div>"#;
        let vue_stack = TechStackDetector::detect("https://vuejs.org", vue_html, None);
        assert!(vue_stack.badges.iter().any(|b| b.name == "Vue.js" && b.category == "UI" && b.icon == "🟢"));

        // Nuxt test
        let nuxt_html = r#"<div id="__nuxt"><script src="/_nuxt/entry.js"></script></div>"#;
        let nuxt_stack = TechStackDetector::detect("https://nuxt.com", nuxt_html, None);
        assert!(nuxt_stack.badges.iter().any(|b| b.name == "Nuxt" && b.category == "Framework" && b.icon == "💚"));
        assert!(nuxt_stack.badges.iter().any(|b| b.name == "Vue.js" && b.category == "UI" && b.icon == "🟢"));
    }

    #[test]
    fn test_tech_stack_detector_tailwind_matching() {
        let html = r#"<div class="flex items-center justify-between p-4 bg-slate-900 text-white tailwindcss">Tailwind UI</div>"#;
        let stack = TechStackDetector::detect("https://tailwind-ui.com", html, None);
        assert!(stack.badges.iter().any(|b| b.name == "Tailwind CSS" && b.category == "Styling" && b.icon == "🎨"));
    }

    #[test]
    fn test_tech_stack_detector_combined_full_stack() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <script id="__NEXT_DATA__">{"props":{}}</script>
                    <link rel="stylesheet" href="/tailwindcss.css">
                </head>
                <body>
                    <div class="flex items-center">
                        <script src="/app_bg.wasm"></script>
                    </div>
                </body>
            </html>
        "#;
        let stack = TechStackDetector::detect("https://fullstack.app", html, Some("gunicorn/20.1.0"));

        let names: Vec<&str> = stack.badges.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"Next.js"));
        assert!(names.contains(&"React 18"));
        assert!(names.contains(&"Rust (Wasm)"));
        assert!(names.contains(&"Tailwind CSS"));
        assert!(names.contains(&"Python (Django)"));
    }

    #[test]
    fn test_tech_stack_detector_fallbacks() {
        let generic_stack = TechStackDetector::detect("https://random-website.xyz", "<html><body>Hello</body></html>", None);
        assert_eq!(generic_stack.badges.len(), 1);
        assert_eq!(generic_stack.badges[0].name, "HTML5 / JavaScript");

        let github_stack = TechStackDetector::detect("https://github.com/thor", "", None);
        assert_eq!(github_stack.badges[0].name, "Ruby / Go / React");
    }

    // =========================================================================
    // 3. Chrome Extensions Parser & Cmd+Shift+K Menu Registry Tests
    // =========================================================================

    #[test]
    fn test_chrome_extensions_parser_items() {
        let extensions = read_chrome_extensions();
        assert_eq!(extensions.len(), 5, "Expected 5 default Chrome extensions");

        for ext in &extensions {
            assert_eq!(ext.item_type, PaletteItemType::Setting);
            assert_eq!(ext.category, "Extensions");
            assert_eq!(ext.icon.as_deref(), Some("🧩"));
            assert_eq!(ext.badge.as_deref(), Some("Extension"));
            assert!(ext.path_or_url.starts_with("chrome-extension://"));
            assert!(ext.action_key.is_some());
        }

        let extension_ids: Vec<&str> = extensions.iter().map(|e| e.id.as_str()).collect();
        assert!(extension_ids.contains(&"ext_react_devtools"));
        assert!(extension_ids.contains(&"ext_redux_devtools"));
        assert!(extension_ids.contains(&"ext_vue_devtools"));
        assert!(extension_ids.contains(&"ext_ublock"));
        assert!(extension_ids.contains(&"ext_tailwind"));
    }

    #[test]
    fn test_cmd_shift_k_menu_registry_integration() {
        // Cmd+Shift+K opens settings mode CmdPaletteMode::SettingsMenu
        let mut palette = CmdPalette::new(CmdPaletteMode::SettingsMenu);
        assert_eq!(palette.mode, CmdPaletteMode::SettingsMenu);

        let settings_and_exts = SettingsRegistry::get_all_settings();
        assert!(settings_and_exts.len() >= 16, "Settings registry should contain built-in settings and extensions");

        palette.set_items(settings_and_exts);

        // Search for built-in setting (Security Shield)
        palette.set_query("Security".into());
        let sec_results = palette.search();
        assert!(!sec_results.is_empty());
        assert!(sec_results[0].title.contains("Security & Anti-Tracking"));

        // Search for built-in setting (Locust Swarm)
        palette.set_query("Locust".into());
        let locust_results = palette.search();
        assert!(!locust_results.is_empty());
        assert!(locust_results[0].title.contains("Locust HTTP Swarm"));

        // Search for extension (React DevTools)
        palette.set_query("React DevTools".into());
        let react_ext_results = palette.search();
        assert!(!react_ext_results.is_empty());
        assert_eq!(react_ext_results[0].id, "ext_react_devtools");

        // Search for extension (uBlock)
        palette.set_query("uBlock".into());
        let ublock_results = palette.search();
        assert!(!ublock_results.is_empty());
        assert_eq!(ublock_results[0].id, "ext_ublock");
    }

    #[test]
    fn test_chrome_bookmarks_parser_structure() {
        let json_data = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "bookmark_bar",
                    "children": [
                        { "type": "url", "name": "Rust Language", "url": "https://www.rust-lang.org" },
                        { "type": "url", "name": "GitHub Dev", "url": "https://github.com/developer" }
                    ]
                }
            }
        }"#;

        let bookmarks = parse_chrome_bookmarks(json_data);
        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].title, "Rust Language");
        assert_eq!(bookmarks[0].item_type, PaletteItemType::Bookmark);
        assert_eq!(bookmarks[1].title, "GitHub Dev");
        assert_eq!(bookmarks[1].icon.as_deref(), Some("🐙"));
    }

    #[test]
    fn test_cmd_k_palette_bookmarks_and_tab_history() {
        let mut palette = CmdPalette::new(CmdPaletteMode::BookmarksAndTabHistory);
        assert_eq!(palette.mode, CmdPaletteMode::BookmarksAndTabHistory);

        let mut tracker = TabHistoryTracker::new();
        tracker.record_visit("tab_1", "https://google.com", "Google");
        tracker.record_visit("tab_1", "https://news.ycombinator.com", "Hacker News");

        let history = tracker.get_history_for_tab("tab_1");
        assert_eq!(history.len(), 2);

        let chrome_bookmarks = get_default_bookmarks();
        assert!(!chrome_bookmarks.is_empty());

        let mut combined = Vec::new();
        for h in history {
            combined.push(PaletteItem::new(
                format!("hist_{}", h.url),
                h.title,
                PaletteItemType::TabHistory,
                h.url,
                Some("Tab History".into()),
                Some("🌐".into()),
                "Tab History",
                None,
            ));
        }
        combined.extend(chrome_bookmarks);

        palette.set_items(combined);
        palette.set_query("hacker".into());

        let results = palette.search();
        assert!(!results.is_empty());
        assert_eq!(results[0].title, "Hacker News");
        assert_eq!(results[0].item_type, PaletteItemType::TabHistory);
    }

    #[test]
    fn test_fuzzy_score_ranking() {
        let exact_score = score_palette_item("github", "github", "https://github.com", None);
        let prefix_score = score_palette_item("git", "github", "https://github.com", None);
        let sub_score = score_palette_item("hub", "github", "https://github.com", None);

        assert!(exact_score > prefix_score);
        assert!(prefix_score > sub_score);
    }

    // =========================================================================
    // 4. Locust Load Test Config & Percentile Calculations Tests
    // =========================================================================

    #[test]
    fn test_request_metric_creation() {
        let metric = RequestMetric::new(1500, 200, true, 2048);
        assert_eq!(metric.response_time_us, 1500);
        assert_eq!(metric.status_code, 200);
        assert!(metric.is_success);
        assert_eq!(metric.bytes_received, 2048);
    }

    #[test]
    fn test_locust_metric_scorer_empty_metrics() {
        let scored = LocustMetricScorer::score_metrics(&[], 10.0);
        assert_eq!(scored.total_requests, 0);
        assert_eq!(scored.success_requests, 0);
        assert_eq!(scored.failed_requests, 0);
        assert_eq!(scored.rps, 0.0);
        assert_eq!(scored.error_rate, 0.0);
        assert_eq!(scored.min_latency_us, 0);
        assert_eq!(scored.max_latency_us, 0);
        assert_eq!(scored.p50_latency_us, 0);
        assert_eq!(scored.p95_latency_us, 0);
        assert_eq!(scored.score, 0.0);
    }

    #[test]
    fn test_locust_metric_scorer_percentile_accuracy() {
        // Create 1000 request metrics with latencies strictly 10, 20, 30, ..., 10000 microseconds
        let mut metrics = Vec::with_capacity(1000);
        for i in 1..=1000 {
            metrics.push(RequestMetric::new(i * 10, 200, true, 512));
        }

        let duration_secs = 5.0;
        let scored = LocustMetricScorer::score_metrics(&metrics, duration_secs);

        assert_eq!(scored.total_requests, 1000);
        assert_eq!(scored.success_requests, 1000);
        assert_eq!(scored.failed_requests, 0);
        assert_eq!(scored.rps, 200.0); // 1000 requests / 5 secs = 200 RPS
        assert_eq!(scored.error_rate, 0.0);

        assert_eq!(scored.min_latency_us, 10);
        assert_eq!(scored.max_latency_us, 10000);
        assert_eq!(scored.avg_latency_us, 5005.0);

        // Percentiles:
        // p50 index = 1000 * 0.50 = 500 -> 501st element (value: 5010)
        // p90 index = 1000 * 0.90 = 900 -> 901st element (value: 9010)
        // p95 index = 1000 * 0.95 = 950 -> 951st element (value: 9510)
        // p99 index = 1000 * 0.99 = 990 -> 991st element (value: 9910)
        assert_eq!(scored.p50_latency_us, 5010);
        assert_eq!(scored.p90_latency_us, 9010);
        assert_eq!(scored.p95_latency_us, 9510);
        assert_eq!(scored.p99_latency_us, 9910);
    }

    #[test]
    fn test_locust_metric_scorer_failures_and_score_penalty() {
        let mut metrics = Vec::new();
        // 80 successful requests with 500us latency
        for _ in 0..80 {
            metrics.push(RequestMetric::new(500, 200, true, 100));
        }
        // 20 failed requests with 5000us latency and 500 status code
        for _ in 0..20 {
            metrics.push(RequestMetric::new(5000, 500, false, 0));
        }

        let scored = LocustMetricScorer::score_metrics(&metrics, 10.0);

        assert_eq!(scored.total_requests, 100);
        assert_eq!(scored.success_requests, 80);
        assert_eq!(scored.failed_requests, 20);
        assert_eq!(scored.error_rate, 20.0); // 20% error rate

        // Error penalty is error_rate * 2.0 = 40.0
        // Score is lower than 100.0 because of error penalty
        assert!(scored.score < 65.0, "Score should reflect error rate penalty");
    }

    #[test]
    fn test_locust_metric_scorer_zero_duration_guard() {
        let metrics = vec![RequestMetric::new(100, 200, true, 100)];
        let scored = LocustMetricScorer::score_metrics(&metrics, 0.0);
        assert_eq!(scored.rps, 0.0);
    }

    #[test]
    fn test_locust_high_volume_swarm_simulation() {
        // Simulate a 10,000 request swarm load test
        let mut metrics = Vec::with_capacity(10000);
        for i in 0..10000 {
            let status = if i % 100 == 0 { 503 } else { 200 };
            let is_success = status == 200;
            let latency = (i % 200) * 50 + 100; // 100us to 10050us
            metrics.push(RequestMetric::new(latency as u64, status, is_success, 1024));
        }

        let scored = LocustMetricScorer::score_metrics(&metrics, 60.0);
        assert_eq!(scored.total_requests, 10000);
        assert_eq!(scored.failed_requests, 100);
        assert_eq!(scored.success_requests, 9900);
        assert_eq!(scored.error_rate, 1.0);
        assert!(scored.rps > 166.0 && scored.rps < 167.0);
    }
}
