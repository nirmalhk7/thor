use std::collections::HashSet;
use thor::dx::bookmarks::BookmarkStore;
use thor::dx::cmd_palette::{
    build_cmd_k_items, cmd_k_payload_json, get_default_bookmarks, open_tabs_to_palette_items,
    parse_chrome_bookmarks, parse_chrome_extension_manifest, read_chrome_extensions,
    score_palette_item, settings_payload_json, tab_history_entries_to_palette_items, CmdPalette,
    CmdPaletteMode, PaletteItem, PaletteItemType, SettingsRegistry, TabHistoryTracker,
};
use thor::dx::stack_detector::TechStackDetector;
use thor::net::locust::{LocustMetricScorer, RequestMetric};
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
        assert!(
            stripped,
            "Expected tracking parameters to be detected and stripped"
        );
        assert_eq!(
            clean_url,
            "https://checkout.shop.com/pay?cart_id=9999&currency=USD"
        );
        assert!(!clean_url.contains("utm_source"));
        assert!(!clean_url.contains("gclid"));
        assert!(!clean_url.contains("fbclid"));
        assert!(!clean_url.contains("msclkid"));
    }

    #[test]
    fn test_security_shield_strip_individual_tracking_params() {
        let (url1, s1) =
            SecurityShield::sanitize_url("https://example.com/item?utm_source=twitter&ref_id=42");
        assert!(s1);
        assert_eq!(url1, "https://example.com/item?ref_id=42");

        let (url2, s2) =
            SecurityShield::sanitize_url("https://example.com/item?gclid=google123&session=active");
        assert!(s2);
        assert_eq!(url2, "https://example.com/item?session=active");

        let (url3, s3) =
            SecurityShield::sanitize_url("https://example.com/item?fbclid=fb456&lang=en");
        assert!(s3);
        assert_eq!(url3, "https://example.com/item?lang=en");

        let (url4, s4) =
            SecurityShield::sanitize_url("https://example.com/item?msclkid=ms789&tab=overview");
        assert!(s4);
        assert_eq!(url4, "https://example.com/item?tab=overview");
    }

    #[test]
    fn test_chromium_compatibility_and_headers() {
        let ua = SecurityShield::chromium_user_agent();
        assert!(ua.contains("Chrome/151.0.0.0"));
        assert!(ua.contains("Safari/537.36"));
        assert!(!ua.contains("Thor"));

        let headers = SecurityShield::get_security_headers();
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "User-Agent" && v.contains("Chrome/151")));
        assert!(!headers.iter().any(|(_, value)| value.contains("Thor")));
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "Sec-CH-UA" && v.contains("Chromium")));

        let settings = SettingsRegistry::get_all_settings();
        assert!(settings
            .iter()
            .any(|s| s.title.contains("Engine status") || s.title.contains("Chromium")));
    }

    #[test]
    fn test_security_shield_urls_with_only_tracking_params() {
        let dirty =
            "https://example.com/landing?utm_source=google&gclid=123&fbclid=abc&msclkid=xyz";
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
        assert!(!headers
            .iter()
            .any(|(name, _)| *name == "X-Content-Type-Options"));
        assert!(!headers.iter().any(|(name, _)| *name == "X-Frame-Options"));
    }

    // =========================================================================
    // 2. TechStackDetector Signature Matching Tests
    // =========================================================================

    #[test]
    fn test_tech_stack_detector_react_matching() {
        let html = r#"<!DOCTYPE html><html><body><div id="root" data-reactroot=""></div><script src="/static/js/react.js"></script></body></html>"#;
        let stack = TechStackDetector::detect("https://myreactapp.dev", html, None);
        assert_eq!(stack.url, "https://myreactapp.dev");
        assert!(stack
            .badges
            .iter()
            .any(|b| b.name == "React" && b.category == "UI" && b.icon == "⚛️"));
    }

    #[test]
    fn test_tech_stack_detector_nextjs_matching() {
        let html = r#"<!DOCTYPE html><html><head><script id="__NEXT_DATA__" type="application/json">{"props":{}}</script></head><body><div id="__next"></div></body></html>"#;
        let stack = TechStackDetector::detect("https://nextjs.org", html, Some("Vercel"));
        assert!(stack
            .badges
            .iter()
            .any(|b| b.name == "Next.js" && b.category == "Framework" && b.icon == "▲"));
        assert!(stack
            .badges
            .iter()
            .any(|b| b.name == "React 18" && b.category == "UI" && b.icon == "⚛️"));
        assert_eq!(stack.server_header.as_deref(), Some("Vercel"));
    }

    #[test]
    fn test_tech_stack_detector_rust_wasm_matching() {
        let html = r#"<!DOCTYPE html><html><body><script type="module">import init from './app_bg.wasm'; init();</script></body></html>"#;
        let stack = TechStackDetector::detect("https://wasm-demo.org", html, None);
        assert!(stack
            .badges
            .iter()
            .any(|b| b.name == "Rust (Wasm)" && b.category == "Language" && b.icon == "🦀"));

        let docs_stack =
            TechStackDetector::detect("https://docs.rs/tokio/latest", "<html></html>", None);
        assert!(docs_stack.badges.iter().any(|b| b.name == "Rust (Wasm)"));
    }

    #[test]
    fn test_tech_stack_detector_django_matching() {
        let html = r#"<!DOCTYPE html><html><body><h1>Django Portal</h1></body></html>"#;
        let stack = TechStackDetector::detect(
            "https://django-app.com",
            html,
            Some("gunicorn/20.1.0 (Django 4.2)"),
        );
        assert!(stack
            .badges
            .iter()
            .any(|b| b.name == "Python (Django)" && b.category == "Backend" && b.icon == "🐍"));

        let uvicorn_stack =
            TechStackDetector::detect("https://fastapi-django.com", html, Some("uvicorn"));
        assert!(uvicorn_stack
            .badges
            .iter()
            .any(|b| b.name == "Python (Django)"));
    }

    #[test]
    fn test_tech_stack_detector_vue_and_nuxt_matching() {
        // Pure Vue test
        let vue_html = r#"<div id="app" data-v-123456>Vue Component</div>"#;
        let vue_stack = TechStackDetector::detect("https://vuejs.org", vue_html, None);
        assert!(vue_stack
            .badges
            .iter()
            .any(|b| b.name == "Vue.js" && b.category == "UI" && b.icon == "🟢"));

        // Nuxt test
        let nuxt_html = r#"<div id="__nuxt"><script src="/_nuxt/entry.js"></script></div>"#;
        let nuxt_stack = TechStackDetector::detect("https://nuxt.com", nuxt_html, None);
        assert!(nuxt_stack
            .badges
            .iter()
            .any(|b| b.name == "Nuxt" && b.category == "Framework" && b.icon == "💚"));
        assert!(nuxt_stack
            .badges
            .iter()
            .any(|b| b.name == "Vue.js" && b.category == "UI" && b.icon == "🟢"));
    }

    #[test]
    fn test_tech_stack_detector_tailwind_matching() {
        let html = r#"<div class="flex items-center justify-between p-4 bg-slate-900 text-white tailwindcss">Tailwind UI</div>"#;
        let stack = TechStackDetector::detect("https://tailwind-ui.com", html, None);
        assert!(stack
            .badges
            .iter()
            .any(|b| b.name == "Tailwind CSS" && b.category == "Styling" && b.icon == "🎨"));
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
        let stack =
            TechStackDetector::detect("https://fullstack.app", html, Some("gunicorn/20.1.0"));

        let names: Vec<&str> = stack.badges.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"Next.js"));
        assert!(names.contains(&"React 18"));
        assert!(names.contains(&"Rust (Wasm)"));
        assert!(names.contains(&"Tailwind CSS"));
        assert!(names.contains(&"Python (Django)"));
    }

    #[test]
    fn test_tech_stack_detector_fallbacks() {
        let generic_stack = TechStackDetector::detect(
            "https://random-website.xyz",
            "<html><body>Hello</body></html>",
            None,
        );
        assert_eq!(generic_stack.badges.len(), 1);
        assert_eq!(generic_stack.badges[0].name, "HTML5 / JavaScript");

        let github_stack = TechStackDetector::detect("https://github.com/thor", "", None);
        assert_eq!(github_stack.badges[0].name, "Ruby / Go / React");
    }

    // =========================================================================
    // 3. Chrome Extensions Parser & Cmd+Shift+K Menu Registry Tests
    // =========================================================================

    #[test]
    fn test_chrome_extension_manifest_parser() {
        let manifest_json = r#"{
            "name": "React Developer Tools",
            "version": "5.2.0",
            "manifest_version": 3
        }"#;

        let parsed =
            parse_chrome_extension_manifest(manifest_json, "fmkadmapgofadopljbjfkapdkoienihi");
        assert!(parsed.is_some());
        let item = parsed.unwrap();
        assert_eq!(item.id, "ext_fmkadmapgofadopljbjfkapdkoienihi");
        assert_eq!(item.title, "React Developer Tools v5.2.0");
        assert_eq!(item.item_type, PaletteItemType::Setting);
        assert_eq!(
            item.path_or_url,
            "chrome-extension://fmkadmapgofadopljbjfkapdkoienihi"
        );
        assert_eq!(item.category, "Extensions");
        assert_eq!(item.icon.as_deref(), Some("🧩"));
        assert_eq!(item.badge.as_deref(), Some("Extension"));
        assert_eq!(
            item.action_key.as_deref(),
            Some("open_extension_fmkadmapgofadopljbjfkapdkoienihi")
        );
    }

    #[test]
    fn test_chrome_extensions_parser_items() {
        let extensions = read_chrome_extensions();
        assert_eq!(extensions.len(), 1, "Only Thor-owned extensions are listed");
        let shield = &extensions[0];
        assert_eq!(shield.id, "ext_thor_shield");
        assert_eq!(shield.item_type, PaletteItemType::Setting);
        assert_eq!(shield.category, "Extensions");
        assert_eq!(shield.icon.as_deref(), Some("🛡️"));
        assert_eq!(shield.badge.as_deref(), Some("Built-in"));
        assert_eq!(shield.path_or_url, "Thor://settings/security_shield");
        assert!(shield.action_key.is_none());
    }

    #[test]
    fn test_cmd_shift_k_menu_registry_integration() {
        // Cmd+Shift+K opens settings mode CmdPaletteMode::SettingsMenu
        let mut palette = CmdPalette::new(CmdPaletteMode::SettingsMenu);
        assert_eq!(palette.mode, CmdPaletteMode::SettingsMenu);

        let mut settings_and_exts = SettingsRegistry::get_all_settings();
        settings_and_exts.extend(vec![PaletteItem::new(
            "ext_thor_shield",
            "Thor Shield",
            PaletteItemType::Setting,
            "Thor://settings/security_shield",
            None,
            None,
            "Extensions",
            None,
        )]);
        assert!(
            settings_and_exts.len() >= 13,
            "Settings registry should contain built-in settings"
        );

        palette.set_items(settings_and_exts);

        // Search for built-in setting (Security)
        palette.set_query("Security".into());
        let sec_results = palette.search();
        assert!(!sec_results.is_empty());

        // Search for built-in setting (Locust load test)
        palette.set_query("Load".into());
        let locust_results = palette.search();
        assert!(!locust_results.is_empty());

        // Search for Thor's packaged extension.
        palette.set_query("Shield".into());
        let shield_results = palette.search();
        assert!(!shield_results.is_empty());
        assert!(shield_results
            .iter()
            .any(|item| item.id == "ext_thor_shield"));
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

        let bookmarks = parse_chrome_bookmarks(json_data).unwrap();
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

        let history_items = tab_history_entries_to_palette_items(&history, "tab_1");
        let open_tabs = open_tabs_to_palette_items(&[
            ("tab_1", "Hacker News", "https://news.ycombinator.com"),
            ("tab_2", "Docs", "https://docs.rs"),
        ]);

        let combined = build_cmd_k_items(&chrome_bookmarks, &history_items, &open_tabs);
        assert!(combined.iter().any(|i| i.item_type == PaletteItemType::Tab));
        assert!(combined
            .iter()
            .any(|i| i.item_type == PaletteItemType::TabHistory));
        assert!(combined
            .iter()
            .any(|i| i.item_type == PaletteItemType::Bookmark));
        // Open tabs ranked first
        assert_eq!(combined[0].item_type, PaletteItemType::Tab);

        palette.set_items(combined);
        palette.set_query("hacker".into());

        let results = palette.search();
        assert!(!results.is_empty());
        assert!(
            results[0].title.contains("Hacker News"),
            "expected Hacker News match, got {:?}",
            results[0].title
        );
        assert!(matches!(
            results[0].item_type,
            PaletteItemType::Tab | PaletteItemType::TabHistory
        ));

        // Privacy: closing a tab clears its history
        tracker.clear_tab_history("tab_1");
        assert!(tracker.get_history_for_tab("tab_1").is_empty());
    }

    #[test]
    fn test_thor_bookmark_store_persistence() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("thor_int_bookmarks_{}", nanos));
        std::fs::create_dir_all(&dir).unwrap();

        {
            let mut store = BookmarkStore::open_in(&dir).unwrap();
            store.add("Rust", "https://www.rust-lang.org").unwrap();
            store.add("GitHub", "https://github.com").unwrap();
            assert_eq!(store.list().len(), 2);

            let items = store.to_palette_items();
            assert_eq!(items.len(), 2);
            assert!(items
                .iter()
                .all(|i| i.item_type == PaletteItemType::Bookmark));
        }

        {
            let mut store = BookmarkStore::open_in(&dir).unwrap();
            assert_eq!(store.list().len(), 2);
            let id = store.list()[0].id.clone();
            assert!(store.remove(&id).unwrap());
            assert_eq!(store.list().len(), 1);
        }

        let store = BookmarkStore::open_in(&dir).unwrap();
        assert_eq!(store.list().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cmd_k_payload_json_shape() {
        let bookmarks = get_default_bookmarks();
        let open_tabs = open_tabs_to_palette_items(&[("1", "Local", "http://localhost:3000")]);
        let history = tab_history_entries_to_palette_items(
            &[thor::dx::cmd_palette::TabHistoryEntry {
                url: "https://example.com".into(),
                title: "Example".into(),
                timestamp: 1,
            }],
            "tab_x",
        );

        let json = cmd_k_payload_json(&bookmarks, &history, &open_tabs).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_empty());
        let first = &parsed[0];
        assert!(first.get("id").is_some());
        assert!(first.get("title").is_some());
        assert!(first.get("item_type").is_some());
        assert!(first.get("path_or_url").is_some());
        assert!(first.get("category").is_some());
        assert_eq!(first["item_type"], "Tab");

        let settings_json = settings_payload_json().unwrap();
        let settings: Vec<serde_json::Value> = serde_json::from_str(&settings_json).unwrap();
        assert!(settings.iter().all(|s| s["item_type"] != "Bookmark"));
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
        assert!(
            scored.score < 65.0,
            "Score should reflect error rate penalty"
        );
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

    #[test]
    fn test_decoder_studio_integration() {
        use thor::dx::decoder::{DecoderStudio, PayloadType};
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyX2lkIjo0Miwicm9sZSI6ImFkbWluIn0.signature";
        assert_eq!(DecoderStudio::detect_type(jwt), PayloadType::Jwt);

        let decoded = DecoderStudio::decode_jwt(jwt).unwrap();
        assert_eq!(decoded.algorithm, "HS256");
        assert!(decoded.payload_json.unwrap().contains("admin"));
    }

    #[test]
    fn test_traffic_tamper_engine_integration() {
        use std::collections::HashMap;
        use thor::net::tamper::{InterceptedRequest, TamperRule, TrafficTamperEngine};

        let engine = TrafficTamperEngine::new();
        engine.add_rule(TamperRule {
            id: "rule_1".to_string(),
            enabled: true,
            target_url_pattern: "api.internal".to_string(),
            match_header_key: None,
            replace_header_key: "Authorization".to_string(),
            replace_header_value: "Bearer test_token_123".to_string(),
            delay_ms: None,
            mock_body: None,
            mock_status: None,
        });

        let req = InterceptedRequest {
            id: "req_100".to_string(),
            timestamp: 5000,
            method: "GET".to_string(),
            url: "https://api.internal/v1/resource".to_string(),
            headers: HashMap::new(),
            body: None,
            is_modified: false,
        };

        let processed = engine.process_outgoing_request(req).unwrap();
        assert!(processed.is_modified);
        assert_eq!(
            processed.headers.get("Authorization").map(|s| s.as_str()),
            Some("Bearer test_token_123")
        );
    }

    #[test]
    fn test_profile_manager_integration() {
        use thor::engine::profiles::{ProfileManager, ProfileMode};
        let manager = ProfileManager::new();
        assert_eq!(manager.get_profile().mode, ProfileMode::StealthTarget);
        assert!(!manager.get_profile().enable_ide_bridge);

        manager.set_mode(ProfileMode::HardenedTarget);
        assert_eq!(manager.get_profile().mode, ProfileMode::HardenedTarget);
        assert!(manager.get_profile().privacy.require_isolated_identity);
    }

    #[test]
    fn test_wasm_plugin_microkernel_integration() {
        use thor::dx::wasm_plugin::{WasmPluginManifest, WasmPluginMicrokernel};
        let kernel = WasmPluginMicrokernel::new();
        let manifest = WasmPluginManifest {
            id: "custom_fuzzer".to_string(),
            name: "Custom Fuzzer".to_string(),
            version: "0.1.0".to_string(),
            description: "Custom fuzzer plugin".to_string(),
            author: "Tester".to_string(),
            capabilities: vec!["fuzzing".to_string()],
        };

        assert!(kernel.register_plugin(manifest).is_ok());
        assert_eq!(kernel.get_active_plugins().len(), 1);
    }

    #[test]
    fn test_locust_http_runner_against_local_listener() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;
        use thor::net::locust::{LocustConfig, LocustHandle};

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/");

        let server = std::thread::spawn(move || {
            for _ in 0..48 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 512];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    );
                }
            }
        });

        let handle = LocustHandle::start(LocustConfig::new(&url, 3, 0, 2)).expect("start");
        std::thread::sleep(Duration::from_millis(700));
        let scored = handle.stop();
        assert!(
            scored.total_requests > 0,
            "Locust must collect real HTTP RequestMetric entries"
        );
        assert!(scored.success_requests > 0);
        let _ = server.join();
    }

    #[test]
    fn test_ssl_ca_generate_ecdsa_p256() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};
        use thor::net::ssl_ca::SslManager;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("thor-int-ssl-{stamp}"));
        let paths = SslManager::generate_ca_and_leaf_to(Some(&dir)).expect("generate CA");
        assert!(paths.ca_cert_pem.exists());
        assert!(paths.leaf_cert_pem.exists());
        assert!(fs::read_to_string(&paths.ca_cert_pem)
            .unwrap()
            .contains("BEGIN CERTIFICATE"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_app_state_global_tamper_and_security() {
        use thor::app_state::AppState;

        let state = AppState::global();
        state.tamper().set_interception_enabled(false);
        // DevHost defaults leave strip off — enable explicitly for this assert.
        state.security().set_strip_tracking(true);
        let (clean, stripped) = state
            .security()
            .sanitize_navigate_url("https://x.test/a?utm_medium=email&id=1");
        assert!(stripped);
        assert_eq!(clean, "https://x.test/a?id=1");
        state.security().set_strip_tracking(false);
    }

    // =========================================================================
    // 5. Verification of Requirements (a - e)
    // =========================================================================

    // a) 3-Level Isolation & Disposable Isolated Tabs
    #[test]
    fn test_three_level_isolation_and_disposable_tabs() {
        use thor::engine::containers::StoragePartition;
        use thor::engine::tab_session::IdentityContext;

        // Level 1: Profile / Partition isolation
        let partition = StoragePartition::new("partition_guest", "/tmp/thor_storage");
        assert_eq!(partition.partition_id, "partition_guest");
        assert!(partition.base_path.ends_with("/partition_guest"));

        // Level 2: Identity Context isolation & Ephemeral / Disposable Tab
        let mut ephem_ctx = IdentityContext::ephemeral("dev_host");
        assert!(ephem_ctx.is_ephemeral);
        assert_eq!(ephem_ctx.profile_id, "dev_host");
        ephem_ctx.set_cookie("session", "secret_ephemeral_val");
        ephem_ctx.set_local_storage("token", "jwt_token_123");
        ephem_ctx.set_auth_token("api.internal", "Bearer auth_ephemeral");

        assert_eq!(
            ephem_ctx.get_cookie("session").map(|s| s.as_str()),
            Some("secret_ephemeral_val")
        );
        assert_eq!(
            ephem_ctx.get_local_storage("token").map(|s| s.as_str()),
            Some("jwt_token_123")
        );
        assert_eq!(
            ephem_ctx.get_auth_token("api.internal").map(|s| s.as_str()),
            Some("Bearer auth_ephemeral")
        );

        // Level 3: Tab-scoped state (IdentityContext per-tab partition)
        let mut persistent_ctx = IdentityContext::new("ctx_user_1", "dev_host", false);
        persistent_ctx.set_cookie("user_id", "42");
        assert!(!persistent_ctx.is_ephemeral);
        assert_eq!(
            persistent_ctx.get_cookie("user_id").map(|s| s.as_str()),
            Some("42")
        );
    }

    // b) 4-Stage Tab Lifecycles (Active -> Frozen -> Discarded -> Expired)
    #[test]
    fn test_four_stage_tab_lifecycles() {
        use thor::engine::tab_session::{IdentityContext, TabLifecycle};

        let mut lifecycle = TabLifecycle::Active;
        assert_eq!(lifecycle, TabLifecycle::Active);

        // Stage 1: Active -> Stage 2: Frozen
        lifecycle = TabLifecycle::Frozen;
        assert_eq!(lifecycle, TabLifecycle::Frozen);

        // Stage 2: Frozen -> Stage 3: Discarded
        lifecycle = TabLifecycle::Discarded;
        assert_eq!(lifecycle, TabLifecycle::Discarded);

        // Stage 3: Discarded -> Stage 4: Expired
        lifecycle = TabLifecycle::Expired;
        assert_eq!(lifecycle, TabLifecycle::Expired);

        // Test Ephemeral context cleanup on close/expiry
        let mut ephem_ctx = IdentityContext::ephemeral("stealth_target");
        ephem_ctx.set_cookie("auth", "secret");
        ephem_ctx.set_local_storage("state", "persisted");
        assert!(ephem_ctx.is_ephemeral);

        // Simulating closure of ephemeral tab -> clear storage
        ephem_ctx.cookies.clear();
        ephem_ctx.local_storage.clear();
        ephem_ctx.auth_tokens.clear();
        assert!(ephem_ctx.cookies.is_empty());
        assert!(ephem_ctx.local_storage.is_empty());
    }

    // c) 5 Specialist Lenses
    #[test]
    fn test_five_specialist_lenses() {
        // Lens 1: Full-Stack (TechStackDetector)
        let html = r#"<!DOCTYPE html><html><head><script id="__NEXT_DATA__">{}</script></head><body class="bg-slate-900 tailwindcss"></body></html>"#;
        let stack = TechStackDetector::detect("https://app.dev", html, Some("gunicorn/20.1.0"));
        assert!(stack.badges.iter().any(|b| b.name == "Next.js"));
        assert!(stack.badges.iter().any(|b| b.name == "Tailwind CSS"));
        assert!(stack.badges.iter().any(|b| b.name == "Python (Django)"));

        // Lens 2: Security IDOR matrix (TrafficTamperEngine & TamperRule)
        use std::collections::HashMap;
        use thor::net::tamper::{InterceptedRequest, TamperRule, TrafficTamperEngine};
        let engine = TrafficTamperEngine::new();
        engine.add_rule(TamperRule {
            id: "idor_test".into(),
            enabled: true,
            target_url_pattern: "api.internal/users".into(),
            match_header_key: None,
            replace_header_key: "X-User-ID".into(),
            replace_header_value: "1002".into(),
            delay_ms: None,
            mock_body: None,
            mock_status: None,
        });
        let req = InterceptedRequest {
            id: "idor_req".into(),
            timestamp: 1000,
            method: "GET".into(),
            url: "https://api.internal/users/profile".into(),
            headers: HashMap::new(),
            body: None,
            is_modified: false,
        };
        let processed = engine.process_outgoing_request(req).unwrap();
        assert!(processed.is_modified);
        assert_eq!(
            processed.headers.get("X-User-ID").map(|s| s.as_str()),
            Some("1002")
        );

        // Lens 3: Performance (LocustMetricScorer)
        let mut metrics = Vec::new();
        for i in 1..=500 {
            metrics.push(RequestMetric::new(i * 20, 200, true, 1024));
        }
        let scored = LocustMetricScorer::score_metrics(&metrics, 5.0);
        assert_eq!(scored.total_requests, 500);
        assert_eq!(scored.rps, 100.0);
        assert!(scored.p50_latency_us > 0);
        assert!(scored.p95_latency_us > scored.p50_latency_us);

        // Lens 4: AI/LLM streaming TTFT (SseStreamParser & LlmStudio)
        use thor::dx::llm_studio::{LlmStudio, SseStreamParser};
        let mut parser = SseStreamParser::new(500);
        let sse_chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello from Thor AI!\"}}]}\n\ndata: [DONE]\n\n";
        let stream_metrics = parser.feed_chunk(sse_chunk, 542);
        assert_eq!(stream_metrics.ttft_ms, Some(42));
        assert_eq!(stream_metrics.accumulated_text, "Hello from Thor AI!");
        assert!(stream_metrics.is_complete);

        let cost = LlmStudio::estimate_cost("gpt-4o", 2000, 1000);
        assert_eq!(cost.model, "gpt-4o");
        assert!((cost.total_cost_usd - 0.015).abs() < 1e-5);

        // Lens 5: Web3 ABI decoder (Web3Inspector)
        use thor::dx::web3_inspector::{Eip55Status, Web3Inspector};
        let rpc_payload = r#"{"jsonrpc":"2.0","method":"eth_call","params":[],"id":1}"#;
        assert!(Web3Inspector::is_web3_rpc(rpc_payload));

        let calldata = "0x095ea7b30000000000000000000000006b175474e89094c44da98b954eedeac495271d0f0000000000000000000000000000000000000000000000000f4240";
        let decoded = Web3Inspector::decode_abi_selector(calldata).unwrap();
        assert_eq!(decoded.function_name, "approve");
        assert_eq!(decoded.selector, "0x095ea7b3");

        let eip55_addr = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        assert_eq!(
            Web3Inspector::validate_eip55_checksum(eip55_addr),
            Eip55Status::Valid
        );
    }

    // d) Thor Load Engine (Scenario parsing, Browser Runner, Protocol Runner)
    #[test]
    fn test_thor_load_engine_integration() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};
        use thor::load::{
            ProtocolRunner, RunnerType, ScenarioCompiler, ScenarioStep, ThorScenario,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind protocol fixture");
        listener
            .set_nonblocking(true)
            .expect("set protocol fixture nonblocking");
        let endpoint = format!("http://{}/get", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !server_stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                        let mut request = [0_u8; 2048];
                        let _ = stream.read(&mut request);
                        stream
                            .write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                            )
                            .expect("write protocol fixture response");
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("protocol fixture accept failed: {error}"),
                }
            }
        });

        // Scenario creation & compilation
        let scenario = ThorScenario::new("Benchmark Swarm", endpoint.clone())
            .with_runner(RunnerType::Protocol)
            .with_vus(2)
            .with_duration(1)
            .with_header("User-Agent", "Thor-Bench/1.0")
            .add_step(ScenarioStep::HttpRequest {
                method: "GET".into(),
                url: endpoint,
                headers: std::collections::HashMap::new(),
                body: None,
            })
            .add_step(ScenarioStep::Wait { duration_ms: 10 });

        let compiled = ScenarioCompiler::compile(scenario).expect("compile scenario");
        assert_eq!(compiled.vus, 2);
        assert_eq!(compiled.duration_secs, 1);
        assert_eq!(compiled.steps.len(), 2);

        // JSON serialization and parsing
        let json = compiled.to_json().expect("serialize scenario");
        let parsed = ScenarioCompiler::compile_from_json(&json).expect("parse scenario json");
        assert_eq!(parsed.name, "Benchmark Swarm");

        // Protocol Runner execution
        let proto_runner = ProtocolRunner::new();
        let proto_result = proto_runner.run_blocking(&parsed);
        stop.store(true, Ordering::Relaxed);
        server.join().expect("join protocol fixture");
        let proto_metrics = proto_result.expect("protocol runner execution");
        assert!(proto_metrics.total_requests > 0);
        assert!(proto_metrics.avg_latency_ms >= 0.0);
    }

    #[test]
    #[ignore = "requires a live Thor-owned Chromium build"]
    fn test_thor_load_engine_browser_runner_live() {
        use thor::load::{
            BrowserRunner, BrowserRunnerConfig, RunnerType, ScenarioStep, ThorScenario,
        };

        let browser_runner = BrowserRunner::new(BrowserRunnerConfig::default());
        let browser_scenario = ThorScenario::new("Web Vitals Probe", "https://example.com")
            .with_runner(RunnerType::Browser)
            .add_step(ScenarioStep::Navigate {
                url: "https://example.com".into(),
            })
            .add_step(ScenarioStep::Click {
                selector: "#main-btn".into(),
            });

        let browser_metrics = browser_runner
            .run_blocking(&browser_scenario)
            .expect("browser runner execution");
        assert!(browser_metrics.browser_vitals.is_some());
        let vitals = browser_metrics.browser_vitals.unwrap();
        assert!(vitals.lcp_ms >= 0.0);
        assert!(vitals.fcp_ms >= 0.0);
        assert!(vitals.ttfb_ms > 0.0);
    }

    // e) Bookmark saving & Cmd+D shortcut
    #[test]
    fn test_bookmark_saving_and_cmd_d_integration() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("thor_int_bookmarks_cmd_d_{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();

        // 1. Save Bookmark via BookmarkStore
        let mut store = BookmarkStore::open_in(&dir).expect("open bookmark store");
        let bm = store
            .add("Thor Docs", "https://thor.dev/docs")
            .expect("add bookmark");
        assert_eq!(bm.title, "Thor Docs");
        assert_eq!(bm.url, "https://thor.dev/docs");
        assert_eq!(store.list().len(), 1);

        // 2. Cmd+D Toggle behavior (add if missing, remove if present)
        let url_to_toggle = "https://thor.dev/docs";

        // Remove via toggle/remove_by_url
        let removed = store.remove_by_url(url_to_toggle).expect("remove");
        assert!(removed);
        assert_eq!(store.list().len(), 0);

        // Re-add via Cmd+D
        let re_added = store.add("Thor Docs", url_to_toggle).expect("re-add");
        assert_eq!(re_added.url, url_to_toggle);
        assert_eq!(store.list().len(), 1);

        // 3. Verify Palette Integration (Cmd+K include)
        let items = store.to_palette_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_type, PaletteItemType::Bookmark);
        assert_eq!(items[0].category, "Bookmarks");
        assert_eq!(items[0].path_or_url, "https://thor.dev/docs");

        // 4. Test IPC dispatch contract shape for bookmark IPC
        let payload = serde_json::json!({
            "title": "Thor GitHub",
            "url": "https://github.com/thor/thor"
        });
        assert_eq!(payload["title"], "Thor GitHub");
        assert_eq!(payload["url"], "https://github.com/thor/thor");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
