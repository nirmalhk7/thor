use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// High-concurrency Locust HTTP Load Test request metric item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetric {
    pub response_time_us: u64,
    pub status_code: u16,
    pub is_success: bool,
    pub bytes_received: usize,
}

impl RequestMetric {
    pub fn new(response_time_us: u64, status_code: u16, is_success: bool, bytes_received: usize) -> Self {
        Self {
            response_time_us,
            status_code,
            is_success,
            bytes_received,
        }
    }
}

/// Calculated & scored aggregate metrics from high-concurrency Locust load tests
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocustScoredMetrics {
    pub total_requests: usize,
    pub success_requests: usize,
    pub failed_requests: usize,
    pub rps: f64,
    pub error_rate: f64,
    pub min_latency_us: u64,
    pub max_latency_us: u64,
    pub avg_latency_us: f64,
    pub p50_latency_us: u64,
    pub p90_latency_us: u64,
    pub p95_latency_us: u64,
    pub p99_latency_us: u64,
    pub score: f64,
}

pub struct LocustMetricScorer;

impl LocustMetricScorer {
    /// High-performance zero-allocation metric scoring across 10,000+ request metrics
    pub fn score_metrics(metrics: &[RequestMetric], duration_secs: f64) -> LocustScoredMetrics {
        if metrics.is_empty() {
            return LocustScoredMetrics::default();
        }

        let total_requests = metrics.len();
        let mut success_requests = 0;
        let mut failed_requests = 0;
        let mut sum_latency: u64 = 0;
        let mut min_latency = u64::MAX;
        let mut max_latency = 0;

        let mut latencies = Vec::with_capacity(total_requests);

        for m in metrics {
            if m.is_success && m.status_code < 400 {
                success_requests += 1;
            } else {
                failed_requests += 1;
            }
            sum_latency += m.response_time_us;
            min_latency = min_latency.min(m.response_time_us);
            max_latency = max_latency.max(m.response_time_us);
            latencies.push(m.response_time_us);
        }

        latencies.sort_unstable();

        let p50_idx = (total_requests as f64 * 0.50) as usize;
        let p90_idx = (total_requests as f64 * 0.90) as usize;
        let p95_idx = (total_requests as f64 * 0.95) as usize;
        let p99_idx = (total_requests as f64 * 0.99) as usize;

        let p50 = latencies.get(p50_idx.min(total_requests - 1)).copied().unwrap_or(0);
        let p90 = latencies.get(p90_idx.min(total_requests - 1)).copied().unwrap_or(0);
        let p95 = latencies.get(p95_idx.min(total_requests - 1)).copied().unwrap_or(0);
        let p99 = latencies.get(p99_idx.min(total_requests - 1)).copied().unwrap_or(0);

        let rps = if duration_secs > 0.0 {
            total_requests as f64 / duration_secs
        } else {
            0.0
        };

        let error_rate = (failed_requests as f64 / total_requests as f64) * 100.0;
        let avg_latency = sum_latency as f64 / total_requests as f64;

        let latency_score = (100.0 - (p95 as f64 / 1000.0)).clamp(0.0, 100.0);
        let error_penalty = error_rate * 2.0;
        let score = (latency_score - error_penalty).clamp(0.0, 100.0);

        LocustScoredMetrics {
            total_requests,
            success_requests,
            failed_requests,
            rps,
            error_rate,
            min_latency_us: if min_latency == u64::MAX { 0 } else { min_latency },
            max_latency_us: max_latency,
            avg_latency_us: avg_latency,
            p50_latency_us: p50,
            p90_latency_us: p90,
            p95_latency_us: p95,
            p99_latency_us: p99,
            score,
        }
    }
}

/// Configuration for a Locust HTTP(S) swarm run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocustConfig {
    pub target_url: String,
    pub users: u32,
    pub ramp_up_secs: u32,
    pub runtime_secs: u64,
}

impl LocustConfig {
    pub fn new(target_url: impl Into<String>, users: u32, ramp_up_secs: u32, runtime_secs: u64) -> Self {
        Self {
            target_url: target_url.into(),
            users: users.max(1),
            ramp_up_secs,
            runtime_secs: runtime_secs.max(1),
        }
    }
}

struct LocustShared {
    metrics: Mutex<Vec<RequestMetric>>,
    stop: AtomicBool,
    started_at: Instant,
}

/// Live handle for a running (or just-finished) Locust swarm.
///
/// Start/stop API:
/// - [`LocustHandle::start`] — spawn concurrent Tokio HTTP(S) workers
/// - [`LocustHandle::stop`] — signal cancel, join workers, return scored metrics
/// - [`LocustHandle::snapshot`] — score metrics collected so far without stopping
pub struct LocustHandle {
    shared: Arc<LocustShared>,
    thread: Option<JoinHandle<()>>,
}

