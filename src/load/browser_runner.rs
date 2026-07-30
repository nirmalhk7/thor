use crate::engine::chromium_runtime::{ChromiumRuntime, ChromiumRuntimeConfig};
use crate::features::cdp_bridge::CdpBridge;
use crate::features::perf_features::PerfEngine;
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

    /// Execute scenario in Chromium Browser runner mode collecting Web Vitals.
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
        let bridge = CdpBridge::new();
        let perf = PerfEngine::new();
        let mut measured_transfer: u64 = 0;
        let mut measured_decoded: u64 = 0;
        let mut measured_lcp_media: Option<u64> = None;
        let mut soft_nav_report: Option<crate::load::SoftNavVitalsReport> = None;
        let mut network_sampled = false;
        let mut last_long_task_blocking_ms = 0.0_f64;

        for step in &scenario.steps {
            let step_start = Instant::now();
            match step {
                ScenarioStep::Navigate { url } => {
                    let target_url = if url.is_empty() {
                        &scenario.target_url
                    } else {
                        url
                    };
                    let nav_start = Instant::now();

                    match tab.navigate(target_url).await {
                        Ok(_) => {
                            let latency = nav_start.elapsed().as_secs_f64() * 1000.0;
                            // Install shared observers (CWV + LoAF/longtask + SoftNav).
                            let _ = tab
                                .evaluate_js(CdpBridge::web_vitals_injection_script())
                                .await;
                            let _ = tab
                                .evaluate_js(CdpBridge::soft_nav_and_speculation_script())
                                .await;
                            // Allow buffered PerformanceObserver entries to settle.
                            sleep(Duration::from_millis(100)).await;

                            let collect = r#"(() => {
                              try {
                                if (typeof window.__thorCollectWebVitals === 'function') {
                                  return window.__thorCollectWebVitals();
                                }
                              } catch (e) {}
                              return '{}';
                            })()"#;
                            if let Ok(Value::String(json_str)) = tab.evaluate_js(collect).await {
                                if let Some((parsed, lcp_media)) =
                                    CdpBridge::parse_web_vitals_ex(&json_str)
                                {
                                    vitals = parsed;
                                    if lcp_media.is_some() {
                                        measured_lcp_media = lcp_media;
                                    }
                                }
                            }

                            // Soft-nav → PerfEngine::soft_nav_vitals (refuse all-zero).
                            let soft_collect = r#"(() => {
                              try {
                                if (typeof window.__thorCollectSoftNavs === 'function') {
                                  return window.__thorCollectSoftNavs();
                                }
                              } catch (e) {}
                              return '[]';
                            })()"#;
                            if let Ok(Value::String(soft_json)) =
                                tab.evaluate_js(soft_collect).await
                            {
                                let soft_recs = CdpBridge::parse_soft_navs(&soft_json);
                                if !soft_recs.is_empty() {
                                    // Retain report on LoadMetrics; all-zero still Err.
                                    soft_nav_report = Some(perf.soft_nav_vitals(&soft_recs)?);
                                }
                            }

                            // Long tasks / LoAF → long_task_radar.
                            let long_collect = r#"(() => {
                              try {
                                if (typeof window.__thorCollectLongTasks === 'function') {
                                  return window.__thorCollectLongTasks();
                                }
                              } catch (e) {}
                              return '[]';
                            })()"#;
                            if let Ok(Value::String(long_json)) =
                                tab.evaluate_js(long_collect).await
                            {
                                let tasks = CdpBridge::parse_long_tasks(&long_json);
                                let radar = perf.long_task_radar(&tasks);
                                last_long_task_blocking_ms = radar.total_blocking_ms;
                                if vitals.tbt_ms <= 0.0 && radar.total_blocking_ms > 0.0 {
                                    vitals.tbt_ms = radar.total_blocking_ms;
                                }
                            }

                            // Resource Timing → CdpBridge network ingest (CDP-shaped JSON).
                            if let Ok(Value::String(net_json)) = tab
                                .evaluate_js(CdpBridge::resource_timing_network_script())
                                .await
                            {
                                let _ = bridge.ingest_cdp_network_json(&net_json);
                                // Prefer process-wide bridge when AppState is live.
                                let _ = crate::app_state::AppState::global()
                                    .features()
                                    .cdp()
                                    .ingest_cdp_network_json(&net_json);
                                network_sampled = true;
                            }

                            let (transfer, decoded) = bridge.byte_totals();
                            measured_transfer = transfer;
                            measured_decoded = decoded;
                            collector.record_sample(200, latency, transfer, true);
                        }
                        Err(_) => {
                            let latency = nav_start.elapsed().as_secs_f64() * 1000.0;
                            collector.record_sample(0, latency, 0, false);
                        }
                    }
                }
                ScenarioStep::HttpRequest { .. } => {
                    return Err(
                        "BrowserRunner refuses HttpRequest stubs — use ProtocolRunner for HTTP load steps"
                            .into(),
                    );
                }
                ScenarioStep::Click { selector } => {
                    let mut ok = false;
                    if let Some(page) = tab.page() {
                        if let Ok(el) = page.find_element(selector.as_str()).await {
                            ok = el.click().await.is_ok();
                        }
                    }
                    let latency = step_start.elapsed().as_secs_f64() * 1000.0;
                    if ok {
                        collector.record_sample(200, latency, 0, true);
                    } else {
                        collector.record_sample(0, latency, 0, false);
                    }
                }
                ScenarioStep::Fill { selector, value } => {
                    let mut ok = false;
                    if let Some(page) = tab.page() {
                        if let Ok(el) = page.find_element(selector.as_str()).await {
                            let _ = el.click().await;
                            ok = el.type_str(value.as_str()).await.is_ok();
                        }
                    }
                    let latency = step_start.elapsed().as_secs_f64() * 1000.0;
                    if ok {
                        collector.record_sample(200, latency, 0, true);
                    } else {
                        collector.record_sample(0, latency, 0, false);
                    }
                }
                ScenarioStep::Wait { duration_ms } => {
                    sleep(Duration::from_millis(*duration_ms)).await;
                }
                ScenarioStep::WebSocketConnect { .. } => {
                    return Err(
                        "BrowserRunner refuses WebSocketConnect stubs — use ProtocolRunner for WS steps"
                            .into(),
                    );
                }
                ScenarioStep::CustomScript { .. } => {
                    return Err(
                        "BrowserRunner refuses CustomScript sleep-stubs — implement via CDP evaluate in scenario or remove step"
                            .into(),
                    );
                }
            }
        }

        // Surface LoAF blocking into vitals when CWV script left TBT empty.
        if vitals.tbt_ms <= 0.0 && last_long_task_blocking_ms > 0.0 {
            vitals.tbt_ms = last_long_task_blocking_ms;
        }
        // Refuse attaching all-zero defaults — would fake-pass CWV budgets.
        let vitals_measured = vitals.lcp_ms > 0.0
            || vitals.inp_ms > 0.0
            || vitals.cls > 0.0
            || vitals.fcp_ms > 0.0
            || vitals.ttfb_ms > 0.0
            || vitals.tbt_ms > 0.0
            || vitals.fid_ms > 0.0;
        if vitals_measured {
            collector.set_browser_vitals(vitals);
        }

        let _ = tab.close().await;
        let _ = runtime.shutdown().await;

        let mut snapshot = collector.snapshot();
        if snapshot.duration_secs < 0.001 {
            snapshot.duration_secs = start_time.elapsed().as_secs_f64();
        }
        if network_sampled {
            snapshot.transfer_bytes = Some(measured_transfer);
            snapshot.decoded_bytes = Some(measured_decoded);
        }
        if let Some(media) = measured_lcp_media {
            snapshot.lcp_media_bytes = Some(media);
        }
        snapshot.soft_nav = soft_nav_report;

        let violations = scenario.budgets.check(&snapshot);
        if !violations.is_empty() {
            return Err(format!(
                "performance budget violated: {}",
                violations.join("; ")
            ));
        }

        Ok(snapshot)
    }

    /// Derive *synthetic* Core Web Vitals estimates from HTTP latencies.
    ///
    /// **Not lab CWV.** Returns `Err` when inputs are empty (nothing to estimate).
    /// Otherwise returns metrics with [`BrowserMetrics::synthetic`] = true, which
    /// [`PerformanceBudgets`] exclude from gates. Prefer
    /// [`crate::features::cdp_bridge::CdpBridge::parse_web_vitals`] on real
    /// browser runs.
    pub fn derive_vitals(
        nav_latencies: &[f64],
        interaction_latencies: &[f64],
        dom_latency: Option<f64>,
    ) -> Result<BrowserMetrics, String> {
        if nav_latencies.is_empty() && interaction_latencies.is_empty() && dom_latency.is_none() {
            return Err(
                "derive_vitals refused: no latencies to estimate from; collect lab CWV via CDP / PerformanceObserver"
                    .into(),
            );
        }
        let ttfb_ms = percentile(nav_latencies, 50.0).unwrap_or(0.0);
        let fcp_ms = if ttfb_ms > 0.0 { ttfb_ms * 1.35 } else { 0.0 };
        let lcp_ms = nav_latencies
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(fcp_ms);

        let fid_ms = interaction_latencies
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        let fid_ms = if fid_ms.is_finite() { fid_ms } else { 0.0 };
        let inp_ms = percentile(interaction_latencies, 98.0).unwrap_or(fid_ms);
        let tbt_ms: f64 = interaction_latencies
            .iter()
            .map(|&l| (l - 50.0).max(0.0))
            .sum();

        let mut cls = 0.0;
        for w in nav_latencies.windows(2) {
            let base = w[0].max(1.0);
            cls += ((w[1] - w[0]).abs() / base).min(0.1);
        }
        cls = cls.min(1.0);

        let dom = dom_latency.unwrap_or(ttfb_ms);
        Ok(BrowserMetrics {
            lcp_ms,
            fid_ms,
            inp_ms,
            tbt_ms,
            cls,
            fcp_ms,
            ttfb_ms,
            dom_content_loaded_ms: dom * 1.25,
            dom_interactive_ms: dom * 1.1,
            synthetic: true,
        })
    }

    pub fn run_blocking(&self, scenario: &ThorScenario) -> Result<LoadMetrics, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
        rt.block_on(self.run(scenario))
    }
}

