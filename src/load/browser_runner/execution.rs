use super::{telemetry::BrowserObservations, BrowserRunner};
use crate::engine::chromium_runtime::{ChromiumRuntime, ChromiumRuntimeConfig};
use crate::engine::tab_session::TabSession;
use crate::load::metrics::{LoadMetrics, MetricsCollector};
use crate::load::scenario::{ScenarioStep, ThorScenario};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
impl BrowserRunner {
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

        let mut observations = BrowserObservations::new();
        for step in &scenario.steps {
            execute_step(&tab, scenario, step, &mut observations, &collector).await?;
        }
        observations.attach_vitals(&collector);
        let _ = tab.close().await;
        let _ = runtime.shutdown().await;
        observations.finish(collector.snapshot(), start_time, scenario)
    }
}
async fn execute_step(
    tab: &TabSession,
    scenario: &ThorScenario,
    step: &ScenarioStep,
    observations: &mut BrowserObservations,
    collector: &MetricsCollector,
) -> Result<(), String> {
    let step_start = Instant::now();
    match step {
        ScenarioStep::Navigate { url } => {
            let target = if url.is_empty() {
                &scenario.target_url
            } else {
                url
            };
            navigate(tab, target, observations, collector).await?;
        }
        ScenarioStep::HttpRequest { .. } => {
            return Err(
                "BrowserRunner refuses HttpRequest stubs — use ProtocolRunner for HTTP load steps"
                    .into(),
            );
        }
        ScenarioStep::Click { selector } => {
            let ok = click(tab, selector).await;
            record_interaction(collector, step_start, ok);
        }
        ScenarioStep::Fill { selector, value } => {
            let ok = fill(tab, selector, value).await;
            record_interaction(collector, step_start, ok);
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
    Ok(())
}
async fn navigate(
    tab: &TabSession,
    url: &str,
    observations: &mut BrowserObservations,
    collector: &MetricsCollector,
) -> Result<(), String> {
    let nav_start = Instant::now();
    match tab.navigate(url).await {
        Ok(_) => {
            let latency = nav_start.elapsed().as_secs_f64() * 1000.0;
            observations.collect(tab).await?;
            observations.record_navigation(collector, latency);
        }
        Err(_) => {
            let latency = nav_start.elapsed().as_secs_f64() * 1000.0;
            collector.record_sample(0, latency, 0, false);
        }
    }
    Ok(())
}
async fn click(tab: &TabSession, selector: &str) -> bool {
    let Some(page) = tab.page() else {
        return false;
    };
    let Ok(element) = page.find_element(selector).await else {
        return false;
    };
    element.click().await.is_ok()
}
async fn fill(tab: &TabSession, selector: &str, value: &str) -> bool {
    let Some(page) = tab.page() else {
        return false;
    };
    let Ok(element) = page.find_element(selector).await else {
        return false;
    };
    let _ = element.click().await;
    element.type_str(value).await.is_ok()
}
fn record_interaction(collector: &MetricsCollector, start: Instant, ok: bool) {
    let latency = start.elapsed().as_secs_f64() * 1000.0;
    let status = if ok { 200 } else { 0 };
    collector.record_sample(status, latency, 0, ok);
}
