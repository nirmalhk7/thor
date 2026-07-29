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
    sla_summary,
);
criterion_main!(benches);
