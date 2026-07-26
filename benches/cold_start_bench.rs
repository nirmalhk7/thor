//! Real microbench: SecurityShield URL sanitize (lean navigate hot path).
//! Replaces sleep-based cold_start theater.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use thor::net::security_shield::SecurityShield;

fn bench_shield_sanitize(c: &mut Criterion) {
    let dirty = "https://example.com/path?utm_source=x&utm_medium=y&ok=1&gclid=abc&keep=yes";
    c.bench_function("shield_sanitize_tracking_params", |b| {
        b.iter(|| {
            let (clean, stripped) = SecurityShield::sanitize_url(black_box(dirty));
            black_box((clean, stripped));
        })
    });
}

criterion_group!(benches, bench_shield_sanitize);
criterion_main!(benches);
