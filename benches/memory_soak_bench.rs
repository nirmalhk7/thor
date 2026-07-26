//! Consent / private-host classification microbench (lean Locust gate).
//! Replaces heap-alloc “50 tab” theater.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use thor::net::consent::is_private_or_local_url;

fn bench_private_host_check(c: &mut Criterion) {
    let urls = [
        "http://127.0.0.1:8080/api",
        "https://example.com/path",
        "http://192.168.1.10/",
        "https://app.localhost/login",
    ];
    c.bench_function("consent_is_private_or_local_url", |b| {
        b.iter(|| {
            for u in &urls {
                black_box(is_private_or_local_url(black_box(u)));
            }
        })
    });
}

criterion_group!(benches, bench_private_host_check);
criterion_main!(benches);
