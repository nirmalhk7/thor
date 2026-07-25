use criterion::{criterion_group, criterion_main, Criterion};

fn bench_memory_soak(c: &mut Criterion) {
    c.bench_function("50_tab_active_vs_suspended", |b| {
        b.iter(|| {
            // Simulate 50 tabs memory allocation
            let mut _tabs: Vec<Vec<u8>> = Vec::new();
            for _ in 0..50 {
                _tabs.push(vec![0; 1024 * 1024]); // 1MB per tab sim
            }
        })
    });
}

criterion_group!(benches, bench_memory_soak);
criterion_main!(benches);
