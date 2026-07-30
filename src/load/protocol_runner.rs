use crate::load::metrics::{LoadMetrics, MetricsCollector};
use crate::load::scenario::{ScenarioStep, ThorScenario};
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub struct ProtocolRunner {
    client: Client,
}

impl Default for ProtocolRunner {
    fn default() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }
}

impl ProtocolRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run(&self, scenario: &ThorScenario) -> Result<LoadMetrics, String> {
        let collector = Arc::new(MetricsCollector::new());
        let vus = scenario.vus.max(1);
        let duration = Duration::from_secs(scenario.duration_secs.max(1));
        let end_time = Instant::now() + duration;

        let mut handles = Vec::new();
        
        let interval = scenario.target_rps.map(|rps| {
            if rps > 0 {
                Duration::from_secs_f64((vus as f64) / (rps as f64))
            } else {
                Duration::from_millis(0)
            }
        });

        for vu_idx in 0..vus {
            let client = self.client.clone();
            let scenario = scenario.clone();
            let collector = Arc::clone(&collector);

            let handle = tokio::spawn(async move {
                if scenario.ramp_up_secs > 0 && vus > 1 {
                    let stagger_ms = (scenario.ramp_up_secs * 1000) / (vus as u64);
                    let delay = Duration::from_millis(stagger_ms * (vu_idx as u64));
                    sleep(delay).await;
                }

                let mut next_wake = tokio::time::Instant::now();
                while Instant::now() < end_time {
                    for step in &scenario.steps {
                        if Instant::now() >= end_time {
                            break;
                        }
                        Self::execute_step(&client, step, &scenario, &collector).await;
                    }
                    if let Some(inv) = interval {
                        next_wake += inv;
                        let now = tokio::time::Instant::now();
                        if next_wake > now {
                            tokio::time::sleep_until(next_wake).await;
                        } else {
                            next_wake = now;
                        }
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        let snapshot = collector.snapshot();
        let violations = scenario.budgets.check(&snapshot);
        if !violations.is_empty() {
            return Err(format!(
                "performance budget violated: {}",
                violations.join("; ")
            ));
        }
        Ok(snapshot)
    }

    /// Synchronous wrapper for non-async callers
    pub fn run_blocking(&self, scenario: &ThorScenario) -> Result<LoadMetrics, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
        rt.block_on(self.run(scenario))
    }

    async fn execute_step(
        client: &Client,
        step: &ScenarioStep,
        scenario: &ThorScenario,
        collector: &MetricsCollector,
    ) {
        match step {
            ScenarioStep::Navigate { url } => {
                let start = Instant::now();
                let mut req = client.get(url);
                for (k, v) in &scenario.headers {
                    req = req.header(k, v);
                }
                match req.send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let bytes = resp.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                        let latency = start.elapsed().as_secs_f64() * 1000.0;
                        collector.record_sample(status, latency, bytes, status < 400);
                    }
                    Err(_) => {
                        let latency = start.elapsed().as_secs_f64() * 1000.0;
                        collector.record_sample(0, latency, 0, false);
                    }
                }
            }
            ScenarioStep::HttpRequest {
                method,
                url,
                headers,
                body,
            } => {
                let start = Instant::now();
                let req_method = match method.to_uppercase().as_str() {
                    "POST" => reqwest::Method::POST,
                    "PUT" => reqwest::Method::PUT,
                    "DELETE" => reqwest::Method::DELETE,
                    "PATCH" => reqwest::Method::PATCH,
                    "HEAD" => reqwest::Method::HEAD,
                    "OPTIONS" => reqwest::Method::OPTIONS,
                    _ => reqwest::Method::GET,
                };

                let mut req = client.request(req_method, url);
                for (k, v) in &scenario.headers {
                    req = req.header(k, v);
                }
                for (k, v) in headers {
                    req = req.header(k, v);
                }
                if let Some(b) = body {
                    req = req.body(b.clone());
                }

                match req.send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let bytes = resp.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                        let latency = start.elapsed().as_secs_f64() * 1000.0;
                        collector.record_sample(status, latency, bytes, status < 400);
                    }
                    Err(_) => {
                        let latency = start.elapsed().as_secs_f64() * 1000.0;
                        collector.record_sample(0, latency, 0, false);
                    }
                }
            }
            ScenarioStep::Wait { duration_ms } => {
                sleep(Duration::from_millis(*duration_ms)).await;
            }
            ScenarioStep::WebSocketConnect { url, messages } => {
                let start = Instant::now();
                
                // We use connect_async to handle actual WS connection
                if let Ok((mut ws_stream, resp)) = tokio_tungstenite::connect_async(url.clone()).await {
                    let mut payload_bytes = 0;
                    use futures::{SinkExt, StreamExt};
                    for msg in messages {
                        if let Ok(_) = ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(msg.clone())).await {
                            payload_bytes += msg.len() as u64;
                            if let Some(Ok(m)) = ws_stream.next().await {
                                payload_bytes += m.into_data().len() as u64;
                            }
                        }
                    }
                    let _ = ws_stream.close(None).await;
                    let status = resp.status().as_u16();
                    let latency = start.elapsed().as_secs_f64() * 1000.0;
                    collector.record_sample(status, latency, payload_bytes, status < 500);
                } else {
                    let latency = start.elapsed().as_secs_f64() * 1000.0;
                    collector.record_sample(0, latency, 0, false);
                }
            }
            ScenarioStep::Click { .. } | ScenarioStep::Fill { .. } => {
                // No DOM in ProtocolRunner — fail sample, never invent success.
                // Prefer BrowserRunner for Click/Fill.
                collector.record_sample(0, 0.0, 0, false);
            }
            ScenarioStep::CustomScript { name, .. } => {
                // Unsupported here — fail sample (no silent 200/success theater).
                let _ = name;
                collector.record_sample(0, 0.0, 0, false);
            }
        }
    }
}
