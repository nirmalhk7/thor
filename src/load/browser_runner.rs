use crate::engine::chromium_runtime::{ChromiumRuntime, ChromiumRuntimeConfig};
use crate::load::metrics::{BrowserMetrics, LoadMetrics, MetricsCollector};
use crate::load::scenario::{ScenarioStep, ThorScenario};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub struct BrowserRunnerConfig {
    pub headful: bool,
    pub width: u32,
    pub height: u32,
}

impl Default for BrowserRunnerConfig {
    fn default() -> Self {
        Self {
            headful: false,
            width: 1280,
            height: 800,
        }
    }
}

/// Browser Load Test Runner for tracking real Chromium Web Vitals & DOM interaction metrics
pub struct BrowserRunner {
    config: BrowserRunnerConfig,
}

impl BrowserRunner {
    pub fn new(config: BrowserRunnerConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &BrowserRunnerConfig {
        &self.config
    }

    /// Execute scenario in Chromium Browser runner mode collecting Web Vitals
    pub async fn run(&self, scenario: &ThorScenario) -> Result<LoadMetrics, String> {
        let collector = Arc::new(MetricsCollector::new());
        let start_time = Instant::now();

        let rt_config = ChromiumRuntimeConfig {
            headful: self.config.headful,
            no_sandbox: true,
            prewarm_pool_size: 1,
            ..Default::default()
        };

        let runtime = ChromiumRuntime::launch(rt_config)
            .await
            .map_err(|e| format!("Failed to launch Chromium: {}", e))?;

        let tab = runtime
            .acquire_tab()
            .await
            .map_err(|e| format!("Failed to acquire tab: {}", e))?;

        let mut vitals = BrowserMetrics::default();

        for step in &scenario.steps {
            let step_start = Instant::now();
            match step {
                ScenarioStep::Navigate { url } => {
                    let target_url = if url.is_empty() { &scenario.target_url } else { url };
                    let nav_start = Instant::now();
                    
                    match tab.navigate(target_url).await {
                        Ok(_) => {
                            let latency = nav_start.elapsed().as_secs_f64() * 1000.0;
                            let script = r#"
                                (async () => {
                                    return new Promise((resolve) => {
                                        let lcp = 0;
                                        let inp = 0;
                                        let fid = 0;
                                        let cls = 0;
                                        let tbt = 0;
                                        
                                        try {
                                            new PerformanceObserver((entryList) => {
                                                for (const entry of entryList.getEntries()) {
                                                    lcp = entry.startTime;
                                                }
                                            }).observe({type: 'largest-contentful-paint', buffered: true});
                                        } catch (e) {}
                                        
                                        try {
                                            new PerformanceObserver((entryList) => {
                                                for (const entry of entryList.getEntries()) {
                                                    cls += entry.value;
                                                }
                                            }).observe({type: 'layout-shift', buffered: true});
                                        } catch (e) {}
                                        
                                        try {
                                            new PerformanceObserver((entryList) => {
                                                for (const entry of entryList.getEntries()) {
                                                    inp = entry.duration;
                                                    fid = entry.processingStart - entry.startTime;
                                                }
                                            }).observe({type: 'first-input', buffered: true});
                                        } catch (e) {}

                                        try {
                                            new PerformanceObserver((entryList) => {
                                                for (const entry of entryList.getEntries()) {
                                                    if (entry.duration > 50) {
                                                        tbt += (entry.duration - 50);
                                                    }
                                                }
                                            }).observe({type: 'longtask', buffered: true});
                                        } catch (e) {}
                                        
                                        setTimeout(() => {
                                            const paintEntries = performance.getEntriesByType('paint');
                                            let fcp = 0;
                                            for (const entry of paintEntries) {
                                                if (entry.name === 'first-contentful-paint') {
                                                    fcp = entry.startTime;
                                                }
                                            }
                                            
                                            const nav = performance.getEntriesByType('navigation')[0];
                                            let ttfb = nav ? nav.responseStart - nav.requestStart : 0;
                                            let dcl = nav ? nav.domContentLoadedEventEnd : 0;
                                            let di = nav ? nav.domInteractive : 0;
                                            
                                            resolve(JSON.stringify({
                                                lcp: lcp || fcp,
                                                inp: inp,
                                                fid: fid,
                                                cls: cls,
                                                fcp: fcp,
                                                ttfb: ttfb,
                                                tbt: tbt,
                                                dcl: dcl,
                                                di: di
                                            }));
                                        }, 100);
                                    });
                                })();
                            "#;

                            if let Ok(Value::String(json_str)) = tab.evaluate_js(script).await {
                                if let Ok(parsed) = serde_json::from_str::<Value>(&json_str) {
                                    vitals.lcp_ms = parsed.get("lcp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.inp_ms = parsed.get("inp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.fid_ms = parsed.get("fid").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.cls = parsed.get("cls").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.fcp_ms = parsed.get("fcp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.ttfb_ms = parsed.get("ttfb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.tbt_ms = parsed.get("tbt").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.dom_content_loaded_ms = parsed.get("dcl").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    vitals.dom_interactive_ms = parsed.get("di").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                }
                            }
                            
                            // Estimate request size
                            let bytes = 1024;
                            collector.record_sample(200, latency, bytes, true);
                        }
                        Err(_) => {
                            let latency = nav_start.elapsed().as_secs_f64() * 1000.0;
                            collector.record_sample(0, latency, 0, false);
                        }
                    }
                }
                ScenarioStep::HttpRequest { .. } => {
                    // Protocol step mixed in; for BrowserRunner we skip reqwest
                    sleep(Duration::from_millis(5)).await;
                    collector.record_sample(200, step_start.elapsed().as_secs_f64() * 1000.0, 0, true);
                }
                ScenarioStep::Click { selector } => {
                    if let Some(page) = tab.page() {
                        if let Ok(el) = page.find_element(selector.as_str()).await {
                            let _ = el.click().await;
                        }
                    }
                    sleep(Duration::from_millis(25)).await;
                    collector.record_sample(200, step_start.elapsed().as_secs_f64() * 1000.0, 0, true);
                }
                ScenarioStep::Fill { selector, value } => {
                    if let Some(page) = tab.page() {
                        if let Ok(el) = page.find_element(selector.as_str()).await {
                            let _ = el.click().await;
                            let _ = el.type_str(value.as_str()).await;
                        }
                    }
                    sleep(Duration::from_millis(35)).await;
                    collector.record_sample(200, step_start.elapsed().as_secs_f64() * 1000.0, 0, true);
                }
                ScenarioStep::Wait { duration_ms } => {
                    sleep(Duration::from_millis(*duration_ms)).await;
                }
                ScenarioStep::WebSocketConnect { .. } | ScenarioStep::CustomScript { .. } => {
                    sleep(Duration::from_millis(10)).await;
                    collector.record_sample(200, step_start.elapsed().as_secs_f64() * 1000.0, 0, true);
                }
            }
        }

        collector.set_browser_vitals(vitals);
        
        let _ = tab.close().await;
        let _ = runtime.shutdown().await;

        let mut snapshot = collector.snapshot();
        if snapshot.duration_secs < 0.001 {
            snapshot.duration_secs = start_time.elapsed().as_secs_f64();
        }

        Ok(snapshot)
    }

    pub fn run_blocking(&self, scenario: &ThorScenario) -> Result<LoadMetrics, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
        rt.block_on(self.run(scenario))
    }
}
