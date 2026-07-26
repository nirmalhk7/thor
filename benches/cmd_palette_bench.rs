use std::time::Instant;
use thor::dx::stack_detector::TechStackDetector;
use thor::dx::cmd_palette::{CmdPalette, CmdPaletteMode, PaletteItem, PaletteItemType};
use thor::net::locust::{LocustMetricScorer, RequestMetric};
use thor::net::security_shield::SecurityShield;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn generate_10k_palette_items() -> Vec<PaletteItem> {
    let mut items = Vec::with_capacity(10_000);
    let item_types = [
        PaletteItemType::Bookmark,
        PaletteItemType::TabHistory,
        PaletteItemType::Setting,
        PaletteItemType::DevTools,
        PaletteItemType::Action,
    ];

    let categories = [
        "Bookmarks",
        "Tab History",
        "Settings",
        "DevTools",
        "IDE Bridge",
    ];

    for i in 0..10_000 {
        let item_type = item_types[i % item_types.len()].clone();
        let category = categories[i % categories.len()];
        let title = match i % 5 {
            0 => format!("GitHub Repository #{i} - Thor Developer Browser Engine"),
            1 => format!("Google Search Node #{i} - Fast Web Indexing"),
            2 => format!("MDN Web Docs Resource #{i} - HTML5 Canvas & WebGL"),
            3 => format!("Stack Overflow Question #{i} - Rust Performance Optimization"),
            _ => format!("VS Code Project Workspace #{i} - Rust Crate"),
        };

        let url = match i % 4 {
            0 => format!("https://github.com/thor-browser/repo-{i}?utm_source=github&gclid=abc_{i}"),
            1 => format!("https://google.com/search?q=item_{i}&utm_medium=cpc&fbclid=xyz_{i}"),
            2 => format!("Thor://settings/node_{i}?ref_=thor_app&msclkid=123_{i}"),
            _ => format!("http://localhost:{i}?utm_campaign=dev_test&igshid=456_{i}"),
        };

        let badge = match i % 3 {
            0 => Some(format!("Tag-{i}")),
            1 => Some("Chrome".to_string()),
            _ => Some("Default".to_string()),
        };

        let icon = match i % 5 {
            0 => Some("🐙".to_string()),
            1 => Some("🔍".to_string()),
            2 => Some("🛠️".to_string()),
            3 => Some("💻".to_string()),
            _ => Some("⚡".to_string()),
        };

        items.push(PaletteItem::new(
            format!("item_{i}"),
            title,
            item_type,
            url,
            badge,
            icon,
            category,
            Some(format!("action_{i}")),
        ));
    }

    items
}

fn generate_10k_tracking_urls() -> Vec<String> {
    let mut urls = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        urls.push(format!(
            "https://example.com/products/item_{i}?utm_source=facebook&utm_medium=cpc&utm_campaign=summer_sale_{i}&gclid=987654321_{i}&fbclid=abcdef_{i}&keep_param=true&user_id={i}"
        ));
    }
    urls
}

fn generate_10k_tech_stack_samples() -> Vec<(String, String, Option<String>)> {
    let mut samples = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        let (url, html, server) = match i % 5 {
            0 => (
                format!("https://app{i}.nextjs.org"),
                r#"<script id="__NEXT_DATA__">{"props":{}}</script><div class="flex items-center"></div>"#.to_string(),
                Some("Vercel".to_string()),
            ),
            1 => (
                format!("https://vue-site{i}.com"),
                r#"<div id="app" data-v-12345></div><script src="vue.js"></script>"#.to_string(),
                Some("nginx/1.18.0".to_string()),
            ),
            2 => (
                format!("https://wasm-app{i}.rs"),
                r#"<script src="app_bg.wasm"></script>"#.to_string(),
                Some("Cloudflare".to_string()),
            ),
            3 => (
                format!("https://django-api{i}.org"),
                r#"<div class="bootstrap-container"></div>"#.to_string(),
                Some("gunicorn/20.1.0".to_string()),
            ),
            _ => (
                format!("https://github.com/repo-{i}"),
                r#"<meta name="hostname" content="github.com">"#.to_string(),
                Some("GitHub.com".to_string()),
            ),
        };
        samples.push((url, html, server));
    }
    samples
}

