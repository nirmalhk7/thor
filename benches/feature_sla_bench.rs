//! Criterion micro-benchmarks for Thor's critical feature paths, plus a
//! pass/fail SLA summary. Budgets live in `thor::features::sla`
//! (`FeaturePathSla` / `FEATURE_PATH_SLAS`) — the single source of truth shared
//! with the `#[cfg(test)]` enforcement tests.
//!
//! Every path is wired to a real engine (no mocks) per AGENTS.md.
//!
//! Run: `cargo bench --bench feature_sla_bench`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use thor::devtools::parse_web_vitals;
use thor::dx::llm_studio::LlmStudio;
use thor::dx::test_generator::TestGenerator;
use thor::features::backend::BackendEngine;
use thor::features::perf_features::PerfEngine;
use thor::features::security::SecurityEngine;
use thor::features::sla::{self, FeaturePathResult};
use thor::features::{FeatureEngine, FeatureRegistry};
use thor::load::LatencyHistogram;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn bench_registry_lookup(c: &mut Criterion) {
    let registry = FeatureRegistry::new();
    c.bench_function("feature_registry_lookup", |b| {
        b.iter(|| black_box(registry.by_slug(black_box("response-diff"))))
    });
}

fn bench_hdr_percentiles(c: &mut Criterion) {
    let samples = sla::hdr_ms_samples(10_000);
    c.bench_function("hdr_percentile_10k", |b| {
        b.iter(|| {
            let mut h = LatencyHistogram::default();
            for &s in &samples {
                h.record(black_box(s));
            }
            black_box(h.percentile(50.0));
            black_box(h.percentile(90.0));
            black_box(h.percentile(99.0));
        })
    });
}

fn bench_authz_matrix(c: &mut Criterion) {
    let engine = SecurityEngine::new();
    let probes = sla::authz_probes();
    c.bench_function("authz_matrix_4x1", |b| {
        b.iter(|| black_box(engine.build_authz_matrix(black_box(&probes))))
    });
}

fn bench_jwt_analysis(c: &mut Criterion) {
    let engine = SecurityEngine::new();
    c.bench_function("jwt_deep_analysis", |b| {
        b.iter(|| {
            black_box(
                engine
                    .inspect_jwt(black_box(sla::SAMPLE_JWT), 1_700_002_000)
                    .unwrap(),
            )
        })
    });
}

fn bench_playwright_export(c: &mut Criterion) {
    let session = sla::playwright_session(100);
    c.bench_function("playwright_export_100", |b| {
        b.iter(|| black_box(TestGenerator::export_playwright_ts(black_box(&session))))
    });
}

fn bench_llm_cost(c: &mut Criterion) {
    c.bench_function("llm_cost_estimate", |b| {
        b.iter(|| {
            black_box(LlmStudio::estimate_cost(
                black_box("claude-3.5-sonnet"),
                black_box(12_000),
                black_box(3_500),
            ))
        })
    });
}

fn bench_response_diff(c: &mut Criterion) {
    let engine = SecurityEngine::new();
    let (a, b_snap) = sla::response_snapshots();
    c.bench_function("response_diff", |b| {
        b.iter(|| black_box(engine.diff_responses(black_box(&a), black_box(&b_snap))))
    });
}

fn bench_feature_invoke_routing(c: &mut Criterion) {
    let engine = FeatureEngine::new();
    c.bench_function("feature_invoke_routing", |b| {
        b.iter(|| black_box(engine.invoke(black_box("webhook-sink")).unwrap()))
    });
}

fn bench_cdp_parse_web_vitals(c: &mut Criterion) {
    c.bench_function("cdp_parse_web_vitals", |b| {
        b.iter(|| black_box(parse_web_vitals(black_box(sla::SAMPLE_WEB_VITALS_JSON)).unwrap()))
    });
}

fn bench_webhook_sink_record(c: &mut Criterion) {
    let engine = BackendEngine::new();
    let event = sla::sample_webhook_event();
    c.bench_function("webhook_sink_record", |b| {
        b.iter(|| engine.record_webhook(black_box(event.clone())))
    });
}

