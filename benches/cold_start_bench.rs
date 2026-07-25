use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn bench_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start");
    group.measurement_time(Duration::from_secs(10));
    
    group.bench_function("pre_warmed_daemon_startup", |b| {
        b.iter(|| {
            // Simulate daemon cold start
            std::thread::sleep(Duration::from_micros(100));
        })
    });
    group.finish();
}

criterion_group!(benches, bench_cold_start);
criterion_main!(benches);