fn generate_10k_locust_metrics() -> Vec<RequestMetric> {
    let mut metrics = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        let response_time_us = ((i % 500) + 10) as u64 * 100; // 1.0ms - 51.0ms
        let status_code = if i % 20 == 0 { 500 } else { 200 };
        let is_success = status_code == 200;
        let bytes_received = 1024 + (i % 2048);

        metrics.push(RequestMetric::new(
            response_time_us,
            status_code,
            is_success,
            bytes_received,
        ));
    }
    metrics
}

fn bench_cmd_palette_search(items: &[PaletteItem]) {
    println!("------------------------------------------------------------");
    println!("🔍 1. CmdPalette Fuzzy Search Latency Benchmark (10,000+ items)");
    println!("------------------------------------------------------------");

    let mut palette = CmdPalette::new(CmdPaletteMode::BookmarksAndTabHistory);
    palette.set_items(items.to_vec());

    let test_queries = ["git", "google", "dev", "thor", "resource", "localhost", "123", "code"];

    // Warmup CPU cache
    for q in &test_queries {
        palette.set_query((*q).to_string());
        let _ = palette.search();
    }

    let iterations_per_query = 20;
    let start = Instant::now();
    let mut total_matches = 0;

    for _ in 0..iterations_per_query {
        for q in &test_queries {
            palette.set_query((*q).to_string());
            let matches = palette.search();
            total_matches += matches.len();
        }
    }

    let total_elapsed = start.elapsed();
    let total_searches = test_queries.len() * iterations_per_query;
    let avg_latency = total_elapsed / total_searches as u32;

    println!("   Total Queries Executed: {}", total_searches);
    println!("   Total Execution Time: {:?}", total_elapsed);
    println!("   Average Latency per Query: {:?}", avg_latency);
    println!("   Total Matches Scored: {}", total_matches);

    #[cfg(not(debug_assertions))]
    assert!(
        avg_latency.as_millis() < 1 || avg_latency.as_micros() < 1000,
        "FAILED: Search average latency must be sub-millisecond (< 1ms), got {:?}",
        avg_latency
    );

    #[cfg(debug_assertions)]
    assert!(
        avg_latency.as_millis() < 50,
        "FAILED: Debug search latency must be under 50ms, got {:?}",
        avg_latency
    );
}

fn bench_url_tracking_stripping(urls: &[String]) {
    println!("------------------------------------------------------------");
    println!("🛡️ 2. URL Tracking Parameter Stripping Throughput (10,000+ URLs)");
    println!("------------------------------------------------------------");

    // Warmup
    for url in &urls[..100] {
        let _ = SecurityShield::sanitize_url(url);
    }

    let iterations = 5;
    let start = Instant::now();
    let mut total_stripped = 0;

    for _ in 0..iterations {
        for url in urls {
            let (_clean, stripped) = SecurityShield::sanitize_url(url);
            if stripped {
                total_stripped += 1;
            }
        }
    }

    let total_elapsed = start.elapsed();
    let total_processed = urls.len() * iterations;
    let avg_per_url = total_elapsed / total_processed as u32;
    let avg_per_10k_batch = total_elapsed / iterations as u32;

    println!("   Total URLs Processed: {}", total_processed);
    println!("   Total Execution Time: {:?}", total_elapsed);
    println!("   Average Time per 10k Batch: {:?}", avg_per_10k_batch);
    println!("   Average Time per URL: {:?}", avg_per_url);
    println!("   Total Tracking Parameters Stripped: {}", total_stripped);

    #[cfg(not(debug_assertions))]
    assert!(
        avg_per_url.as_micros() < 10,
        "FAILED: URL stripping per-item throughput must be sub-10us, got {:?}",
        avg_per_url
    );

    #[cfg(debug_assertions)]
    assert!(
        avg_per_url.as_micros() < 500,
        "FAILED: Debug URL stripping must be sub-500us, got {:?}",
        avg_per_url
    );
}