fn percentile(samples: &[f64], q: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((q / 100.0) * (sorted.len() as f64)).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    Some(sorted[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_vitals_from_latencies() {
        let nav = vec![40.0, 80.0, 60.0];
        let interactions = vec![20.0, 120.0, 45.0];
        let vitals = BrowserRunner::derive_vitals(&nav, &interactions, Some(60.0))
            .expect("estimate ok");
        assert!(vitals.synthetic, "HTTP-derived vitals must be marked synthetic");
        assert!(vitals.ttfb_ms > 0.0);
        assert!(vitals.lcp_ms >= vitals.fcp_ms);
        assert!(vitals.inp_ms >= vitals.fid_ms);
        assert!((vitals.tbt_ms - 70.0).abs() < 1e-6);
        assert!(vitals.cls >= 0.0 && vitals.cls <= 1.0);

        let budgets = crate::load::PerformanceBudgets::core_web_vitals();
        let eval = budgets.evaluate(Some(&vitals), None, None, None, None);
        assert_eq!(
            eval.checked, 0,
            "synthetic vitals must be excluded from CWV budget gates"
        );
    }

    #[test]
    fn test_derive_vitals_refuses_empty() {
        let err = BrowserRunner::derive_vitals(&[], &[], None).expect_err("empty refused");
        assert!(err.contains("refused"), "{err}");
    }

    #[test]
    fn test_record_sample_uses_transfer_totals_not_1024() {
        // Navigate path records CdpBridge::byte_totals(), never a fake constant size.
        let bridge = CdpBridge::new();
        let n = bridge.ingest_cdp_network_json(
            r#"[{"url":"https://example.com/","transfer_bytes":4096,"decoded_bytes":8192,"bytes":4096}]"#,
        );
        assert_eq!(n, 1);
        let (transfer, decoded) = bridge.byte_totals();
        assert_eq!(transfer, 4096);
        assert_eq!(decoded, 8192);
        assert_ne!(transfer, 1024);
    }

    #[test]
    fn test_soft_nav_attaches_to_load_metrics() {
        let perf = PerfEngine::new();
        let report = perf
            .soft_nav_vitals(&[crate::load::SoftNavRecord {
                navigation_id: "n1".into(),
                url: "/spa".into(),
                lcp_ms: 900.0,
                inp_ms: 40.0,
                cls: 0.02,
                ttfb_ms: 0.0,
            }])
            .expect("soft-nav ok");
        let mut metrics = MetricsCollector::new().snapshot();
        metrics.soft_nav = Some(report);
        let soft = metrics.soft_nav.as_ref().expect("soft_nav attached");
        assert_eq!(soft.navigations, 1);
        assert!((soft.worst_lcp_ms - 900.0).abs() < 1e-6);
        assert!((soft.avg_inp_ms - 40.0).abs() < 1e-6);
    }

    #[test]
    fn test_lcp_media_bytes_from_vitals_json() {
        let (_, media) = CdpBridge::parse_web_vitals_ex(
            r#"{"lcp":1500.0,"cls":0.01,"lcpSize":64000,"lcpElement":{"size":64000}}"#,
        )
        .expect("parse");
        assert_eq!(media, Some(64_000));
        let mut snapshot = MetricsCollector::new().snapshot();
        snapshot.lcp_media_bytes = media;
        assert_eq!(snapshot.lcp_media_bytes, Some(64_000));

        let budgets = crate::load::PerformanceBudgets {
            max_lcp_media_bytes: Some(50_000),
            ..Default::default()
        };
        let violations = budgets.check(&snapshot);
        assert!(
            violations.iter().any(|v| v.contains("lcp_media_bytes")),
            "{violations:?}"
        );
    }

    #[test]
    fn test_soft_nav_folds_into_budget_check() {
        let perf = PerfEngine::new();
        let report = perf
            .soft_nav_vitals(&[crate::load::SoftNavRecord {
                navigation_id: "n1".into(),
                url: "/r".into(),
                lcp_ms: 4000.0,
                inp_ms: 10.0,
                cls: 0.01,
                ttfb_ms: 0.0,
            }])
            .unwrap();
        let mut metrics = MetricsCollector::new().snapshot();
        metrics.browser_vitals = Some(BrowserMetrics {
            lcp_ms: 1000.0,
            inp_ms: 50.0,
            cls: 0.01,
            ..Default::default()
        });
        metrics.soft_nav = Some(report);
        let budgets = crate::load::PerformanceBudgets {
            max_lcp_ms: Some(2500.0),
            ..Default::default()
        };
        let violations = budgets.check(&metrics);
        assert!(
            violations.iter().any(|v| v.contains("LCP")),
            "soft-nav worst LCP should trip budget: {violations:?}"
        );
    }
}
