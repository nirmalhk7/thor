use crate::engine::tab_session::TabSession;
use crate::features::cdp_bridge::CdpBridge;
use crate::features::perf_features::PerfEngine;
use crate::load::metrics::{BrowserMetrics, LoadMetrics, MetricsCollector};
use crate::load::scenario::ThorScenario;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub(super) struct BrowserObservations {
    vitals: BrowserMetrics,
    bridge: CdpBridge,
    perf: PerfEngine,
    measured_transfer: u64,
    measured_decoded: u64,
    measured_lcp_media: Option<u64>,
    soft_nav_report: Option<crate::load::SoftNavVitalsReport>,
    network_sampled: bool,
    last_long_task_blocking_ms: f64,
}
impl BrowserObservations {
    pub(super) fn new() -> Self {
        Self {
            vitals: BrowserMetrics::default(),
            bridge: CdpBridge::new(),
            perf: PerfEngine::new(),
            measured_transfer: 0,
            measured_decoded: 0,
            measured_lcp_media: None,
            soft_nav_report: None,
            network_sampled: false,
            last_long_task_blocking_ms: 0.0_f64,
        }
    }
    pub(super) async fn collect(&mut self, tab: &TabSession) -> Result<(), String> {
        // Install shared observers, then allow buffered entries to settle.
        let _ = tab
            .evaluate_js(CdpBridge::web_vitals_injection_script())
            .await;
        let _ = tab
            .evaluate_js(CdpBridge::soft_nav_and_speculation_script())
            .await;
        sleep(Duration::from_millis(100)).await;
        self.collect_vitals(tab).await;
        self.collect_soft_nav(tab).await?;
        self.collect_long_tasks(tab).await;
        self.collect_network(tab).await;
        Ok(())
    }
    async fn collect_vitals(&mut self, tab: &TabSession) {
        let collect = r#"(() => {
                              try {
                                if (typeof window.__thorCollectWebVitals === 'function') {
                                  return window.__thorCollectWebVitals();
                                }
                              } catch (e) {}
                              return '{}';
                            })()"#;
        if let Ok(Value::String(json_str)) = tab.evaluate_js(collect).await {
            if let Some((parsed, lcp_media)) = CdpBridge::parse_web_vitals_ex(&json_str) {
                self.vitals = parsed;
                if lcp_media.is_some() {
                    self.measured_lcp_media = lcp_media;
                }
            }
        }
    }
    async fn collect_soft_nav(&mut self, tab: &TabSession) -> Result<(), String> {
        let soft_collect = r#"(() => {
                              try {
                                if (typeof window.__thorCollectSoftNavs === 'function') {
                                  return window.__thorCollectSoftNavs();
                                }
                              } catch (e) {}
                              return '[]';
                            })()"#;
        if let Ok(Value::String(soft_json)) = tab.evaluate_js(soft_collect).await {
            let soft_recs = CdpBridge::parse_soft_navs(&soft_json);
            if !soft_recs.is_empty() {
                // Retain report on LoadMetrics; all-zero still Err.
                self.soft_nav_report = Some(self.perf.soft_nav_vitals(&soft_recs)?);
            }
        }
        Ok(())
    }
    async fn collect_long_tasks(&mut self, tab: &TabSession) {
        let long_collect = r#"(() => {
                              try {
                                if (typeof window.__thorCollectLongTasks === 'function') {
                                  return window.__thorCollectLongTasks();
                                }
                              } catch (e) {}
                              return '[]';
                            })()"#;
        if let Ok(Value::String(long_json)) = tab.evaluate_js(long_collect).await {
            let tasks = CdpBridge::parse_long_tasks(&long_json);
            let radar = self.perf.long_task_radar(&tasks);
            self.last_long_task_blocking_ms = radar.total_blocking_ms;
            if self.vitals.tbt_ms <= 0.0 && radar.total_blocking_ms > 0.0 {
                self.vitals.tbt_ms = radar.total_blocking_ms;
            }
        }
    }
    async fn collect_network(&mut self, tab: &TabSession) {
        if let Ok(Value::String(net_json)) = tab
            .evaluate_js(CdpBridge::resource_timing_network_script())
            .await
        {
            let _ = self.bridge.ingest_cdp_network_json(&net_json);
            // Prefer process-wide bridge when AppState is live.
            let _ = crate::app_state::AppState::global()
                .features()
                .cdp()
                .ingest_cdp_network_json(&net_json);
            self.network_sampled = true;
        }
    }
    pub(super) fn record_navigation(&mut self, collector: &MetricsCollector, latency: f64) {
        let (transfer, decoded) = self.bridge.byte_totals();
        self.measured_transfer = transfer;
        self.measured_decoded = decoded;
        collector.record_sample(200, latency, transfer, true);
    }
    pub(super) fn attach_vitals(&mut self, collector: &MetricsCollector) {
        // Surface LoAF blocking into vitals when CWV script left TBT empty.
        if self.vitals.tbt_ms <= 0.0 && self.last_long_task_blocking_ms > 0.0 {
            self.vitals.tbt_ms = self.last_long_task_blocking_ms;
        }
        // Refuse attaching all-zero defaults — would fake-pass CWV budgets.
        let vitals_measured = [
            self.vitals.lcp_ms,
            self.vitals.inp_ms,
            self.vitals.cls,
            self.vitals.fcp_ms,
            self.vitals.ttfb_ms,
            self.vitals.tbt_ms,
            self.vitals.fid_ms,
        ]
        .into_iter()
        .any(|value| value > 0.0);
        if vitals_measured {
            collector.set_browser_vitals(self.vitals.clone());
        }
    }
    pub(super) fn finish(
        self,
        mut snapshot: LoadMetrics,
        start_time: Instant,
        scenario: &ThorScenario,
    ) -> Result<LoadMetrics, String> {
        if snapshot.duration_secs < 0.001 {
            snapshot.duration_secs = start_time.elapsed().as_secs_f64();
        }
        if self.network_sampled {
            snapshot.transfer_bytes = Some(self.measured_transfer);
            snapshot.decoded_bytes = Some(self.measured_decoded);
        }
        if let Some(media) = self.measured_lcp_media {
            snapshot.lcp_media_bytes = Some(media);
        }
        snapshot.soft_nav = self.soft_nav_report;

        let violations = scenario.budgets.check(&snapshot);
        if !violations.is_empty() {
            return Err(format!(
                "performance budget violated: {}",
                violations.join("; ")
            ));
        }

        Ok(snapshot)
    }
}
