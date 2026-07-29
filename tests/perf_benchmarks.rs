use std::time::{Duration, Instant};

#[test]
fn test_perf_benchmarks_local_fixture() {
    let start = Instant::now();
    // Simulate some lightweight workload
    for _ in 0..10_000 {
        let _ = vec![0; 100];
    }
    let elapsed = start.elapsed();
    
    // Target performance budget: < 50ms
    assert!(
        elapsed < Duration::from_millis(50),
        "Performance benchmark failed: expected < 50ms, got {:?}", 
        elapsed
    );
}