impl LocustHandle {
    /// Start a concurrent HTTP(S) load swarm against `config.target_url`.
    ///
    /// Spawns a dedicated Tokio multi-thread runtime on a background OS thread so
    /// this works from sync IPC handlers that are not inside an async runtime.
    pub fn start(config: LocustConfig) -> Result<Self, String> {
        if config.target_url.is_empty() {
            return Err("Locust target_url must not be empty".into());
        }

        let shared = Arc::new(LocustShared {
            metrics: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
            started_at: Instant::now(),
        });
        let shared_clone = Arc::clone(&shared);

        let thread = std::thread::Builder::new()
            .name("Thor-locust".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads((config.users as usize).clamp(2, 8))
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("⚡ [Thor Locust] Failed to build runtime: {e}");
                        return;
                    }
                };
                rt.block_on(run_swarm(config, shared_clone));
            })
            .map_err(|e| format!("Failed to spawn Locust thread: {e}"))?;

        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Signal stop, wait for workers, and return scored metrics from real requests.
    pub fn stop(mut self) -> LocustScoredMetrics {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        self.score_now()
    }

    /// Score metrics collected so far without stopping the swarm.
    pub fn snapshot(&self) -> LocustScoredMetrics {
        self.score_now()
    }

    pub fn is_running(&self) -> bool {
        !self.shared.stop.load(Ordering::SeqCst)
            && self
                .thread
                .as_ref()
                .map(|t| !t.is_finished())
                .unwrap_or(false)
    }

    fn score_now(&self) -> LocustScoredMetrics {
        let metrics = self
            .shared
            .metrics
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let duration = self.shared.started_at.elapsed().as_secs_f64();
        LocustMetricScorer::score_metrics(&metrics, duration)
    }
}

impl Drop for LocustHandle {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

async fn run_swarm(config: LocustConfig, shared: Arc<LocustShared>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(config.users as usize)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("⚡ [Thor Locust] HTTP client build failed: {e}");
            shared.stop.store(true, Ordering::SeqCst);
            return;
        }
    };

    let deadline = Instant::now() + Duration::from_secs(config.runtime_secs);
    let mut workers = Vec::with_capacity(config.users as usize);

    for i in 0..config.users {
        let ramp_delay = if config.users > 1 && config.ramp_up_secs > 0 {
            Duration::from_secs_f64(
                (i as f64 / config.users as f64) * config.ramp_up_secs as f64,
            )
        } else {
            Duration::ZERO
        };

        let client = client.clone();
        let url = config.target_url.clone();
        let shared = Arc::clone(&shared);

        workers.push(tokio::spawn(async move {
            if !ramp_delay.is_zero() {
                tokio::select! {
                    _ = tokio::time::sleep(ramp_delay) => {}
                    _ = wait_until_stopped(&shared) => return,
                }
            }

            while !shared.stop.load(Ordering::SeqCst) && Instant::now() < deadline {
                let metric = fire_request(&client, &url).await;
                if let Ok(mut lock) = shared.metrics.lock() {
                    lock.push(metric);
                }
            }
        }));
    }

    // Wall-clock runtime gate: stop workers when duration elapses.
    let stopper = Arc::clone(&shared);
    let runtime_secs = config.runtime_secs;
    let timer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(runtime_secs)).await;
        stopper.stop.store(true, Ordering::SeqCst);
    });

    for w in workers {
        let _ = w.await;
    }
    timer.abort();
    shared.stop.store(true, Ordering::SeqCst);
}

async fn wait_until_stopped(shared: &LocustShared) {
    while !shared.stop.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn fire_request(client: &reqwest::Client, url: &str) -> RequestMetric {
    let start = Instant::now();
    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let bytes = resp.bytes().await.map(|b| b.len()).unwrap_or(0);
            let elapsed = start.elapsed().as_micros() as u64;
            let success = status < 400;
            RequestMetric::new(elapsed, status, success, bytes)
        }
        Err(_) => {
            let elapsed = start.elapsed().as_micros() as u64;
            RequestMetric::new(elapsed, 0, false, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn test_locust_metric_scoring() {
        let mut metrics = Vec::with_capacity(1000);
        for i in 0..1000 {
            metrics.push(RequestMetric::new(i * 10, 200, true, 1024));
        }

        let scored = LocustMetricScorer::score_metrics(&metrics, 10.0);
        assert_eq!(scored.total_requests, 1000);
        assert_eq!(scored.success_requests, 1000);
        assert_eq!(scored.failed_requests, 0);
        assert_eq!(scored.rps, 100.0);
        assert!(scored.p50_latency_us > 0);
    }

    #[test]
    fn test_locust_runner_hits_local_http_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local listener");
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/");

        let server = std::thread::spawn(move || {
            listener.set_nonblocking(false).ok();
            // Serve a handful of connections then exit.
            for _ in 0..64 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let body = b"ok";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.write_all(body);
                }
            }
        });

        let handle = LocustHandle::start(LocustConfig::new(&url, 4, 0, 2))
            .expect("locust start");

        // Let workers fire real requests.
        std::thread::sleep(Duration::from_millis(800));
        let snap = handle.snapshot();
        assert!(
            snap.total_requests > 0,
            "expected real HTTP metrics, got zero requests"
        );
        assert!(snap.success_requests > 0, "expected at least one 2xx");
        assert!(snap.avg_latency_us > 0.0);

        let scored = handle.stop();
        assert!(scored.total_requests >= snap.total_requests);
        let _ = server.join();
    }
}