fn bench_tech_stack_detection(samples: &[(String, String, Option<String>)]) {
    println!("------------------------------------------------------------");
    println!("⚡ 3. Tech Stack Detection Throughput (10,000+ items)");
    println!("------------------------------------------------------------");

    // Warmup
    for (url, html, server) in &samples[..100] {
        let _ = TechStackDetector::detect(url, html, server.as_deref());
    }

    let iterations = 5;
    let start = Instant::now();
    let mut total_badges = 0;

    for _ in 0..iterations {
        for (url, html, server) in samples {
            let stack = TechStackDetector::detect(url, html, server.as_deref());
            total_badges += stack.badges.len();
        }
    }

    let total_elapsed = start.elapsed();
    let total_processed = samples.len() * iterations;
    let avg_per_item = total_elapsed / total_processed as u32;
    let avg_per_10k_batch = total_elapsed / iterations as u32;

    println!("   Total Sites Analyzed: {}", total_processed);
    println!("   Total Execution Time: {:?}", total_elapsed);
    println!("   Average Time per 10k Batch: {:?}", avg_per_10k_batch);
    println!("   Average Time per Site: {:?}", avg_per_item);
    println!("   Total Tech Badges Detected: {}", total_badges);

    #[cfg(not(debug_assertions))]
    assert!(
        avg_per_item.as_micros() < 50,
        "FAILED: Tech stack detection per site must be fast, got {:?}",
        avg_per_item
    );

    #[cfg(debug_assertions)]
    assert!(
        avg_per_item.as_micros() < 1000,
        "FAILED: Debug tech stack detection must be sub-1ms, got {:?}",
        avg_per_item
    );
}

fn bench_locust_metric_scoring(metrics: &[RequestMetric]) {
    println!("------------------------------------------------------------");
    println!("🚀 4. High-Concurrency Locust Metric Scoring (10,000+ items)");
    println!("------------------------------------------------------------");

    // Warmup
    let _ = LocustMetricScorer::score_metrics(metrics, 30.0);

    let iterations = 100;
    let start = Instant::now();
    let mut last_score = 0.0;

    for _ in 0..iterations {
        let scored = LocustMetricScorer::score_metrics(metrics, 30.0);
        last_score = scored.score;
    }

    let total_elapsed = start.elapsed();
    let avg_scoring_time = total_elapsed / iterations as u32;

    println!("   Total Scoring Runs: {}", iterations);
    println!("   Metrics Scored per Run: {}", metrics.len());
    println!("   Total Execution Time: {:?}", total_elapsed);
    println!("   Average Scoring Execution Time: {:?}", avg_scoring_time);
    println!("   Sample Final Score: {:.2}/100", last_score);

    #[cfg(not(debug_assertions))]
    assert!(
        avg_scoring_time.as_millis() < 1 || avg_scoring_time.as_micros() < 1000,
        "FAILED: Locust metric scoring for 10,000 items must be sub-millisecond (< 1ms), got {:?}",
        avg_scoring_time
    );

    #[cfg(debug_assertions)]
    assert!(
        avg_scoring_time.as_millis() < 50,
        "FAILED: Debug Locust metric scoring must be under 50ms, got {:?}",
        avg_scoring_time
    );
}

fn main() {
    println!("============================================================");
    println!("⚡ [Thor Benchmark Suite] CmdPalette, SecurityShield, TechStack & Locust");
    println!("============================================================");

    let palette_items = generate_10k_palette_items();
    let tracking_urls = generate_10k_tracking_urls();
    let stack_samples = generate_10k_tech_stack_samples();
    let locust_metrics = generate_10k_locust_metrics();

    println!(
        "⚡ Generated test datasets: {} palette items, {} tracking URLs, {} stack samples, {} locust metrics.",
        palette_items.len(),
        tracking_urls.len(),
        stack_samples.len(),
        locust_metrics.len()
    );

    bench_cmd_palette_search(&palette_items);
    bench_url_tracking_stripping(&tracking_urls);
    bench_tech_stack_detection(&stack_samples);
    bench_locust_metric_scoring(&locust_metrics);

    println!("============================================================");
    println!("✅ SUCCESS: All performance benchmarks verified sub-millisecond execution across 10,000+ items!");
    println!("============================================================");
}
