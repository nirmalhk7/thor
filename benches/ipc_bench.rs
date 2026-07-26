//! Real microbench: Locust metric scoring (zero-alloc path intent).
//! Replaces dummy SPSC theater.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use thor::net::locust::{LocustMetricScorer, RequestMetric};

fn sample_metrics(n: usize) -> Vec<RequestMetric> {
    (0..n)
        .map(|i| {
            RequestMetric::new(
                800 + (i as u64 % 400) * 10,
                if i % 20 == 0 { 500 } else { 200 },
                i % 20 != 0,
                256,
            )
        })
        .collect()
}

fn bench_locust_score(c: &mut Criterion) {
    let metrics = sample_metrics(10_000);
    c.bench_function("locust_score_10k_metrics", |b| {
        b.iter(|| {
            let scored = LocustMetricScorer::score_metrics(black_box(&metrics), 10.0);
            black_box(scored);
        })
    });
}

criterion_group!(benches, bench_locust_score);
criterion_main!(benches);