fn bench_idor_matrix(c: &mut Criterion) {
    let engine = BackendEngine::new();
    let probes = sla::idor_probes();
    c.bench_function("idor_matrix_4ctx", |b| {
        b.iter(|| black_box(engine.analyze_idor(black_box(&probes))))
    });
}

fn bench_lcp_element_forensics(c: &mut Criterion) {
    let engine = PerfEngine::new();
    let payload = thor::features::perf_features::LcpElementPayload {
        vitals_json: Some(
            r#"{"lcp":2100,"lcpElement":{"url":"https://cdn/hero.webp","type":"image","size":82000,"discoverTime":120,"loadTime":900,"renderTime":2100}}"#
                .into(),
        ),
        ..Default::default()
    };
    c.bench_function("lcp_element_forensics", |b| {
        b.iter(|| black_box(engine.lcp_element_forensics(black_box(&payload))))
    });
}

fn bench_long_task_radar(c: &mut Criterion) {
    let engine = PerfEngine::new();
    let tasks: Vec<_> = (0..1_000)
        .map(|i| thor::features::perf_features::LongTaskEvent {
            duration_ms: 60.0 + (i % 40) as f64,
            start_ms: i as f64,
            script_url: Some(format!("https://cdn/chunk-{}.js", i % 17)),
            name: Some("script".into()),
            inp_overlap: i % 11 == 0,
        })
        .collect();
    c.bench_function("long_task_radar_1k", |b| {
        b.iter(|| black_box(engine.long_task_radar(black_box(&tasks))))
    });
}

fn bench_conn_pool_inspector(c: &mut Criterion) {
    let engine = PerfEngine::new();
    let entries: Vec<_> = (0..1_000)
        .map(|i| thor::features::perf_features::ConnPoolEntry {
            url: format!("https://api.example.com/{i}"),
            connection_id: format!("c{}", i % 32),
            wait_ms: (i % 25) as f64,
            reused: i % 3 != 0,
            protocol: Some(if i % 5 == 0 { "h3" } else { "h2" }.into()),
        })
        .collect();
    c.bench_function("conn_pool_inspector_1k", |b| {
        b.iter(|| black_box(engine.conn_pool_inspect(black_box(&entries))))
    });
}

fn bench_continuous_profiler(c: &mut Criterion) {
    let engine = PerfEngine::new();
    let samples: Vec<_> = (0..500)
        .map(|i| thor::features::perf_features::StackSample {
            stack: vec![
                "main".into(),
                format!("fn_{}", i % 20),
                format!("leaf_{}", i % 7),
            ],
            weight: 1 + (i % 5) as u64,
        })
        .collect();
    c.bench_function("continuous_profiler_500", |b| {
        b.iter(|| black_box(engine.continuous_profile(black_box(&samples))))
    });
}

/// Print a pass/fail SLA summary using the shared measurement harness.
fn sla_summary(_c: &mut Criterion) {
    println!("\n============================================================");
    println!("Thor Feature-Path SLA Summary (best per-op vs budget)");
    println!("============================================================");
    let reports: Vec<FeaturePathResult> = sla::run_feature_path_slas();
    let mut all_pass = true;
    for r in &reports {
        all_pass &= r.passed;
        println!(
            "  [{}] {:<46} {:>12?}  budget {:>10?}",
            if r.passed { "PASS" } else { "FAIL" },
            r.sla.description,
            r.per_op,
            r.effective_threshold(),
        );
    }
    println!("------------------------------------------------------------");
    println!(
        "  Overall: {}",
        if all_pass {
            "ALL SLAs MET"
        } else {
            "SLA VIOLATION(S) DETECTED"
        }
    );
    println!("============================================================\n");
}

criterion_group!(
    benches,
    bench_registry_lookup,
    bench_hdr_percentiles,
    bench_authz_matrix,
    bench_jwt_analysis,
    bench_playwright_export,
    bench_llm_cost,
    bench_response_diff,
    bench_feature_invoke_routing,
    bench_cdp_parse_web_vitals,
    bench_webhook_sink_record,
    bench_idor_matrix,
    bench_lcp_element_forensics,
    bench_long_task_radar,
    bench_conn_pool_inspector,
    bench_continuous_profiler,
    sla_summary,
);
criterion_main!(benches);
