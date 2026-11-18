mod transports;
mod virtual_user;

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

            let handle = tokio::spawn(virtual_user::run(
                client,
                scenario,
                collector,
                virtual_user::Schedule {
                    index: vu_idx,
                    count: vus,
                    deadline: end_time,
                    interval,
                },
            ));

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
                transports::send_http(req, start, collector).await;
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

                transports::send_http(req, start, collector).await;
            }
            ScenarioStep::Wait { duration_ms } => {
                sleep(Duration::from_millis(*duration_ms)).await;
            }
            ScenarioStep::WebSocketConnect { url, messages } => {
                transports::execute_websocket(url, messages, collector).await;
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

#[cfg(test)]
mod tests;
